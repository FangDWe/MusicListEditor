use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;

use crate::dto::*;
use crate::error::{AppError, AppResult};
use crate::m3u::{self, make_track_entry, serialize_playlist};
use crate::model::{
    derive_title, display_path, ext_of, norm_id, Encoding, Entry, EntryKind, Eol, Fingerprint,
    Playlist, Track, TrackId,
};
use crate::scanner::{self, ProgressFn};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Playlist,
    Unclassified,
    All,
    Single,
}

struct ViewSel {
    kind: ViewKind,
    playlist_id: Option<u64>,
}

struct SingleFile {
    path: PathBuf,
    name: String,
    entries: Vec<Entry>,
}

struct Snapshot {
    playlist_id: u64,
    entries: Vec<Entry>,
    dirty: bool,
}

struct UndoOp {
    snapshots: Vec<Snapshot>,
}

pub struct WorkspaceStore {
    root: Option<PathBuf>,
    playlists: Vec<Playlist>,
    tracks: HashMap<TrackId, Track>,
    path_by_id: HashMap<TrackId, PathBuf>,
    unclassified: Vec<TrackId>,
    view: ViewSel,
    single: Option<SingleFile>,
    next_id: u64,
    undo: Vec<UndoOp>,
    skipped_dirs: usize,
    blocked_dirs: usize,
    blocked_rel: Vec<String>,
    failed_playlists: Vec<FailedPlaylist>,
}

impl Default for WorkspaceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceStore {
    pub fn new() -> Self {
        Self {
            root: None,
            playlists: Vec::new(),
            tracks: HashMap::new(),
            path_by_id: HashMap::new(),
            unclassified: Vec::new(),
            view: ViewSel {
                kind: ViewKind::All,
                playlist_id: None,
            },
            single: None,
            next_id: 1,
            undo: Vec::new(),
            skipped_dirs: 0,
            blocked_dirs: 0,
            blocked_rel: Vec::new(),
            failed_playlists: Vec::new(),
        }
    }

    pub fn scan_stats(&self) -> ScanStats {
        ScanStats {
            skipped_dirs: self.skipped_dirs,
            blocked_dirs: self.blocked_dirs,
            failed_playlists: self.failed_playlists.clone(),
        }
    }

    /// 当前工作区根（未打开时 `None`）
    pub fn root(&self) -> Option<PathBuf> {
        self.root.clone()
    }

    // ------------------------------------------------------------ workspace

