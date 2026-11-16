//! Bastion HUD backend.
//!
//! Security model: the window talks to this process only through Tauri's
//! private IPC channel. Nothing listens on a network port, so no website or
//! other program can reach the shells this app opens. The page can only ask
//! for a shell profile by id; the programs behind each id are fixed here.

mod profiles;
mod pty;
mod stats;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(pty::PtyState::default())
        .manage(stats::StatsState::default())
        .invoke_handler(tauri::generate_handler![
            profiles::list_profiles,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            stats::stats_subscribe,
            stats::system_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bastion HUD");
}
