mod commands;
mod config;
mod dto;
mod error;
mod m3u;
mod model;
mod scanner;
mod store;
mod tags;
mod watcher;

use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use commands::AppContext;

/// 日志：控制台 + 文件（本地应用日志目录，2MB × 5 份滚动，§5.5 / §4.6）
fn logger() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("music-list-editor".into()),
        }))
        .max_file_size(2 * 1024 * 1024)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .level(log::LevelFilter::Info)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(logger())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let loaded = config::load(&app.handle());
            log::info!(
                "应用启动，上次工作区：{}",
                loaded.last_workspace.clone().unwrap_or_else(|| "无".to_string())
            );
            app.manage(AppContext::new(loaded));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace_open,
            commands::workspace_rescan,
            commands::workspace_close,
            commands::folder_block,
            commands::single_open,
            commands::single_promote,
            commands::view_select,
            commands::entries_add_to_playlists,
            commands::entries_remove,
            commands::playlist_remove_tracks,
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
            commands::tracks_delete,
            commands::track_tags_get,
            commands::track_tags_save,
            commands::image_data_url,
            commands::clipboard_write,
            commands::cancel_scan,
            commands::scan_stats,
            commands::undo,
            commands::config_get,
            commands::config_set,
            commands::log_write,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
