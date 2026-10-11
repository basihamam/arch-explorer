// du_cache.rs — Klasör boyutu cache + büyük dosya listesi
// Cache: ~/.local/share/arch-explorer/du_cache.json
// Stale süresi: normal klasör 5dk, derin olmayan (depth≤2) 15dk, büyük dosyalar 1sa
// inotify dir-changed gelince o path invalidate edilir

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

// ---- Sabitler ----
const STALE_SHALLOW: i64 = 3600;  // depth ≤ 2  → 1 saat
const STALE_NORMAL:  i64 = 1800;  // normal     → 30 dakika
const STALE_BIGFILE: i64 = 3600;  // büyük dosya listesi → 1 saat

// ---- Tipler ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuEntry {
    pub size:       u64,   // byte
    pub file_count: u64,   // dosya+klasör sayısı (1 derinlik)
    pub timestamp:  i64,   // unix
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BigFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BigFilesEntry {
    pub files:     Vec<BigFile>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DuCache {
    pub dirs:      HashMap<String, DuEntry>,
    pub big_files: HashMap<String, BigFilesEntry>,
}

// ---- Dönüş tipleri (frontend için) ----

#[derive(Debug, Serialize, Deserialize)]
pub struct DuSizeResult {
    pub size:              u64,
    pub file_count:        u64,
    pub from_cache:        bool,
    pub permission_denied: bool,
    pub is_calculating:    bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub dir_count:      usize,
    pub bigfile_count:  usize,
    pub cache_path:     String,
}

// ---- Global cache (Mutex korumalı) ----
static CACHE: Mutex<Option<DuCache>> = Mutex::new(None);

fn cache_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("du_cache.json")
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn depth(path: &str) -> usize {
    path.trim_matches('/').split('/').filter(|s| !s.is_empty()).count()
}

fn stale_limit(path: &str) -> i64 {
    if depth(path) <= 2 { STALE_SHALLOW } else { STALE_NORMAL }
}

/// Dosya sisteminin gerçek kullanılan byte boyutunu alır (statvfs).
/// Hiçbir alt klasörün boyutu dosya sisteminin toplam kullanılan boyutunu aşamaz!
fn get_fs_max_used(path: &str) -> Option<u64> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    let c_path = CString::new(path).ok()?;
    unsafe {
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
        if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) == 0 {
            let s = stat.assume_init();
            let total = (s.f_blocks as u64).saturating_mul(s.f_frsize as u64);
            let free = (s.f_bavail as u64).saturating_mul(s.f_frsize as u64);
            let used = total.saturating_sub(free);
            Some(used)
        } else {
            None
        }
    }
}

// ---- Cache yükle / kaydet ----

fn load_cache() -> DuCache {
    let p = cache_path();
    if !p.exists() { return DuCache::default(); }
    let data = fs::read_to_string(&p).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_cache(cache: &DuCache) {
    let p = cache_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(data) = serde_json::to_string(cache) {
        fs::write(&p, data).ok();
    }
}

pub fn with_cache<F, R>(f: F) -> R
where
    F: FnOnce(&mut DuCache) -> R,
{
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_cache());
    }
    f(guard.as_mut().unwrap())
}

// ---- Aktif taramalar (aynı dizin için birden fazla du çalıştırmamak için) ----
static RUNNING_SCANS: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

fn is_scan_running(path: &str) -> bool {
    let mut guard = RUNNING_SCANS.lock().unwrap_or_else(|e| e.into_inner());
    let set = guard.get_or_insert_with(std::collections::HashSet::new);
    set.contains(path)
}

fn mark_scan_start(path: &str) -> bool {
    let mut guard = RUNNING_SCANS.lock().unwrap_or_else(|e| e.into_inner());
    let set = guard.get_or_insert_with(std::collections::HashSet::new);
    if set.contains(path) {
        false
    } else {
        set.insert(path.to_string());
        true
    }
}

fn mark_scan_done(path: &str) {
    let mut guard = RUNNING_SCANS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(set) = guard.as_mut() {
        set.remove(path);
    }
}

// ---- Klasör boyutu ----

