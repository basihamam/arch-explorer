// thumbnails.rs — 3 kalite tierli thumbnail cache
// Cache key: {hash}_{max_size}.jpg
// 64px → kalite 50 (küçük zoom)
// 128px → kalite 72 (orta zoom)
// 256px → kalite 88 (büyük zoom)
// GIF + SVG → Rust'ta işlenmez, frontend direkt kullanır

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs;
use std::path::{Path, PathBuf};
use std::os::unix::fs::MetadataExt;

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("thumbnails")
}

fn cache_key(path: &str, mtime: u64, max_size: u32) -> String {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    mtime.hash(&mut h);
    format!("{:016x}_{}.jpg", h.finish(), max_size)
}

fn is_video(ext: &str) -> bool {
    matches!(ext, "mp4"|"mkv"|"webm"|"avi"|"mov"|"wmv"|"flv"|"m4v"|"ts"|"mpg"|"mpeg"|"3gp"|"ogv")
}

fn can_thumbnail(path: &Path) -> bool {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"webp"|"bmp"|"tiff"|"tif"|"avif"|"ico"|"gif") || is_video(&ext)
}

fn quality_for_size(max_size: u32) -> u8 {
    match max_size {
        0..=64   => 70,
        65..=128 => 82,
        129..=256 => 90,
        _        => 95,
    }
}

/// Thumbnail üret veya cache'ten oku — data URL (base64 JPEG) döndür
/// max_size: 64 | 128 | 256 — zoom tier'ına göre frontend belirler
use std::process::{Command, Stdio};
use std::io::Write;

