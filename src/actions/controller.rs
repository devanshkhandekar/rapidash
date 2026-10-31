use crate::types::AppAction;
use std::process::Command;

pub fn execute_action(action: AppAction) {
    match action {
        AppAction::ToggleFreeze {
            pid,
            currently_frozen,
        } => {
            let sig = if currently_frozen {
                libc::SIGCONT
            } else {
                libc::SIGSTOP
            };
            unsafe {
                libc::kill(pid, sig);
            }
        }
        AppAction::ToggleFreezeGroup { pids, freeze } => {
            let sig = if freeze { libc::SIGSTOP } else { libc::SIGCONT };
            for pid in pids {
                unsafe {
                    libc::kill(pid, sig);
                }
            }
        }
        AppAction::TerminateGraceful { pid } => unsafe {
            libc::kill(pid, libc::SIGTERM);
        },
        AppAction::TerminateGroup { pids } => {
            for pid in pids {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
        }
        AppAction::ForceKill { pid } => unsafe {
            libc::kill(pid, libc::SIGKILL);
        },
        AppAction::RecycleChromeGpu { pid } => unsafe {
            libc::kill(pid, libc::SIGTERM);
        },
        AppAction::RestartGnomePortal => {
            let _ = Command::new("systemctl")
                .args(["--user", "restart", "xdg-desktop-portal-gnome"])
                .spawn();
        }
        AppAction::Chrome(_) => {}
    }
}