mod actions;
mod collectors;
mod types;
mod vfx;

use vfx::{VfxKind, VfxOverlay};
use actions::controller::execute_action;
use adw::prelude::*;
use gtk4::prelude::*; // <-- Add this line to bring WindowExt into scope
use collectors::chrome_bridge::{run_chrome_ws_server, SharedChromeState};
use collectors::proc_linux::LinuxCollector;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use types::{
    AppAction, AppGroupSummary, ChromeBridgePayload, ChromeCommand, ProcessCategory, ProcessEntry,
    SystemSnapshot,
};

const CUSTOM_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/rapidash.css"));


fn format_mib(bytes: u64) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if mib >= 1024.0 {
        format!("{:.2} GiB", mib / 1024.0)
    } else {
        format!("{:.0} MiB", mib)
    }
}

fn safe_text(input: &str) -> String {
    gtk::glib::markup_escape_text(input).to_string()
}

fn apply_memory_badge_style(label: &gtk::Label, bytes: u64) {
    for cls in [
        "mem-badge-low",
        "mem-badge-med",
        "mem-badge-high",
        "mem-badge-crit",
    ] {
        label.remove_css_class(cls);
    }
    let mib = bytes / (1024 * 1024);
    if mib >= 700 {
        label.add_css_class("mem-badge-crit");
    } else if mib >= 300 {
        label.add_css_class("mem-badge-high");
    } else if mib >= 100 {
        label.add_css_class("mem-badge-med");
    } else {
        label.add_css_class("mem-badge-low");
    }
}

fn trigger_fire_action<W, F>(widget: &W, css_class: &'static str, delay_ms: u64, on_complete: F)
where
    W: IsA<gtk::Widget> + Clone + 'static,
    F: FnOnce() + 'static,
{
    widget.add_css_class(css_class);
    let w = widget.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
        on_complete();
        // Remove transient pulse classes if the widget remains alive (like Freeze/Hibernate)
        w.remove_css_class(css_class);
    });
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterMode {
    All,
    DesktopApps,
    DevTools,
    Browser,
    System,
    FrozenOnly,
}

// =============================================================================
// Zero-Lag Custom Visual Process Row (In-Place Diffing + Highlighted Memory Bar)
// =============================================================================
struct VisualProcessRow {
    row_widget: gtk::ListBoxRow,
    app_badge: gtk::Label,
    frozen_badge: gtk::Label,
    role_label: gtk::Label,
    meta_label: gtk::Label,
    mem_badge: gtk::Label,
    mem_bar: gtk::ProgressBar,
    cpu_badge: gtk::Label,
    freeze_btn: gtk::Button,
    is_frozen: Rc<Cell<bool>>,
}

struct ProcessExplorerManager {
    list_box: gtk::ListBox,
    rows: HashMap<i32, VisualProcessRow>,
    act_tx: async_channel::Sender<AppAction>,
    vfx: VfxOverlay
}

impl ProcessExplorerManager {
    fn new(act_tx: async_channel::Sender<AppAction>, vfx: VfxOverlay) -> Self {
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        Self {
            list_box,
            rows: HashMap::new(),
            act_tx,
            vfx,
        }
    }

