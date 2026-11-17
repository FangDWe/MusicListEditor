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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub write_bom: bool,
    pub backup: bool,
    pub extensions: Vec<String>,
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub last_workspace: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            write_bom: true,
            backup: true,
            extensions: DEFAULT_AUDIO_EXTS.iter().map(|value| value.to_string()).collect(),
            recent_workspaces: Vec::new(),
            last_workspace: None,
        }
    }
}


pub const DEFAULT_AUDIO_EXTS: [&str; 12] = [
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape", "aiff", "aif", "wv",
];

pub const PLAYLIST_EXTS: [&str; 2] = ["m3u", "m3u8"];