    pub fn open_workspace(
        &mut self,
        target: &Path,
        config: &AppConfig,
        cancel: &AtomicBool,
        progress: ProgressFn<'_>,
    ) -> AppResult<AppState> {
        let root = target
            .canonicalize()
            .map_err(|_| AppError::new("E1001", "无法访问该文件夹"))?;
        if !root.is_dir() {
            return Err(AppError::new("E1001", "请选择一个文件夹"));
        }

        if let Some(current) = &self.root {
            let current_text = current.to_string_lossy().to_lowercase();
            let next_text = root.to_string_lossy().to_lowercase();
            if current_text != next_text
                && (next_text.starts_with(&format!("{current_text}\\"))
                    || current_text.starts_with(&format!("{next_text}\\")))
            {
                return Err(AppError::new(
                    "E4006",
                    "该文件夹与当前工作区存在包含关系，请先关闭当前工作区",
                ));
            }
        }

        let blocked = config.blocked_for(&root);
        let scan = scanner::scan_folder(&root, &config.extensions, &blocked, cancel, progress);
        if scan.cancelled {
            return Err(AppError::new("E1002", "扫描已取消"));
        }

        let (loaded, failed) = scanner::load_playlists(
            &scan.playlist_paths,
            cancel,
            progress,
            std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(4),
        );
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AppError::new("E1002", "扫描已取消"));
        }

        self.root = Some(root.clone());
        self.single = None;
        self.skipped_dirs = scan.skipped_dirs;
        self.blocked_dirs = scan.blocked_dirs;
        self.blocked_rel = blocked;
        self.failed_playlists = failed;

        let mut playlists: Vec<Playlist> = Vec::new();
        for item in loaded {
            let rel_path = relative_path(&root, &item.path);
            playlists.push(self.make_record(
                &item.path,
                rel_path,
                item.result.entries,
                item.result.encoding,
                item.result.has_bom,
                item.result.eol,
            ));
        }
        self.playlists = playlists;
        self.sort_playlists();

        self.tracks = HashMap::new();
        for track_path in &scan.track_paths {
            let id = norm_id(track_path);
            if self.tracks.contains_key(&id) {
                continue;
            }
            let size = fs::metadata(track_path).map(|meta| meta.len()).unwrap_or(0);
            self.tracks.insert(
                id.clone(),
                Track {
                    id,
                    path: track_path.clone(),
                    title: derive_title(track_path),
                    ext: ext_of(track_path),
                    size,
                    ref_count: 0,
                    ref_names: Vec::new(),
                },
            );
        }

        self.rebuild_references();
        self.rebuild_paths();
        self.view = ViewSel {
            kind: ViewKind::All,
            playlist_id: None,
        };
        Ok(self.state())
    }

    pub fn rescan(
        &mut self,
        config: &AppConfig,
        cancel: &AtomicBool,
        progress: ProgressFn<'_>,
    ) -> AppResult<AppState> {
        let root = self.root.clone().ok_or_else(|| AppError::new("E1001", "尚未打开工作区"))?;
        let previous_kind = self.view.kind;
        let previous_rel = self
            .view
            .playlist_id
            .and_then(|id| self.playlists.iter().find(|item| item.id == id))
            .map(|item| item.rel_path.clone());

        let state = self.open_workspace(&root, config, cancel, progress)?;

        if previous_kind == ViewKind::Playlist {
            if let Some(rel) = previous_rel {
                if let Some(playlist) = self.playlists.iter().find(|item| item.rel_path == rel) {
                    self.view = ViewSel {
                        kind: ViewKind::Playlist,
                        playlist_id: Some(playlist.id),
                    };
                    return Ok(self.state());
                }
            }
        }
        Ok(state)
    }

    pub fn close(&mut self) -> AppState {
        *self = WorkspaceStore::new();
        self.state()
    }

    pub fn open_single(&mut self, path: &Path) -> AppResult<AppState> {
        if !path.is_file() {
            return Err(AppError::new("E1001", "文件不存在"));
        }
        let parsed = m3u::read_playlist_file(path)?;
        *self = WorkspaceStore::new();
        self.single = Some(SingleFile {
            path: path.to_path_buf(),
            name: path
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
            entries: parsed.entries,
        });
        self.view = ViewSel {
            kind: ViewKind::Single,
            playlist_id: None,
        };
        Ok(self.state())
    }

    pub fn promote_single(
        &mut self,
        config: &AppConfig,
        cancel: &AtomicBool,
        progress: ProgressFn<'_>,
    ) -> AppResult<AppState> {
        let single = self
            .single
            .as_ref()
            .ok_or_else(|| AppError::new("E1001", "没有打开的临时列表"))?;
        let dir = single
            .path
            .parent()
            .map(|value| value.to_path_buf())
            .ok_or_else(|| AppError::new("E1001", "无法确定所在目录"))?;
        self.open_workspace(&dir, config, cancel, progress)
    }

    // ---------------------------------------------------------------- views

    pub fn select_view(&mut self, kind: &str, playlist_id: Option<u64>) -> AppResult<AppState> {
        match kind {
            "playlist" => {
                let id = playlist_id.ok_or_else(|| AppError::new("E4001", "播放列表不存在"))?;
                if !self.playlists.iter().any(|item| item.id == id) {
                    return Err(AppError::new("E4001", "播放列表不存在"));
                }
                self.view = ViewSel {
                    kind: ViewKind::Playlist,
                    playlist_id: Some(id),
                };
            }
            "unclassified" => {
                self.view = ViewSel {
                    kind: ViewKind::Unclassified,
                    playlist_id: None,
                };
            }
            "single" => {
                if self.single.is_none() {
                    return Err(AppError::new("E1001", "没有临时列表"));
                }
                self.view = ViewSel {
                    kind: ViewKind::Single,
                    playlist_id: None,
                };
            }
            _ => {
                self.view = ViewSel {
                    kind: ViewKind::All,
                    playlist_id: None,
                };
            }
        }
        Ok(self.state())
    }

    pub fn state(&self) -> AppState {
        let view = self.view_state();
        let missing = self
            .playlists
            .iter()
            .map(|playlist| self.count_missing(playlist))
            .sum();
        AppState {
            workspace: self.root.as_ref().map(|root| WorkspaceInfo {
                root: display_path(root),
                name: root
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| display_path(root)),
            }),
            playlists: self.playlists.iter().map(|item| self.summary(item)).collect(),
            counts: Counts {
                playlists: self.playlists.len(),
                tracks: self.tracks.len(),
                unclassified: self.unclassified.len(),
                missing,
            },
            view,
            can_undo: !self.undo.is_empty(),
            blocked_dirs: self.blocked_rel.clone(),
        }
    }

    fn view_state(&self) -> ViewState {
        if self.view.kind == ViewKind::Single {
            if let Some(single) = &self.single {
                return ViewState {
                    kind: "single".to_string(),
                    playlist_id: None,
                    title: single.name.clone(),
                    subtitle: single.path.to_string_lossy().to_string(),
                    rows: self.entry_rows(&single.entries),
                    can_save: false,
                };
            }
        }
        if self.view.kind == ViewKind::Playlist {
            if let Some(playlist) = self
                .view
                .playlist_id
                .and_then(|id| self.playlists.iter().find(|item| item.id == id))
            {
                return ViewState {
                    kind: "playlist".to_string(),
                    playlist_id: Some(playlist.id),
                    title: playlist.name.clone(),
                    subtitle: playlist.rel_path.clone(),
                    rows: self.entry_rows(&playlist.entries),
                    can_save: playlist.dirty,
                };
            }
        }
        if self.view.kind == ViewKind::Unclassified {
            return ViewState {
                kind: "unclassified".to_string(),
                playlist_id: None,
                title: "未归类".to_string(),
                subtitle: format!("{} 首 · 未被任何播放列表引用", self.unclassified.len()),
                rows: self.track_rows(&self.unclassified),
                can_save: false,
            };
        }
        let all: Vec<TrackId> = self.tracks.keys().cloned().collect();
        ViewState {
            kind: "all".to_string(),
            playlist_id: None,
            title: "全部曲目".to_string(),
            subtitle: format!("{} 首", self.tracks.len()),
            rows: self.track_rows(&all),
            can_save: false,
        }
    }

    fn entry_rows(&self, entries: &[Entry]) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut sequence = 0usize;
        for (index, entry) in entries.iter().enumerate() {
            let status = match entry.kind {
                EntryKind::Track => "ok",
                EntryKind::Missing => "missing",
                EntryKind::Stream => "stream",
                _ => continue,
            };
            sequence += 1;
            let track = entry
                .track_id
                .as_ref()
                .and_then(|id| self.tracks.get(id));
            rows.push(Row {
                key: format!("e:{index}"),
                index: sequence,
                entry_index: Some(index),
                track_id: entry.track_id.clone(),
                title: m3u::entry_title(entry),
                artist: entry.extinf.as_ref().and_then(|info| info.artist.clone()),
                album: None,
                duration: entry.extinf.as_ref().and_then(|info| info.duration),
                ext: m3u::entry_ext(entry),
                path: entry.abs_path.as_ref().map(|path| display_path(path)),
                status: status.to_string(),
                ref_count: track.map(|item| item.ref_count).unwrap_or(0),
                ref_names: track.map(|item| item.ref_names.clone()).unwrap_or_default(),
            });
        }
        rows
    }

    fn track_rows(&self, ids: &[TrackId]) -> Vec<Row> {
        let mut list: Vec<&Track> = ids.iter().filter_map(|id| self.tracks.get(id)).collect();
        list.sort_by(|left, right| {
            left.title
                .to_lowercase()
                .cmp(&right.title.to_lowercase())
        });
        list.into_iter()
            .enumerate()
            .map(|(index, track)| Row {
                key: format!("t:{}", track.id),
                index: index + 1,
                entry_index: None,
                track_id: Some(track.id.clone()),
                title: track.title.clone(),
                artist: None,
                album: None,
                duration: None,
                ext: Some(track.ext.clone()),
                path: Some(display_path(&track.path)),
                status: "ok".to_string(),
                ref_count: track.ref_count,
                ref_names: track.ref_names.clone(),
            })
            .collect()
    }

    // ------------------------------------------------------------ mutations

    pub fn add_to_playlists(
        &mut self,
        track_ids: &[String],
        playlist_ids: &[u64],
        allow_dup: bool,
    ) -> AppResult<AddResult> {
        if self.root.is_none() {
            return Err(AppError::new("E1001", "请先打开工作区"));
        }
        let mut ids: Vec<String> = Vec::new();
        for id in track_ids {
            if !id.is_empty() && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        if ids.is_empty() {
            return Err(AppError::new("E4003", "没有可加入的曲目"));
        }
        let targets: Vec<u64> = playlist_ids
            .iter()
            .copied()
            .filter(|id| self.playlists.iter().any(|item| item.id == *id))
            .collect();
        if targets.is_empty() {
            return Err(AppError::new("E4001", "目标播放列表不存在"));
        }

        let mut snapshots: Vec<Snapshot> = Vec::new();
        let mut added: HashSet<String> = HashSet::new();
        let mut skipped = 0usize;
        let mut failed: Vec<AddFailure> = Vec::new();

        for playlist_id in &targets {
            let index = match self.playlists.iter().position(|item| item.id == *playlist_id) {
                Some(value) => value,
                None => continue,
            };
            let snapshot = Snapshot {
                playlist_id: *playlist_id,
                entries: self.playlists[index].entries.clone(),
                dirty: self.playlists[index].dirty,
            };
            let name = self.playlists[index].name.clone();
            let before = self.playlists[index].entries.len();

            for track_id in &ids {
                let abs = match self.path_by_id.get(track_id) {
                    Some(value) => value.clone(),
                    None => {
                        if !failed.iter().any(|item| item.playlist_id == *playlist_id) {
                            failed.push(AddFailure {
                                playlist_id: *playlist_id,
                                name: name.clone(),
                                reason: "找不到文件路径".to_string(),
                            });
                        }
                        continue;
                    }
                };
                let duplicate = self.playlists[index].entries.iter().any(|entry| {
                    entry.kind == EntryKind::Track
                        && entry.track_id.as_deref() == Some(track_id.as_str())
                });
                if !allow_dup && duplicate {
                    skipped += 1;
                    continue;
                }
                self.playlists[index].entries.push(make_track_entry(&abs));
                added.insert(track_id.clone());
            }

            if self.playlists[index].entries.len() != before {
                snapshots.push(snapshot);
                self.playlists[index].dirty = true;
            }
        }

        if !snapshots.is_empty() {
            self.push_undo(UndoOp { snapshots });
        }
        self.rebuild_references();

        let target_names = targets
            .iter()
            .filter_map(|id| self.playlists.iter().find(|item| item.id == *id))
            .map(|item| item.name.clone())
            .collect();

        Ok(AddResult {
            report: AddReport {
                added: added.len(),
                skipped,
                failed,
                target_names,
            },
            state: self.state(),
        })
    }

    pub fn remove_entries(&mut self, playlist_id: u64, indexes: &[usize]) -> AppResult<AppState> {
        let index = self.require_index(playlist_id)?;
        let remove: HashSet<usize> = indexes.iter().copied().collect();
        if remove.is_empty() {
            return Ok(self.state());
        }
        let snapshot = Snapshot {
            playlist_id,
            entries: self.playlists[index].entries.clone(),
            dirty: self.playlists[index].dirty,
        };
        self.push_undo(UndoOp {
            snapshots: vec![snapshot],
        });
        let kept: Vec<Entry> = self.playlists[index]
            .entries
            .iter()
            .enumerate()
            .filter(|(position, _)| !remove.contains(position))
            .map(|(_, entry)| entry.clone())
            .collect();
        self.playlists[index].entries = kept;
        self.playlists[index].dirty = true;
        self.rebuild_references();
        Ok(self.state())
    }

    pub fn reorder(&mut self, playlist_id: u64, from: usize, to: usize) -> AppResult<AppState> {
        let index = self.require_index(playlist_id)?;
        let length = self.playlists[index].entries.len();
        if from == to || from >= length {
            return Ok(self.state());
        }
        let snapshot = Snapshot {
            playlist_id,
            entries: self.playlists[index].entries.clone(),
            dirty: self.playlists[index].dirty,
        };
        self.push_undo(UndoOp {
            snapshots: vec![snapshot],
        });
        let entry = self.playlists[index].entries.remove(from);
        let target = to.min(self.playlists[index].entries.len());
        self.playlists[index].entries.insert(target, entry);
        self.playlists[index].dirty = true;
        Ok(self.state())
    }

    pub fn create_playlist(
        &mut self,
        name: &str,
        dir: Option<&Path>,
        config: &AppConfig,
    ) -> AppResult<AppState> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| AppError::new("E1001", "请先打开工作区"))?;
        let clean = name.trim();
        if clean.is_empty() {
            return Err(AppError::new("E4004", "播放列表名称不能为空"));
        }
        let lower = clean.to_lowercase();
        let file_name = if lower.ends_with(".m3u") || lower.ends_with(".m3u8") {
            clean.to_string()
        } else {
            format!("{clean}.m3u")
        };
        let target_dir = dir.map(|value| value.to_path_buf()).unwrap_or_else(|| root.clone());
        let full = target_dir.join(file_name);
        if full.exists() {
            return Err(AppError::new("E4005", "同名播放列表已存在"));
        }

        let mut bytes: Vec<u8> = Vec::new();
        if config.write_bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(b"#EXTM3U\r\n");
        fs::write(&full, bytes).map_err(|error| AppError::io("创建播放列表失败", error))?;

        let initial = vec![Entry {
            raw: "#EXTM3U".to_string(),
            kind: EntryKind::Directive,
            abs_path: None,
            track_id: None,
            extinf: None,
        }];
        let rel_path = relative_path(&root, &full);
        let playlist = self.make_record(
            &full,
            rel_path,
            initial,
            if config.write_bom { Encoding::Utf8Bom } else { Encoding::Utf8 },
            config.write_bom,
            Eol::Crlf,
        );
        let id = playlist.id;
        self.playlists.push(playlist);
        self.sort_playlists();
        self.rebuild_references();
        self.view = ViewSel {
            kind: ViewKind::Playlist,
            playlist_id: Some(id),
        };
        Ok(self.state())
    }

    pub fn rename_playlist(&mut self, playlist_id: u64, new_name: &str) -> AppResult<AppState> {
        let index = self.require_index(playlist_id)?;
        let clean = new_name.trim();
        if clean.is_empty() {
            return Err(AppError::new("E4004", "播放列表名称不能为空"));
        }
        let current = self.playlists[index].path.clone();
        let extension = current
            .extension()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "m3u".to_string());
        let lower = clean.to_lowercase();
        let file_name = if lower.ends_with(&format!(".{}", extension.to_lowercase())) {
            clean.to_string()
        } else {
            format!("{clean}.{extension}")
        };
        let parent = current
            .parent()
            .map(|value| value.to_path_buf())
            .unwrap_or_default();
        let next = parent.join(file_name);
        if next == current {
            return Ok(self.state());
        }
        if next.exists() {
            return Err(AppError::new("E4005", "同名播放列表已存在"));
        }
        fs::rename(&current, &next).map_err(|error| AppError::io("重命名失败", error))?;

        let root = self.root.clone();
        self.playlists[index].path = next.clone();
        self.playlists[index].name = next
            .file_stem()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        self.playlists[index].rel_path = root
            .as_ref()
            .map(|root| relative_path(root, &next))
            .unwrap_or_else(|| self.playlists[index].name.clone());
        self.playlists[index].fingerprint = fingerprint(&next);
        self.sort_playlists();
        Ok(self.state())
    }

    pub fn delete_playlist(&mut self, playlist_id: u64) -> AppResult<AppState> {
        let index = self.require_index(playlist_id)?;
        let path = self.playlists[index].path.clone();
        trash::delete(&path).map_err(|error| {
            AppError::new("E3005", format!("移入回收站失败：{error}"))
        })?;
        self.playlists.remove(index);
        self.undo.retain(|op| {
            !op.snapshots
                .iter()
                .any(|snapshot| snapshot.playlist_id == playlist_id)
        });
        self.rebuild_references();
        if self.view.kind == ViewKind::Playlist && self.view.playlist_id == Some(playlist_id) {
            self.view = ViewSel {
                kind: ViewKind::All,
                playlist_id: None,
            };
        }
        Ok(self.state())
    }

    pub fn reload_playlist(&mut self, playlist_id: u64) -> AppResult<AppState> {
        let index = self.require_index(playlist_id)?;
        let path = self.playlists[index].path.clone();
        let parsed = m3u::read_playlist_file(&path)?;
        self.playlists[index].entries = parsed.entries;
        self.playlists[index].src_encoding = parsed.encoding;
        self.playlists[index].has_bom = parsed.has_bom;
        self.playlists[index].eol = parsed.eol;
        self.playlists[index].dirty = false;
        self.playlists[index].fingerprint = fingerprint(&path);
        self.rebuild_references();
        self.rebuild_paths();
        Ok(self.state())
    }

    // ---------------------------------------------------------------- save

    pub fn save_playlist(
        &mut self,
        playlist_id: u64,
        force: bool,
        config: &AppConfig,
    ) -> AppResult<SaveResult> {
        let index = self.require_index(playlist_id)?;
        let path = self.playlists[index].path.clone();
        let meta = fs::metadata(&path)
            .map_err(|_| AppError::new("E3002", "文件不存在或不可访问"))?;
        let current = Fingerprint {
            mtime_ms: mtime_ms(&meta),
            size: meta.len(),
        };
        let stored = self.playlists[index].fingerprint;
        if !force && (current.mtime_ms != stored.mtime_ms || current.size != stored.size) {
            return Err(AppError::new("E3001", "播放列表已在外部修改"));
        }

        let converting = !matches!(
            self.playlists[index].src_encoding,
            Encoding::Utf8 | Encoding::Utf8Bom
        );
        let mut backed_up = false;
        if config.backup || converting {
            let backup = backup_path(&path);
            if fs::copy(&path, &backup).is_ok() {
                backed_up = true;
            }
        }

        let text = serialize_playlist(&self.playlists[index]);
        let mut bytes: Vec<u8> = Vec::new();
        if config.write_bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(text.as_bytes());

        let temp = temp_path(&path);
        {
            let mut file = fs::File::create(&temp)
                .map_err(|error| AppError::io("创建临时文件失败", error))?;
            file.write_all(&bytes)
                .map_err(|error| AppError::io("写入失败", error))?;
            file.sync_all()
                .map_err(|error| AppError::io("同步失败", error))?;
        }
        fs::rename(&temp, &path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            AppError::io("替换原文件失败", error)
        })?;

        self.playlists[index].fingerprint = fingerprint(&path);
        self.playlists[index].dirty = false;
        self.playlists[index].src_encoding = if config.write_bom {
            Encoding::Utf8Bom
        } else {
            Encoding::Utf8
        };
        self.playlists[index].has_bom = config.write_bom;

        // 写回是标准化过程（F4.3）：落盘后按新文件重建内存条目，保证内存与磁盘一致
        if let Ok(parsed) = m3u::read_playlist_file(&path) {
            self.playlists[index].entries = parsed.entries;
            self.playlists[index].eol = parsed.eol;
            self.playlists[index].fingerprint = fingerprint(&path);
            self.rebuild_references();
            self.rebuild_paths();
        }

        Ok(SaveResult {
            id: playlist_id,
            name: self.playlists[index].name.clone(),
            ok: true,
            error: None,
            backed_up: Some(backed_up),
            converted: Some(converting),
        })
    }

    pub fn save_all(&mut self, force: bool, config: &AppConfig) -> SavePayload {
        let dirty: Vec<u64> = self
            .playlists
            .iter()
            .filter(|item| item.dirty)
            .map(|item| item.id)
            .collect();
        let mut results = Vec::new();
        for id in dirty {
            match self.save_playlist(id, force, config) {
                Ok(result) => results.push(result),
                Err(error) => {
                    let name = self
                        .playlists
                        .iter()
                        .find(|item| item.id == id)
                        .map(|item| item.name.clone())
                        .unwrap_or_default();
                    results.push(SaveResult {
                        id,
                        name,
                        ok: false,
                        error: Some(error.message),
                        backed_up: None,
                        converted: None,
                    });
                }
            }
        }
        SavePayload {
            results,
            state: self.state(),
        }
    }

    // ----------------------------------------------------------------- misc

    pub fn inclusion(&self, track_ids: &[String]) -> Vec<Inclusion> {
        let mut ids: Vec<&String> = Vec::new();
        for id in track_ids {
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
        }
        self.playlists
            .iter()
            .map(|playlist| {
                let present: HashSet<&str> = playlist
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == EntryKind::Track)
                    .filter_map(|entry| entry.track_id.as_deref())
                    .collect();
                let included = ids
                    .iter()
                    .filter(|id| present.contains(id.as_str()))
                    .count();
                Inclusion {
                    playlist_id: playlist.id,
                    included,
                    total: ids.len(),
                }
            })
            .collect()
    }

    pub fn track_props(&self, track_id: &str) -> AppResult<TrackProps> {
        let path = self
            .path_by_id
            .get(track_id)
            .ok_or_else(|| AppError::new("E4001", "找不到该曲目"))?
            .clone();
        let track = self.tracks.get(track_id);
        let mut size = track.map(|item| item.size).unwrap_or(0);
        let mut exists = false;
        if let Ok(meta) = fs::metadata(&path) {
            exists = meta.is_file();
            size = meta.len();
        }
        Ok(TrackProps {
            path: display_path(&path),
            size,
            ext: ext_of(&path),
            ref_count: track.map(|item| item.ref_count).unwrap_or(0),
            ref_names: track.map(|item| item.ref_names.clone()).unwrap_or_default(),
            exists,
        })
    }

    pub fn track_path(&self, track_id: &str) -> AppResult<PathBuf> {
        self.path_by_id
            .get(track_id)
            .cloned()
            .ok_or_else(|| AppError::new("E4001", "找不到该曲目"))
    }

    // ------------------------------------------------------- 删除歌曲（F4.6）

    /// 物理删除歌曲文件（进回收站），并从全部播放列表移除其条目（F4.6）。
    /// 返回：（被删除的文件名，失败项，受影响的播放列表数量）
    pub fn delete_tracks(
        &mut self,
        track_ids: &[String],
    ) -> AppResult<(Vec<String>, Vec<DeleteFailure>, usize)> {
        if self.root.is_none() {
            return Err(AppError::new("E1001", "请先打开工作区"));
        }
        let mut ids: Vec<String> = Vec::new();
        for id in track_ids {
            if !id.is_empty() && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        if ids.is_empty() {
            return Err(AppError::new("E4003", "没有可删除的曲目"));
        }

        let mut deleted_names: Vec<String> = Vec::new();
        let mut failed: Vec<DeleteFailure> = Vec::new();
        let mut removed: HashSet<TrackId> = HashSet::new();

        for id in &ids {
            let path = self
                .tracks
                .get(id)
                .map(|track| track.path.clone())
                .or_else(|| self.path_by_id.get(id).cloned());
            let Some(path) = path else {
                failed.push(DeleteFailure {
                    track_id: id.clone(),
                    name: id.clone(),
                    reason: "找不到该曲目".to_string(),
                });
                continue;
            };
            let name = path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| display_path(&path));
            if !path.is_file() {
                failed.push(DeleteFailure {
                    track_id: id.clone(),
                    name,
                    reason: "文件不存在".to_string(),
                });
                continue;
            }
            match trash::delete(&path) {
                Ok(()) => {
                    log::info!("删除歌曲文件：{}", display_path(&path));
                    deleted_names.push(name);
                    removed.insert(id.clone());
                }
                Err(error) => failed.push(DeleteFailure {
                    track_id: id.clone(),
                    name,
                    reason: format!("移入回收站失败：{error}"),
                }),
            }
        }

        if removed.is_empty() {
            return Ok((deleted_names, failed, 0));
        }

        // 从所有播放列表移除对应条目，并置为未保存（由用户决定何时写回）
        let mut snapshots: Vec<Snapshot> = Vec::new();
        let mut affected = 0usize;
        for index in 0..self.playlists.len() {
            let hit = self.playlists[index]
                .entries
                .iter()
                .any(|entry| entry.track_id.as_ref().is_some_and(|id| removed.contains(id)));
            if !hit {
                continue;
            }
            snapshots.push(Snapshot {
                playlist_id: self.playlists[index].id,
                entries: self.playlists[index].entries.clone(),
                dirty: self.playlists[index].dirty,
            });
            self.playlists[index]
                .entries
                .retain(|entry| !entry.track_id.as_ref().is_some_and(|id| removed.contains(id)));
            self.playlists[index].dirty = true;
            affected += 1;
        }
        if !snapshots.is_empty() {
            self.push_undo(UndoOp { snapshots });
        }

        for id in &removed {
            self.tracks.remove(id);
            self.path_by_id.remove(id);
        }
        self.rebuild_references();
        self.rebuild_paths();

        Ok((deleted_names, failed, affected))
    }

    // ------------------------------------------------------- 标签编辑（F6）

    pub fn track_tags(&self, track_id: &str) -> AppResult<TrackTags> {
        let path = self.track_path(track_id)?;
        crate::tags::read(&path)
    }

    pub fn save_track_tags(&mut self, track_id: &str, patch: &TagPatch) -> AppResult<TrackTags> {
        let path = self.track_path(track_id)?;
        let info = crate::tags::write(&path, patch)?;
        // 列表标题跟随新标签，避免「改了看不到」
        if let Some(title) = info.title.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            if let Some(track) = self.tracks.get_mut(track_id) {
                track.title = title.to_string();
            }
        }
        Ok(info)
    }

    pub fn undo(&mut self) -> AppResult<AppState> {
        let op = self
            .undo
            .pop()
            .ok_or_else(|| AppError::new("E5002", "没有可撤销的操作"))?;
        for snapshot in op.snapshots {
            if let Some(playlist) = self
                .playlists
                .iter_mut()
                .find(|item| item.id == snapshot.playlist_id)
            {
                playlist.entries = snapshot.entries;
                let differs = content_differs(playlist);
                playlist.dirty = snapshot.dirty || differs;
            }
        }
        self.rebuild_references();
        Ok(self.state())
    }

    // ------------------------------------------------------------ internals

    fn require_index(&self, playlist_id: u64) -> AppResult<usize> {
        self.playlists
            .iter()
            .position(|item| item.id == playlist_id)
            .ok_or_else(|| AppError::new("E4001", "播放列表不存在"))
    }

    fn make_record(
        &mut self,
        path: &Path,
        rel_path: String,
        entries: Vec<Entry>,
        encoding: Encoding,
        has_bom: bool,
        eol: Eol,
    ) -> Playlist {
        let id = self.next_id;
        self.next_id += 1;
        Playlist {
            id,
            path: path.to_path_buf(),
            rel_path,
            name: path
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default(),
            entries,
            src_encoding: encoding,
            has_bom,
            eol,
            dirty: false,
            fingerprint: fingerprint(path),
        }
    }

    fn sort_playlists(&mut self) {
        self.playlists
            .sort_by(|left, right| left.rel_path.to_lowercase().cmp(&right.rel_path.to_lowercase()));
    }

    fn summary(&self, playlist: &Playlist) -> PlaylistSummary {
        let entry_count = playlist
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    EntryKind::Track | EntryKind::Missing | EntryKind::Stream
                )
            })
            .count();
        PlaylistSummary {
            id: playlist.id,
            name: playlist.name.clone(),
            rel_path: playlist.rel_path.clone(),
            path: display_path(&playlist.path),
            entry_count,
            missing_count: self.count_missing(playlist),
            dirty: playlist.dirty,
        }
    }

    fn count_missing(&self, playlist: &Playlist) -> usize {
        playlist
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Missing)
            .count()
    }

    fn push_undo(&mut self, op: UndoOp) {
        self.undo.push(op);
        if self.undo.len() > 20 {
            self.undo.remove(0);
        }
    }

    /// 重算引用关系与未归类（F2.5）
    fn rebuild_references(&mut self) {
        let mut referenced: HashSet<TrackId> = HashSet::new();
        let mut counts: HashMap<TrackId, (usize, Vec<String>)> = HashMap::new();
        for playlist in &self.playlists {
            for entry in &playlist.entries {
                if entry.kind != EntryKind::Track {
                    continue;
                }
                if let Some(id) = &entry.track_id {
                    referenced.insert(id.clone());
                    let slot = counts.entry(id.clone()).or_insert((0, Vec::new()));
                    slot.0 += 1;
                    if !slot.1.contains(&playlist.name) {
                        slot.1.push(playlist.name.clone());
                    }
                }
            }
        }
        for track in self.tracks.values_mut() {
            let (ref_count, ref_names) = counts
                .get(&track.id)
                .cloned()
                .unwrap_or((0, Vec::new()));
            track.ref_count = ref_count;
            track.ref_names = ref_names;
        }
        self.unclassified = self
            .tracks
            .keys()
            .filter(|id| !referenced.contains(*id))
            .cloned()
            .collect();
    }

    /// 曲目 id → 真实路径，供「加入播放列表」「打开」「定位」使用
    fn rebuild_paths(&mut self) {
        self.path_by_id.clear();
        for track in self.tracks.values() {
            self.path_by_id.insert(track.id.clone(), track.path.clone());
        }
        for playlist in &self.playlists {
            for entry in &playlist.entries {
                if let (Some(id), Some(path)) = (&entry.track_id, &entry.abs_path) {
                    self.path_by_id.entry(id.clone()).or_insert_with(|| path.clone());
                }
            }
        }
    }
}

fn relative_path(root: &Path, target: &Path) -> String {
    match target.strip_prefix(root) {
        Ok(rest) => rest.to_string_lossy().to_string(),
        Err(_) => display_path(target),
    }
}

fn fingerprint(path: &Path) -> Fingerprint {
    match fs::metadata(path) {
        Ok(meta) => Fingerprint {
            mtime_ms: mtime_ms(&meta),
            size: meta.len(),
        },
        Err(_) => Fingerprint::default(),
    }
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut text = path.to_string_lossy().to_string();
    text.push_str(".bak");
    PathBuf::from(text)
}

fn temp_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let mut text = path.to_string_lossy().to_string();
    text.push_str(&format!(".tmp{}-{}", std::process::id(), stamp));
    PathBuf::from(text)
}

/// 撤销后判断内存内容是否与磁盘不一致
fn content_differs(playlist: &Playlist) -> bool {
    let text = serialize_playlist(playlist);
    match fs::read(&playlist.path) {
        Ok(bytes) => {
            let mut expected: Vec<u8> = Vec::new();
            if playlist.has_bom {
                expected.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            expected.extend_from_slice(text.as_bytes());
            expected != bytes
        }
        Err(_) => true,
    }
}