    fn sync_processes(&mut self, entries: &[&ProcessEntry], max_uss_bytes: u64) {
        let active_pids: HashSet<i32> = entries.iter().map(|p| p.pid).collect();

        // Remove exited or filtered-out PIDs
        self.rows.retain(|pid, cached| {
            if !active_pids.contains(pid) {
                self.list_box.remove(&cached.row_widget);
                false
            } else {
                true
            }
        });

        let denom = (max_uss_bytes as f64).max(100.0 * 1024.0 * 1024.0);

        for proc in entries {
            // let mem_text = format!("USS: {}", format_mib(proc.uss_bytes));
            // Replace USS with Reclaimable
            let mem_text = format!("Reclaimable: {}", format_mib(proc.uss_bytes));
            let cpu_text = format!("CPU {:>4.1}%", proc.cpu_percent);
            let meta_text = format!(
                "PID {}  |  Total Share (PSS): {}  |  Swap: {}  |  {}",
                proc.pid,
                format_mib(proc.pss_bytes),
                format_mib(proc.swap_bytes),
                proc.category.label()
            );
            let bar_frac = (proc.uss_bytes as f64 / denom).clamp(0.03, 1.0);

            if let Some(cached) = self.rows.get(&proc.pid) {
                if cached.app_badge.label() != proc.app_group {
                    cached.app_badge.set_label(&proc.app_group);
                }
                if cached.role_label.label() != proc.display_role {
                    cached.role_label.set_label(&proc.display_role);
                }
                if cached.meta_label.label() != meta_text {
                    cached.meta_label.set_label(&meta_text);
                }
                if cached.mem_badge.label() != mem_text {
                    cached.mem_badge.set_label(&mem_text);
                    apply_memory_badge_style(&cached.mem_badge, proc.uss_bytes);
                }
                cached.mem_bar.set_fraction(bar_frac);
                if cached.cpu_badge.label() != cpu_text {
                    cached.cpu_badge.set_label(&cpu_text);
                }
                cached.frozen_badge.set_visible(proc.is_frozen());

                if proc.is_frozen() {
                    cached.row_widget.add_css_class("row-frozen-state");
                } else {
                    cached.row_widget.remove_css_class("row-frozen-state");
                }

                // if cached.is_frozen.get() != proc.is_frozen() {
                //     cached.is_frozen.set(proc.is_frozen());
                //     if proc.is_frozen() {
                //         cached.freeze_btn.set_label("Resume");
                //         cached.freeze_btn.add_css_class("suggested-action");
                //     } else {
                //         cached.freeze_btn.set_label("Silence");
                //         cached.freeze_btn.remove_css_class("suggested-action");
                //     }
                // }
                if cached.is_frozen.get() != proc.is_frozen() {
                    cached.is_frozen.set(proc.is_frozen());
                    if proc.is_frozen() {
                        cached.freeze_btn.set_label("Wake");
                        cached.freeze_btn.add_css_class("suggested-action");
                    } else {
                        cached.freeze_btn.set_label("Freeze (0% CPU)");
                        cached.freeze_btn.remove_css_class("suggested-action");
                    }
                }
            } else {
                let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 14);
                row_box.add_css_class("proc-card-row");

                // Left Column: App Pill + Role Title + Meta
                let left_col = gtk::Box::new(gtk::Orientation::Vertical, 4);
                left_col.set_hexpand(true);
                left_col.set_valign(gtk::Align::Center);

                let top_badges = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let app_badge = gtk::Label::builder()
                    .label(&proc.app_group)
                    .css_classes(["app-pill"])
                    .build();
                let frozen_badge = gtk::Label::builder()
                    .label("FROZEN (0% CPU)")
                    .css_classes(["frozen-pill"])
                    .visible(proc.is_frozen())
                    .build();
                let role_label = gtk::Label::builder()
                    .label(&proc.display_role)
                    .css_classes(["proc-title"])
                    .halign(gtk::Align::Start)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();

                top_badges.append(&app_badge);
                top_badges.append(&frozen_badge);
                top_badges.append(&role_label);

                let meta_label = gtk::Label::builder()
                    .label(&meta_text)
                    .css_classes(["proc-subtitle"])
                    .halign(gtk::Align::Start)
                    .build();

                left_col.append(&top_badges);
                left_col.append(&meta_label);

                // Center Column: Highlighted Memory Pill + Proportional Bar + CPU Pill
                let telemetry_col = gtk::Box::new(gtk::Orientation::Vertical, 5);
                telemetry_col.set_valign(gtk::Align::Center);
                telemetry_col.set_size_request(260, -1);

                let badge_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                badge_row.set_halign(gtk::Align::End);

                let cpu_badge = gtk::Label::builder()
                    .label(&cpu_text)
                    .css_classes(["cpu-pill"])
                    .xalign(0.5)
                    .build();

                let mem_badge = gtk::Label::builder()
                    .label(&mem_text)
                    .xalign(0.5)
                    .build();
                apply_memory_badge_style(&mem_badge, proc.uss_bytes);

                badge_row.append(&cpu_badge);
                badge_row.append(&mem_badge);

                let mem_bar = gtk::ProgressBar::builder().fraction(bar_frac).build();

                telemetry_col.append(&badge_row);
                telemetry_col.append(&mem_bar);

                // Right Column: Interactive Action Buttons
                let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                btn_box.set_valign(gtk::Align::Center);

                let is_frozen = Rc::new(Cell::new(proc.is_frozen()));
                // let freeze_btn = gtk::Button::builder()
                //     .label(if proc.is_frozen() { "Resume" } else { "Silence" })
                //     .tooltip_text("Pause process CPU (SIGSTOP) or wake it (SIGCONT)")
                //     .build();
                // Replace button instantiations:
                let freeze_btn = gtk::Button::builder()
                    .label(if proc.is_frozen() { "Wake" } else { "Freeze (0% CPU)" })
                    .width_request(135)
                    .tooltip_text("Pause process CPU (SIGSTOP) or wake it (SIGCONT)")
                    .build();
                if proc.is_frozen() {
                    freeze_btn.add_css_class("suggested-action");
                }

                // Replace Silence/Resume cache logic:
                // if proc.is_frozen() {
                //     cached.freeze_btn.set_label("Wake");
                //     cached.freeze_btn.add_css_class("suggested-action");
                // } else {
                //     cached.freeze_btn.set_label("Freeze (0% CPU)");
                //     cached.freeze_btn.remove_css_class("suggested-action");
                // }

                {
                    let tx = self.act_tx.clone();
                    let pid = proc.pid;
                    let frozen_cell = is_frozen.clone();
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    freeze_btn.connect_clicked(move |_| {
                        let currently_frozen = frozen_cell.get();
                        let (kind, msg) = if currently_frozen {
                            (VfxKind::MorningSun, "UNICORN REVIVAL — Waking process".to_string())
                        } else {
                            (VfxKind::EmberStasis, "EMBER STASIS — Freezing process".to_string())
                        };
                        let tx_clone = tx.clone();
                        vfx_c.trigger_on_widget(&row_anim, kind, msg, 0, move || {
                            let _ = tx_clone.send_blocking(AppAction::ToggleFreeze {
                                pid,
                                currently_frozen,
                            });
                        });
                    });
                }

                let kill_btn = gtk::Button::builder()
                    .label("Quit")
                    .tooltip_text("Gracefully terminate process (SIGTERM)")
                    .css_classes(["destructive-action"])
                    .build();
                {
                    let tx = self.act_tx.clone();
                    let pid = proc.pid;
                    let uss = proc.uss_bytes;
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    kill_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        let tx_clone = tx.clone();
                        let banner = format!("INFERNO — Quit requested · {} private RAM", format_mib(uss));
                        vfx_c.trigger_on_widget(&row_anim, VfxKind::Incinerate, banner, 160, move || {
                            let _ = tx_clone.send_blocking(AppAction::TerminateGraceful { pid });
                        });
                    });
                }

                let force_btn = gtk::Button::builder()
                    .label("Force Quit")
                    .tooltip_text("Immediate force kill (SIGKILL -9)")
                    .build();
                {
                    let tx = self.act_tx.clone();
                    let pid = proc.pid;
                    let uss = proc.uss_bytes;
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    force_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        let tx_clone = tx.clone();
                        let banner = format!("INFERNO OVERDRIVE — Force kill requested · {} private RAM", format_mib(uss));
                        vfx_c.trigger_on_widget(&row_anim, VfxKind::InfernoOverdrive, banner, 160, move || {
                            let _ = tx_clone.send_blocking(AppAction::ForceKill { pid });
                        });
                    });
                }

                btn_box.append(&freeze_btn);
                btn_box.append(&kill_btn);
                btn_box.append(&force_btn);

                row_box.append(&left_col);
                row_box.append(&telemetry_col);
                row_box.append(&btn_box);

                let row_widget = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .child(&row_box)
                    .build();

                self.list_box.append(&row_widget);
                self.rows.insert(
                    proc.pid,
                    VisualProcessRow {
                        row_widget,
                        app_badge,
                        frozen_badge,
                        role_label,
                        meta_label,
                        mem_badge,
                        mem_bar,
                        cpu_badge,
                        freeze_btn,
                        is_frozen,
                    },
                );
            }
        }
    }
}

// =============================================================================
// Zero-Lag Application Groups Manager (Obsidian, AnyDesk, VS Code, Chrome...)
// =============================================================================
struct VisualAppGroupRow {
    row_widget: gtk::ListBoxRow,
    title_label: gtk::Label,
    meta_label: gtk::Label,
    mem_badge: gtk::Label,
    mem_bar: gtk::ProgressBar,
    cpu_badge: gtk::Label,
    pids: Rc<RefCell<Vec<i32>>>,
}

