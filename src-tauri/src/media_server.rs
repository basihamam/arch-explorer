// media_server.rs — Local HTTP media streaming with Range (HTTP 206) support
// Enables native HTML5 <video> and <audio> playback on WebKitGTK / GStreamer on Linux.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

static MEDIA_PORT: AtomicU16 = AtomicU16::new(0);

pub fn to_hex(s: &str) -> String {
    s.bytes().map(|b| format!("{:02x}", b)).collect()
}

pub fn from_hex(s: &str) -> Option<String> {
    if s.len() % 2 != 0 { return None; }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let b = u8::from_str_radix(&s[i..i+2], 16).ok()?;
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

pub fn start_media_server() {
    std::thread::Builder::new()
        .name("media-server".into())
        .spawn(|| {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[media_server] failed to create runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async {
                let listener = match TcpListener::bind("127.0.0.1:0").await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[media_server] bind failed: {}", e);
                        return;
                    }
                };

                if let Ok(addr) = listener.local_addr() {
                    MEDIA_PORT.store(addr.port(), Ordering::SeqCst);
                    println!("[media_server] Local media streamer listening on 127.0.0.1:{}", addr.port());
                }

                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            tokio::spawn(handle_connection(stream));
                        }
                        Err(e) => {
                            eprintln!("[media_server] accept error: {}", e);
                            break;
                        }
                    }
                }
            });
        })
        .ok();
}

/// OBS gibi yazılımlar moov atomunu dosyanın en sonuna yazar. WebKit/GStreamer ise
/// moov en başta (faststart) olduğunda anında ve kusursuz oynatır.
/// ffmpeg -c copy -movflags +faststart komutu 0.2 saniyede kayıpsız indeks yerleşimi yapar.
fn ensure_faststart_streamable(orig_path: &PathBuf) -> PathBuf {
    let ext = orig_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "mp4" && ext != "mov" && ext != "m4v" {
        return orig_path.clone();
    }

    // Top-level box analizi: Eğer 'moov' atomu 'mdat'tan önce geliyorsa dosya faststart'tır.
    // OBS gibi programlarda 'mdat' devasadır ve 'moov' dosyanın sonundadır.
    let mut is_faststart = false;
    if let Ok(mut f) = std::fs::File::open(orig_path) {
        use std::io::{Read, Seek, SeekFrom};
        let mut header = [0u8; 8];
        while let Ok(()) = f.read_exact(&mut header) {
            let size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
            let box_type = &header[4..8];

            if box_type == b"moov" {
                is_faststart = true;
                break;
            }
            if box_type == b"mdat" {
                // mdat, moov'dan önce geldi -> faststart DEĞİL!
                is_faststart = false;
                break;
            }

            let box_size = if size == 1 {
                let mut ext = [0u8; 8];
                if f.read_exact(&mut ext).is_err() { break; }
                u64::from_be_bytes(ext)
            } else if size == 0 {
                break;
            } else {
                size
            };

            let offset = if size == 1 { 16 } else { 8 };
            if box_size < offset { break; }
            if f.seek(SeekFrom::Current((box_size - offset) as i64)).is_err() { break; }
        }
    }

    if is_faststart {
        return orig_path.clone();
    }

    // /tmp/arch_explorer_media_cache dizininde faststart versiyonunu oluştur
    let cache_dir = std::env::temp_dir().join("arch_explorer_media_cache");
    let _ = std::fs::create_dir_all(&cache_dir);

    // Orijinal dosyanın mtime + boyutuyla benzersiz bir ad üret
    let meta = std::fs::metadata(orig_path).ok();
    let mtime = meta.as_ref().and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs()).unwrap_or(0);
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

    let hex_name = format!("{}_{}_{}.mp4", to_hex(&orig_path.to_string_lossy()), mtime, size);
    let cached_path = cache_dir.join(hex_name);

    if cached_path.exists() && cached_path.metadata().map(|m| m.len() > 1000).unwrap_or(false) {
        return cached_path;
    }

    // Hızlıca -c copy ile faststart yap (re-encode yapmaz, 0.2 saniye sürer!)
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", &orig_path.to_string_lossy(),
            "-c", "copy",
            "-movflags", "+faststart",
            cached_path.to_str().unwrap_or(""),
        ])
        .output();

    if let Ok(o) = out {
        if o.status.success() && cached_path.exists() {
            return cached_path;
        }
    }

    orig_path.clone()
}

#[tauri::command]
pub fn get_media_url(path: String) -> Result<String, String> {
    let port = MEDIA_PORT.load(Ordering::SeqCst);
    if port == 0 {
        return Err("Media server is not yet started".into());
    }

    let orig_path = PathBuf::from(&path);
    let stream_path = ensure_faststart_streamable(&orig_path);
    let stream_path_str = stream_path.to_string_lossy().to_string();

    let filename = std::path::Path::new(&path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "media.mp4".to_string());

    let safe_filename: String = filename
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();

    Ok(format!("http://127.0.0.1:{}/stream/{}?p={}", port, safe_filename, to_hex(&stream_path_str)))
}

