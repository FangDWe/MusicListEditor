use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::config;
use crate::dto::*;
use crate::error::{AppError, AppResult};
use crate::store::WorkspaceStore;
use crate::watcher::WorkspaceWatcher;

pub struct AppContext {
    pub store: Arc<Mutex<WorkspaceStore>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub cancel: Arc<AtomicBool>,
    pub watcher: Arc<Mutex<WorkspaceWatcher>>,
}

impl AppContext {
    pub fn new(config: AppConfig) -> Self {
        Self {
            store: Arc::new(Mutex::new(WorkspaceStore::new())),
            config: Arc::new(Mutex::new(config)),
            cancel: Arc::new(AtomicBool::new(false)),
            watcher: Arc::new(Mutex::new(WorkspaceWatcher::new())),
        }
    }
}

/// 在阻塞线程池中执行需要持有 store 的操作，避免卡住 UI 线程
async fn with_store<T, F>(app: AppHandle, state: State<'_, AppContext>, action: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut WorkspaceStore, &AppConfig, &AtomicBool, &AppHandle) -> AppResult<T>
        + Send
        + 'static,
{
    let store = state.store.clone();
    let config_arc = state.config.clone();
    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let config_value = config_arc
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        action(&mut guard, &config_value, &cancel, &app)
    })
    .await
    .map_err(|error| AppError::new("E5000", format!("任务失败：{error}")))?
}


// ---------------------------------------------------------------- workspace

#[tauri::command]
pub async fn workspace_open(
    app: AppHandle,
    state: State<'_, AppContext>,
    path: Option<String>,
) -> AppResult<OpenResult> {
    let store = state.store.clone();
    let config_arc = state.config.clone();
    let cancel = state.cancel.clone();
    let app_handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        cancel.store(false, Ordering::Relaxed);
        let target = match path {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => {
                let picked = app_handle.dialog().file().blocking_pick_folder();
                match picked {
                    Some(file_path) => file_path
                        .into_path()
                        .map_err(|error| AppError::new("E1001", format!("无效路径：{error}")))?,
                    None => return Err(AppError::new("E1002", "已取消")),
                }
            }
        };

        let config_value = config_arc
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let progress_app = app_handle.clone();
        let progress = move |payload: ScanProgress| {
            let _ = progress_app.emit("scan:progress", payload);
        };

        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        let state_value = guard.open_workspace(&target, &config_value, &cancel, &progress)?;
        let stats = guard.scan_stats();
        drop(guard);

        Ok(OpenResult {
            state: state_value,
            scan: stats,
        })
    })
    .await
    .map_err(|error| AppError::new("E5000", format!("任务失败：{error}")))??;

    if let Ok(mut guard) = state.config.lock() {
        if let Some(root) = result.state.workspace.as_ref() {
            let root_text = root.root.clone();
            let mut recent = vec![root_text.clone()];
            for item in guard.recent_workspaces.iter() {
                if item != &root_text && recent.len() < 5 {
                    recent.push(item.clone());
                }
            }
            guard.last_workspace = Some(root_text);
            guard.recent_workspaces = recent;
            let snapshot = guard.clone();
            drop(guard);
            config::save(&app, &snapshot);
        }
    }

    if let Some(workspace) = result.state.workspace.as_ref() {
        if let Ok(mut guard) = state.watcher.lock() {
            guard.watch(app.clone(), PathBuf::from(&workspace.root));
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn workspace_rescan(app: AppHandle, state: State<'_, AppContext>) -> AppResult<OpenResult> {
    let store = state.store.clone();
    let config_arc = state.config.clone();
    let cancel = state.cancel.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancel.store(false, Ordering::Relaxed);
        let config_value = config_arc
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let progress_app = app_handle.clone();
        let progress = move |payload: ScanProgress| {
            let _ = progress_app.emit("scan:progress", payload);
        };
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        let state_value = guard.rescan(&config_value, &cancel, &progress)?;
        let stats = guard.scan_stats();
        Ok(OpenResult {
            state: state_value,
            scan: stats,
        })
    })
    .await
    .map_err(|error| AppError::new("E5000", format!("任务失败：{error}")))?
}

#[tauri::command]
pub async fn workspace_close(app: AppHandle, state: State<'_, AppContext>) -> AppResult<AppState> {
    if let Ok(mut guard) = state.watcher.lock() {
        guard.stop();
    }
    with_store(app, state, |store, _cfg, _cancel, _app| Ok(store.close())).await
}