struct AppGroupListManager {
    list_box: gtk::ListBox,
    rows: HashMap<String, VisualAppGroupRow>,
    act_tx: async_channel::Sender<AppAction>,
    vfx: VfxOverlay
}

impl AppGroupListManager {
    fn new(act_tx: async_channel::Sender<AppAction>, vfx: VfxOverlay) -> Self {
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        Self {
            list_box,
            rows: HashMap::new(),
            act_tx,
            vfx,
        }
    }

    fn sync_groups(&mut self, groups: &[AppGroupSummary], total_ram_bytes: u64) {
        let active_names: HashSet<String> = groups.iter().map(|g| g.app_name.clone()).collect();

        self.rows.retain(|name, cached| {
            if !active_names.contains(name) {
                self.list_box.remove(&cached.row_widget);
                false
            } else {
                true
            }
        });

        let denom = (total_ram_bytes as f64 * 0.35).max(512.0 * 1024.0 * 1024.0);

        for group in groups {
            // let frozen_note = if group.frozen_count > 0 {
            //     format!("  |  {} Silenced", group.frozen_count)
            // } else {
            //     String::new()
            // };
            // Replace Silenced with Frozen
            let frozen_note = if group.frozen_count > 0 {
                format!("  |  {} Frozen", group.frozen_count)
            } else {
                String::new()
            };
            let title_text = format!("{} ({} Processes)", group.app_name, group.process_count);
            let meta_text = format!(
                "Category: {}  |  Total Share (PSS): {}  |  Swap: {}{}",
                group.category.label(),
                format_mib(group.total_pss_bytes),
                format_mib(group.total_swap_bytes),
                frozen_note
            );
            // let mem_text = format!("Total USS: {}", format_mib(group.total_uss_bytes));
            // Replace Total USS with Total Reclaimable
            let mem_text = format!("Total Reclaimable: {}", format_mib(group.total_uss_bytes));
            let cpu_text = format!("CPU {:>4.1}%", group.total_cpu_percent);
            let bar_frac = (group.total_uss_bytes as f64 / denom).clamp(0.03, 1.0);

            if let Some(cached) = self.rows.get(&group.app_name) {
                *cached.pids.borrow_mut() = group.pids.clone();
                if cached.title_label.label() != title_text {
                    cached.title_label.set_label(&title_text);
                }
                if cached.meta_label.label() != meta_text {
                    cached.meta_label.set_label(&meta_text);
                }
                if cached.mem_badge.label() != mem_text {
                    cached.mem_badge.set_label(&mem_text);
                    apply_memory_badge_style(&cached.mem_badge, group.total_uss_bytes);
                }
                cached.mem_bar.set_fraction(bar_frac);
                if cached.cpu_badge.label() != cpu_text {
                    cached.cpu_badge.set_label(&cpu_text);
                }
            } else {
                let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 14);
                row_box.add_css_class("proc-card-row");

                let left_col = gtk::Box::new(gtk::Orientation::Vertical, 4);
                left_col.set_hexpand(true);
                left_col.set_valign(gtk::Align::Center);

                let title_label = gtk::Label::builder()
                    .label(&title_text)
                    .css_classes(["proc-title"])
                    .halign(gtk::Align::Start)
                    .build();

                let meta_label = gtk::Label::builder()
                    .label(&meta_text)
                    .css_classes(["proc-subtitle"])
                    .halign(gtk::Align::Start)
                    .build();

                left_col.append(&title_label);
                left_col.append(&meta_label);

                let telemetry_col = gtk::Box::new(gtk::Orientation::Vertical, 5);
                telemetry_col.set_valign(gtk::Align::Center);
                telemetry_col.set_size_request(230, -1);

                let badge_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                badge_row.set_halign(gtk::Align::End);

                let cpu_badge = gtk::Label::builder()
                    .label(&cpu_text)
                    .css_classes(["cpu-pill"])
                    .build();
                let mem_badge = gtk::Label::builder().label(&mem_text).build();
                apply_memory_badge_style(&mem_badge, group.total_uss_bytes);

                badge_row.append(&cpu_badge);
                badge_row.append(&mem_badge);

                let mem_bar = gtk::ProgressBar::builder().fraction(bar_frac).build();

                telemetry_col.append(&badge_row);
                telemetry_col.append(&mem_bar);

                let pids_rc = Rc::new(RefCell::new(group.pids.clone()));
                let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                btn_box.set_valign(gtk::Align::Center);

                let silence_all_btn = gtk::Button::builder()
                    .label("Freeze App")
                    .tooltip_text("Pause all processes of this application (0% CPU)")
                    .build();
                {
                    let tx = self.act_tx.clone();
                    let pids = pids_rc.clone();
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    silence_all_btn.connect_clicked(move |_| {
                        let tx_clone = tx.clone();
                        let pids_vec = pids.borrow().clone();
                        vfx_c.trigger_on_widget(
                            &row_anim,
                            VfxKind::EmberStasis,
                            "EMBER STASIS — Freezing app".to_string(),
                            0,
                            move || {
                                let _ = tx_clone.send_blocking(AppAction::ToggleFreezeGroup {
                                    pids: pids_vec,
                                    freeze: true,
                                });
                            },
                        );
                    });
                }

                let resume_all_btn = gtk::Button::builder()
                    .label("Wake App")
                    .css_classes(["suggested-action"])
                    .build();
                {
                    let tx = self.act_tx.clone();
                    let pids = pids_rc.clone();
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    resume_all_btn.connect_clicked(move |_| {
                        let tx_clone = tx.clone();
                        let pids_vec = pids.borrow().clone();
                        vfx_c.trigger_on_widget(
                            &row_anim,
                            VfxKind::MorningSun,
                            "UNICORN REVIVAL — Waking app".to_string(),
                            0,
                            move || {
                                let _ = tx_clone.send_blocking(AppAction::ToggleFreezeGroup {
                                    pids: pids_vec,
                                    freeze: false,
                                });
                            },
                        );
                    });
                }

                let close_app_btn = gtk::Button::builder()
                    .label("Quit App")
                    .css_classes(["destructive-action"])
                    .build();
                {
                    let tx = self.act_tx.clone();
                    let pids = pids_rc.clone();
                    let uss = group.total_uss_bytes;
                    let row_anim = row_box.clone();
                    let vfx_c = self.vfx.clone();
                    close_app_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        let tx_clone = tx.clone();
                        let pids_vec = pids.borrow().clone();
                        let banner = format!("INFERNO — Quit app requested · {}", format_mib(uss));
                        vfx_c.trigger_on_widget(&row_anim, VfxKind::Incinerate, banner, 160, move || {
                            let _ = tx_clone.send_blocking(AppAction::TerminateGroup { pids: pids_vec });
                        });
                    });
                }