fn sudo_read_file(src: &Path, password: &str) -> Result<Vec<u8>, String> {
    let mut child = Command::new("sudo")
        .args(["-S", "cat", &src.to_string_lossy()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", password).as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(out.stdout)
}

fn sudo_video_thumb(src: &Path, cache_path: &Path, max_size: u32, password: &str) -> Result<(), String> {
    let scale_arg = format!("scale='min({},iw)':-2", max_size);
    let cache_str = cache_path.to_string_lossy().to_string();
    let src_str   = src.to_string_lossy().to_string();

    let mut child = Command::new("sudo")
        .args([
            "-S", "ffmpeg",
            "-hide_banner", "-loglevel", "error",
            "-ss", "00:00:01",
            "-i", &src_str,
            "-frames:v", "1",
            "-vf", &scale_arg,
            "-y",
            &cache_str,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", password).as_bytes());
    }
    let _ = child.wait_with_output();

    if !cache_path.exists() {
        let mut child2 = Command::new("sudo")
            .args([
                "-S", "ffmpeg",
                "-hide_banner", "-loglevel", "error",
                "-i", &src_str,
                "-frames:v", "1",
                "-vf", &scale_arg,
                "-y",
                &cache_str,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = child2.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", password).as_bytes());
        }
        let _ = child2.wait_with_output();
    }

    if cache_path.exists() {
        // Cache dosyası root sahipliğinde kalmasın, mevcut kullanıcıya devret
        let mut ch_child = Command::new("sudo")
            .args(["-S", "chmod", "666", &cache_str])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut stdin) = ch_child.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", password).as_bytes());
        }
        let _ = ch_child.wait_with_output();
        Ok(())
    } else {
        Err("Sudo ffmpeg ile thumbnail oluşturulamadı".into())
    }
}

/// Thumbnail üret veya cache'ten oku — data URL (base64 JPEG) döndür
/// max_size: 64 | 128 | 256 — zoom tier'ına göre frontend belirler
static ADB_THUMB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tauri::command]
pub fn get_thumbnail(path: String, max_size: u32) -> Result<String, String> {
    let max_size = max_size.clamp(32, 512);
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let is_adb = path.starts_with("adb://");
    if is_adb {
        let ext = Path::new(&path)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if is_video(&ext) {
            return Err("Video thumbnails over ADB are skipped to prevent USB freeze".into());
        }

        // Cache hit kontrolü (önceki çekilmişler için)
        let key = cache_key(&path, 0, max_size);
        let cache_path = dir.join(&key);
        if cache_path.exists() {
            if let Ok(bytes) = fs::read(&cache_path) {
                return Ok(to_data_url(&bytes));
            }
        }
    }

    let local_path = if is_adb {
        let _guard = ADB_THUMB_LOCK.lock().map_err(|e| e.to_string())?;
        // Lock sonrasında cache kontrolünü tekrar yap
        let key = cache_key(&path, 0, max_size);
        let cache_path = dir.join(&key);
        if cache_path.exists() {
            if let Ok(bytes) = fs::read(&cache_path) {
                return Ok(to_data_url(&bytes));
            }
        }
        crate::adb::adb_cache_file(&path)?.to_string_lossy().to_string()
    } else {
        path.clone()
    };

    let src = Path::new(&local_path);
    if !can_thumbnail(src) { return Err("Desteklenmiyor (GIF/SVG frontend'de)".into()); }

    let sudo_pwd = if is_adb { None } else { crate::fs_ops::get_session_sudo_password() };

    let mtime = if is_adb {
        0
    } else {
        match fs::metadata(src) {
            Ok(m) => m.mtime() as u64,
            Err(_) => {
                if let Some(ref pwd) = sudo_pwd {
                    let mut child = Command::new("sudo")
                        .args(["-S", "stat", "-c", "%Y", &path])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                    }
                    let out = child.wait_with_output().map_err(|e| e.to_string())?;
                    if out.status.success() {
                        String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    return Err("Dosyaya erişilemiyor (İzin yok)".into());
                }
            }
        }
    };

    let key = if is_adb { cache_key(&path, 0, max_size) } else { cache_key(&path, mtime, max_size) };
    let cache_path = dir.join(&key);

    // Cache hit
    if cache_path.exists() {
        if let Ok(bytes) = fs::read(&cache_path) {
            return Ok(to_data_url(&bytes));
        }
    }

    let ext = src.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Video dosyaları için ffmpeg thumbnail üretimi
    if is_video(&ext) {
        let cache_str = cache_path.to_string_lossy().to_string();
        let src_str = src.to_string_lossy().to_string();
        let scale_arg = format!("scale='min({},iw)':-2", max_size);

        // 1. Standart ffmpeg dene
        let _ = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner", "-loglevel", "error",
                "-ss", "00:00:01",
                "-i", &src_str,
                "-frames:v", "1",
                "-vf", &scale_arg,
                "-y",
                &cache_str,
            ])
            .output();

        if !cache_path.exists() {
            let _ = std::process::Command::new("ffmpeg")
                .args([
                    "-hide_banner", "-loglevel", "error",
                    "-i", &src_str,
                    "-frames:v", "1",
                    "-vf", &scale_arg,
                    "-y",
                    &cache_str,
                ])
                .output();
        }

        // 2. Normal ffmpeg başarısız olduysa ve sudo şifresi varsa sudo ile dene
        if !cache_path.exists() {
            if let Some(ref pwd) = sudo_pwd {
                let _ = sudo_video_thumb(src, &cache_path, max_size, pwd);
            }
        }

        if cache_path.exists() {
            if let Ok(bytes) = fs::read(&cache_path) {
                return Ok(to_data_url(&bytes));
            }
        }
        return Err("Video thumbnail üretilemedi".into());
    }

    // Cache miss — resim: önce normal açmayı dene, hata alırsan sudo ile oku
    let img = match image::open(src) {
        Ok(img) => img,
        Err(e) => {
            if let Some(ref pwd) = sudo_pwd {
                let bytes = sudo_read_file(src, pwd)
                    .map_err(|se| format!("Sudo ile resim okunamadı: {}", se))?;
                image::load_from_memory(&bytes)
                    .map_err(|ie| format!("Resim decode edilemedi: {}", ie))?
            } else {
                return Err(format!("Resim açılamadı: {}", e));
            }
        }
    };

    let thumb = img.thumbnail(max_size, max_size);
    let quality = quality_for_size(max_size);

    let buf = encode_jpeg(&thumb, quality)?;
    fs::write(&cache_path, &buf).ok();

    Ok(to_data_url(&buf))
}

