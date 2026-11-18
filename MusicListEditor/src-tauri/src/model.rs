use std::path::{Path, PathBuf};

pub type TrackId = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Track,
    Missing,
    Stream,
    Directive,
    Comment,
    Blank,
}

#[derive(Clone, Debug)]
pub struct ExtInf {
    pub duration: Option<f64>,
    pub title: String,
    pub artist: Option<String>,
    pub track: Option<String>,
}

/// 播放列表中的一行；`raw` 为原始文本，写回时原样输出（F4.3）
#[derive(Clone, Debug)]
pub struct Entry {
    pub raw: String,
    pub kind: EntryKind,
    pub abs_path: Option<PathBuf>,
    pub track_id: Option<TrackId>,
    pub extinf: Option<ExtInf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Gb18030,
    Lossy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    Crlf,
    Lf,
    Cr,
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::Crlf => "\r\n",
            Eol::Lf => "\n",
            Eol::Cr => "\r",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub mtime_ms: u64,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: u64,
    pub path: PathBuf,
    pub rel_path: String,
    pub name: String,
    pub entries: Vec<Entry>,
    pub src_encoding: Encoding,
    pub has_bom: bool,
    pub eol: Eol,
    pub dirty: bool,
    pub fingerprint: Fingerprint,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: String,
    pub ext: String,
    pub size: u64,
    pub ref_count: usize,
    pub ref_names: Vec<String>,
}

/// 去掉 Windows verbatim 前缀（`\\?\`、`\\?\UNC\`），只做文本处理，不访问磁盘
pub fn strip_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text.to_string()
    }
}

/// 展示用路径：去 verbatim 前缀，保留原分隔符（`canonicalize` 会带出 `\\?\`，不应出现在界面上）
pub fn display_path(path: &Path) -> String {
    strip_verbatim(&path.to_string_lossy())
}

/// 标准格式路径：转为绝对路径 → 去 verbatim 前缀 → 分隔符统一为 `/`（F4.3 写回规则）
pub fn standard_path(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|dir| dir.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    display_path(&absolute).replace('\\', "/")
}

/// 规范化 id：绝对路径 → 去 \\?\ 前缀 → 统一反斜杠 → 小写（F1.4 / F2.5）
pub fn norm_id(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|dir| dir.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    strip_verbatim(&absolute.to_string_lossy())
        .replace('/', "\\")
        .to_lowercase()
}

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 从文件名推导标题：去扩展名、去前导序号、下划线转空格（F2.4）
pub fn derive_title(path: &Path) -> String {
    let base = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let mut cleaned = base.as_str();
    // 去掉形如 "01 - "、"02."、"03_" 的前导序号
    let stripped = strip_leading_index(cleaned);
    cleaned = stripped;

    let replaced = cleaned.replace('_', " ");
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() { base } else { collapsed }
}

fn strip_leading_index(input: &str) -> &str {
    let trimmed = input.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 3 {
        return trimmed;
    }
    let rest = &trimmed[digits.len()..];
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('-') {
        return stripped.trim_start();
    }
    if let Some(stripped) = rest.strip_prefix('.') {
        return stripped.trim_start();
    }
    if let Some(stripped) = rest.strip_prefix('_') {
        return stripped.trim_start();
    }
    if let Some(stripped) = rest.strip_prefix(')') {
        return stripped.trim_start();
    }
    trimmed
}