                btn_box.append(&silence_all_btn);
                btn_box.append(&resume_all_btn);
                btn_box.append(&close_app_btn);

                row_box.append(&left_col);
                row_box.append(&telemetry_col);
                row_box.append(&btn_box);

                let row_widget = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .child(&row_box)
                    .build();

                self.list_box.append(&row_widget);
                self.rows.insert(
                    group.app_name.clone(),
                    VisualAppGroupRow {
                        row_widget,
                        title_label,
                        meta_label,
                        mem_badge,
                        mem_bar,
                        cpu_badge,
                        pids: pids_rc,
                    },
                );
            }
        }
    }
}

fn build_hero_card(title: &str) -> (gtk::Box, gtk::Label, gtk::Label, gtk::ProgressBar) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("hero-card");
    card.set_hexpand(true);

    let title_lbl = gtk::Label::builder()
        .label(title)
        .css_classes(["hero-title"])
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    let val_lbl = gtk::Label::builder()
        .label("--")
        .css_classes(["hero-value"])
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    let sub_lbl = gtk::Label::builder()
        .label("Initializing...")
        .css_classes(["hero-sub"])
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    let bar = gtk::ProgressBar::builder().hexpand(true).build();

    card.append(&title_lbl);
    card.append(&val_lbl);
    card.append(&bar);
    card.append(&sub_lbl);

    (card, val_lbl, sub_lbl, bar)
}

// fn main() {
//     let app = adw::Application::builder()
//         .application_id("io.sysguard.ubuntu")
//         .build();

//     app.connect_startup(|_| {
//         adw::StyleManager::default().set_color_scheme(adw::ColorScheme::PreferDark);
//         let provider = gtk::CssProvider::new();
//         provider.load_from_data(CUSTOM_CSS);
//         if let Some(display) = gtk::gdk::Display::default() {
//             gtk::style_context_add_provider_for_display(
//                 &display,
//                 &provider,
//                 gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
//             );
//         }
//     });

//     app.connect_activate(build_ui);
//     app.run();
// }