#[tauri::command]
pub async fn single_open(
    app: AppHandle,
    state: State<'_, AppContext>,
    path: Option<String>,
) -> AppResult<AppState> {
    let store = state.store.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let target = match path {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => {
                let picked = app_handle
                    .dialog()
                    .file()
                    .add_filter("M3U 播放列表", &["m3u", "m3u8"])
                    .blocking_pick_file();
                match picked {
                    Some(file_path) => file_path
                        .into_path()
                        .map_err(|error| AppError::new("E1001", format!("无效路径：{error}")))?,
                    None => return Err(AppError::new("E1002", "已取消")),
                }
            }
        };
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        guard.open_single(&target)
    })
    .await
    .map_err(|error| AppError::new("E5000", format!("任务失败：{error}")))?
}

#[tauri::command]
pub async fn single_promote(app: AppHandle, state: State<'_, AppContext>) -> AppResult<AppState> {
    let store = state.store.clone();
    let config_arc = state.config.clone();
    let cancel = state.cancel.clone();
    let app_handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let config_value = config_arc
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let progress_app = app_handle.clone();
        let progress = move |payload: ScanProgress| {
            let _ = progress_app.emit("scan:progress", payload);
        };
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        guard.promote_single(&config_value, &cancel, &progress)
    })
    .await
    .map_err(|error| AppError::new("E5000", format!("任务失败：{error}")))??;
    if let Some(workspace) = result.workspace.as_ref() {
        if let Ok(mut guard) = state.watcher.lock() {
            guard.watch(app.clone(), PathBuf::from(&workspace.root));
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------- views

#[tauri::command]
pub async fn view_select(
    app: AppHandle,
    state: State<'_, AppContext>,
    kind: String,
    playlist_id: Option<u64>,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.select_view(&kind, playlist_id)
    })
    .await
}

// ---------------------------------------------------------------- mutations

#[tauri::command]
pub async fn entries_add_to_playlists(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_ids: Vec<String>,
    playlist_ids: Vec<u64>,
    allow_dup: Option<bool>,
) -> AppResult<AddResult> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.add_to_playlists(&track_ids, &playlist_ids, allow_dup.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub async fn entries_remove(
    app: AppHandle,
    state: State<'_, AppContext>,
    playlist_id: u64,
    entry_indexes: Vec<usize>,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.remove_entries(playlist_id, &entry_indexes)
    })
    .await
}

#[tauri::command]
pub async fn entries_reorder(
    app: AppHandle,
    state: State<'_, AppContext>,
    playlist_id: u64,
    from: usize,
    to: usize,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.reorder(playlist_id, from, to)
    })
    .await
}

#[tauri::command]
pub async fn playlist_create(
    app: AppHandle,
    state: State<'_, AppContext>,
    name: String,
    dir: Option<String>,
) -> AppResult<AppState> {
    with_store(app, state, move |store, cfg, _cancel, _app| {
        store.create_playlist(&name, dir.as_deref().map(Path::new), cfg)
    })
    .await
}

#[tauri::command]
pub async fn playlist_rename(
    app: AppHandle,
    state: State<'_, AppContext>,
    id: u64,
    new_name: String,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.rename_playlist(id, &new_name)
    })
    .await
}

#[tauri::command]
pub async fn playlist_delete(
    app: AppHandle,
    state: State<'_, AppContext>,
    id: u64,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.delete_playlist(id)
    })
    .await
}

#[tauri::command]
pub async fn playlist_reload(
    app: AppHandle,
    state: State<'_, AppContext>,
    id: u64,
) -> AppResult<AppState> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.reload_playlist(id)
    })
    .await
}

#[tauri::command]
pub async fn playlist_save(
    app: AppHandle,
    state: State<'_, AppContext>,
    id: u64,
    force: Option<bool>,
) -> AppResult<SavePayload> {
    with_store(app, state, move |store, cfg, _cancel, _app| {
        let result = store.save_playlist(id, force.unwrap_or(false), cfg)?;
        Ok(SavePayload {
            results: vec![result],
            state: store.state(),
        })
    })
    .await
}

