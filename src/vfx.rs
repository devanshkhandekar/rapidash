//! Rapidash VFX Fusion: action-specific, click-through Cairo particles plus
//! accessible GTK symbolic-icon notification banners.
//!
//! Uses only gtk4/cairo/glib already depended on by Rapidash.
//! No additional threads, no frame source when idle, no network/assets for particles.
use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Effects intentionally map to commands, not arbitrary celebratory clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfxKind {
    EmberStasis,       // Pause process/app, or disable extension: rotating six-flame seal.
    MorningSun,        // Wake process/app, or enable extension: dawn rays + unicorn horn.
    FlameDash,         // Hibernate Chrome tabs/windows: golden streak + hoofprints.
    Incinerate,        // Graceful Quit / Close tab: rising orange flames + embers.
    InfernoOverdrive,  // Force Quit: stronger crimson twin shockwaves + eruption.
    Supernova,         // GPU recycle: solar rings + radiant wheel.
    HornBlink,         // Focus existing tab: small unicorn-horn warp + sparkles.
}

impl VfxKind {
    fn icon(self) -> &'static str {
        match self {
            Self::EmberStasis => "rapidash-freeze-symbolic",
            Self::MorningSun | Self::HornBlink => "rapidash-sparkle-symbolic",
            Self::FlameDash => "rapidash-gallop-symbolic",
            Self::Incinerate | Self::InfernoOverdrive => "rapidash-fire-symbolic",
            Self::Supernova => "rapidash-gpu-symbolic",
        }
    }
    fn banner_class(self) -> &'static str {
        match self {
            Self::EmberStasis => "fx-ember",
            Self::MorningSun => "fx-unicorn",
            Self::FlameDash => "fx-gallop",
            Self::Incinerate => "fx-inferno",
            Self::InfernoOverdrive => "fx-overdrive",
            Self::Supernova => "fx-supernova",
            Self::HornBlink => "fx-blink",
        }
    }
    fn row_class(self) -> &'static str {
        match self {
            Self::EmberStasis => "anim-ember-pulse",
            Self::MorningSun => "anim-morning-sun",
            Self::FlameDash => "anim-flame-dash",
            Self::Incinerate => "anim-incinerate",
            Self::InfernoOverdrive => "anim-inferno-overdrive",
            Self::Supernova => "anim-supernova",
            Self::HornBlink => "anim-horn-blink",
        }
    }
    fn duration_ms(self) -> u64 {
        match self {
            Self::EmberStasis => 1050,
            Self::MorningSun => 1100,
            Self::FlameDash => 1100,
            Self::Incinerate => 1080,
            Self::InfernoOverdrive => 1200,
            Self::Supernova => 1200,
            Self::HornBlink => 680,
        }
    }
}

#[derive(Clone, Copy)]
struct Particle {
    x: f64, y: f64, vx: f64, vy: f64, size: f64, seed: f64,
}

struct ActiveEffect {
    kind: VfxKind,
    rect: (f64, f64, f64, f64),
    start: Instant,
    duration_ms: u64,
    particles: Vec<Particle>,
}

#[derive(Clone)]
pub struct VfxOverlay {
    pub overlay: gtk::Overlay,
    canvas: gtk::DrawingArea,
    effects: Rc<RefCell<Vec<ActiveEffect>>>,
    loop_running: Rc<Cell<bool>>,
    revealer: gtk::Revealer,
    toast: gtk::Box,
    toast_icon: gtk::Image,
    toast_label: gtk::Label,
    toast_seq: Rc<Cell<u64>>,
}