fn main() {
    let app = adw::Application::builder()
        .application_id("io.sysguard.ubuntu")
        .build();

    app.connect_startup(|_| {
        // Change to PreferLight to match the new Rapidash theme
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        // Bundle our own symbolic SVGs: no dependency on the current icon theme.
        if let Some(display) = gtk::gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            theme.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/rapidash-icons"));
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_data(CUSTOM_CSS);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &adw::Application) {
    // Register icon paths after a display is available and diagnose missing assets.
    if let Some(display) = gtk::gdk::Display::default() {
        let icons = gtk::IconTheme::for_display(&display);
        icons.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/rapidash-icons"));
        for name in [
            "rapidash-groups-symbolic",
            "rapidash-process-symbolic",
            "rapidash-browser-symbolic",
            "rapidash-fire-symbolic",
            "rapidash-sparkle-symbolic",
            "rapidash-moon-symbolic",
        ] {
            if !icons.has_icon(name) {
                eprintln!("Rapidash missing bundled icon: {name}");
            }
        }
    }

    let (snap_tx, snap_rx) = async_channel::bounded::<SystemSnapshot>(2);
    let (act_tx, act_rx) = async_channel::unbounded::<AppAction>();
    let (chrome_cmd_tx, chrome_cmd_rx) = async_channel::unbounded::<ChromeCommand>();

    let shared_chrome = SharedChromeState::new();

    // 1. Spawn Chrome WebSocket Bridge Server (127.0.0.1:17421)
    {
        let shared_chrome_ws = shared_chrome.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to start Tokio runtime");
            rt.block_on(run_chrome_ws_server(shared_chrome_ws, chrome_cmd_rx));
        });
    }

    // 2. Spawn Linux /proc + GPU Collector Thread
    {
        let shared_chrome_collector = shared_chrome.clone();
        std::thread::spawn(move || {
            let mut collector = LinuxCollector::new();
            loop {
                while let Ok(action) = act_rx.try_recv() {
                    match action {
                        AppAction::Chrome(cmd) => {
                            let _ = chrome_cmd_tx.send_blocking(cmd);
                        }
                        other => execute_action(other),
                    }
                }
                let (connected, chrome_state) = shared_chrome_collector.get();
                let snapshot = collector.collect_snapshot(connected, chrome_state);
                let _ = snap_tx.try_send(snapshot);
                std::thread::sleep(Duration::from_millis(1500));
            }
        });
    }

    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let vfx = VfxOverlay::new(&root_box);

    let view_stack = adw::ViewStack::new();
    let view_switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();

    let header_bar = adw::HeaderBar::builder()
        .title_widget(&view_switcher)
        .build();

    // =========================================================================
    // TOP-LEFT BRANDING & LOGO IN HEADER BAR
    // =========================================================================
    let brand_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    brand_box.set_valign(gtk::Align::Center);
    brand_box.add_css_class("brand-box");

    let logo_frame = gtk::Frame::new(None);
    logo_frame.add_css_class("brand-logo-frame");

    if let Ok(texture) = gtk::gdk::Texture::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/firefly.jpg"))
        .or_else(|_| gtk::gdk::Texture::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/firefly.png")))
    {
        let logo_image = gtk::Image::from_paintable(Some(&texture));
        logo_image.set_pixel_size(44);
        logo_image.set_size_request(44, 44);
        logo_image.add_css_class("brand-logo");
        logo_frame.set_child(Some(&logo_image));
    }

    let brand_text_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let brand_title = gtk::Label::builder()
        .label("Rapidash")
        .halign(gtk::Align::Start)
        .build();
    brand_title.add_css_class("brand-title");

    let brand_subtitle = gtk::Label::builder()
        .label("Blazing Memory Cleaner")
        .halign(gtk::Align::Start)
        .build();
    brand_subtitle.add_css_class("brand-subtitle");

    brand_text_box.append(&brand_title);
    brand_text_box.append(&brand_subtitle);

    brand_box.append(&logo_frame);
    brand_box.append(&brand_text_box);

    header_bar.pack_start(&brand_box);

    // Header Action Buttons (Chrome GPU Recycler + Portal Leak Fix)
    let recycle_gpu_btn = gtk::Button::builder()
        .label("Purge Chrome GPU")
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();
    let fix_portal_btn = gtk::Button::builder()
        .label("Fix Portal Leak")
        .css_classes(["destructive-action"])
        .visible(false)
        .build();

    // Sun/Moon appearance toggle. Light is the default; no new crate needed.
    let theme_toggle = gtk::ToggleButton::builder()
        .tooltip_text("Switch between Solar Cream and Midnight Ember")
        .css_classes(["theme-toggle"])
        .build();
    let theme_contents = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    let theme_icon = gtk::Image::from_icon_name("rapidash-moon-symbolic");
    theme_icon.set_pixel_size(16);
    let theme_text = gtk::Label::new(Some("Night Ride"));
    theme_contents.append(&theme_icon);
    theme_contents.append(&theme_text);
    theme_toggle.set_child(Some(&theme_contents));

    header_bar.pack_end(&theme_toggle);
    header_bar.pack_end(&recycle_gpu_btn);
    root_box.append(&header_bar);

    // =========================================================================
    // TOP 4 HERO TELEMETRY CARDS (RAM, SWAP, CHROME FOOTPRINT, 4GB GPU VRAM)
    // =========================================================================
    let hero_grid = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hero_grid.set_homogeneous(true); // Locks all 4 cards to identical 25% widths
    hero_grid.set_margin_top(12);
    hero_grid.set_margin_bottom(12);
    hero_grid.set_margin_start(16);
    hero_grid.set_margin_end(16);

    let (ram_card, ram_val, ram_sub, ram_bar) = build_hero_card("SYSTEM RAM (32 GB)");
    let (swap_card, swap_val, swap_sub, swap_bar) = build_hero_card("LINUX SWAP SPACE");
    let (chrome_card, chrome_val, chrome_sub, chrome_bar) =
        build_hero_card("CHROME TOTAL PRIVATE RAM");
    let (gpu_card, gpu_val, gpu_sub, gpu_bar) = build_hero_card("GPU & 4GB VRAM GUARD");

    hero_grid.append(&ram_card);
    hero_grid.append(&swap_card);
    hero_grid.append(&chrome_card);
    hero_grid.append(&gpu_card);
    root_box.append(&hero_grid);

    // =========================================================================
    // INTERACTIVE SEARCH & CATEGORY FILTER BAR
    // =========================================================================
    let filter_bar = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    filter_bar.add_css_class("filter-bar");

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(
            "Search any app, tab, extension, PID, or script (e.g., obsidian, anydesk, code)...",
        )
        .hexpand(true)
        .build();

    let filter_mode = Rc::new(Cell::new(FilterMode::All));
    let filter_buttons: Vec<(FilterMode, &str)> = vec![
        (FilterMode::All, "All"),
        (FilterMode::DesktopApps, "Desktop Apps"),
        (FilterMode::DevTools, "Dev & IDEs"),
        (FilterMode::Browser, "Browsers"),
        (FilterMode::System, "System"),
        (FilterMode::FrozenOnly, "Frozen Only"),
    ];

    filter_bar.append(&search_entry);

    let mut pill_btns = Vec::new();
    for (mode, label) in filter_buttons {
        let btn = gtk::Button::builder().label(label).build();
        if mode == FilterMode::All {
            btn.add_css_class("suggested-action");
        }
        pill_btns.push((mode, btn.clone()));
        filter_bar.append(&btn);
    }
    let pill_btns_rc = Rc::new(pill_btns);
    for (mode, btn) in pill_btns_rc.iter() {
        let mode_val = *mode;
        let fm = filter_mode.clone();
        let all_btns = pill_btns_rc.clone();
        btn.connect_clicked(move |_| {
            fm.set(mode_val);
            for (m, b) in all_btns.iter() {
                if *m == mode_val {
                    b.add_css_class("suggested-action");
                } else {
                    b.remove_css_class("suggested-action");
                }
            }
        });
    }

    root_box.append(&filter_bar);

    // =========================================================================
    // VIEW 1: Application Groups (Obsidian, AnyDesk, VS Code, Chrome...)
    // =========================================================================
    let groups_container = gtk::Box::new(gtk::Orientation::Vertical, 12);
    groups_container.set_margin_top(12);
    groups_container.set_margin_bottom(12);
    groups_container.set_margin_start(16);
    groups_container.set_margin_end(16);

    let mut app_group_manager = AppGroupListManager::new(act_tx.clone(),vfx.clone());
    groups_container.append(&app_group_manager.list_box);

    let groups_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&groups_container)
        .build();

    // =========================================================================
    // VIEW 2: Live Process Explorer (USS Visual Bars + Heat Badges)
    // =========================================================================
    let explorer_container = gtk::Box::new(gtk::Orientation::Vertical, 12);
    explorer_container.set_margin_top(12);
    explorer_container.set_margin_bottom(12);
    explorer_container.set_margin_start(16);
    explorer_container.set_margin_end(16);

    let mut proc_explorer_manager = ProcessExplorerManager::new(act_tx.clone(),vfx.clone());
    explorer_container.append(&proc_explorer_manager.list_box);

    let explorer_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&explorer_container)
        .build();

    // =========================================================================
    // VIEW 3: Dynamic Multi-Window Chrome & Extensions Controller
    // =========================================================================
    let windows_container = gtk::Box::new(gtk::Orientation::Vertical, 16);
    windows_container.set_margin_top(12);
    windows_container.set_margin_bottom(12);
    windows_container.set_margin_start(16);
    windows_container.set_margin_end(16);

    let windows_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&windows_container)
        .build();


    // Register each page ONCE. These icons are shipped with this patch.
    let page1 = view_stack.add_titled(&groups_scroll, Some("groups"), "App Groups");
    page1.set_icon_name(Some("rapidash-groups-symbolic"));

    let page2 = view_stack.add_titled(&explorer_scroll, Some("explorer"), "Process Explorer");
    page2.set_icon_name(Some("rapidash-process-symbolic"));

    let page3 = view_stack.add_titled(&windows_scroll, Some("windows"), "Chrome Tabs");
    page3.set_icon_name(Some("rapidash-browser-symbolic"));

    root_box.append(&view_stack);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Rapidash — Blazing Ubuntu Memory, App Group & Chrome Controller")
        .default_width(1140)
        .default_height(780)
        .content(&vfx.overlay)
        .build();

    window.add_css_class("rapidash-light");
    {
        let window_for_theme = window.clone();
        let icon_for_theme = theme_icon.clone();
        let text_for_theme = theme_text.clone();
        theme_toggle.connect_toggled(move |toggle| {
            let dark = toggle.is_active();
            window_for_theme.remove_css_class(if dark { "rapidash-light" } else { "rapidash-dark" });
            window_for_theme.add_css_class(if dark { "rapidash-dark" } else { "rapidash-light" });
            adw::StyleManager::default().set_color_scheme(if dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            icon_for_theme.set_icon_name(Some(if dark {
                "rapidash-sun-symbolic"
            } else {
                "rapidash-moon-symbolic"
            }));
            text_for_theme.set_label(if dark { "Sunrise" } else { "Night Ride" });
        });
    }

    // GTK4 uses a themed icon name, not window.set_icon(texture).
    // Place firefly.png in assets/; manifest-relative path also works when
    // the executable is launched outside the repository root.
    if let Some(display) = gtk::gdk::Display::default() {
        let icon_theme = gtk::IconTheme::for_display(&display);
        icon_theme.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));
        window.set_icon_name(Some("firefly"));
    }
    window.present();

    {
        let tx = act_tx.clone();
        fix_portal_btn.connect_clicked(move |_| {
            let _ = tx.send_blocking(AppAction::RestartGnomePortal);
        });
    }

    let current_gpu_pid = Rc::new(Cell::new(None::<i32>));
    {
        let tx = act_tx.clone();
        let gpu_pid_cell = current_gpu_pid.clone();
        let vfx_c = vfx.clone();
        recycle_gpu_btn.connect_clicked(move |btn| {
            if let Some(pid) = gpu_pid_cell.get() {
                let tx_clone = tx.clone();
                vfx_c.trigger_on_widget(
                    btn,
                    VfxKind::Supernova,
                    "SOLAR FLARE — GPU restart requested".to_string(),
                    0,
                    move || {
                        let _ = tx_clone.send_blocking(AppAction::RecycleChromeGpu { pid });
                    },
                );
            }
        });
    }

    let mut last_chrome_signature = String::new();

    gtk::glib::spawn_future_local(async move {
        while let Ok(snap) = snap_rx.recv().await {
            // 1. Update Top 4 Hero Cards
            let ram_frac = if snap.total_ram_bytes > 0 {
                snap.used_ram_bytes as f64 / snap.total_ram_bytes as f64
            } else {
                0.0
            };
            ram_val.set_label(&format!(
                "{} / {}",
                format_mib(snap.used_ram_bytes),
                format_mib(snap.total_ram_bytes)
            ));
            ram_sub.set_label(&format!(
                "{:.1}% Used  |  {} Available",
                ram_frac * 100.0,
                format_mib(snap.available_ram_bytes)
            ));
            // Rapidash Pokedex Lore: Top gallop is 150 MPH when system is light & free!
            let gallop_mph = ((1.0 - ram_frac.clamp(0.0, 0.95)) * 150.0).round() as u32;
            ram_sub.set_label(&format!(
                "{:.1}% Used  |  {} Available  |  Gallop {} MPH",
                ram_frac * 100.0,
                format_mib(snap.available_ram_bytes),
                gallop_mph
            ));
            ram_bar.set_fraction(ram_frac.clamp(0.0, 1.0));

            let swap_frac = if snap.total_swap_bytes > 0 {
                snap.used_swap_bytes as f64 / snap.total_swap_bytes as f64
            } else {
                0.0
            };
            swap_val.set_label(&format!(
                "{} / {}",
                format_mib(snap.used_swap_bytes),
                format_mib(snap.total_swap_bytes)
            ));
            swap_sub.set_label(&format!("{:.1}% Paged to Disk", swap_frac * 100.0));
            swap_bar.set_fraction(swap_frac.clamp(0.0, 1.0));

            let chrome_frac = if snap.total_ram_bytes > 0 {
                snap.chrome_total_uss_bytes as f64 / snap.total_ram_bytes as f64
            } else {
                0.0
            };
            chrome_val.set_label(&format_mib(snap.chrome_total_uss_bytes));
            chrome_sub.set_label(&format!(
                "{} Active Tab Renderers  |  {} Windows",
                snap.chrome_tab_count,
                snap.chrome_state.windows.len()
            ));
            chrome_bar.set_fraction(chrome_frac.clamp(0.0, 1.0));

            if snap.gpu.available {
                let vram_frac = if snap.gpu.vram_total_mib > 0 {
                    snap.gpu.vram_used_mib as f64 / snap.gpu.vram_total_mib as f64
                } else {
                    0.0
                };
                gpu_val.set_label(&format!(
                    "{} / {} MiB",
                    snap.gpu.vram_used_mib, snap.gpu.vram_total_mib
                ));
                gpu_sub.set_label(&format!(
                    "Core: {}%  |  Temp: {} C  |  {}",
                    snap.gpu.core_utilization, snap.gpu.temperature_c, snap.gpu.gpu_name
                ));
                gpu_bar.set_fraction(vram_frac.clamp(0.0, 1.0));
            } else {
                gpu_val.set_label("Integrated / DRM");
                gpu_sub.set_label("Use 'Purge Chrome GPU' to flush textures");
                gpu_bar.set_fraction(0.0);
            }

            if let Some((gpu_pid, gpu_uss)) = snap.chrome_gpu_pid {
                current_gpu_pid.set(Some(gpu_pid));
                recycle_gpu_btn.set_sensitive(true);
                recycle_gpu_btn.set_label(&format!("Purge Chrome GPU ({})", format_mib(gpu_uss)));
            } else {
                current_gpu_pid.set(None);
                recycle_gpu_btn.set_sensitive(false);
            }

            if let Some((_pid, portal_uss)) = snap.portal_leak_pid {
                fix_portal_btn.set_visible(true);
                fix_portal_btn.set_label(&format!("Fix Portal Leak ({})", format_mib(portal_uss)));
            } else {
                fix_portal_btn.set_visible(false);
            }

            // Read live search query & category filter
            let query = search_entry.text().to_lowercase();
            let active_filter = filter_mode.get();

            // 2. Sync Application Groups View (Filtered by Search & Category)
            let filtered_groups: Vec<AppGroupSummary> = snap
                .app_groups
                .iter()
                .filter(|g| {
                    let matches_search =
                        query.is_empty() || g.app_name.to_lowercase().contains(&query);
                    let matches_cat = match active_filter {
                        FilterMode::All => true,
                        FilterMode::DesktopApps => g.category == ProcessCategory::DesktopApp,
                        FilterMode::DevTools => g.category == ProcessCategory::DevTool,
                        FilterMode::Browser => g.category == ProcessCategory::Browser,
                        FilterMode::System => g.category == ProcessCategory::SystemDaemon,
                        FilterMode::FrozenOnly => g.frozen_count > 0,
                    };
                    matches_search && matches_cat
                })
                .cloned()
                .collect();
            app_group_manager.sync_groups(&filtered_groups, snap.total_ram_bytes);

            // 3. Sync Live Process Explorer View (In-Place Zero-Lag Diffing)
            let max_uss = snap
                .processes
                .first()
                .map(|p| p.uss_bytes)
                .unwrap_or(500 * 1024 * 1024);

            let filtered_procs: Vec<&ProcessEntry> = snap
                .processes
                .iter()
                .filter(|p| {
                    let matches_search = query.is_empty()
                        || p.app_group.to_lowercase().contains(&query)
                        || p.display_role.to_lowercase().contains(&query)
                        || p.pid.to_string().contains(&query);
                    let matches_cat = match active_filter {
                        FilterMode::All => true,
                        FilterMode::DesktopApps => p.category == ProcessCategory::DesktopApp,
                        FilterMode::DevTools => p.category == ProcessCategory::DevTool,
                        FilterMode::Browser => p.category == ProcessCategory::Browser,
                        FilterMode::System => p.category == ProcessCategory::SystemDaemon,
                        FilterMode::FrozenOnly => p.is_frozen(),
                    };
                    matches_search && matches_cat
                })
                .take(65)
                .collect();
            proc_explorer_manager.sync_processes(&filtered_procs, max_uss);

            // 4. Sync Dynamic N-Window Chrome Inspector (Only when state or search changes)
            let current_sig = format!(
                "{}|{}",
                query,
                serde_json::to_string(&snap.chrome_state).unwrap_or_default()
            );
            if current_sig != last_chrome_signature {
                last_chrome_signature = current_sig;
                rebuild_chrome_windows_view(
                    &windows_container,
                    snap.chrome_bridge_connected,
                    &snap.chrome_state,
                    &query,
                    &act_tx,&vfx
                );
            }
        }
    });
}

