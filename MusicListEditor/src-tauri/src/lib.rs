mod commands;
mod config;
mod dto;
mod error;
mod m3u;
mod model;
mod scanner;
mod store;
mod watcher;

use tauri::Manager;

use commands::AppContext;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let loaded = config::load(&app.handle());
            app.manage(AppContext::new(loaded));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace_open,
            commands::workspace_rescan,
            commands::workspace_close,
            commands::single_open,
            commands::single_promote,
            commands::view_select,
            commands::entries_add_to_playlists,
            commands::entries_remove,
            commands::entries_reorder,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_reload,
            commands::playlist_save,
            commands::playlist_save_all,
            commands::playlists_inclusion,
            commands::track_open_external,
            commands::track_reveal,
            commands::track_properties,
            commands::clipboard_write,
            commands::cancel_scan,
            commands::scan_stats,
            commands::undo,
            commands::config_get,
            commands::config_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
