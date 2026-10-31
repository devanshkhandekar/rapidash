use crate::types::{
    AppGroupSummary, ChromeBridgePayload, GpuSnapshot, ProcessCategory, ProcessEntry,
    SystemSnapshot,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub struct LinuxCollector {
    current_uid: u32,
    prev_proc_ticks: HashMap<i32, u64>,
    prev_total_cpu_ticks: u64,
    num_cores: usize,
    cached_gpu: GpuSnapshot,
    last_gpu_poll: Instant,
}

impl LinuxCollector {
    pub fn new() -> Self {
        let current_uid = unsafe { libc::getuid() };
        let num_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        Self {
            current_uid,
            prev_proc_ticks: HashMap::new(),
            prev_total_cpu_ticks: Self::read_total_cpu_ticks(),
            num_cores,
            cached_gpu: Self::poll_gpu_metrics(),
            last_gpu_poll: Instant::now(),
        }
    }

    pub fn collect_snapshot(
        &mut self,
        chrome_connected: bool,
        chrome_state: ChromeBridgePayload,
    ) -> SystemSnapshot {
        let (total_ram, avail_ram, total_swap, free_swap) = Self::read_meminfo();
        let used_ram = total_ram.saturating_sub(avail_ram);
        let used_swap = total_swap.saturating_sub(free_swap);

        if self.last_gpu_poll.elapsed() >= Duration::from_secs(3) {
            self.cached_gpu = Self::poll_gpu_metrics();
            self.last_gpu_poll = Instant::now();
        }

        let current_total_ticks = Self::read_total_cpu_ticks();
        let total_ticks_delta = current_total_ticks
            .saturating_sub(self.prev_total_cpu_ticks)
            .max(1);
        self.prev_total_cpu_ticks = current_total_ticks;

        let ext_map: HashMap<&str, &str> = chrome_state
            .extensions
            .iter()
            .map(|e| (e.id.as_str(), e.name.as_str()))
            .collect();

        let mut next_proc_ticks = HashMap::new();
        let mut processes = Vec::new();
        let mut chrome_total_uss_bytes = 0u64;
        let mut chrome_tab_count = 0usize;
        let mut portal_leak_pid = None;
        let mut chrome_gpu_pid = None;

        let my_pid = std::process::id() as i32;

        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let Some(pid_str) = file_name.to_str() else { continue };
                let Ok(pid) = pid_str.parse::<i32>() else { continue };

                if pid == my_pid {
                    continue;
                }

                let proc_path = entry.path();
                if !self.is_owned_by_user(&proc_path) {
                    continue;
                }

                let Some((comm, state, proc_ticks)) = Self::read_stat(&proc_path) else {
                    continue;
                };

                let (uss_bytes, pss_bytes, rss_bytes, swap_bytes) =
                    Self::read_smaps_rollup(&proc_path);

                // Filter out tiny < 3 MiB background threads
                if uss_bytes < 3 * 1024 * 1024 {
                    continue;
                }

                let prev_ticks = self.prev_proc_ticks.get(&pid).copied().unwrap_or(proc_ticks);
                next_proc_ticks.insert(pid, proc_ticks);

                let ticks_delta = proc_ticks.saturating_sub(prev_ticks);
                let cpu_percent = ((ticks_delta as f32 / total_ticks_delta as f32)
                    * (self.num_cores as f32)
                    * 100.0)
                    .clamp(0.0, 100.0 * self.num_cores as f32);

                let cmdline = Self::read_cmdline(&proc_path);
                let exe_path = Self::read_exe(&proc_path);

                let (app_group, display_role, category, is_chrome_tab, is_chrome_gpu) =
                    Self::classify_universal_process(
                        &comm,
                        &cmdline,
                        &exe_path,
                        pid,
                        &proc_path,
                        &ext_map,
                    );

                if app_group == "Google Chrome" {
                    chrome_total_uss_bytes += uss_bytes;
                    if is_chrome_tab {
                        chrome_tab_count += 1;
                    }
                    if is_chrome_gpu {
                        chrome_gpu_pid = Some((pid, uss_bytes));
                    }
                }

                if app_group == "GNOME Portal" && uss_bytes > 300 * 1024 * 1024 {
                    portal_leak_pid = Some((pid, uss_bytes));
                }

                processes.push(ProcessEntry {
                    pid,
                    app_group,
                    display_role,
                    category,
                    state,
                    cpu_percent,
                    uss_bytes,
                    pss_bytes,
                    rss_bytes,
                    swap_bytes,
                });
            }
        }

        self.prev_proc_ticks = next_proc_ticks;
        processes.sort_unstable_by(|a, b| b.uss_bytes.cmp(&a.uss_bytes));

        // Build Aggregated Application Groups (Obsidian, AnyDesk, Chrome, VS Code, etc.)
        let mut group_map: HashMap<String, AppGroupSummary> = HashMap::new();
        for p in &processes {
            let entry = group_map
                .entry(p.app_group.clone())
                .or_insert_with(|| AppGroupSummary {
                    app_name: p.app_group.clone(),
                    category: p.category,
                    pids: Vec::new(),
                    process_count: 0,
                    frozen_count: 0,
                    total_uss_bytes: 0,
                    total_pss_bytes: 0,
                    total_swap_bytes: 0,
                    total_cpu_percent: 0.0,
                });
            entry.pids.push(p.pid);
            entry.process_count += 1;
            if p.is_frozen() {
                entry.frozen_count += 1;
            }
            entry.total_uss_bytes += p.uss_bytes;
            entry.total_pss_bytes += p.pss_bytes;
            entry.total_swap_bytes += p.swap_bytes;
            entry.total_cpu_percent += p.cpu_percent;
        }

        let mut app_groups: Vec<AppGroupSummary> = group_map.into_values().collect();
        app_groups.sort_unstable_by(|a, b| b.total_uss_bytes.cmp(&a.total_uss_bytes));

        SystemSnapshot {
            total_ram_bytes: total_ram,
            used_ram_bytes: used_ram,
            available_ram_bytes: avail_ram,
            total_swap_bytes: total_swap,
            used_swap_bytes: used_swap,
            chrome_total_uss_bytes,
            chrome_tab_count,
            portal_leak_pid,
            chrome_gpu_pid,
            gpu: self.cached_gpu.clone(),
            processes,
            app_groups,
            chrome_bridge_connected: chrome_connected,
            chrome_state,
        }
    }

    /// Queries NVIDIA GPU & 4GB VRAM metrics via nvidia-smi (with zero-crash fallback)
    fn poll_gpu_metrics() -> GpuSnapshot {
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 5 {
                        return GpuSnapshot {
                            available: true,
                            gpu_name: parts[0].to_string(),
                            core_utilization: parts[1].parse().unwrap_or(0),
                            vram_used_mib: parts[2].parse().unwrap_or(0),
                            vram_total_mib: parts[3].parse().unwrap_or(4096),
                            temperature_c: parts[4].parse().unwrap_or(0),
                        };
                    }
                }
            }
        }
        GpuSnapshot::default()
    }

    fn is_owned_by_user(&self, proc_path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(proc_path)
            .map(|m| m.uid() == self.current_uid)
            .unwrap_or(false)
    }

    fn read_meminfo() -> (u64, u64, u64, u64) {
        let mut total_ram = 0;
        let mut avail_ram = 0;
        let mut total_swap = 0;
        let mut free_swap = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if let Some(val) = Self::parse_kb_line(line, "MemTotal:") {
                    total_ram = val * 1024;
                } else if let Some(val) = Self::parse_kb_line(line, "MemAvailable:") {
                    avail_ram = val * 1024;
                } else if let Some(val) = Self::parse_kb_line(line, "SwapTotal:") {
                    total_swap = val * 1024;
                } else if let Some(val) = Self::parse_kb_line(line, "SwapFree:") {
                    free_swap = val * 1024;
                }
            }
        }
        (total_ram, avail_ram, total_swap, free_swap)
    }

    fn read_total_cpu_ticks() -> u64 {
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            if let Some(first_line) = content.lines().next() {
                return first_line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|s| s.parse::<u64>().ok())
                    .sum();
            }
        }
        0
    }

    fn read_stat(proc_path: &Path) -> Option<(String, char, u64)> {
        let content = fs::read_to_string(proc_path.join("stat")).ok()?;
        let open_paren = content.find('(')?;
        let close_paren = content.rfind(')')?;
        let comm = content[open_paren + 1..close_paren].to_string();

        let rest = &content[close_paren + 2..];
        let mut parts = rest.split_whitespace();
        let state = parts.next()?.chars().next()?;

        let utime: u64 = parts.nth(10)?.parse().ok()?;
        let stime: u64 = parts.next()?.parse().ok()?;

        Some((comm, state, utime + stime))
    }

    fn read_smaps_rollup(proc_path: &Path) -> (u64, u64, u64, u64) {
        let mut rss_kb = 0u64;
        let mut pss_kb = 0u64;
        let mut priv_clean_kb = 0u64;
        let mut priv_dirty_kb = 0u64;
        let mut swap_kb = 0u64;

        if let Ok(content) = fs::read_to_string(proc_path.join("smaps_rollup")) {
            for line in content.lines() {
                if let Some(v) = Self::parse_kb_line(line, "Rss:") {
                    rss_kb = v;
                } else if let Some(v) = Self::parse_kb_line(line, "Pss:") {
                    pss_kb = v;
                } else if let Some(v) = Self::parse_kb_line(line, "Private_Clean:") {
                    priv_clean_kb = v;
                } else if let Some(v) = Self::parse_kb_line(line, "Private_Dirty:") {
                    priv_dirty_kb = v;
                } else if let Some(v) = Self::parse_kb_line(line, "SwapPss:") {
                    swap_kb = v;
                } else if swap_kb == 0 {
                    if let Some(v) = Self::parse_kb_line(line, "Swap:") {
                        swap_kb = v;
                    }
                }
            }
        }
        let uss_kb = priv_clean_kb + priv_dirty_kb;
        (uss_kb * 1024, pss_kb * 1024, rss_kb * 1024, swap_kb * 1024)
    }

    fn read_cmdline(proc_path: &Path) -> String {
        if let Ok(bytes) = fs::read(proc_path.join("cmdline")) {
            let s: String = bytes
                .into_iter()
                .map(|b| if b == 0 { ' ' } else { b as char })
                .collect();
            return s.trim().to_string();
        }
        String::new()
    }

    fn read_exe(proc_path: &Path) -> String {
        fs::read_link(proc_path.join("exe"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn detect_chrome_extension_from_fd(
        proc_path: &Path,
        ext_map: &HashMap<&str, &str>,
    ) -> Option<String> {
        let fd_dir = proc_path.join("fd");
        let entries = fs::read_dir(fd_dir).ok()?;
        for entry in entries.flatten().take(64) {
            if let Ok(target) = fs::read_link(entry.path()) {
                let path_str = target.to_string_lossy();
                if let Some(idx) = path_str.find("/Extensions/") {
                    let rest = &path_str[idx + "/Extensions/".len()..];
                    if let Some(ext_id) = rest.split('/').next() {
                        if let Some(&ext_name) = ext_map.get(ext_id) {
                            return Some(format!("Extension: {ext_name}"));
                        }
                        return Some(format!("Extension ({ext_id})"));
                    }
                }
            }
        }
        None
    }

    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Universal Classifier: Detects Chrome, Obsidian, AnyDesk, VS Code, Postman, Slack,
    /// Discord, Snaps, Flatpaks, AppImages, Python/Node scripts, and System Daemons.
    fn classify_universal_process(
        comm: &str,
        cmdline: &str,
        exe_path: &str,
        pid: i32,
        proc_path: &Path,
        ext_map: &HashMap<&str, &str>,
    ) -> (String, String, ProcessCategory, bool, bool) {
        let lower_cmd = cmdline.to_lowercase();
        let lower_comm = comm.to_lowercase();
        let lower_exe = exe_path.to_lowercase();
        let combined = format!("{lower_comm} {lower_exe} {lower_cmd}");

        // 1. GNOME Portal / System Daemons
        if combined.contains("xdg-desktop-portal") {
            return (
                "GNOME Portal".to_string(),
                format!("Desktop Portal Service ({comm})"),
                ProcessCategory::SystemDaemon,
                false,
                false,
            );
        }

        // 2. Google Chrome / Chromium / Brave
        if (lower_comm.contains("chrome") || lower_exe.contains("/chrome"))
            && !combined.contains("vscode")
            && !combined.contains("/usr/share/code")
            && !combined.contains("obsidian")
            && !combined.contains("postman")
            && !combined.contains("slack")
            && !combined.contains("discord")
        {
            let app = "Google Chrome".to_string();
            if !lower_cmd.contains("--type=") {
                return (
                    app,
                    "Main Browser & Window Manager".to_string(),
                    ProcessCategory::Browser,
                    false,
                    false,
                );
            }
            if lower_cmd.contains("--type=gpu-process") {
                return (
                    app,
                    "GPU & VRAM Hardware Compositor".to_string(),
                    ProcessCategory::Browser,
                    false,
                    true,
                );
            }
            if lower_cmd.contains("--extension-process") {
                let role = Self::detect_chrome_extension_from_fd(proc_path, ext_map)
                    .unwrap_or_else(|| format!("Extension Background Worker (PID {pid})"));
                return (app, role, ProcessCategory::Browser, false, false);
            }
            if lower_cmd.contains("--type=renderer") {
                if let Some(ext_role) = Self::detect_chrome_extension_from_fd(proc_path, ext_map) {
                    return (app, ext_role, ProcessCategory::Browser, false, false);
                }
                return (
                    app,
                    format!("Web Tab Renderer (PID {pid})"),
                    ProcessCategory::Browser,
                    true,
                    false,
                );
            }
            if lower_cmd.contains("--type=utility") {
                let sub = cmdline
                    .split_whitespace()
                    .find(|arg| arg.starts_with("--utility-sub-type="))
                    .and_then(|arg| arg.split('=').nth(1))
                    .and_then(|s| s.split('.').next_back())
                    .unwrap_or("Utility");
                return (
                    app,
                    format!("Browser Service ({sub})"),
                    ProcessCategory::Browser,
                    false,
                    false,
                );
            }
            return (
                app,
                format!("Worker ({comm})"),
                ProcessCategory::Browser,
                false,
                false,
            );
        }

        // 3. Detect Known & Generic Desktop / Dev Apps (Obsidian, AnyDesk, VS Code, Postman, etc.)
        let known_app: Option<(&str, ProcessCategory)> = if combined.contains("obsidian") {
            Some(("Obsidian", ProcessCategory::DesktopApp))
        } else if combined.contains("anydesk") {
            Some(("AnyDesk", ProcessCategory::DesktopApp))
        } else if lower_comm == "code"
            || combined.contains("/usr/share/code")
            || combined.contains("/snap/code/")
        {
            Some(("VS Code", ProcessCategory::DevTool))
        } else if combined.contains("postman") {
            Some(("Postman", ProcessCategory::DevTool))
        } else if combined.contains("cursor") {
            Some(("Cursor IDE", ProcessCategory::DevTool))
        } else if combined.contains("slack") {
            Some(("Slack", ProcessCategory::DesktopApp))
        } else if combined.contains("discord") {
            Some(("Discord", ProcessCategory::DesktopApp))
        } else if combined.contains("telegram") {
            Some(("Telegram", ProcessCategory::DesktopApp))
        } else if combined.contains("spotify") {
            Some(("Spotify", ProcessCategory::DesktopApp))
        } else if combined.contains("firefox") {
            Some(("Firefox", ProcessCategory::Browser))
        } else if combined.contains("brave") {
            Some(("Brave Browser", ProcessCategory::Browser))
        } else if combined.contains("gnome-shell") {
            Some(("GNOME Shell", ProcessCategory::SystemDaemon))
        } else if combined.contains("nautilus") {
            Some(("Ubuntu Files (Nautilus)", ProcessCategory::SystemDaemon))
        } else if lower_comm.starts_with("python") || lower_exe.contains("/python") {
            Some(("Python Runtime", ProcessCategory::DevTool))
        } else if lower_comm == "node" || lower_exe.ends_with("/node") {
            Some(("Node.js", ProcessCategory::DevTool))
        } else if lower_comm == "java" || lower_exe.ends_with("/java") {
            Some(("Java VM", ProcessCategory::DevTool))
        } else if combined.contains("docker") || combined.contains("containerd") {
            Some(("Docker Engine", ProcessCategory::DevTool))
        } else if combined.contains("postgres") {
            Some(("PostgreSQL", ProcessCategory::DevTool))
        } else {
            None
        };

        let (app_name, category) = if let Some((name, cat)) = known_app {
            (name.to_string(), cat)
        } else if let Some(snap_app) = Self::extract_snap_app(&exe_path, cmdline) {
            (snap_app, ProcessCategory::DesktopApp)
        } else {
            let clean_name = exe_path
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(comm);
            let cat = if lower_exe.starts_with("/usr/libexec")
                || lower_exe.starts_with("/lib/systemd")
                || lower_comm.starts_with("gvfs")
                || lower_comm.starts_with("ibus")
                || lower_comm.starts_with("evolution")
                || lower_comm.starts_with("tracker")
                || lower_comm.starts_with("pipewire")
            {
                ProcessCategory::SystemDaemon
            } else {
                ProcessCategory::DesktopApp
            };
            (Self::capitalize_first(clean_name), cat)
        };

        // 4. Universal Role Dissector (Works for Electron apps like Obsidian/VS Code/Postman + CLI scripts)
        let role = if lower_cmd.contains("--type=extensionhost") {
            "Extension Host Process".to_string()
        } else if lower_cmd.contains("pylance") || lower_cmd.contains("pyright") {
            "Python Language Server (Pylance)".to_string()
        } else if lower_cmd.contains("tsserver") {
            "TypeScript Language Server".to_string()
        } else if lower_cmd.contains("rust-analyzer") {
            "Rust Analyzer Server".to_string()
        } else if lower_cmd.contains("--type=gpu-process") {
            "GPU Hardware Renderer".to_string()
        } else if lower_cmd.contains("--type=renderer") {
            "Window UI Renderer".to_string()
        } else if lower_cmd.contains("--type=utility") {
            "Background Utility Service".to_string()
        } else if lower_cmd.contains("--type=zygote") {
            "Process Zygote Spawner".to_string()
        } else if app_name == "Python Runtime" || app_name == "Node.js" || app_name == "Java VM" {
            let script = cmdline
                .split_whitespace()
                .skip(1)
                .find(|arg| !arg.starts_with('-'))
                .map(|arg| arg.rsplit('/').next().unwrap_or(arg))
                .unwrap_or(comm);
            format!("Script / Service: {script}")
        } else if lower_cmd.contains("--service") || lower_cmd.contains("--tray") {
            format!("{app_name} Background Tray / Daemon")
        } else {
            format!("{app_name} Main Process ({comm})")
        };

        (app_name, role, category, false, false)
    }

    fn extract_snap_app(exe_path: &str, cmdline: &str) -> Option<String> {
        for source in [exe_path, cmdline] {
            if let Some(idx) = source.find("/snap/") {
                let rest = &source[idx + "/snap/".len()..];
                if let Some(app) = rest.split('/').next() {
                    if !app.is_empty() && app != "core" && !app.starts_with("core2") && app != "gnome" {
                        return Some(Self::capitalize_first(app));
                    }
                }
            }
        }
        None
    }

    fn parse_kb_line(line: &str, prefix: &str) -> Option<u64> {
        if line.starts_with(prefix) {
            line[prefix.len()..]
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        } else {
            None
        }
    }
}