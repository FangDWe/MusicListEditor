use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::dto::{FailedPlaylist, ScanProgress};
use crate::m3u::{read_playlist_file, ReadResult};

pub struct FolderScan {
    pub playlist_paths: Vec<PathBuf>,
    pub track_paths: Vec<PathBuf>,
    pub skipped_dirs: usize,
    pub blocked_dirs: usize,
    pub cancelled: bool,
}

const MAX_DEPTH: usize = 32;
const SKIP_DIRS: [&str; 2] = ["$recycle.bin", "system volume information"];

pub type ProgressFn<'a> = &'a (dyn Fn(ScanProgress) + Sync);

/// 判断相对目录是否被用户屏蔽（目录本身或其祖先被屏蔽，F2.8）
fn is_blocked(rel: &str, blocked: &[String]) -> bool {
    blocked.iter().any(|item| {
        let item = item.trim_matches(['\\', '/']);
        !item.is_empty() && (rel == item || rel.starts_with(&format!("{item}\\")))
    })
}

pub fn scan_folder(
    root: &Path,
    audio_exts: &[String],
    blocked: &[String],
    cancel: &AtomicBool,
    progress: ProgressFn<'_>,
) -> FolderScan {
    let mut playlist_paths = Vec::new();
    let mut track_paths = Vec::new();
    let mut skipped_dirs = 0usize;
    let mut blocked_dirs = 0usize;
    let mut visited = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut cancelled = false;

    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        if depth > MAX_DEPTH {
            continue;
        }
        let read = match fs::read_dir(&dir) {
            Ok(value) => value,
            Err(_) => {
                skipped_dirs += 1;
                continue;
            }
        };
        for entry in read.flatten() {
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(value) => value,
                Err(_) => continue,
            };
            // 跳过符号链接 / 目录联接，防止目录环（F2.7）
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .map(|value| value.to_string_lossy().replace('/', "\\"))
                    .unwrap_or_default();
                if is_blocked(&rel, blocked) {
                    blocked_dirs += 1;
                    continue;
                }
                stack.push((path, depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|value| value.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if crate::dto::PLAYLIST_EXTS.contains(&ext.as_str()) {
                playlist_paths.push(path.clone());
            } else if audio_exts.iter().any(|item| *item == ext) {
                track_paths.push(path.clone());
            }
            visited += 1;
            if visited % 200 == 0 {
                progress(ScanProgress {
                    phase: "扫描文件".to_string(),
                    done: visited,
                    total: 0,
                    current_path: path.to_string_lossy().to_string(),
                });
            }
        }
        if cancelled {
            break;
        }
    }

    FolderScan {
        playlist_paths,
        track_paths,
        skipped_dirs,
        blocked_dirs,
        cancelled,
    }
}

pub struct LoadedPlaylist {
    pub path: PathBuf,
    pub result: ReadResult,
}

/// 并发读取播放列表；单个失败不影响其它（F2.6 / §5.3）
pub fn load_playlists(
    paths: &[PathBuf],
    cancel: &AtomicBool,
    progress: ProgressFn<'_>,
    threads: usize,
) -> (Vec<LoadedPlaylist>, Vec<FailedPlaylist>) {
    let total = paths.len();
    let results: Mutex<Vec<Option<ReadResult>>> = Mutex::new((0..total).map(|_| None).collect());
    let failures: Mutex<Vec<FailedPlaylist>> = Mutex::new(Vec::new());
    let cursor = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let worker_count = threads.min(total.max(1));

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let index = cursor.fetch_add(1, Ordering::Relaxed);
                if index >= paths.len() {
                    return;
                }
                let path = &paths[index];
                match read_playlist_file(path) {
                    Ok(result) => {
                        if let Ok(mut guard) = results.lock() {
                            guard[index] = Some(result);
                        }
                    }
                    Err(error) => {
                        if let Ok(mut guard) = failures.lock() {
                            guard.push(FailedPlaylist {
                                path: path.to_string_lossy().to_string(),
                                reason: error.message,
                            });
                        }
                    }
                }
                let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                if finished % 20 == 0 || finished == total {
                    progress(ScanProgress {
                        phase: "解析播放列表".to_string(),
                        done: finished,
                        total,
                        current_path: path.to_string_lossy().to_string(),
                    });
                }
            });
        }
    });

    let mut loaded = Vec::new();
    if let Ok(guard) = results.into_inner() {
        for (index, item) in guard.into_iter().enumerate() {
            if let Some(result) = item {
                loaded.push(LoadedPlaylist {
                    path: paths[index].clone(),
                    result,
                });
            }
        }
    }
    let failed = failures.into_inner().unwrap_or_default();
    (loaded, failed)
}