impl VfxOverlay {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(child));

        // Cairo paints only the VFX shapes; the rest is transparent. The canvas
        // cannot receive pointer events, including during its animation frames.
        let canvas = gtk::DrawingArea::new();
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);
        canvas.set_halign(gtk::Align::Fill);
        canvas.set_valign(gtk::Align::Fill);
        canvas.set_can_target(false);
        let effects: Rc<RefCell<Vec<ActiveEffect>>> = Rc::new(RefCell::new(Vec::new()));
        let effects_to_draw = effects.clone();
        canvas.set_draw_func(move |_, cr, _, _| {
            let now = Instant::now();
            for effect in effects_to_draw.borrow().iter() {
                let elapsed = now.saturating_duration_since(effect.start).as_secs_f64();
                let time = effect.duration_ms as f64 / 1000.0;
                let t = (elapsed / time).clamp(0.0, 1.0);
                draw_effect(cr, effect, t);
            }
        });
        overlay.add_overlay(&canvas);
        overlay.set_measure_overlay(&canvas, false);

        // Independent GTK widget notification: real themed image + legible label.
        // No Cairo text (and no Unicode emoji) so symbols are reliable in GTK.
        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .transition_duration(260)
            .reveal_child(false)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Start)
            .build();
        revealer.set_margin_top(76);
        revealer.set_can_target(false);
        let toast = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        toast.add_css_class("rapidash-toast");
        toast.set_can_target(false);
        if std::env::var("RAPIDASH_REDUCED_MOTION").as_deref() == Ok("1") {
            revealer.set_transition_duration(0);
            toast.add_css_class("fx-reduced-motion");
        }
        let frame = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        frame.add_css_class("rapidash-toast-icon-frame");
        let toast_icon = gtk::Image::from_icon_name("rapidash-fire-symbolic");
        toast_icon.add_css_class("rapidash-toast-icon");
        toast_icon.set_pixel_size(26);
        frame.append(&toast_icon);
        let toast_label = gtk::Label::new(None);
        toast_label.add_css_class("rapidash-toast-label");
        toast_label.set_xalign(0.0);
        toast_label.set_wrap(true);
        toast_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        toast_label.set_max_width_chars(56);
        toast.append(&frame);
        toast.append(&toast_label);
        revealer.set_child(Some(&toast));
        overlay.add_overlay(&revealer);
        overlay.set_measure_overlay(&revealer, false);

        Self {
            overlay, canvas, effects,
            loop_running: Rc::new(Cell::new(false)),
            revealer, toast, toast_icon, toast_label,
            toast_seq: Rc::new(Cell::new(0)),
        }
    }

    /// Runs a matching effect at the clicked widget, then invokes the caller's
    /// *unchanged* backend action. `action_delay_ms` is NOT the VFX duration.
    /// The banner says "requested" until the backend verifies success.
    pub fn trigger_on_widget<W, F>(
        &self,
        widget: &W,
        kind: VfxKind,
        message: String,
        action_delay_ms: u64,
        on_action: F,
    ) where
        W: IsA<gtk::Widget> + Clone + 'static,
        F: FnOnce() + 'static,
    {
        let reduced_motion = std::env::var("RAPIDASH_REDUCED_MOTION")
            .map(|v| v == "1").unwrap_or(false);

        // Update the GTK notification even with reduced-motion enabled.
        let id = self.toast_seq.get().wrapping_add(1);
        self.toast_seq.set(id);
        for class in ["fx-ember", "fx-unicorn", "fx-gallop", "fx-inferno",
                      "fx-overdrive", "fx-supernova", "fx-blink"] {
            self.toast.remove_css_class(class);
        }
        self.toast.add_css_class(kind.banner_class());
        self.toast_icon.set_icon_name(Some(kind.icon()));
        self.toast_label.set_label(&message);
        self.revealer.set_reveal_child(true);
        let hide = self.revealer.clone();
        let seq = self.toast_seq.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(1900), move || {
            if seq.get() == id { hide.set_reveal_child(false); }
        });

        if !reduced_motion {
            // Coordinates relative to the overlay, not the screen or scrolled row.
            // A centered fallback handles initial/unmapped widgets gracefully.
            let rect = if let Some(bounds) = widget.compute_bounds(&self.overlay) {
                (bounds.x() as f64, bounds.y() as f64,
                 (bounds.width() as f64).max(8.0), (bounds.height() as f64).max(8.0))
            } else {
                let w = self.canvas.allocated_width().max(300) as f64;
                let h = self.canvas.allocated_height().max(200) as f64;
                (w * 0.35, h * 0.4, w * 0.3, 60.0)
            };
            let (x, y, w, h) = rect;
            let mut particles = Vec::with_capacity(28);
            // Deterministic scatter: no RNG dependency; per-click phase varies.
            for i in 0..28 {
                let a = i as f64 * 2.399_963_229_728_653;
                let f = ((i as f64 * 18.17 + id as f64).sin() * 437.58).fract().abs();
                let vx = a.cos() * (38.0 + 80.0 * f);
                let vy = -46.0 - f * 135.0;
                particles.push(Particle {
                    x: x + w * ((i as f64 + 0.5) / 28.0),
                    y: y + h * (0.45 + 0.45 * f),
                    vx, vy,
                    size: 2.5 + (i % 4) as f64 * 1.7,
                    seed: f,
                });
            }
            // Limit stacked effects for repeated rapid clicks or bulk actions.
            let mut list = self.effects.borrow_mut();
            if list.len() >= 5 { list.remove(0); }
            list.push(ActiveEffect {
                kind, rect, start: Instant::now(),
                duration_ms: kind.duration_ms(), particles,
            });
            drop(list);
            self.canvas.queue_draw();
            self.run_ticker_if_needed();

            // Row CSS is a supportive accent, not the only animation layer.
            let class = kind.row_class();
            widget.remove_css_class(class);
            widget.add_css_class(class);
            let row = widget.clone();
            gtk::glib::timeout_add_local_once(
                Duration::from_millis(kind.duration_ms()),
                move || { row.remove_css_class(class); },
            );
        }

        // No VFX frame loop delays actual process control.
        if action_delay_ms == 0 {
            on_action();
        } else {
            gtk::glib::timeout_add_local_once(
                Duration::from_millis(action_delay_ms), on_action,
            );
        }
    }

    fn run_ticker_if_needed(&self) {
        if self.loop_running.get() { return; }
        self.loop_running.set(true);
        let canvas = self.canvas.clone();
        let effects = self.effects.clone();
        let running = self.loop_running.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(16), move || {
            let now = Instant::now();
            let mut list = effects.borrow_mut();
            list.retain(|ef| now.saturating_duration_since(ef.start) <
                         Duration::from_millis(ef.duration_ms));
            let more = !list.is_empty();
            drop(list);
            canvas.queue_draw();
            if more {
                gtk::glib::ControlFlow::Continue
            } else {
                running.set(false);
                gtk::glib::ControlFlow::Break
            }
        });
    }
}