fn encode_jpeg(img: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    use image::codecs::jpeg::JpegEncoder;
    // JPEG alpha kanalını desteklemez — RGB'ye dönüştür
    let rgb = img.to_rgb8();
    let mut buf = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
    enc.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    ).map_err(|e| format!("JPEG encode: {}", e))?;
    Ok(buf)
}

fn to_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{}", b64)
}

#[tauri::command]
pub fn clear_thumbnail_cache() -> Result<usize, String> {
    let dir = cache_dir();
    if !dir.exists() { return Ok(0); }
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut count = 0;
    for e in entries.filter_map(|e| e.ok()) {
        if fs::remove_file(e.path()).is_ok() { count += 1; }
    }
    Ok(count)
}

#[tauri::command]
pub fn get_thumbnail_cache_stats() -> serde_json::Value {
    let dir      = cache_dir();
    let path_str = dir.to_string_lossy().to_string();
    if !dir.exists() {
        return serde_json::json!({ "count": 0, "size_mb": 0.0, "path": path_str });
    }
    let (count, total_bytes) = fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok())
            .fold((0usize, 0u64), |(c, s), e| {
                (c + 1, s + e.metadata().map(|m| m.len()).unwrap_or(0))
            }))
        .unwrap_or((0, 0));
    serde_json::json!({
        "count":   count,
        "size_mb": (total_bytes as f64 / 1_048_576.0 * 10.0).round() / 10.0,
        "path":    path_str,
    })
}

/// Video üzerinde fare gezdirildiğinde kare kaydırma (hover scrubbing)
/// percent: 0.0 - 1.0 (farenin thumbnail üzerindeki yatay konumu)
#[tauri::command]
pub fn get_video_hover_frame(path: String, percent: f32, max_size: u32) -> Result<String, String> {
    let src = Path::new(&path);
    if !src.exists() {
        return Err("Video dosyası bulunamadı".into());
    }

    let ext = src.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !is_video(&ext) {
        return Err("Geçerli bir video değil".into());
    }

    let max_size = max_size.clamp(48, 384);
    let clamped_pct = percent.clamp(0.0, 1.0);
    // 10 parçaya böl (slot 0..9)
    let slot = ((clamped_pct * 9.99).floor() as u32).min(9);

    let mtime = fs::metadata(src).map(|m| m.mtime() as u64).unwrap_or(0);
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    mtime.hash(&mut h);
    let key = format!("{:016x}_scrub_{}_{}.jpg", h.finish(), slot, max_size);
    let cache_path = dir.join(&key);

    if cache_path.exists() {
        if let Ok(bytes) = fs::read(&cache_path) {
            return Ok(to_data_url(&bytes));
        }
    }

    // Video süresini ffprobe ile al (hızlı: < 15ms)
    let duration_out = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            &path,
        ])
        .output();

    let duration_secs: f64 = duration_out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(60.0);

    let target_time = (duration_secs * (clamped_pct as f64)).max(0.2);
    let target_time_str = format!("{:.2}", target_time);

    let scale_arg = format!("scale='min({},iw)':-2", max_size);
    let cache_str = cache_path.to_string_lossy().to_string();

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error",
            "-ss", &target_time_str,
            "-i", &path,
            "-frames:v", "1",
            "-vf", &scale_arg,
            "-y",
            &cache_str,
        ])
        .output();

    if cache_path.exists() {
        if let Ok(bytes) = fs::read(&cache_path) {
            return Ok(to_data_url(&bytes));
        }
    }

    // Hata durumunda standart thumbnail'i döndür
    get_thumbnail(path, max_size)
}

