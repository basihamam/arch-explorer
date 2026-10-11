// preview.rs — Dosya önizleme
// image → base64, text → ilk 100 satır, video/audio → ffprobe, binary → hex dump

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024; // 20 MB
const MAX_TEXT_BYTES:  u64 =  1 * 1024 * 1024; //  1 MB
const PREVIEW_LINES:   usize = 100;
const HEX_BYTES:       usize = 512;

#[derive(Debug, Serialize, Deserialize)]
pub struct FilePreview {
    /// "image" | "text" | "video" | "audio" | "dir" | "binary"
    pub kind:         String,
    pub mime_type:    String,
    pub size:         u64,
    pub image_b64:    Option<String>,
    pub text_content: Option<String>,
    pub media_info:   Option<serde_json::Value>,
    pub line_count:   Option<usize>,
    pub is_truncated: bool,
    pub language:     Option<String>, // text highlight hint
}

#[tauri::command]
pub fn get_file_preview(path: String) -> Result<FilePreview, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Bulunamadı: {}", path));
    }

    let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    let real_meta = fs::metadata(&p).unwrap_or(meta);
    let size = if real_meta.is_dir() { 0 } else { real_meta.len() };

    let mime = if real_meta.is_dir() {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(&p).first_or_octet_stream().to_string()
    };

    // ── Klasör ──────────────────────────────────────────────────────
    if real_meta.is_dir() {
        let count = fs::read_dir(&p).ok().map(|rd| rd.count()).unwrap_or(0);
        return Ok(FilePreview {
            kind: "dir".into(),
            mime_type: mime,
            size: count as u64,
            image_b64: None,
            text_content: None,
            media_info: None,
            line_count: Some(count),
            is_truncated: false,
            language: None,
        });
    }

    // ── Resim ────────────────────────────────────────────────────────
    if mime.starts_with("image/") {
        if size > MAX_IMAGE_BYTES {
            return Ok(FilePreview {
                kind: "image".into(),
                mime_type: mime,
                size,
                image_b64: None,
                text_content: Some(format!("Dosya çok büyük ({:.1} MB > 20 MB)", size as f64 / 1_048_576.0)),
                media_info: None,
                line_count: None,
                is_truncated: false,
                language: None,
            });
        }
        let bytes = fs::read(&p).map_err(|e| format!("Resim okunamadı: {}", e))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Ok(FilePreview {
            kind: "image".into(),
            mime_type: mime,
            size,
            image_b64: Some(b64),
            text_content: None,
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    // ── Video / Audio — ffprobe ──────────────────────────────────────
    if mime.starts_with("video/") || mime.starts_with("audio/") {
        let kind = if mime.starts_with("video/") { "video" } else { "audio" };
        let media_info = try_ffprobe(&path);
        return Ok(FilePreview {
            kind: kind.into(),
            mime_type: mime,
            size,
            image_b64: None,
            text_content: None,
            media_info,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    // ── Metin ────────────────────────────────────────────────────────
    let lang = detect_language(&p);
    let is_text = lang.is_some()
        || mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("javascript")
        || mime.contains("xml");

    if is_text && size <= MAX_TEXT_BYTES {
        match fs::read_to_string(&p) {
            Ok(content) => {
                let all_lines: Vec<&str> = content.lines().collect();
                let total = all_lines.len();
                let truncated = total > PREVIEW_LINES;
                let preview = all_lines[..all_lines.len().min(PREVIEW_LINES)].join("\n");
                return Ok(FilePreview {
                    kind: "text".into(),
                    mime_type: mime,
                    size,
                    image_b64: None,
                    text_content: Some(preview),
                    media_info: None,
                    line_count: Some(total),
                    is_truncated: truncated,
                    language: lang,
                });
            }
            Err(_) => { /* binary'ye düş */ }
        }
    }

    // ── Binary — hex dump ────────────────────────────────────────────
    let mut f = fs::File::open(&p).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; HEX_BYTES];
    let n = f.read(&mut buf).unwrap_or(0);
    buf.truncate(n);

    let hex = buf
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let hex_part: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
            let ascii: String = chunk
                .iter()
                .map(|b| if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '.' })
                .collect();
            format!("{:08X}  {:48} |{}|", i * 16, hex_part, ascii)
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(FilePreview {
        kind: "binary".into(),
        mime_type: mime,
        size,
        image_b64: None,
        text_content: Some(hex),
        media_info: None,
        line_count: None,
        is_truncated: size > HEX_BYTES as u64,
        language: Some("hex".into()),
    })
}

/// Dosya uzantısından syntax highlight dili tahmin et
fn detect_language(p: &PathBuf) -> Option<String> {
    let ext = p.extension()?.to_str()?.to_lowercase();
    let lang = match ext.as_str() {
        "rs"                         => "rust",
        "py"                         => "python",
        "js" | "mjs" | "cjs"        => "javascript",
        "ts"                         => "typescript",
        "svelte"                     => "svelte",
        "html" | "htm"               => "html",
        "css" | "scss" | "less"      => "css",
        "json"                       => "json",
        "toml"                       => "toml",
        "yaml" | "yml"               => "yaml",
        "md" | "markdown"            => "markdown",
        "sh" | "bash" | "zsh"        => "bash",
        "lua"                        => "lua",
        "go"                         => "go",
        "c" | "h"                    => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java"                       => "java",
        "kt"                         => "kotlin",
        "dart"                       => "dart",
        "sql"                        => "sql",
        "xml"                        => "xml",
        "txt" | "log" | "conf"
        | "cfg" | "ini" | "env"      => "plaintext",
        _                            => return None,
    };
    Some(lang.to_string())
}

/// ffprobe ile media metadata al
fn try_ffprobe(path: &str) -> Option<serde_json::Value> {
    let out = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .ok()?;

    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&text).ok()
}
