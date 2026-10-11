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

fn get_adb_file_preview(path: &str) -> Result<FilePreview, String> {
    let entry = crate::adb::adb_get_file_entry(path)?;
    let mime = entry.mime_type.clone();
    let size = entry.size;
    let ext = entry.extension.to_lowercase();

    if entry.is_dir {
        let count = crate::adb::adb_list_directory(path, true)
            .map(|l| l.entries.len())
            .unwrap_or(0);
        return Ok(FilePreview {
            kind: "dir".into(),
            mime_type: "inode/directory".into(),
            size: count as u64,
            image_b64: None,
            text_content: None,
            media_info: None,
            line_count: Some(count),
            is_truncated: false,
            language: None,
        });
    }

    if mime.starts_with("image/") || matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"webp"|"bmp"|"gif"|"svg") {
        if size > MAX_IMAGE_BYTES && size != 0 {
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
        let cached = crate::adb::adb_cache_file(path)?;
        let bytes = fs::read(&cached).map_err(|e| e.to_string())?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Ok(FilePreview {
            kind: "image".into(),
            mime_type: mime,
            size: if size == 0 { bytes.len() as u64 } else { size },
            image_b64: Some(b64),
            text_content: None,
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    let is_video_ext = matches!(ext.as_str(), "mp4"|"mkv"|"webm"|"avi"|"mov"|"flv"|"wmv"|"m4v"|"ts");
    let is_audio_ext = matches!(ext.as_str(), "mp3"|"wav"|"ogg"|"flac"|"m4a"|"aac"|"opus"|"wma");

    if mime.starts_with("video/") || is_video_ext {
        return Ok(FilePreview {
            kind: "video".into(),
            mime_type: if is_video_ext && !mime.starts_with("video/") { format!("video/{}", ext) } else { mime },
            size,
            image_b64: None,
            text_content: Some("Video on phone — Double-click to open".into()),
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    if mime.starts_with("audio/") || is_audio_ext {
        return Ok(FilePreview {
            kind: "audio".into(),
            mime_type: mime,
            size,
            image_b64: None,
            text_content: Some("Audio on phone — Double-click to open".into()),
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    let p_dummy = PathBuf::from(&entry.name);
    let lang = detect_language(&p_dummy);
    let is_text = lang.is_some() || mime.starts_with("text/") || mime.contains("json") || mime.contains("javascript") || mime.contains("xml");

    if is_text {
        let (serial, remote) = crate::adb::parse_adb_path(path).ok_or_else(|| "Invalid ADB path".to_string())?;
        let escaped = remote.replace('\'', "'\\''");
        let out = Command::new("adb")
            .args(["-s", &serial, "shell", &format!("head -n 200 '{}'", escaped)])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            let content = String::from_utf8_lossy(&out.stdout).to_string();
            let all_lines: Vec<&str> = content.lines().collect();
            let total = all_lines.len();
            let truncated = total > PREVIEW_LINES;
            let preview_text = all_lines[..all_lines.len().min(PREVIEW_LINES)].join("\n");
            return Ok(FilePreview {
                kind: "text".into(),
                mime_type: mime,
                size,
                image_b64: None,
                text_content: Some(preview_text),
                media_info: None,
                line_count: Some(total),
                is_truncated: truncated,
                language: lang,
            });
        }
    }

    Ok(FilePreview {
        kind: "binary".into(),
        mime_type: mime,
        size,
        image_b64: None,
        text_content: None,
        media_info: None,
        line_count: None,
        is_truncated: false,
        language: None,
    })
}

#[tauri::command]
pub fn get_file_preview(path: String) -> Result<FilePreview, String> {
    if path.starts_with("adb://") {
        return get_adb_file_preview(&path);
    }

    let p = PathBuf::from(&path);
    let sudo_pwd = crate::fs_ops::get_session_sudo_password();

    let mut real_size = 0u64;
    let mut is_directory = false;

    let _meta = match fs::symlink_metadata(&p) {
        Ok(m) => {
            let real = fs::metadata(&p).unwrap_or(m);
            is_directory = real.is_dir() || path.ends_with('/');
            real_size = if is_directory { 0 } else { real.len() };
            true
        }
        Err(e) => {
            if let Some(ref pwd) = sudo_pwd {
                let mut child = Command::new("sudo")
                    .args(["-S", "stat", "-c", "%s|%F|%Y", &path])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|_| e.to_string())?;
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                }
                let out = child.wait_with_output().map_err(|_| e.to_string())?;
                if out.status.success() {
                    let out_str = String::from_utf8_lossy(&out.stdout);
                    let parts: Vec<&str> = out_str.trim().split('|').collect();
                    if !parts.is_empty() {
                        real_size = parts[0].parse::<u64>().unwrap_or(0);
                    }
                    if parts.len() > 1 && parts[1].to_lowercase().contains("directory") {
                        is_directory = true;
                        real_size = 0;
                    }
                    true
                } else {
                    return Err(e.to_string());
                }
            } else {
                return Err(e.to_string());
            }
        }
    };

    let is_dir = is_directory || path.ends_with('/');
    let size = real_size;

    let mime = if is_dir {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(&p).first_or_octet_stream().to_string()
    };

    // ── Klasör ──────────────────────────────────────────────────────
    if is_dir {
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
        if size > MAX_IMAGE_BYTES && size != 0 {
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
        let bytes = fs::read(&p).or_else(|e| {
            if let Some(ref pwd) = sudo_pwd {
                use std::process::Stdio;
                use std::io::Write;
                let mut child = Command::new("sudo")
                    .args(["-S", "cat", &path])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|_| e.to_string())?;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                }
                let out = child.wait_with_output().map_err(|_| e.to_string())?;
                if out.status.success() {
                    return Ok(out.stdout);
                }
            }
            Err(format!("Resim okunamadı: {}", e))
        })?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Ok(FilePreview {
            kind: "image".into(),
            mime_type: mime,
            size: if size == 0 { bytes.len() as u64 } else { size },
            image_b64: Some(b64),
            text_content: None,
            media_info: None,
            line_count: None,
            is_truncated: false,
            language: None,
        });
    }

    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_video_ext = matches!(ext.as_str(), "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "ts");
    let is_audio_ext = matches!(ext.as_str(), "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "opus" | "wma");

    // ── Video / Audio — ffprobe ──────────────────────────────────────
    if mime.starts_with("video/") || mime.starts_with("audio/") || is_video_ext || is_audio_ext {
        let kind = if mime.starts_with("video/") || is_video_ext { "video" } else { "audio" };
        let media_info = try_ffprobe(&path);
        return Ok(FilePreview {
            kind: kind.into(),
            mime_type: if is_video_ext && !mime.starts_with("video/") { format!("video/{}", ext) } else { mime },
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

    if is_text && (size <= MAX_TEXT_BYTES || size == 0) {
        let content_res: Result<String, String> = fs::read_to_string(&p).map_err(|e| e.to_string()).or_else(|_| {
            if let Some(ref pwd) = sudo_pwd {
                use std::process::Stdio;
                use std::io::Write;
                let mut child = Command::new("sudo")
                    .args(["-S", "head", "-c", "1048576", &path])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                }
                let out = child.wait_with_output().map_err(|e| e.to_string())?;
                if out.status.success() {
                    return String::from_utf8(out.stdout).map_err(|e| e.to_string());
                }
            }
            Err("Okunamadı".into())
        });

        if let Ok(content) = content_res {
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
    }

    // ── Binary — hex dump ────────────────────────────────────────────
    let buf = match fs::File::open(&p) {
        Ok(mut f) => {
            let mut buf = vec![0u8; HEX_BYTES];
            let n = f.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            buf
        }
        Err(_) => {
            if let Some(ref pwd) = sudo_pwd {
                use std::process::Stdio;
                use std::io::Write;
                let mut child = Command::new("sudo")
                    .args(["-S", "head", "-c", "512", &path])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                }
                let out = child.wait_with_output().map_err(|e| e.to_string())?;
                out.stdout
            } else {
                return Err("Dosya açılamıyor".into());
            }
        }
    };

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
    // 2 saniye timeout ile çalıştır (büyük/ağ dosyalarında asılı kalmasın)
    let out = Command::new("timeout")
        .args([
            "2s",
            "ffprobe",
            "-v", "quiet",
            "-probesize", "1000000",
            "-analyzeduration", "1000000",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .ok();

    if let Some(o) = out {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            if let Ok(val) = serde_json::from_str(&text) {
                return Some(val);
            }
        }
    }

    // Normal başarısız olduysa ve sudo şifresi varsa sudo ile ffprobe dene
    if let Some(pwd) = crate::fs_ops::get_session_sudo_password() {
        use std::process::Stdio;
        use std::io::Write;
        let mut child = Command::new("sudo")
            .args([
                "-S", "timeout", "2s",
                "ffprobe",
                "-v", "quiet",
                "-probesize", "1000000",
                "-analyzeduration", "1000000",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                path,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
        }
        let out = child.wait_with_output().ok()?;
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            return serde_json::from_str(&text).ok();
        }
    }

    None
}
