// src/backend/du_cache.rs
// Klasör boyutu önbelleği — du -sb sonuçlarını saklar
// Stale süreleri: sığ dizinler 15dk, derin 5dk
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuEntry {
    pub size_bytes: u64,
    pub timestamp:  u64,  // unix seconds
}

fn cache_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("du_cache.json")
}

use std::sync::{OnceLock, RwLock};

static MEM_CACHE: OnceLock<RwLock<HashMap<String, DuEntry>>> = OnceLock::new();
static DIR_COUNTS: OnceLock<RwLock<HashMap<String, usize>>> = OnceLock::new();
static PENDING_DIR_COUNTS: OnceLock<RwLock<std::collections::HashSet<String>>> = OnceLock::new();

fn load_cache() -> HashMap<String, DuEntry> {
    let p = cache_path();
    if !p.exists() { return HashMap::new(); }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &HashMap<String, DuEntry>) {
    let p = cache_path();
    if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).ok(); }
    if let Ok(json) = serde_json::to_string(cache) {
        std::fs::write(&p, json).ok();
    }
    if let Ok(mut mem) = get_mem_cache().write() {
        *mem = cache.clone();
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn depth(path: &str) -> usize {
    Path::new(path).components().count()
}

fn stale_secs(path: &str) -> u64 {
    if depth(path) <= 3 { 900 } else { 300 }
}

fn get_mount_used_bytes(path: &str) -> Option<u64> {
    let out = std::process::Command::new("df")
        .args(["-B1", "--output=used", path])
        .output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().nth(1)?.trim().parse::<u64>().ok()
}

fn get_mem_cache() -> &'static RwLock<HashMap<String, DuEntry>> {
    MEM_CACHE.get_or_init(|| RwLock::new(load_cache()))
}

fn get_dir_counts() -> &'static RwLock<HashMap<String, usize>> {
    DIR_COUNTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn get_pending_counts() -> &'static RwLock<std::collections::HashSet<String>> {
    PENDING_DIR_COUNTS.get_or_init(|| RwLock::new(std::collections::HashSet::new()))
}

/// Önbellekten boyut döner — bellekten anında, yoksa None
pub fn get_cached_size(path: &str) -> Option<u64> {
    let cache_lock = get_mem_cache().read().ok()?;
    let entry = cache_lock.get(path)?;
    let age   = now_secs().saturating_sub(entry.timestamp);
    if age < stale_secs(path) {
        if let Some(mount_used) = get_mount_used_bytes(path) {
            if mount_used > 0 && entry.size_bytes > mount_used {
                return Some(mount_used);
            }
        }
        Some(entry.size_bytes)
    } else {
        None
    }
}

/// Klasör içindeki öğe sayısını önbellekten döner
pub fn get_dir_item_count(path: &str) -> Option<usize> {
    get_dir_counts().read().ok().and_then(|c| c.get(path).copied())
}

/// Klasör öğe sayısını arka planda tek seferlik hesaplatır (flood engellemeli)
pub fn request_dir_item_count(path: &str, ctx: egui::Context) {
    if get_dir_item_count(path).is_some() {
        return;
    }
    let is_already_pending = {
        if let Ok(mut pending) = get_pending_counts().write() {
            if pending.contains(path) {
                true
            } else {
                pending.insert(path.to_string());
                false
            }
        } else {
            true
        }
    };

    if !is_already_pending {
        let p = path.to_string();
        std::thread::spawn(move || {
            let count = std::fs::read_dir(&p).map(|rd| rd.count()).unwrap_or(0);
            if let Ok(mut map) = get_dir_counts().write() {
                map.insert(p.clone(), count);
            }
            if let Ok(mut pending) = get_pending_counts().write() {
                pending.remove(&p);
            }
            ctx.request_repaint();
        });
    }
}

/// du ve btrfs ile gerçek disk kullanımını hesapla ve cache'e yaz
pub fn compute_and_cache(path: String) -> u64 {
    // 1. Önce btrfs filesystem du dene (CoW, reflink, snapshot katlarını engellemek için)
    let btrfs_size = std::process::Command::new("btrfs")
        .args(["filesystem", "du", "-s", "--raw", "--", &path])
        .output()
        .ok()
        .and_then(|o| {
            if !o.status.success() { return None; }
            let s = String::from_utf8_lossy(&o.stdout);
            let line = s.lines().nth(1)?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let excl = parts[1].parse::<u64>().unwrap_or(0);
                let shared = parts[2].parse::<u64>().unwrap_or(0);
                if excl + shared > 0 {
                    return Some(excl + shared);
                }
                parts[0].parse::<u64>().ok()
            } else {
                None
            }
        });

    let raw_size = if let Some(bsz) = btrfs_size {
        bsz
    } else {
        let out = std::process::Command::new("du")
            .args(["-s", "-B1", "-x", "--", &path])
            .output();

        out.ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.split_whitespace().next()
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .unwrap_or(0)
    };

    // 2. Mantıksal sınır: Bir klasör, bulunduğu diskin toplam kullanılan alanından (df used) daha büyük olamaz!
    let size = if let Some(mount_used) = get_mount_used_bytes(&path) {
        if mount_used > 0 && raw_size > mount_used {
            mount_used
        } else {
            raw_size
        }
    } else {
        raw_size
    };

    let mut cache = load_cache();
    cache.insert(path, DuEntry { size_bytes: size, timestamp: now_secs() });
    save_cache(&cache);
    size
}

/// Önbellekten invalidate et (inotify değişikliğinde)
pub fn invalidate(path: &str) {
    let mut cache = load_cache();
    cache.remove(path);
    // Üst dizini de invalidate et
    if let Some(parent) = Path::new(path).parent() {
        cache.remove(&parent.to_string_lossy().to_string());
    }
    save_cache(&cache);
}

/// Tüm cache'i temizle
pub fn clear_all() {
    let p = cache_path();
    std::fs::remove_file(p).ok();
}

pub fn cache_entry_count() -> usize {
    load_cache().len()
}
