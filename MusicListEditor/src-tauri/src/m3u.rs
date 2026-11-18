use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::model::{
    derive_title, ext_of, norm_id, standard_path, Encoding, Entry, EntryKind, Eol, ExtInf, Playlist,
};

pub struct Decoded {
    pub text: String,
    pub encoding: Encoding,
    pub has_bom: bool,
}

/// 读取解码：UTF-8(BOM) → 严格 UTF-8 → GB18030 → lossy（F1.3）
pub fn decode(bytes: &[u8]) -> Decoded {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, _, _) = encoding_rs::UTF_8.decode(&bytes[3..]);
        return Decoded {
            text: text.into_owned(),
            encoding: Encoding::Utf8Bom,
            has_bom: true,
        };
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Decoded {
            text: text.to_string(),
            encoding: Encoding::Utf8,
            has_bom: false,
        };
    }
    let (text, _, had_errors) = encoding_rs::GB18030.decode(bytes);
    if !had_errors {
        Decoded {
            text: text.into_owned(),
            encoding: Encoding::Gb18030,
            has_bom: false,
        }
    } else {
        Decoded {
            text: String::from_utf8_lossy(bytes).into_owned(),
            encoding: Encoding::Lossy,
            has_bom: false,
        }
    }
}

pub fn detect_eol(text: &str) -> Eol {
    let bytes = text.as_bytes();
    if let Some(index) = bytes.iter().position(|value| *value == b'\n') {
        return if index > 0 && bytes[index - 1] == b'\r' { Eol::Crlf } else { Eol::Lf };
    }
    if bytes.contains(&b'\r') { Eol::Cr } else { Eol::Crlf }
}

/// 按 CRLF / LF / CR 切行；结尾的换行符不产生多余空行，保证读写往返稳定
fn split_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                lines.push(&text[start..index]);
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    index += 1;
                }
                start = index + 1;
            }
            b'\n' => {
                lines.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn parse_extinf(line: &str) -> Option<ExtInf> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("#extinf:") {
        return None;
    }
    let rest = &line[8..];
    let (duration_part, title_part) = match rest.split_once(',') {
        Some((duration, title)) => (duration.trim(), title.trim()),
        None => (rest.trim(), ""),
    };
    let duration = duration_part.parse::<f64>().ok().filter(|value| *value >= 0.0);
    let title = title_part.to_string();
    let mut artist = None;
    let mut track = None;
    if let Some((left, right)) = title.split_once(" - ") {
        let left = left.trim();
        let right = right.trim();
        if !left.is_empty() {
            artist = Some(left.to_string());
        }
        if !right.is_empty() {
            track = Some(right.to_string());
        }
    }
    Some(ExtInf {
        duration,
        title,
        artist,
        track,
    })
}

fn file_exists(path: &Path) -> bool {
    fs::metadata(path).map(|meta| meta.is_file()).unwrap_or(false)
}

/// 逐行解析，保留原始行文本（F1.2 / F4.3）
pub fn parse_m3u(text: &str, dir: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut pending: Option<ExtInf> = None;

    for line in split_lines(text) {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            entries.push(Entry {
                raw: line.to_string(),
                kind: EntryKind::Blank,
                abs_path: None,
                track_id: None,
                extinf: None,
            });
            continue;
        }

        if trimmed.starts_with('#') {
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("#extinf:") {
                pending = parse_extinf(trimmed);
            }
            let is_directive = lower.starts_with("#ext") || lower.starts_with("#playlist");
            entries.push(Entry {
                raw: line.to_string(),
                kind: if is_directive { EntryKind::Directive } else { EntryKind::Comment },
                abs_path: None,
                track_id: None,
                extinf: None,
            });
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("rtsp://")
            || lower.starts_with("mms://")
        {
            entries.push(Entry {
                raw: line.to_string(),
                kind: EntryKind::Stream,
                abs_path: None,
                track_id: None,
                extinf: pending.take(),
            });
            continue;
        }

        let candidate = if Path::new(trimmed).is_absolute() {
            PathBuf::from(trimmed)
        } else {
            dir.join(trimmed)
        };
        let exists = file_exists(&candidate);
        entries.push(Entry {
            raw: line.to_string(),
            kind: if exists { EntryKind::Track } else { EntryKind::Missing },
            abs_path: Some(candidate.clone()),
            track_id: if exists { Some(norm_id(&candidate)) } else { None },
            extinf: pending.take(),
        });
    }

    entries
}

pub struct ReadResult {
    pub entries: Vec<Entry>,
    pub encoding: Encoding,
    pub has_bom: bool,
    pub eol: Eol,
}

pub fn read_playlist_file(path: &Path) -> AppResult<ReadResult> {
    let bytes = fs::read(path).map_err(|error| AppError::io("读取播放列表失败", error))?;
    let decoded = decode(&bytes);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let entries = parse_m3u(&decoded.text, dir);
    Ok(ReadResult {
        entries,
        encoding: decoded.encoding,
        has_bom: decoded.has_bom,
        eol: detect_eol(&decoded.text),
    })
}

/// 写回序列化：一律输出标准格式（F4.3 / 增量需求 1）
///
/// 标准格式 = 第一行 `#EXTM3U`，其后每行一个曲目路径；路径为绝对路径、分隔符为 `/`。
/// 因此 `#EXTINF`、注释、其它指令与空行不会被写回；读取侧仍然保持宽容（F1.2）。
pub fn serialize_playlist(playlist: &Playlist) -> String {
    let eol = playlist.eol.as_str();
    let mut lines: Vec<String> = Vec::with_capacity(playlist.entries.len() + 1);
    lines.push("#EXTM3U".to_string());

    for entry in &playlist.entries {
        match entry.kind {
            EntryKind::Track | EntryKind::Missing => {
                if let Some(path) = &entry.abs_path {
                    lines.push(standard_path(path));
                }
            }
            EntryKind::Stream => {
                // 网络流不是文件路径，原样保留（去掉首尾空白）
                let text = entry.raw.trim();
                if !text.is_empty() {
                    lines.push(text.to_string());
                }
            }
            EntryKind::Directive | EntryKind::Comment | EntryKind::Blank => {}
        }
    }

    let mut out = lines.join(eol);
    out.push_str(eol);
    out
}

/// 新增曲目对应的条目：标准格式下只有一行绝对路径（F4.3）
pub fn make_track_entry(abs_path: &Path) -> Entry {
    Entry {
        raw: standard_path(abs_path),
        kind: EntryKind::Track,
        abs_path: Some(abs_path.to_path_buf()),
        track_id: Some(norm_id(abs_path)),
        extinf: None,
    }
}

/// 从条目推导展示标题
pub fn entry_title(entry: &Entry) -> String {
    if let Some(extinf) = &entry.extinf {
        if !extinf.title.is_empty() {
            return extinf.title.clone();
        }
        if let Some(track) = &extinf.track {
            return track.clone();
        }
    }
    match &entry.abs_path {
        Some(path) => derive_title(path),
        None => entry.raw.clone(),
    }
}

/// 从条目推导扩展名
pub fn entry_ext(entry: &Entry) -> Option<String> {
    entry.abs_path.as_ref().map(|path| ext_of(path))
}