// ---------- Vector graphics: fiery Rapidash-inspired, never emoji text ----------
fn draw_effect(cr: &gtk::cairo::Context, ef: &ActiveEffect, t: f64) {
    let (x, y, w, h) = ef.rect;
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let fade = (t / 0.12).min(1.0) * ((1.0 - t) / 0.28).min(1.0);
    let dt = t * ef.duration_ms as f64 / 1000.0;
    let _ = cr.save();
    cr.set_line_cap(gtk::cairo::LineCap::Round);

    match ef.kind {
        VfxKind::Incinerate | VfxKind::InfernoOverdrive => {
            let overdrive = ef.kind == VfxKind::InfernoOverdrive;
            let flames: usize = if w < 160.0 { 7 } else { 16 };
            let step = w / flames as f64;
            let rise = (PI * t).sin().max(0.0);
            cr.set_source_rgba(0.86, 0.18, 0.09, 0.20 * fade);
            rounded_rect(cr, x, y, w, h, 10.0);
            let _ = cr.fill();
            if overdrive {
                for j in 0..2 {
                    let r = 10.0 + t * (h * 1.8 + 65.0 + j as f64 * 28.0);
                    cr.set_line_width(if j == 0 { 4.8 } else { 3.1 });
                    cr.set_source_rgba(1.0, if j == 0 { 0.25 } else { 0.72 }, 0.16, 0.77 * fade);
                    cr.arc(cx, cy, r, 0.0, PI * 2.0);
                    let _ = cr.stroke();
                }
            }
            for i in 0..flames {
                let fx = x + (i as f64 + 0.5) * step;
                let wave = (t * 15.0 + i as f64 * 1.61).sin() * 11.0;
                let fh = (h * 0.9 + (i % 4) as f64 * 11.0) * (0.28 + rise);
                cr.set_source_rgba(0.82, 0.13, 0.07, 0.88 * fade);
                flame(cr, fx, y + h, step * 1.15, fh, wave);
                let _ = cr.fill();
                cr.set_source_rgba(1.0, 0.49, 0.11, 0.98 * fade);
                flame(cr, fx, y + h, step * 0.74, fh * 0.76, wave * 0.6);
                let _ = cr.fill();
                cr.set_source_rgba(1.0, 0.88, 0.36, 0.92 * fade);
                flame(cr, fx, y + h, step * 0.35, fh * 0.52, wave * 0.3);
                let _ = cr.fill();
            }
            draw_particles(cr, ef, dt, fade, (1.0, 0.69, 0.20));
        }
        VfxKind::FlameDash => {
            // Comet head gallops across its own row; three golden hoofmarks follow.
            let head_x = x + w * (t * 1.12).min(1.06);
            let tail = (w * 0.43).min((head_x - x).max(6.0));
            let gradient = gtk::cairo::LinearGradient::new(head_x - tail, cy, head_x, cy);
            gradient.add_color_stop_rgba(0.0, 0.91, 0.26, 0.10, 0.0);
            gradient.add_color_stop_rgba(0.42, 0.99, 0.52, 0.11, 0.39 * fade);
            gradient.add_color_stop_rgba(1.0, 1.0, 0.89, 0.40, 0.88 * fade);
            let _ = cr.set_source(&gradient);
            rounded_rect(cr, head_x - tail, y + h * 0.18, tail, h * 0.64, 8.0);
            let _ = cr.fill();
            for i in 1..=4 {
                let hx = x + w * i as f64 * 0.18;
                if head_x > hx {
                    cr.set_source_rgba(1.0, 0.83, 0.35, 0.96 * fade);
                    horseshoe(cr, hx, cy + if i % 2 == 0 { -9.0 } else { 9.0 }, 7.5);
                    let _ = cr.stroke();
                }
            }
            fire_wheel(cr, head_x, cy, h.clamp(24.0, 64.0) * 0.38, t * 12.0, fade);
            draw_particles(cr, ef, dt, fade * 0.75, (1.0, 0.75, 0.24));
        }
        VfxKind::EmberStasis => {
            cr.set_source_rgba(1.0, 0.54, 0.16, 0.86 * fade);
            cr.set_line_width(2.8);
            rounded_rect(cr, x + 2.0, y + 2.0, (w - 4.0).max(6.0), (h - 4.0).max(6.0), 9.0);
            let _ = cr.stroke();
            let radius = (14.0 + 22.0 * (t * PI).sin().abs()).min(h * 0.6 + 12.0);
            cr.set_source_rgba(0.93, 0.39, 0.13, 0.78 * fade);
            cr.set_line_width(2.5);
            cr.arc(cx, cy, radius, 0.0, PI * 2.0);
            let _ = cr.stroke();
            for i in 0..6 {
                let a = t * 5.0 + i as f64 * PI / 3.0;
                let fx = cx + a.cos() * radius;
                let fy = cy + a.sin() * radius;
                cr.set_source_rgba(1.0, 0.56, 0.17, 0.90 * fade);
                flame(cr, fx, fy + 8.0, 10.0, 19.0, 2.0 * (t * 9.0).sin());
                let _ = cr.fill();
                cr.set_source_rgba(1.0, 0.89, 0.38, 0.97 * fade);
                star(cr, fx, fy - 4.0, 5.5);
                let _ = cr.fill();
            }
        }
        VfxKind::MorningSun => {
            // Unicorn revival: pastel corona, golden dawn, rising horn lance.
            cr.set_source_rgba(1.0, 0.75, 0.92, 0.25 * fade);
            cr.arc(cx, cy, 18.0 + t * 56.0, 0.0, PI * 2.0);
            let _ = cr.fill();
            for i in 0..12 {
                let a = (i as f64) * PI / 6.0 + 0.18 * (t * PI).sin();
                cr.set_source_rgba(1.0, 0.87, 0.41, 0.87 * fade);
                cr.set_line_width(2.6);
                cr.move_to(cx + a.cos() * 12.0, cy + a.sin() * 12.0);
                cr.line_to(cx + a.cos() * (22.0 + t * 88.0), cy + a.sin() * (22.0 + t * 88.0));
                let _ = cr.stroke();
            }
            horn(cr, cx, cy, 50.0 * (0.25 + t * 0.75), fade);
            draw_particles(cr, ef, dt, fade * 0.85, (1.0, 0.84, 0.95));
        }
        VfxKind::Supernova => {
            // Solar flare radiates from the clicked GPU button.
            for i in 0..3 {
                let radius = 12.0 + t * (85.0 + i as f64 * 61.0);
                cr.set_source_rgba(1.0, if i == 1 { 0.39 } else { 0.85 }, 0.24, (0.92 - i as f64 * 0.16) * fade);
                cr.set_line_width(4.4 - i as f64);
                cr.arc(cx, cy, radius, 0.0, PI * 2.0);
                let _ = cr.stroke();
            }
            fire_wheel(cr, cx, cy, 28.0 * (1.0 - t * 0.2), t * 11.0, fade);
            draw_particles(cr, ef, dt, fade, (1.0, 0.88, 0.31));
        }
        VfxKind::HornBlink => {
            // Short, quiet focus/teleport effect. Not a destructive flare.
            cr.set_source_rgba(1.0, 0.81, 0.94, 0.80 * fade);
            cr.set_line_width(2.6);
            for i in 0..3 {
                let r = 10.0 + t * (16.0 + i as f64 * 16.0);
                cr.arc(cx, cy, r, 0.0, PI * 2.0);
                let _ = cr.stroke();
            }
            horn(cr, cx, cy, 27.0, fade);
            for i in 0..8 {
                let a = i as f64 * PI / 4.0 + t * 3.0;
                cr.set_source_rgba(1.0, 0.91, 0.55, fade);
                star(cr, cx + a.cos() * (16.0 + t * 31.0), cy + a.sin() * (16.0 + t * 31.0), 4.8);
                let _ = cr.fill();
            }
        }
    }
    let _ = cr.restore();
}