async fn handle_connection(mut stream: TcpStream) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let first_line = match req_str.lines().next() {
        Some(l) => l,
        None => return,
    };

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    }
    let method = parts[0];
    if method != "GET" && method != "HEAD" {
        let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n").await;
        return;
    }

    let url_path = parts[1];
    let hex_param = if let Some(idx) = url_path.find("p=") {
        &url_path[idx + 2..]
    } else {
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    };

    let hex_clean = hex_param.split('&').next().unwrap_or("");
    let file_path_str = match from_hex(hex_clean) {
        Some(s) => s,
        None => {
            let _ = stream.write_all(b"HTTP/1.1 400 Invalid Hex\r\n\r\n").await;
            return;
        }
    };

    let file_path = PathBuf::from(&file_path_str);
    let sudo_pwd = crate::fs_ops::get_session_sudo_password();
    if !file_path.exists() && sudo_pwd.is_none() {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
        return;
    }

    let (mut file, file_len) = match tokio::fs::File::open(&file_path).await {
        Ok(f) => {
            let meta = match f.metadata().await {
                Ok(m) => m,
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 500 Internal Error\r\n\r\n").await;
                    return;
                }
            };
            (f, meta.len())
        }
        Err(_) => {
            // Sudo şifresi varsa dosyayı oku/stream et
            if let Some(pwd) = crate::fs_ops::get_session_sudo_password() {
                // Dosyaya geçici okuma hakkı (o+r) ver veya akış sağla
                use std::process::{Command, Stdio};
                use std::io::Write;
                let child = Command::new("sudo")
                    .args(["-S", "chmod", "o+r", &file_path_str])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok();
                if let Some(mut c) = child {
                    if let Some(mut stdin) = c.stdin.take() {
                        let _ = stdin.write_all(format!("{}\n", pwd).as_bytes());
                    }
                    let _ = c.wait_with_output();
                }

                // Yeniden açmayı dene
                match tokio::fs::File::open(&file_path).await {
                    Ok(f) => {
                        let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
                        (f, len)
                    }
                    Err(_) => {
                        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
                        return;
                    }
                }
            } else {
                let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
                return;
            }
        }
    };

    let ext_lower = file_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
    let mime = match ext_lower.as_deref() {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/mp4", // WebKit GStreamer handles MKV h264 streams when tagged as video/mp4
        Some("mov") => "video/mp4",
        Some("ogv") | Some("ogg") => "video/ogg",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        _ => mime_guess::from_path(&file_path)
            .first_or_octet_stream()
            .to_string()
            .leak(),
    };

    // Check Range header
    let mut range_header = None;
    for line in req_str.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("range:") {
            range_header = Some(trimmed[6..].trim().to_string());
            break;
        }
    }

    if let Some(ref range) = range_header {
        if let Some(bytes_spec) = range.strip_prefix("bytes=") {
            let bytes_spec = bytes_spec.trim();
            let parts: Vec<&str> = bytes_spec.split('-').collect();
            let (start, end) = if parts.len() == 2 {
                if parts[0].is_empty() {
                    // Suffix range: bytes=-500000 (son 500KB)
                    let suffix_len: u64 = parts[1].parse().unwrap_or(0);
                    let s = file_len.saturating_sub(suffix_len);
                    (s, file_len.saturating_sub(1))
                } else if parts[1].is_empty() {
                    // Prefix range: bytes=500- (unbounded to EOF)
                    let s: u64 = parts[0].parse().unwrap_or(0);
                    (s, file_len.saturating_sub(1))
                } else {
                    // Regular range: bytes=0-1024
                    let s: u64 = parts[0].parse().unwrap_or(0);
                    let requested_end: u64 = parts[1].parse().unwrap_or(file_len - 1);
                    let e = requested_end.min(file_len.saturating_sub(1));
                    (s, e)
                }
            } else {
                (0, file_len.saturating_sub(1))
            };

            if start <= end && start < file_len {
                let content_len = end - start + 1;
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\n\
Accept-Ranges: bytes\r\n\
Content-Range: bytes {}-{}/{}\r\n\
Content-Length: {}\r\n\
Content-Type: {}\r\n\
Connection: close\r\n\
Access-Control-Allow-Origin: *\r\n\
\r\n",
                    start, end, file_len, content_len, mime
                );

                if stream.write_all(header.as_bytes()).await.is_err() {
                    return;
                }

                if method == "HEAD" {
                    return;
                }

                if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                    return;
                }

                let mut to_send = content_len;
                let mut chunk = [0u8; 128 * 1024];
                while to_send > 0 {
                    let to_read = std::cmp::min(to_send, chunk.len() as u64) as usize;
                    match file.read(&mut chunk[..to_read]).await {
                        Ok(n) if n > 0 => {
                            if stream.write_all(&chunk[..n]).await.is_err() {
                                break;
                            }
                            to_send -= n as u64;
                            tokio::task::yield_now().await;
                        }
                        _ => break,
                    }
                }
                return;
            } else {
                // Unsatisfiable range
                let header = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\n\
Content-Range: bytes */{}\r\n\
Connection: close\r\n\
\r\n",
                    file_len
                );
                let _ = stream.write_all(header.as_bytes()).await;
                return;
            }
        }
    }

    // No Range: send full 200 OK
    let header = format!(
        "HTTP/1.1 200 OK\r\n\
Accept-Ranges: bytes\r\n\
Content-Length: {}\r\n\
Content-Type: {}\r\n\
Connection: close\r\n\
Access-Control-Allow-Origin: *\r\n\
\r\n",
        file_len, mime
    );

    if stream.write_all(header.as_bytes()).await.is_err() {
        return;
    }

    if method == "HEAD" {
        return;
    }

    let mut chunk = [0u8; 256 * 1024];
    loop {
        match file.read(&mut chunk).await {
            Ok(n) if n > 0 => {
                if stream.write_all(&chunk[..n]).await.is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
            _ => break,
        }
    }
}