#[tauri::command]
pub async fn playlist_save_all(
    app: AppHandle,
    state: State<'_, AppContext>,
    force: Option<bool>,
) -> AppResult<SavePayload> {
    with_store(app, state, move |store, cfg, _cancel, _app| {
        Ok(store.save_all(force.unwrap_or(false), cfg))
    })
    .await
}

#[tauri::command]
pub async fn playlists_inclusion(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_ids: Vec<String>,
) -> AppResult<Vec<Inclusion>> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        Ok(store.inclusion(&track_ids))
    })
    .await
}

#[tauri::command]
pub async fn undo(app: AppHandle, state: State<'_, AppContext>) -> AppResult<AppState> {
    with_store(app, state, |store, _cfg, _cancel, _app| store.undo()).await
}

// ---------------------------------------------------------------- tracks

#[tauri::command]
pub async fn track_open_external(
    _app: AppHandle,
    state: State<'_, AppContext>,
    track_id: String,
) -> AppResult<()> {
    let store = state.store.clone();
    let path = {
        let guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        guard.track_path(&track_id)?
    };
    open_with_default(&path)
}

#[tauri::command]
pub async fn track_reveal(
    _app: AppHandle,
    state: State<'_, AppContext>,
    track_id: String,
) -> AppResult<()> {
    let store = state.store.clone();
    let path = {
        let guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        guard.track_path(&track_id)?
    };
    reveal_in_explorer(&path)
}

#[tauri::command]
pub async fn track_properties(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_id: String,
) -> AppResult<TrackProps> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.track_props(&track_id)
    })
    .await
}

#[tauri::command]
pub fn clipboard_write(text: String) -> AppResult<()> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| AppError::new("E5003", format!("剪贴板不可用：{error}")))?;
    clipboard
        .set_text(text)
        .map_err(|error| AppError::new("E5003", format!("写入剪贴板失败：{error}")))
}

// ---------------------------------------------------------------- misc

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppContext>) -> AppResult<()> {
    state.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn scan_stats(state: State<'_, AppContext>) -> AppResult<ScanStats> {
    let guard = state
        .store
        .lock()
        .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
    Ok(guard.scan_stats())
}

#[tauri::command]
pub fn config_get(state: State<'_, AppContext>) -> AppResult<AppConfig> {
    let guard = state
        .config
        .lock()
        .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
    Ok(guard.clone())
}

#[tauri::command]
pub fn config_set(
    app: AppHandle,
    state: State<'_, AppContext>,
    patch: serde_json::Value,
) -> AppResult<AppConfig> {
    let mut guard = state
        .config
        .lock()
        .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
    if let Some(value) = patch.get("writeBom").and_then(|item| item.as_bool()) {
        guard.write_bom = value;
    }
    if let Some(value) = patch.get("backup").and_then(|item| item.as_bool()) {
        guard.backup = value;
    }
    if let Some(values) = patch.get("extensions").and_then(|item| item.as_array()) {
        guard.extensions = values
            .iter()
            .filter_map(|item| item.as_str())
            .map(|item| item.to_lowercase())
            .collect();
    }
    if let Some(values) = patch.get("recentWorkspaces").and_then(|item| item.as_array()) {
        guard.recent_workspaces = values
            .iter()
            .filter_map(|item| item.as_str())
            .map(|item| item.to_string())
            .collect();
    }
    if let Some(value) = patch.get("lastWorkspace").and_then(|item| item.as_str()) {
        guard.last_workspace = Some(value.to_string());
    }
    let snapshot = guard.clone();
    drop(guard);
    config::save(&app, &snapshot);
    Ok(snapshot)
}

// ---------------------------------------------------------------- helpers

#[cfg(windows)]
fn open_with_default(path: &Path) -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::io("无法用默认程序打开", error))
}

#[cfg(not(windows))]
fn open_with_default(_path: &Path) -> AppResult<()> {
    Err(AppError::new("E5003", "当前平台暂不支持"))
}

#[cfg(windows)]
fn reveal_in_explorer(path: &Path) -> AppResult<()> {
    let argument = format!("/select,{}", path.to_string_lossy());
    std::process::Command::new("explorer")
        .arg(argument)
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::io("打开资源管理器失败", error))
}

#[cfg(not(windows))]
fn reveal_in_explorer(_path: &Path) -> AppResult<()> {
    Err(AppError::new("E5003", "当前平台暂不支持"))
}