fn draw_particles(cr: &gtk::cairo::Context, ef: &ActiveEffect, dt: f64, fade: f64, rgb: (f64, f64, f64)) {
    for (idx, p) in ef.particles.iter().enumerate() {
        let spread = if ef.kind == VfxKind::FlameDash { 0.55 } else { 1.0 };
        let px = p.x + p.vx * dt * spread;
        let py = p.y + p.vy * dt + (dt * 8.0 + p.seed * 3.0).sin() * 4.0;
        let bright = if idx % 3 == 0 { 1.0 } else { 0.72 };
        cr.set_source_rgba(rgb.0, rgb.1 * bright, rgb.2 * bright, fade * 0.84);
        star(cr, px, py, p.size * (0.45 + 0.55 * fade));
        let _ = cr.fill();
    }
}

fn rounded_rect(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(1.0);
    cr.new_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, PI * 1.5);
    cr.close_path();
}
fn flame(cr: &gtk::cairo::Context, x: f64, bottom: f64, width: f64, height: f64, drift: f64) {
    let hw = width * 0.5;
    cr.new_path();
    cr.move_to(x - hw, bottom);
    cr.curve_to(x - hw, bottom - height * 0.48,
                x + drift - hw * 0.19, bottom - height * 0.8,
                x + drift, bottom - height);
    cr.curve_to(x + drift + hw * 0.28, bottom - height * 0.72,
                x + hw, bottom - height * 0.4,
                x + hw, bottom);
    cr.close_path();
}
fn star(cr: &gtk::cairo::Context, cx: f64, cy: f64, r: f64) {
    let small = r * 0.30;
    cr.new_path();
    cr.move_to(cx, cy-r);
    cr.line_to(cx+small, cy-small);
    cr.line_to(cx+r, cy);
    cr.line_to(cx+small, cy+small);
    cr.line_to(cx, cy+r);
    cr.line_to(cx-small, cy+small);
    cr.line_to(cx-r, cy);
    cr.line_to(cx-small, cy-small);
    cr.close_path();
}
fn horseshoe(cr: &gtk::cairo::Context, cx: f64, cy: f64, r: f64) {
    cr.set_line_width(3.0);
    cr.new_path();
    cr.arc(cx, cy, r, PI * 0.18, PI * 0.82);
}
fn fire_wheel(cr: &gtk::cairo::Context, cx: f64, cy: f64, r: f64, angle: f64, alpha: f64) {
    for i in 0..8 {
        let a = angle + i as f64 * PI / 4.0;
        cr.set_source_rgba(0.90, 0.16, 0.08, 0.91 * alpha);
        cr.new_path();
        cr.move_to(cx + (a - 0.33).cos() * r * 0.63, cy + (a - 0.33).sin() * r * 0.63);
        cr.line_to(cx + a.cos() * r * 1.37, cy + a.sin() * r * 1.37);
        cr.line_to(cx + (a + 0.33).cos() * r * 0.63, cy + (a + 0.33).sin() * r * 0.63);
        cr.close_path();
        let _ = cr.fill();
    }
    cr.set_source_rgba(1.0, 0.51, 0.13, 0.91 * alpha);
    cr.arc(cx, cy, r * 0.68, 0.0, 2.0 * PI);
    let _ = cr.fill();
    cr.set_source_rgba(1.0, 0.91, 0.45, alpha);
    cr.arc(cx, cy, r * 0.30, 0.0, 2.0 * PI);
    let _ = cr.fill();
}
fn horn(cr: &gtk::cairo::Context, cx: f64, cy: f64, height: f64, alpha: f64) {
    cr.set_source_rgba(1.0, 0.81, 0.97, alpha);
    cr.new_path();
    cr.move_to(cx - 5.0, cy + 13.0);
    cr.line_to(cx + 9.0, cy - height);
    cr.line_to(cx + 11.0, cy + 12.0);
    cr.close_path();
    let _ = cr.fill();
    cr.set_source_rgba(1.0, 0.92, 0.55, alpha);
    cr.set_line_width(2.0);
    cr.move_to(cx + 5.0, cy - height * 0.66);
    cr.line_to(cx + 7.0, cy - height * 0.22);
    let _ = cr.stroke();
}
