use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};
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
    .map_err(|error| {
        log::error!("后台任务失败：{error}");
        AppError::new("E5000", format!("任务失败：{error}"))
    })?
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

        log::info!("打开工作区：{}", crate::model::display_path(&target));
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        let state_value = guard.open_workspace(&target, &config_value, &cancel, &progress)?;
        let stats = guard.scan_stats();
        drop(guard);
        log::info!(
            "扫描完成：{} 个播放列表 / {} 首曲目 / {} 未归类，跳过 {} 个无权限目录，屏蔽 {} 个目录，失败 {} 个列表",
            state_value.counts.playlists,
            state_value.counts.tracks,
            state_value.counts.unclassified,
            stats.skipped_dirs,
            stats.blocked_dirs,
            stats.failed_playlists.len()
        );
        for item in &stats.failed_playlists {
            log::warn!("播放列表解析失败：{}（{}）", item.path, item.reason);
        }

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

/// 屏蔽 / 恢复工作区内的某个目录（含其子目录），随后重新扫描（F2.8）
#[tauri::command]
pub async fn folder_block(
    app: AppHandle,
    state: State<'_, AppContext>,
    dir: String,
    blocked: bool,
) -> AppResult<OpenResult> {
    let store = state.store.clone();
    let config_arc = state.config.clone();
    let cancel = state.cancel.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let root = {
            let guard = store
                .lock()
                .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
            guard
                .root()
                .ok_or_else(|| AppError::new("E1001", "请先打开工作区"))?
        };
        let key = crate::model::display_path(&root).to_lowercase();
        let rel = dir
            .trim_matches(|value| value == '\\' || value == '/')
            .to_string();
        if rel.is_empty() {
            return Err(AppError::new("E4007", "不能屏蔽工作区根目录"));
        }

        {
            let mut guard = config_arc
                .lock()
                .map_err(|_| AppError::new("E5000", "配置锁失败"))?;
            if blocked {
                let exists = guard
                    .blocked_dirs
                    .iter()
                    .any(|item| item.root.to_lowercase() == key && item.rel.eq_ignore_ascii_case(&rel));
                if !exists {
                    guard.blocked_dirs.push(BlockedDir {
                        root: key.clone(),
                        rel: rel.clone(),
                    });
                }
            } else {
                guard.blocked_dirs.retain(|item| {
                    !(item.root.to_lowercase() == key && item.rel.eq_ignore_ascii_case(&rel))
                });
            }
        }

        let snapshot = config_arc
            .lock()
            .map_err(|_| AppError::new("E5000", "配置锁失败"))?
            .clone();
        config::save(&app_handle, &snapshot);
        log::info!(
            "{}目录：{}",
            if blocked { "屏蔽" } else { "恢复" },
            rel
        );

        cancel.store(false, Ordering::Relaxed);
        let progress_app = app_handle.clone();
        let progress = move |payload: ScanProgress| {
            let _ = progress_app.emit("scan:progress", payload);
        };
        let mut guard = store
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        let state_value = guard.rescan(&snapshot, &cancel, &progress)?;
        let stats = guard.scan_stats();
        Ok(OpenResult {
            state: state_value,
            scan: stats,
        })
    })
    .await
    .map_err(|error| {
        log::error!("屏蔽目录失败：{error}");
        AppError::new("E5000", format!("任务失败：{error}"))
    })?
}