fn rebuild_chrome_windows_view(
    container: &gtk::Box,
    connected: bool,
    state: &ChromeBridgePayload,
    query: &str,
    act_tx: &async_channel::Sender<AppAction>,
    vfx: &VfxOverlay
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    if !connected {
        let info_group = adw::PreferencesGroup::builder()
            .title("Load the Companion Extension in Chrome (No Restart Required)")
            .description(
                "1. Open chrome://extensions in Chrome\n\
                 2. Enable 'Developer mode' (top-right)\n\
                 3. Click 'Load unpacked' and select 'sysguard-rs/companion-extension'.\n\
                 All open Chrome Windows (1..N), Tabs, and Extensions will appear here automatically!",
            )
            .build();
        container.append(&info_group);
        return;
    }

    // Global Action Header for All Open Chrome Windows
    let global_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let win_count_lbl = gtk::Label::builder()
        .label(&format!(
            "{} Chrome Windows Connected  |  Use the search bar above to filter tabs across all windows",
            state.windows.len()
        ))
        .halign(gtk::Align::Start)
        .hexpand(true)
        .css_classes(["proc-subtitle"])
        .build();

    let hibernate_all_btn = gtk::Button::builder()
        .label("Hibernate All Inactive Tabs Across All Windows")
        .css_classes(["suggested-action"])
        .build();
    {
        let tx = act_tx.clone();
        let win_ids: Vec<i64> = state.windows.iter().map(|w| w.window_id).collect();
        let vfx_c = vfx.clone();
        hibernate_all_btn.connect_clicked(move |btn| {
            let tx_clone = tx.clone();
            let ids = win_ids.clone();
            vfx_c.trigger_on_widget(
                btn, VfxKind::FlameDash,
                "FLAME GALLOP — Hibernate all windows requested".to_string(),
                0,
                move || {
                    for wid in ids {
                        let _ = tx_clone.send_blocking(AppAction::Chrome(
                            ChromeCommand::HibernateWindow { window_id: wid },
                        ));
                    }
                },
            );
        });
    }
    global_bar.append(&win_count_lbl);
    global_bar.append(&hibernate_all_btn);
    container.append(&global_bar);

    // Dynamically render all N Chrome Windows
    for win in &state.windows {
        let active_tabs_count = win.tabs.iter().filter(|t| !t.discarded).count();
        let discarded_count = win.tabs.len().saturating_sub(active_tabs_count);

        let focus_badge = if win.focused { " [FOCUSED]" } else { "" };
        let group_title = safe_text(&format!(
            "Chrome Window #{}{} - {} Loaded in RAM, {} Hibernated (0 MB)",
            win.window_number, focus_badge, active_tabs_count, discarded_count
        ));

        let hibernate_win_btn = gtk::Button::builder()
            .label("Hibernate Window")
            .css_classes(["suggested-action"])
            .valign(gtk::Align::Center)
            .build();

        {
            let tx = act_tx.clone();
            let win_id = win.window_id;
            let vfx_c = vfx.clone();
            hibernate_win_btn.connect_clicked(move |btn| {
                let tx_clone = tx.clone();
                vfx_c.trigger_on_widget(
                    btn, VfxKind::FlameDash,
                    "FLAME GALLOP — Window hibernation requested".to_string(),
                    0,
                    move || {
                        let _ = tx_clone.send_blocking(AppAction::Chrome(
                            ChromeCommand::HibernateWindow { window_id: win_id },
                        ));
                    },
                );
            });
        }

        let win_group = adw::PreferencesGroup::builder()
            .title(&group_title)
            .header_suffix(&hibernate_win_btn)
            .build();

        for tab in &win.tabs {
            if !query.is_empty()
                && !tab.title.to_lowercase().contains(query)
                && !tab.url.to_lowercase().contains(query)
            {
                continue;
            }

            let row_title = safe_text(&tab.title);
            let row_subtitle = safe_text(&tab.url);

            let row = adw::ActionRow::builder()
                .title(&row_title)
                .subtitle(&row_subtitle)
                .build();

            // Visual Memory State Pill Badge for each Tab
            let state_badge = if tab.discarded {
                gtk::Label::builder()
                    .label("HIBERNATED (0 MB)")
                    .css_classes(["mem-badge-low"])
                    .valign(gtk::Align::Center)
                    .build()
            } else if tab.active {
                gtk::Label::builder()
                    .label("ACTIVE TAB")
                    .css_classes(["mem-badge-med"])
                    .valign(gtk::Align::Center)
                    .build()
            } else if tab.audible {
                gtk::Label::builder()
                    .label("AUDIO PLAYING")
                    .css_classes(["mem-badge-high"])
                    .valign(gtk::Align::Center)
                    .build()
            } else {
                gtk::Label::builder()
                    .label("IN RAM")
                    .css_classes(["mem-badge-high"])
                    .valign(gtk::Align::Center)
                    .build()
            };

            let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            btn_box.set_valign(gtk::Align::Center);
            btn_box.append(&state_badge);

            let focus_btn = gtk::Button::builder().label("Switch To").width_request(95).build();
            {
                let tx = act_tx.clone();
                let tab_id = tab.tab_id;
                let win_id = tab.window_id;
                let vfx_c = vfx.clone();
                let row_anim = row.clone();
                focus_btn.connect_clicked(move |_| {
                    let tx_clone = tx.clone();
                    vfx_c.trigger_on_widget(
                        &row_anim, VfxKind::HornBlink,
                        "UNICORN BLINK — Switching to tab".to_string(),
                        0,
                        move || {
                            let _ = tx_clone.send_blocking(AppAction::Chrome(
                                ChromeCommand::FocusTab { tab_id, window_id: win_id },
                            ));
                        },
                    );
                });
            }

            let discard_btn = gtk::Button::builder()
                .label(if tab.discarded { "Freed" } else { "Hibernate" })
                .width_request(95)
                .sensitive(!tab.discarded && !tab.active)
                .build();
            {
                let tx = act_tx.clone();
                let tab_id = tab.tab_id;
                let row_anim = row.clone();
                let vfx_c = vfx.clone();
                discard_btn.connect_clicked(move |btn| {
                    btn.set_sensitive(false);
                    let tx_clone = tx.clone();
                    vfx_c.trigger_on_widget(
                        &row_anim,
                        VfxKind::FlameDash,
                        "FLAME GALLOP — Tab discard requested".to_string(),
                        160,
                        move || {
                            let _ = tx_clone.send_blocking(AppAction::Chrome(
                                ChromeCommand::DiscardTab { tab_id },
                            ));
                        },
                    );
                });
            }

            let close_btn = gtk::Button::builder()
                .label("Close")
                .width_request(75)
                .css_classes(["destructive-action"])
                .build();
            {
                let tx = act_tx.clone();
                let tab_id = tab.tab_id;
                let row_anim = row.clone();
                let vfx_c = vfx.clone();
                close_btn.connect_clicked(move |btn| {
                    btn.set_sensitive(false);
                    let tx_clone = tx.clone();
                    vfx_c.trigger_on_widget(
                        &row_anim,
                        VfxKind::Incinerate,
                        "INFERNO — Tab close requested".to_string(),
                        160,
                        move || {
                            let _ = tx_clone.send_blocking(AppAction::Chrome(
                                ChromeCommand::CloseTab { tab_id },
                            ));
                        },
                    );
                });
            }

            btn_box.append(&focus_btn);
            btn_box.append(&discard_btn);
            btn_box.append(&close_btn);
            row.add_suffix(&btn_box);
            win_group.add(&row);
        }

        container.append(&win_group);
    }

    // Installed Chrome Extensions Section
    if !state.extensions.is_empty() {
        let ext_group = adw::PreferencesGroup::builder()
            .title("Installed Chrome Extensions (Toggle Off Unused Extensions to Reclaim RAM)")
            .build();

        for ext in &state.extensions {
            if !query.is_empty() && !ext.name.to_lowercase().contains(query) {
                continue;
            }

            let title = safe_text(&ext.name);
            let subtitle = safe_text(&format!("Extension ID: {}", ext.id));

            let row = adw::ActionRow::builder()
                .title(&title)
                .subtitle(&subtitle)
                .build();

            let badge = gtk::Label::builder()
                .label(if ext.enabled { "ACTIVE" } else { "DISABLED (0 MB)" })
                .css_classes([if ext.enabled {
                    "mem-badge-med"
                } else {
                    "mem-badge-low"
                }])
                .valign(gtk::Align::Center)
                .build();

            let toggle_btn = gtk::Button::builder()
                .label(if ext.enabled { "Disable" } else { "Enable" })
                .valign(gtk::Align::Center)
                .build();

            if !ext.enabled {
                toggle_btn.add_css_class("suggested-action");
            }

            {
                let tx = act_tx.clone();
                let ext_id = ext.id.clone();
                let next_state = !ext.enabled;
                let vfx_c = vfx.clone();
                toggle_btn.connect_clicked(move |btn| {
                    let tx_clone = tx.clone();
                    let id = ext_id.clone();
                    let kind = if next_state { VfxKind::MorningSun } else { VfxKind::EmberStasis };
                    let msg = if next_state {
                        "UNICORN REVIVAL — Enable extension requested"
                    } else {
                        "EMBER STASIS — Disable extension requested"
                    };
                    vfx_c.trigger_on_widget(
                        btn, kind, msg.to_string(), 0,
                        move || {
                            let _ = tx_clone.send_blocking(AppAction::Chrome(
                                ChromeCommand::ToggleExtension { ext_id: id, enabled: next_state },
                            ));
                        },
                    );
                });
            }

            let suffix_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            suffix_box.set_valign(gtk::Align::Center);
            suffix_box.append(&badge);
            suffix_box.append(&toggle_btn);

            row.add_suffix(&suffix_box);
            ext_group.add(&row);
        }

        container.append(&ext_group);
    }
}