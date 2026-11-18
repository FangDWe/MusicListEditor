use serde::{Deserialize, Serialize};

/// IPC DTO：字段名与前端 `shared/types.ts` 一一对应（camelCase）

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub index: usize,
    pub entry_index: Option<usize>,
    pub track_id: Option<String>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub ext: Option<String>,
    pub path: Option<String>,
    pub status: String,
    pub ref_count: usize,
    pub ref_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: u64,
    pub name: String,
    pub rel_path: String,
    pub path: String,
    pub entry_count: usize,
    pub missing_count: usize,
    pub dirty: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub kind: String,
    pub playlist_id: Option<u64>,
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<Row>,
    pub can_save: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub root: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub playlists: usize,
    pub tracks: usize,
    pub unclassified: usize,
    pub missing: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub workspace: Option<WorkspaceInfo>,
    pub playlists: Vec<PlaylistSummary>,
    pub counts: Counts,
    pub view: ViewState,
    pub can_undo: bool,
    /// 当前工作区被屏蔽的相对目录（F2.8）
    pub blocked_dirs: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub current_path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FailedPlaylist {
    pub path: String,
    pub reason: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub skipped_dirs: usize,
    pub blocked_dirs: usize,
    pub failed_playlists: Vec<FailedPlaylist>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    pub state: AppState,
    pub scan: ScanStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFailure {
    pub playlist_id: u64,
    pub name: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddReport {
    pub added: usize,
    pub skipped: usize,
    pub failed: Vec<AddFailure>,
    pub target_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    pub report: AddReport,
    pub state: AppState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub id: u64,
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backed_up: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub converted: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePayload {
    pub results: Vec<SaveResult>,
    pub state: AppState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inclusion {
    pub playlist_id: u64,
    pub included: usize,
    pub total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackProps {
    pub path: String,
    pub size: u64,
    pub ext: String,
    pub ref_count: usize,
    pub ref_names: Vec<String>,
    pub exists: bool,
}

/// 歌曲标签（F6）：标题 / 艺术家 / 专辑 / 封面预览
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// `data:<mime>;base64,...`，无封面时为 None
    pub cover_data_url: Option<String>,
    pub ext: String,
    pub editable: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TagPatch {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// 用磁盘上的图片替换第一张封面（未截取时）
    pub cover_path: Option<String>,
    /// 前端截取后的图片（`data:<mime>;base64,...`），优先于 coverPath
    pub cover_data_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTagsResult {
    pub tags: TrackTags,
    pub state: AppState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFailure {
    pub track_id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub deleted: usize,
    pub failed: Vec<DeleteFailure>,
    /// 因此被置为「未保存」的播放列表数量
    pub affected_playlists: usize,
    /// 删除记录文件路径（文件名逐行追加）
    pub record_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub report: DeleteReport,
    pub state: AppState,
}

/// 被屏蔽的目录（F2.8）：`root` 为工作区根的小写展示路径，`rel` 为相对该根的目录
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockedDir {
    pub root: String,
    pub rel: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub write_bom: bool,
    pub backup: bool,
    pub extensions: Vec<String>,
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub last_workspace: Option<String>,
    /// 启动时自动恢复上次工作区（F2.2）
    #[serde(default = "default_true")]
    pub restore_last: bool,
    /// 用户屏蔽的目录（F2.8）
    #[serde(default)]
    pub blocked_dirs: Vec<BlockedDir>,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            write_bom: true,
            backup: true,
            extensions: DEFAULT_AUDIO_EXTS.iter().map(|value| value.to_string()).collect(),
            recent_workspaces: Vec::new(),
            last_workspace: None,
            restore_last: true,
            blocked_dirs: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 当前工作区被屏蔽的相对目录列表（大小写不敏感）
    pub fn blocked_for(&self, root: &std::path::Path) -> Vec<String> {
        let key = crate::model::display_path(root).to_lowercase();
        self.blocked_dirs
            .iter()
            .filter(|item| item.root.to_lowercase() == key)
            .map(|item| item.rel.clone())
            .collect()
    }
}


pub const DEFAULT_AUDIO_EXTS: [&str; 12] = [
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape", "aiff", "aif", "wv",
];

pub const PLAYLIST_EXTS: [&str; 2] = ["m3u", "m3u8"];
