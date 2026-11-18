//! 音频标签读写（F6）：标题 / 艺术家 / 专辑 / 专辑封面。
//!
//! 只依赖 `lofty`，不额外引入 base64 crate：封面以 `data:` URL 交给前端预览。

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, Tag};

use crate::dto::{TagPatch, TrackTags};
use crate::error::{AppError, AppResult};

/// lofty 能读写的常见格式；其它（如 wma）只读展示，不允许编辑
const EDITABLE_EXTS: [&str; 10] = [
    "mp3", "flac", "m4a", "mp4", "ogg", "opus", "wav", "aiff", "aif", "ape",
];

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_editable(path: &Path) -> bool {
    EDITABLE_EXTS.contains(&ext_of(path).as_str())
}

pub fn read(path: &Path) -> AppResult<TrackTags> {
    let ext = ext_of(path);
    if !is_editable(path) {
        return Ok(TrackTags {
            title: None,
            artist: None,
            album: None,
            cover_data_url: None,
            ext,
            editable: false,
        });
    }

    let tagged = lofty::read_from_path(path)
        .map_err(|error| AppError::new("E4011", format!("读取标签失败：{error}")))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let (title, artist, album) = match tag {
        Some(tag) => (
            tag.title().map(|value| value.to_string()),
            tag.artist().map(|value| value.to_string()),
            tag.album().map(|value| value.to_string()),
        ),
        None => (None, None, None),
    };

    let cover_data_url = tag
        .and_then(|tag| tag.pictures().first())
        .map(|picture| {
            let mime = picture
                .mime_type()
                .map(|value| value.as_str().to_string())
                .unwrap_or_else(|| "image/jpeg".to_string());
            format!("data:{mime};base64,{}", base64_encode(picture.data()))
        });

    Ok(TrackTags {
        title,
        artist,
        album,
        cover_data_url,
        ext,
        editable: true,
    })
}

pub fn write(path: &Path, patch: &TagPatch) -> AppResult<TrackTags> {
    if !is_editable(path) {
        return Err(AppError::new(
            "E4010",
            "该音频格式暂不支持编辑标签（支持 mp3 / flac / m4a / ogg / opus / wav / aiff / ape）",
        ));
    }

    let mut tagged = lofty::read_from_path(path)
        .map_err(|error| AppError::new("E4011", format!("读取标签失败：{error}")))?;

    if tagged.primary_tag_mut().is_none() {
        let tag_type = tagged.file_type().primary_tag_type();
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| AppError::new("E4011", "无法创建标签"))?;

    apply_text(tag, patch.title.as_deref(), "title");
    apply_text(tag, patch.artist.as_deref(), "artist");
    apply_text(tag, patch.album.as_deref(), "album");

    // 封面：文件可能有多张，只替换**第一张**，其余原样保留（F6.2）
    if let Some(data_url) = patch
        .cover_data_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let (mime, bytes) = decode_data_url(data_url)?;
        tag.set_picture(0, Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes));
    } else if let Some(cover_path) = patch.cover_path.as_deref().filter(|value| !value.is_empty()) {
        let bytes = std::fs::read(cover_path)
            .map_err(|error| AppError::new("E4012", format!("读取封面图片失败：{error}")))?;
        let mime = mime_of(Path::new(cover_path));
        tag.set_picture(0, Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes));
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| AppError::new("E4013", format!("写入标签失败：{error}")))?;

    read(path)
}

fn apply_text(tag: &mut Tag, value: Option<&str>, field: &str) {
    let Some(value) = value else { return };
    let text = value.trim();
    match field {
        "title" => {
            if text.is_empty() {
                tag.remove_title();
            } else {
                tag.set_title(text.to_string());
            }
        }
        "artist" => {
            if text.is_empty() {
                tag.remove_artist();
            } else {
                tag.set_artist(text.to_string());
            }
        }
        _ => {
            if text.is_empty() {
                tag.remove_album();
            } else {
                tag.set_album(text.to_string());
            }
        }
    }
}

fn mime_of(path: &Path) -> MimeType {
    match ext_of(path).as_str() {
        "png" => MimeType::Png,
        "gif" => MimeType::Gif,
        "bmp" => MimeType::Bmp,
        "tif" | "tiff" => MimeType::Tiff,
        _ => MimeType::Jpeg,
    }
}

/// 读取图片文件 → data URL，供前端做封面预览/截取（避免开放 fs 插件权限）
pub fn image_data_url(path: &Path) -> AppResult<String> {
    let meta = std::fs::metadata(path)
        .map_err(|error| AppError::new("E4012", format!("读取图片失败：{error}")))?;
    if meta.len() > 16 * 1024 * 1024 {
        return Err(AppError::new("E4012", "图片过大（上限 16MB）"));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| AppError::new("E4012", format!("读取图片失败：{error}")))?;
    let mime = mime_of(path);
    Ok(format!(
        "data:{};base64,{}",
        mime.as_str(),
        base64_encode(&bytes)
    ))
}

fn decode_data_url(value: &str) -> AppResult<(MimeType, Vec<u8>)> {
    let rest = value
        .strip_prefix("data:")
        .ok_or_else(|| AppError::new("E4012", "封面数据格式不正确"))?;
    let (header, payload) = rest
        .split_once(',')
        .ok_or_else(|| AppError::new("E4012", "封面数据格式不正确"))?;
    let mime = header.strip_suffix(";base64").unwrap_or(header);
    let bytes = base64_decode(payload);
    if bytes.is_empty() {
        return Err(AppError::new("E4012", "封面数据为空"));
    }
    Ok((MimeType::from_str(mime), bytes))
}

fn base64_decode(text: &str) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0xFFu8; 256];
    for (index, byte) in TABLE.iter().enumerate() {
        lookup[*byte as usize] = index as u8;
    }
    let mut out: Vec<u8> = Vec::with_capacity(text.len() / 4 * 3 + 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let value = lookup[byte as usize];
        if value == 0xFF {
            continue;
        }
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

/// 极简 base64（标准字母表 + 填充），避免为预览封面多引一个依赖
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let bits = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((bits >> 18) & 63) as usize] as char);
        out.push(TABLE[((bits >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((bits >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(bits & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