#[tauri::command]
pub async fn get_dir_size(path: String) -> Result<DuSizeResult, String> {
    // 1. Cache'e bak
    let cached = with_cache(|c| c.dirs.get(&path).cloned());
    if let Some(entry) = cached {
        if now() - entry.timestamp < stale_limit(&path) {
            mark_scan_done(&path);
            return Ok(DuSizeResult {
                size: entry.size,
                file_count: entry.file_count,
                from_cache: true,
                permission_denied: false,
                is_calculating: false,
            });
        }
    }

    // 2. Halihazırda bu dizin taranıyor mu?
    if !mark_scan_start(&path) {
        let fast_count = fs::read_dir(&path)
            .map(|rd| rd.count() as u64)
            .unwrap_or(0);
        return Ok(DuSizeResult {
            size: 0,
            file_count: fast_count,
            from_cache: false,
            permission_denied: false,
            is_calculating: true,
        });
    }

    // 3. Tek bir görev başlat
    let p_clone = path.clone();
    let compute_task = tokio::task::spawn_blocking(move || {
        let read_res = fs::read_dir(&p_clone);
        let is_denied = match &read_res {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => true,
            _ => false,
        };

        let file_count = read_res
            .map(|rd| rd.count() as u64)
            .unwrap_or(0);

        let du_res = Command::new("du")
            .args(["-s", "-x", "-B1", "--", &p_clone])
            .output();

        let (raw_size, perm_err): (u64, bool) = match du_res {
            Ok(out) => {
                let s = String::from_utf8_lossy(&out.stdout);
                let sz = s.split_whitespace()
                    .next()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);
                if sz > 0 {
                    (sz, false)
                } else if is_denied {
                    (0, true)
                } else {
                    (0, false)
                }
            }
            _ => (0, is_denied),
        };

        // Eğer du absürt bir boyut döndürdüyse (sparse dosyalar, döngüsel mount'lar veya btrfs reflink katlaması),
        // dosya sisteminin toplam kullanılan alanından büyük olamaz.
        let size = if let Some(max_used) = get_fs_max_used(&p_clone) {
            if raw_size > max_used { max_used } else { raw_size }
        } else {
            raw_size
        };

        // Hesaplama bitince doğrudan cache'e kaydet ve tarama bayrağını temizle
        if size > 0 || !perm_err {
            let entry = DuEntry { size, file_count, timestamp: now() };
            with_cache(|c| {
                c.dirs.insert(p_clone.clone(), entry);
                save_cache(c);
            });
        }
        mark_scan_done(&p_clone);

        (size, file_count, perm_err)
    });

    // 3 saniye bekle - çoğu dizin anında dönsün
    match tokio::time::timeout(std::time::Duration::from_millis(3000), compute_task).await {
        Ok(Ok((size, file_count, perm_err))) => {
            Ok(DuSizeResult {
                size,
                file_count,
                from_cache: false,
                permission_denied: perm_err,
                is_calculating: false,
            })
        }
        Ok(Err(e)) => {
            mark_scan_done(&path);
            Err(format!("Task execution error: {}", e))
        }
        Err(_) => {
            // 3 saniyede bitmedi — compute_task arka planda tek başına çalışmaya devam ediyor!
            // Kesinlikle YENİ bir du görevi başlatmıyoruz.
            let fast_count = fs::read_dir(&path)
                .map(|rd| rd.count() as u64)
                .unwrap_or(0);
            Ok(DuSizeResult {
                size: 0,
                file_count: fast_count,
                from_cache: false,
                permission_denied: false,
                is_calculating: true,
            })
        }
    }
}