#[tauri::command]
pub async fn workspace_close(app: AppHandle, state: State<'_, AppContext>) -> AppResult<AppState> {
    if let Ok(mut guard) = state.watcher.lock() {
        guard.stop();
    }
    // 用户主动关闭工作区后，下次启动不再自动恢复（F2.2）
    let snapshot = {
        let mut guard = state
            .config
            .lock()
            .map_err(|_| AppError::new("E5000", "状态锁失败"))?;
        guard.last_workspace = None;
        guard.clone()
    };
    config::save(&app, &snapshot);
    log::info!("关闭工作区");
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
        log::info!(
            "保存播放列表：{}（备份 {}，编码转换 {}）",
            result.name,
            result.backed_up.unwrap_or(false),
            result.converted.unwrap_or(false)
        );
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
        let payload = store.save_all(force.unwrap_or(false), cfg);
        let ok = payload.results.iter().filter(|item| item.ok).count();
        let failed = payload.results.len() - ok;
        if failed > 0 {
            log::warn!("批量保存：成功 {ok} 个，失败 {failed} 个");
            for item in payload.results.iter().filter(|item| !item.ok) {
                log::warn!(
                    "保存失败：{}（{}）",
                    item.name,
                    item.error.clone().unwrap_or_default()
                );
            }
        } else {
            log::info!("批量保存：成功 {ok} 个");
        }
        Ok(payload)
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

/// 删除歌曲文件（进回收站）+ 从全部播放列表移除 + 记录文件名（F4.6）
#[tauri::command]
pub async fn tracks_delete(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_ids: Vec<String>,
) -> AppResult<DeleteResult> {
    with_store(app, state, move |store, _cfg, _cancel, app_handle| {
        let (names, failed, affected) = store.delete_tracks(&track_ids)?;
        let record_path = record_deleted(app_handle, &names);
        Ok(DeleteResult {
            report: DeleteReport {
                deleted: names.len(),
                failed,
                affected_playlists: affected,
                record_path,
            },
            state: store.state(),
        })
    })
    .await
}

#[tauri::command]
pub async fn track_tags_get(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_id: String,
) -> AppResult<TrackTags> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        store.track_tags(&track_id)
    })
    .await
}

#[tauri::command]
pub async fn track_tags_save(
    app: AppHandle,
    state: State<'_, AppContext>,
    track_id: String,
    patch: TagPatch,
) -> AppResult<TrackTagsResult> {
    with_store(app, state, move |store, _cfg, _cancel, _app| {
        let tags = store.save_track_tags(&track_id, &patch)?;
        Ok(TrackTagsResult {
            tags,
            state: store.state(),
        })
    })
    .await
}

/// 读取本地图片并返回 data URL（封面预览 / 截取用，避免开放 fs 插件）
#[tauri::command]
pub fn image_data_url(path: String) -> AppResult<String> {
    crate::tags::image_data_url(Path::new(&path))
}

/// 把删除的歌曲文件名逐行追加到应用数据目录（与日志同级），永久保留、允许重复
fn record_deleted(app: &AppHandle, names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let Ok(dir) = app.path().app_local_data_dir() else {
        log::warn!("无法定位应用数据目录，删除记录未写入");
        return String::new();
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::warn!("创建应用数据目录失败：{error}");
    }
    let path = dir.join("deleted-songs.txt");
    let mut text = String::new();
    for name in names {
        text.push_str(name);
        text.push_str("\r\n");
    }
    use std::io::Write as _;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(error) = file.write_all(text.as_bytes()).and_then(|_| file.flush()) {
                log::error!("写入删除记录失败：{error}");
            }
        }
        Err(error) => log::error!("打开删除记录失败：{error}"),
    }
    crate::model::display_path(&path)
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
    log::warn!("用户取消扫描");
    Ok(())
}

/// 前端日志入口（§5.5）：WebView 侧的错误与关键操作统一写入 Rust 日志文件
#[tauri::command]
pub fn log_write(level: String, message: String) -> AppResult<()> {
    match level.as_str() {
        "error" => log::error!(target: "webview", "{message}"),
        "warn" => log::warn!(target: "webview", "{message}"),
        "debug" => log::debug!(target: "webview", "{message}"),
        _ => log::info!(target: "webview", "{message}"),
    }
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
    if let Some(value) = patch.get("restoreLast").and_then(|item| item.as_bool()) {
        guard.restore_last = value;
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
    use std::os::windows::process::CommandExt;
    // `/select,` 与路径必须拼成一条「原始参数」：若作为普通 arg 传入，
    // std 会给含空格的整串加引号（`"/select,C:\My Music\a.mp3"`），explorer 会把它当文件名。
    let argument = format!("/select,\"{}\"", crate::model::display_path(path));
    std::process::Command::new("explorer")
        .raw_arg(argument)
        .spawn()
        .map(|_| ())
        .map_err(|error| AppError::io("打开资源管理器失败", error))
}

#[cfg(not(windows))]
fn reveal_in_explorer(_path: &Path) -> AppResult<()> {
    Err(AppError::new("E5003", "当前平台暂不支持"))
}
