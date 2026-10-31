use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessCategory {
    Browser,
    DesktopApp,
    DevTool,
    SystemDaemon,
}

impl ProcessCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Browser => "Browser",
            Self::DesktopApp => "Desktop App",
            Self::DevTool => "Dev / IDE",
            Self::SystemDaemon => "System",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: i32,
    pub app_group: String,    // e.g., "Obsidian", "AnyDesk", "Google Chrome", "VS Code"
    pub display_role: String, // e.g., "UI Renderer", "Extension: Bitwarden", "Python — server.py"
    pub category: ProcessCategory,
    pub state: char,
    pub cpu_percent: f32,
    pub uss_bytes: u64,       // Unique Set Size (exact RAM freed immediately on kill/discard)
    pub pss_bytes: u64,       // Proportional Set Size
    pub rss_bytes: u64,
    pub swap_bytes: u64,
}

impl ProcessEntry {
    pub fn is_frozen(&self) -> bool {
        self.state == 'T' || self.state == 't'
    }
}

#[derive(Debug, Clone)]
pub struct AppGroupSummary {
    pub app_name: String,
    pub category: ProcessCategory,
    pub pids: Vec<i32>,
    pub process_count: usize,
    pub frozen_count: usize,
    pub total_uss_bytes: u64,
    pub total_pss_bytes: u64,
    pub total_swap_bytes: u64,
    pub total_cpu_percent: f32,
}

#[derive(Debug, Clone, Default)]
pub struct GpuSnapshot {
    pub available: bool,
    pub gpu_name: String,
    pub core_utilization: u32,
    pub vram_used_mib: u64,
    pub vram_total_mib: u64,
    pub temperature_c: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChromeTabInfo {
    pub tab_id: i64,
    pub window_id: i64,
    pub title: String,
    pub url: String,
    pub active: bool,
    pub discarded: bool,
    pub audible: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChromeWindowInfo {
    pub window_id: i64,
    pub window_number: usize,
    pub focused: bool,
    pub tabs: Vec<ChromeTabInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChromeExtensionInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChromeBridgePayload {
    pub windows: Vec<ChromeWindowInfo>,
    pub extensions: Vec<ChromeExtensionInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub total_ram_bytes: u64,
    pub used_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub total_swap_bytes: u64,
    pub used_swap_bytes: u64,
    pub chrome_total_uss_bytes: u64,
    pub chrome_tab_count: usize,
    pub portal_leak_pid: Option<(i32, u64)>,
    pub chrome_gpu_pid: Option<(i32, u64)>,
    pub gpu: GpuSnapshot,
    pub processes: Vec<ProcessEntry>,
    pub app_groups: Vec<AppGroupSummary>,
    pub chrome_bridge_connected: bool,
    pub chrome_state: ChromeBridgePayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action")]
pub enum ChromeCommand {
    DiscardTab { tab_id: i64 },
    CloseTab { tab_id: i64 },
    FocusTab { tab_id: i64, window_id: i64 },
    HibernateWindow { window_id: i64 },
    ToggleExtension { ext_id: String, enabled: bool },
}

#[derive(Debug, Clone)]
pub enum AppAction {
    ToggleFreeze { pid: i32, currently_frozen: bool },
    ToggleFreezeGroup { pids: Vec<i32>, freeze: bool },
    TerminateGraceful { pid: i32 },
    TerminateGroup { pids: Vec<i32> },
    ForceKill { pid: i32 },
    RecycleChromeGpu { pid: i32 },
    RestartGnomePortal,
    Chrome(ChromeCommand),
}