/// Sudo ile klasör boyutu hesapla (kullanıcı şifre girdiğinde)
#[tauri::command]
pub async fn sudo_get_dir_size(path: String, password: String) -> Result<DuSizeResult, String> {
    let p_clone = path.clone();
    let compute_task = tokio::task::spawn_blocking(move || {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("sudo")
            .args(["-S", "du", "-s", "-x", "-B1", "--", &p_clone])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", password).as_bytes());
        }

        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();

        if crate::mounts::is_wrong_password(&stderr) {
            return Err("WRONG_PASSWORD".to_string());
        }

        if !out.status.success() {
            return Err(format!("sudo du hatası: {}", stderr.trim()));
        }

        let raw_size: u64 = stdout.split_whitespace()
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);

        let size = if let Some(max_used) = get_fs_max_used(&p_clone) {
            if raw_size > max_used { max_used } else { raw_size }
        } else {
            raw_size
        };

        // Dosya sayısını sudo find ile hızlıca al
        let mut f_child = Command::new("sudo")
            .args(["-S", "find", &p_clone, "-maxdepth", "1", "-mindepth", "1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = f_child.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", password).as_bytes());
        }
        let f_out = f_child.wait_with_output().map_err(|e| e.to_string())?;
        let f_count = String::from_utf8_lossy(&f_out.stdout).lines().count() as u64;

        Ok((size, f_count))
    });

    match compute_task.await {
        Ok(Ok((size, file_count))) => {
            let entry = DuEntry { size, file_count, timestamp: now() };
            with_cache(|c| {
                c.dirs.insert(path.clone(), entry);
                save_cache(c);
            });
            Ok(DuSizeResult {
                size,
                file_count,
                from_cache: false,
                permission_denied: false,
                is_calculating: false,
            })
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(e.to_string()),
    }
}

// ---- Büyük dosyalar ----

#[tauri::command]
pub async fn get_big_files(mount_point: String) -> Result<Vec<BigFile>, String> {
    // Cache'e bak
    let cached = with_cache(|c| c.big_files.get(&mount_point).cloned());
    if let Some(entry) = cached {
        if now() - entry.timestamp < STALE_BIGFILE {
            return Ok(entry.files);
        }
    }

    let mp_clone = mount_point.clone();
    // 2 saniye timeout ile sınırlandırılmış find (tüm diski günlerce taramasın)
    let scan_task = tokio::task::spawn_blocking(move || {
        let out = Command::new("sh")
            .args([
                "-c",
                &format!(
                    "find '{}' -maxdepth 4 -type f -size +10M -printf '%s\\t%p\\n' 2>/dev/null | sort -rn | head -10",
                    mp_clone
                ),
            ])
            .output();

        match out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                let files: Vec<BigFile> = text
                    .lines()
                    .filter_map(|line| {
                        let mut parts = line.splitn(2, '\t');
                        let size: u64 = parts.next()?.parse().ok()?;
                        let path = parts.next()?.to_string();
                        Some(BigFile { path, size })
                    })
                    .collect();
                files
            }
            Err(_) => Vec::new(),
        }
    });

    let files = match tokio::time::timeout(std::time::Duration::from_millis(2000), scan_task).await {
        Ok(Ok(f)) => f,
        _ => Vec::new(),
    };

    let entry = BigFilesEntry { files: files.clone(), timestamp: now() };
    with_cache(|c| {
        c.big_files.insert(mount_point.clone(), entry);
        save_cache(c);
    });

    Ok(files)
}

// ---- Cache invalidation ----

/// inotify dir-changed gelince o path'i ve parent'larını temizle
#[tauri::command]
pub fn invalidate_du_cache(path: String) {
    with_cache(|c| {
        c.dirs.remove(&path);
        // parent'ı da invalidate et — boyutu değişmiştir
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string());
        if let Some(p) = parent {
            c.dirs.remove(&p);
        }
        save_cache(c);
    });
}

/// Tüm cache'i temizle (ayarlar panelinden)
#[tauri::command]
pub fn clear_du_cache() -> Result<usize, String> {
    let count = with_cache(|c| {
        let n = c.dirs.len() + c.big_files.len();
        *c = DuCache::default();
        n
    });
    let p = cache_path();
    if p.exists() {
        let _ = fs::remove_file(&p);
    }
    Ok(count)
}

/// Cache istatistikleri (ayarlar için)
#[tauri::command]
pub fn get_du_cache_stats() -> CacheStats {
    with_cache(|c| CacheStats {
        dir_count:     c.dirs.len(),
        bigfile_count: c.big_files.len(),
        cache_path:    cache_path().to_string_lossy().to_string(),
    })
}
