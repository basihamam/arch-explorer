use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;
use chrono::{DateTime, Local};
use walkdir::WalkDir;
use anyhow::Result;
use once_cell::sync::Lazy;

// ── Directory listing cache ────────────────────────────────────────────────
#[derive(Clone)]
struct CachedDir {
    entries:     Vec<FileEntry>,
    dir_mtime:   SystemTime,
    show_hidden: bool,
}

static DIR_CACHE: Lazy<Mutex<HashMap<String, CachedDir>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn invalidate_dir_cache(path: &str) {
    let mut cache = DIR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.remove(path);
    if let Some(parent) = std::path::Path::new(path).parent() {
        cache.remove(&parent.to_string_lossy().to_string());
    }
}

fn list_directory_inner(path: String, show_hidden: bool) -> Result<DirectoryListing, String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_list_directory(&path, show_hidden);
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    let mut resolved_path = path.clone();

    if resolved_path == "trash://" || resolved_path == "trash:" || resolved_path == "Trash" {
        resolved_path = home.join(".local/share/Trash/files").to_string_lossy().to_string();
    } else if resolved_path.trim_end_matches('/').ends_with(".local/share/Trash") {
        resolved_path = home.join(".local/share/Trash/files").to_string_lossy().to_string();
    }

    let dir_path = PathBuf::from(&resolved_path);
    if resolved_path.contains(".local/share/Trash") {
        let _ = fs::create_dir_all(&dir_path);
    }

    if !dir_path.exists() { return Err(format!("Path bulunamadı: {}", path)); }
    if !dir_path.is_dir() { return Err(format!("Dizin değil: {}", path)); }

    let dir_meta  = fs::metadata(&dir_path).map_err(|e| e.to_string())?;
    let dir_mtime = dir_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    {
        let cache = DIR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cache.get(&resolved_path) {
            if c.dir_mtime == dir_mtime && c.show_hidden == show_hidden {
                let total = c.entries.len();
                return Ok(DirectoryListing {
                    path:        resolved_path.clone(),
                    entries:     c.entries.clone(),
                    parent:      dir_path.parent().map(|p| p.to_string_lossy().to_string()),
                    total_items: total,
                });
            }
        }
    }

    let read_dir = fs::read_dir(&dir_path)
        .map_err(|e| format!("Dizin okunamadı: {}", e))?;

    let mut entries: Vec<FileEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(|e| build_entry(&e.path()))
        .filter(|e| show_hidden || !e.hidden)
        .collect();

    entries.sort_unstable_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let ca = a.name.chars().flat_map(|c| c.to_lowercase());
                let cb = b.name.chars().flat_map(|c| c.to_lowercase());
                ca.cmp(cb)
            }
        }
    });

    {
        let mut cache = DIR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let total_items: usize = cache.values().map(|c| c.entries.len()).sum();
        if cache.len() >= 40 || total_items >= 15000 {
            let oldest = cache.keys().next().cloned();
            if let Some(k) = oldest { cache.remove(&k); }
        }
        if entries.len() <= 6000 {
            cache.insert(resolved_path.clone(), CachedDir {
                entries:     entries.clone(),
                dir_mtime,
                show_hidden,
            });
        }
    }

    let total  = entries.len();
    let parent = dir_path.parent().map(|p| p.to_string_lossy().to_string());
    Ok(DirectoryListing { path: resolved_path, entries, parent, total_items: total })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: String,
    pub created: String,
    pub permissions: String,
    pub mime_type: String,
    pub extension: String,
    pub hidden: bool,
    #[serde(default)]
    pub children_count: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub parent: Option<String>,
    pub total_items: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub fs_type: String,
    pub device: String,
}

/// Gelişmiş bookmark — icon, pinned, sıralama vs.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub icon: String,          // "home", "downloads", "folder", "star" vs.
    #[serde(default)]
    pub pinned: bool,          // Sidebar'ın üstünde sabit dursun mu
    #[serde(default)]
    pub order: i32,            // Manuel sıralama
    #[serde(default)]
    pub color: String,         // Opsiyonel renk etiketi (#ff6b6b gibi)
}

/// Drag-drop taşıma isteği
#[derive(Debug, Serialize, Deserialize)]
pub struct DragDropRequest {
    pub sources: Vec<String>,   // Sürüklenen dosyaların path'leri
    pub dest_dir: String,       // Bırakılan dizin
    pub copy: bool,             // true = Ctrl basılıysa kopyala, false = taşı
}

/// Drag-drop sonucu
#[derive(Debug, Serialize, Deserialize)]
pub struct DragDropResult {
    pub moved: Vec<String>,
    pub failed: Vec<String>,
    pub errors: Vec<String>,
}

fn format_permissions(meta: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode();
    let chars = ['x', 'w', 'r'];
    let mut perm = String::with_capacity(10);

    if meta.is_dir() { perm.push('d'); }
    else if meta.file_type().is_symlink() { perm.push('l'); }
    else { perm.push('-'); }

    for shift in [6u32, 3, 0] {
        for (i, &c) in chars.iter().rev().enumerate() {
            if mode & (1 << (shift + i as u32)) != 0 {
                perm.push(c);
            } else {
                perm.push('-');
            }
        }
    }
    perm
}

fn build_entry(path: &Path) -> Option<FileEntry> {
    let meta = fs::symlink_metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let is_symlink = meta.file_type().is_symlink();
    let real_meta = if is_symlink {
        fs::metadata(path).unwrap_or(meta.clone())
    } else {
        meta.clone()
    };

    let modified = real_meta.modified().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let created = real_meta.created().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();

    let extension = path.extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let mime_type = if real_meta.is_dir() {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string()
    };

    let (dir_size, children_count) = if real_meta.is_dir() {
        let path_str = path.to_string_lossy();
        let cached_sz = crate::du_cache::with_cache(|c| c.dirs.get(path_str.as_ref()).map(|d| d.size)).unwrap_or(0);
        let count = fs::read_dir(path).map(|rd| rd.count()).ok();
        (cached_sz, count)
    } else {
        (real_meta.len(), None)
    };

    Some(FileEntry {
        name: name.clone(),
        path: path.to_string_lossy().to_string(),
        is_dir: real_meta.is_dir(),
        is_symlink,
        size: dir_size,
        modified,
        created,
        permissions: format_permissions(&real_meta),
        mime_type,
        extension,
        hidden: name.starts_with('.'),
        children_count,
    })
}

#[tauri::command]
pub async fn list_directory(path: String, show_hidden: bool) -> Result<DirectoryListing, String> {
    tokio::task::spawn_blocking(move || {
        list_directory_inner(path, show_hidden)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_file_info(path: String) -> Result<FileEntry, String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_get_file_entry(&path);
    }
    let p = PathBuf::from(&path);
    build_entry(&p).ok_or_else(|| format!("Stat edilemedi: {}", path))
}

#[tauri::command]
pub fn create_dir(path: String) -> Result<(), String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_create_dir(&path);
    }
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_create_file(&path);
    }
    fs::File::create(&path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_item(path: String, permanent: bool) -> Result<(), String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_delete_item(&path);
    }
    if permanent {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())
        } else {
            fs::remove_file(&path).map_err(|e| e.to_string())
        }
    } else {
        trash::delete(&path).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn rename_item(old_path: String, new_path: String) -> Result<(), String> {
    if old_path.starts_with("adb://") {
        return crate::adb::adb_rename_item(&old_path, &new_path);
    }
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_item(src: String, dst: String) -> Result<(), String> {
    let res = crate::clipboard::execute_file_batch(crate::clipboard::BatchExecuteRequest {
        op: "copy".into(),
        items: vec![crate::clipboard::BatchItemPlan {
            src,
            dst,
            conflict_action: "replace".into(),
        }],
    })?;
    if let Some((_, err)) = res.failed_items.into_iter().next() {
        return Err(err);
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    crate::clipboard::copy_recursive(src, dst).map_err(|e| anyhow::anyhow!(e))
}

#[tauri::command]
pub fn move_item(src: String, dst: String) -> Result<(), String> {
    let res = crate::clipboard::execute_file_batch(crate::clipboard::BatchExecuteRequest {
        op: "move".into(),
        items: vec![crate::clipboard::BatchItemPlan {
            src,
            dst,
            conflict_action: "replace".into(),
        }],
    })?;
    if let Some((_, err)) = res.failed_items.into_iter().next() {
        return Err(err);
    }
    Ok(())
}

// ── Sudo-backed File Operations ─────────────────────────────────────────────

fn run_sudo_sh(cmd: &str, password: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("sudo")
        .args(["-S", "sh", "-c", cmd])
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

    if crate::mounts::is_wrong_password(&stderr) {
        return Err("WRONG_PASSWORD".into());
    }
    if !out.status.success() {
        return Err(stderr.trim().to_string());
    }
    Ok(())
}

static SESSION_SUDO_PASSWORD: Lazy<Mutex<Option<String>>> =
    Lazy::new(|| Mutex::new(None));

pub fn get_session_sudo_password() -> Option<String> {
    SESSION_SUDO_PASSWORD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn set_session_sudo_password_val(pwd: Option<String>) {
    let mut lock = SESSION_SUDO_PASSWORD.lock().unwrap_or_else(|e| e.into_inner());
    *lock = pwd;
}

#[tauri::command]
pub fn set_session_sudo_password(password: Option<String>) -> Result<(), String> {
    set_session_sudo_password_val(password);
    Ok(())
}

#[tauri::command]
pub fn verify_sudo_password(password: String) -> Result<bool, String> {
    run_sudo_sh("true", &password).map(|_| {
        set_session_sudo_password_val(Some(password));
        true
    })
}

#[tauri::command]
pub async fn sudo_list_directory(path: String, password: String, show_hidden: bool) -> Result<DirectoryListing, String> {
    tokio::task::spawn_blocking(move || {
        let python_script = r#"
import os, sys, json, stat, datetime, mimetypes

target = sys.argv[1]
show_hidden = sys.argv[2].lower() == "true"

try:
    entries = []
    with os.scandir(target) as it:
        for entry in it:
            name = entry.name
            if not show_hidden and name.startswith('.'):
                continue
            try:
                sm = entry.stat(follow_symlinks=False)
                is_symlink = entry.is_symlink()
                try:
                    m = entry.stat(follow_symlinks=True) if is_symlink else sm
                except Exception:
                    m = sm
                is_dir = stat.S_ISDIR(m.st_mode)
                size = 0 if is_dir else m.st_size
                modified = datetime.datetime.fromtimestamp(m.st_mtime).strftime('%Y-%m-%d %H:%M')
                created = datetime.datetime.fromtimestamp(getattr(m, 'st_birthtime', m.st_ctime)).strftime('%Y-%m-%d %H:%M')
                mode = m.st_mode
                perm = ['d' if is_dir else ('l' if is_symlink else '-')]
                for shift in (6, 3, 0):
                    perm.append('r' if mode & (1 << (shift + 2)) else '-')
                    perm.append('w' if mode & (1 << (shift + 1)) else '-')
                    perm.append('x' if mode & (1 << shift) else '-')
                ext = name.rsplit('.', 1)[-1] if (not is_dir and '.' in name) else ''
                mime_type = 'inode/directory' if is_dir else (mimetypes.guess_type(entry.path)[0] or 'application/octet-stream')
                entries.append({
                    'name': name,
                    'path': entry.path,
                    'is_dir': is_dir,
                    'is_symlink': is_symlink,
                    'size': size,
                    'modified': modified,
                    'created': created,
                    'permissions': ''.join(perm),
                    'mime_type': mime_type,
                    'extension': ext,
                    'hidden': name.startswith('.')
                })
            except Exception:
                pass
    entries.sort(key=lambda x: (not x['is_dir'], x['name'].lower()))
    p = os.path.abspath(target)
    parent = os.path.dirname(p)
    if parent == p:
        parent = None
    res = {
        'path': p,
        'entries': entries,
        'parent': parent,
        'total_items': len(entries)
    }
    print(json.dumps(res))
except Exception as e:
    sys.stderr.write(str(e))
    sys.exit(1)
"#;

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("sudo")
            .args(["-S", "python3", "-c", python_script, &path, if show_hidden { "true" } else { "false" }])
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

        if crate::mounts::is_wrong_password(&stderr) {
            return Err("WRONG_PASSWORD".into());
        }
        if !out.status.success() {
            return Err(stderr.trim().to_string());
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let listing: DirectoryListing = serde_json::from_str(stdout.trim())
            .map_err(|e| format!("JSON parse hatası: {}: {}", e, stdout))?;

        set_session_sudo_password_val(Some(password));
        Ok(listing)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn sudo_create_dir(path: String, password: String) -> Result<(), String> {
    let cmd = format!("mkdir -p '{}'", path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn sudo_create_file(path: String, password: String) -> Result<(), String> {
    let cmd = format!("touch '{}'", path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn sudo_delete_item(path: String, _permanent: bool, password: String) -> Result<(), String> {
    let cmd = format!("rm -rf '{}'", path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn sudo_rename_item(old_path: String, new_path: String, password: String) -> Result<(), String> {
    let cmd = format!("mv '{}' '{}'", old_path.replace('\'', "'\\''"), new_path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn sudo_copy_item(src: String, dst: String, password: String) -> Result<(), String> {
    let cmd = format!("cp -a '{}' '{}'", src.replace('\'', "'\\''"), dst.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn sudo_move_item(src: String, dst: String, password: String) -> Result<(), String> {
    let cmd = format!("mv '{}' '{}'", src.replace('\'', "'\\''"), dst.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

/// Drag-drop işlemi — birden fazla dosyayı hedef dizine taşı/kopyala
/// Frontend'den sürükle-bırak eventi gelince bunu çağır
#[tauri::command]
pub fn drag_drop_files(req: DragDropRequest) -> Result<DragDropResult, String> {
    let dest = PathBuf::from(&req.dest_dir);

    if !dest.is_dir() {
        return Err(format!("Hedef dizin bulunamadı: {}", req.dest_dir));
    }

    let mut plans = vec![];
    for src_path in &req.sources {
        let src = PathBuf::from(src_path);
        if !src.exists() {
            continue;
        }
        let file_name = match src.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let dest_path = dest.join(&file_name).to_string_lossy().to_string();
        plans.push(crate::clipboard::BatchItemPlan {
            src: src_path.clone(),
            dst: dest_path,
            conflict_action: "rename".into(),
        });
    }

    let op = if req.copy { "copy" } else { "move" };
    let res = crate::clipboard::execute_file_batch(crate::clipboard::BatchExecuteRequest {
        op: op.into(),
        items: plans,
    })?;

    let moved = (0..res.success_count).map(|i| format!("item_{}", i)).collect();
    let failed = res.failed_items.iter().map(|(p, _)| p.clone()).collect();
    let errors = res.failed_items.iter().map(|(p, e)| format!("{}: {}", p, e)).collect();

    Ok(DragDropResult { moved, failed, errors })
}

/// Sidebar bookmark'larına dizin sürükle-bırak ile ekle
#[tauri::command]
pub fn drop_to_bookmark_sidebar(path: String) -> Result<Bookmark, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Sadece klasörler bookmark olabilir: {}", path));
    }

    let name = p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let bm = Bookmark {
        name: name.clone(),
        path: path.clone(),
        icon: "folder".to_string(),
        pinned: false,
        order: get_next_bookmark_order(),
        color: String::new(),
    };

    add_bookmark_full(bm.clone())?;
    Ok(bm)
}

fn get_next_bookmark_order() -> i32 {
    let bms = get_bookmarks();
    bms.iter().map(|b| b.order).max().unwrap_or(0) + 1
}

/// Dosya yolu var mı kontrol et (frontend için utility)
#[tauri::command]
pub fn path_exists(path: String) -> bool {
    if path.starts_with("adb://") {
        return crate::adb::adb_get_file_entry(&path).is_ok();
    }
    PathBuf::from(&path).exists()
}

/// Bir path'in dizin mi dosya mı olduğunu döndür
#[tauri::command]
pub fn path_is_dir(path: String) -> bool {
    if path.starts_with("adb://") {
        return crate::adb::adb_get_file_entry(&path).map(|e| e.is_dir).unwrap_or(false);
    }
    PathBuf::from(&path).is_dir()
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_drives() -> Vec<DriveInfo> {
    let output = Command::new("df")
        .args(["-h", "--output=source,target,size,used,avail,fstype"])
        .output();

    let Ok(out) = output else { return vec![]; };
    let text = String::from_utf8_lossy(&out.stdout);

    text.lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 { return None; }
            let device = parts[0];
            let mount  = parts[1];

            if !device.starts_with("/dev/") { return None; }

            Some(DriveInfo {
                device: device.to_string(),
                mount_point: mount.to_string(),
                total: parse_human_size(parts[2]),
                used: parse_human_size(parts[3]),
                available: parse_human_size(parts[4]),
                fs_type: parts[5].to_string(),
                name: mount.split('/').last().unwrap_or(device).to_string(),
            })
        })
        .collect()
}

fn parse_human_size(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() || s == "-" { return 0; }
    let (num, suffix) = s.split_at(s.len().saturating_sub(1));
    let base: f64 = num.parse().unwrap_or(0.0);
    match suffix {
        "G" => (base * 1_073_741_824.0) as u64,
        "M" => (base * 1_048_576.0) as u64,
        "K" => (base * 1_024.0) as u64,
        "T" => (base * 1_099_511_627_776.0) as u64,
        _ => base as u64,
    }
}

#[tauri::command]
pub fn search_files(
    root: String,
    query: String,
    max_results: Option<usize>,
) -> Vec<FileEntry> {
    let limit = max_results.unwrap_or(200);
    let q = query.to_lowercase();

    WalkDir::new(&root)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains(&q)
        })
        .take(limit)
        .filter_map(|e| build_entry(e.path()))
        .collect()
}

// ---- Bookmark CRUD ----

fn bookmarks_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("bookmarks.json")
}

#[tauri::command]
pub fn get_bookmarks() -> Vec<Bookmark> {
    let path = bookmarks_path();
    if !path.exists() {
        return default_bookmarks();
    }
    let data = fs::read_to_string(&path).unwrap_or_default();
    let mut bms: Vec<Bookmark> = serde_json::from_str(&data)
        .unwrap_or_else(|_| default_bookmarks());

    // Eğer Trash yoksa listeye ekle
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    let trash_path = home.join(".local/share/Trash/files").to_string_lossy().to_string();
    if !bms.iter().any(|b| b.name.to_lowercase() == "trash" || b.path.contains(".local/share/Trash")) {
        let max_order = bms.iter().map(|b| b.order).max().unwrap_or(0);
        bms.push(Bookmark {
            name: "Trash".into(),
            path: trash_path,
            icon: "trash".into(),
            pinned: false,
            order: max_order + 1,
            color: String::new(),
        });
    }

    // Sıralamayı her zaman kullanıcının belirlediği sırada tut (sabitlenen yerinde kalsın)
    bms.sort_by(|a, b| a.order.cmp(&b.order));

    bms
}

fn default_bookmarks() -> Vec<Bookmark> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    vec![
        Bookmark {
            name: "Home".into(),
            path: home.to_string_lossy().to_string(),
            icon: "home".into(),
            pinned: true,
            order: 0,
            color: String::new(),
        },
        Bookmark {
            name: "Desktop".into(),
            path: home.join("Desktop").to_string_lossy().to_string(),
            icon: "desktop".into(),
            pinned: false,
            order: 1,
            color: String::new(),
        },
        Bookmark {
            name: "Downloads".into(),
            path: home.join("Downloads").to_string_lossy().to_string(),
            icon: "download".into(),
            pinned: false,
            order: 2,
            color: String::new(),
        },
        Bookmark {
            name: "Documents".into(),
            path: home.join("Documents").to_string_lossy().to_string(),
            icon: "document".into(),
            pinned: false,
            order: 3,
            color: String::new(),
        },
        Bookmark {
            name: "Pictures".into(),
            path: home.join("Pictures").to_string_lossy().to_string(),
            icon: "image".into(),
            pinned: false,
            order: 4,
            color: String::new(),
        },
        Bookmark {
            name: "Music".into(),
            path: home.join("Music").to_string_lossy().to_string(),
            icon: "music".into(),
            pinned: false,
            order: 5,
            color: String::new(),
        },
        Bookmark {
            name: "Videos".into(),
            path: home.join("Videos").to_string_lossy().to_string(),
            icon: "video".into(),
            pinned: false,
            order: 6,
            color: String::new(),
        },
        Bookmark {
            name: "Root".into(),
            path: "/".into(),
            icon: "server".into(),
            pinned: false,
            order: 7,
            color: String::new(),
        },
        Bookmark {
            name: "Media".into(),
            path: "/run/media".into(),
            icon: "hdd".into(),
            pinned: false,
            order: 8,
            color: String::new(),
        },
        Bookmark {
            name: "Trash".into(),
            path: home.join(".local/share/Trash/files").to_string_lossy().to_string(),
            icon: "trash".into(),
            pinned: false,
            order: 9,
            color: String::new(),
        },
    ]
}

/// Basit bookmark ekleme (eski API uyumu)
#[tauri::command]
pub fn add_bookmark(name: String, path: String) -> Result<(), String> {
    let order = get_next_bookmark_order();
    let bm = Bookmark {
        name,
        path,
        icon: "folder".to_string(),
        pinned: false,
        order,
        color: String::new(),
    };
    add_bookmark_full(bm)
}

/// Tam bookmark ekleme (icon, pinned, color ile)
#[tauri::command]
pub fn add_bookmark_full(bm: Bookmark) -> Result<(), String> {
    let mut bms = get_bookmarks();
    // Aynı path zaten varsa güncelle
    if let Some(existing) = bms.iter_mut().find(|b| b.path == bm.path) {
        *existing = bm;
    } else {
        bms.push(bm);
    }
    save_bookmarks(&bms)
}

/// Bookmark'ı güncelle (rename, renk, pin vs.)
#[tauri::command]
pub fn update_bookmark(path: String, name: Option<String>, icon: Option<String>, pinned: Option<bool>, color: Option<String>) -> Result<(), String> {
    let mut bms = get_bookmarks();
    let bm = bms.iter_mut()
        .find(|b| b.path == path)
        .ok_or_else(|| format!("Bookmark bulunamadı: {}", path))?;

    if let Some(n) = name   { bm.name   = n; }
    if let Some(i) = icon   { bm.icon   = i; }
    if let Some(p) = pinned { bm.pinned = p; }
    if let Some(c) = color  { bm.color  = c; }

    save_bookmarks(&bms)
}

/// Bookmark sırasını yeniden düzenle (drag-drop ile)
#[tauri::command]
pub fn reorder_bookmarks(ordered_paths: Vec<String>) -> Result<(), String> {
    let mut bms = get_bookmarks();
    for (i, path) in ordered_paths.iter().enumerate() {
        if let Some(bm) = bms.iter_mut().find(|b| &b.path == path) {
            bm.order = i as i32;
        }
    }
    save_bookmarks(&bms)
}

#[tauri::command]
pub fn remove_bookmark(path: String) -> Result<(), String> {
    let bm: Vec<Bookmark> = get_bookmarks()
        .into_iter()
        .filter(|b| b.path != path)
        .collect();
    save_bookmarks(&bm)
}

fn save_bookmarks(bm: &[Bookmark]) -> Result<(), String> {
    let path = bookmarks_path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let data = serde_json::to_string_pretty(bm).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

// ---- Internal helpers ----

fn resolve_name_conflict(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() { return dest; }

    let stem = Path::new(name).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let ext = Path::new(name).extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 2..=999 {
        let candidate = dir.join(format!("{} ({}){}", stem, i, ext));
        if !candidate.exists() { return candidate; }
    }

    dir.join(format!("{}_copy{}", stem, ext))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileProperties {
    pub name: String,
    pub path: String,
    pub parent_dir: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub size: u64,
    pub modified: String,
    pub created: String,
    pub accessed: String,
    pub permissions_str: String,
    pub permissions_octal: String,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub owner_name: String,
    pub group_name: String,
    pub mime_type: String,
    pub extension: String,
    pub file_count: Option<usize>,
    pub is_readonly: bool,
}

#[tauri::command]
pub fn get_file_properties(path: String) -> Result<FileProperties, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::ffi::CStr;

    let p = PathBuf::from(&path);
    if !p.exists() && !p.is_symlink() {
        return Err(format!("Dosya bulunamadı: {}", path));
    }

    let sym_meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    let is_symlink = sym_meta.file_type().is_symlink();
    let symlink_target = if is_symlink {
        fs::read_link(&p).ok().map(|l| l.to_string_lossy().to_string())
    } else {
        None
    };

    let meta = if is_symlink {
        fs::metadata(&p).unwrap_or(sym_meta.clone())
    } else {
        sym_meta.clone()
    };

    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
    let parent_dir = p.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let is_dir = meta.is_dir();

    let size = if is_dir { 0 } else { meta.len() };

    let modified = meta.modified().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Bilinmiyor".into());

    let created = meta.created().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Bilinmiyor".into());

    let accessed = meta.accessed().ok()
        .map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Bilinmiyor".into());

    let mode = meta.permissions().mode();
    let permissions_octal = format!("{:04o}", mode & 0o7777);
    let permissions_str = format_permissions(&meta);

    let uid = meta.uid();
    let gid = meta.gid();

    let owner_name = unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            CStr::from_ptr((*pw).pw_name).to_string_lossy().to_string()
        } else {
            uid.to_string()
        }
    };

    let group_name = unsafe {
        let gr = libc::getgrgid(gid);
        if !gr.is_null() && !(*gr).gr_name.is_null() {
            CStr::from_ptr((*gr).gr_name).to_string_lossy().to_string()
        } else {
            gid.to_string()
        }
    };

    let extension = p.extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let mime_type = if is_dir {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(&p).first_or_octet_stream().to_string()
    };

    let file_count = if is_dir {
        fs::read_dir(&p).ok().map(|rd| rd.count())
    } else {
        None
    };

    let is_readonly = meta.permissions().readonly();

    Ok(FileProperties {
        name,
        path,
        parent_dir,
        is_dir,
        is_symlink,
        symlink_target,
        size,
        modified,
        created,
        accessed,
        permissions_str,
        permissions_octal,
        owner_uid: uid,
        owner_gid: gid,
        owner_name,
        group_name,
        mime_type,
        extension,
        file_count,
        is_readonly,
    })
}

#[tauri::command]
pub fn set_file_permissions(path: String, mode: u32, recursive: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Dosya bulunamadı: {}", path));
    }

    if recursive && p.is_dir() {
        let out = Command::new("chmod")
            .args(["-R", &format!("{:o}", mode), &path])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(err.trim().to_string());
        }
    } else {
        fs::set_permissions(p, fs::Permissions::from_mode(mode))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn sudo_set_file_permissions(path: String, mode: u32, recursive: bool, password: String) -> Result<(), String> {
    let rec_flag = if recursive { "-R " } else { "" };
    let cmd = format!("chmod {}{:o} '{}'", rec_flag, mode, path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[tauri::command]
pub fn set_file_owner(path: String, owner: String, group: String, recursive: bool) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Dosya bulunamadı: {}", path));
    }
    let target = if group.is_empty() { owner } else { format!("{}:{}", owner, group) };
    let mut cmd = Command::new("chown");
    if recursive {
        cmd.arg("-R");
    }
    cmd.arg(&target).arg(&path);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn sudo_set_file_owner(path: String, owner: String, group: String, recursive: bool, password: String) -> Result<(), String> {
    let rec_flag = if recursive { "-R " } else { "" };
    let target = if group.is_empty() { owner } else { format!("{}:{}", owner, group) };
    let cmd = format!("chown {}{} '{}'", rec_flag, target, path.replace('\'', "'\\''"));
    run_sudo_sh(&cmd, &password)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemAccounts {
    pub users: Vec<String>,
    pub groups: Vec<String>,
}

#[tauri::command]
pub fn get_system_users_and_groups() -> Result<SystemAccounts, String> {
    let mut users = Vec::new();
    let mut groups = Vec::new();

    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                let name = parts[0].trim();
                let uid: u32 = parts[2].parse().unwrap_or(0);
                if (!name.starts_with('_') && uid >= 1000) || name == "root" || name == "nobody" {
                    users.push(name.to_string());
                }
            }
        }
    }
    users.sort();
    users.dedup();

    if let Ok(content) = fs::read_to_string("/etc/group") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if !parts.is_empty() {
                let gname = parts[0].trim();
                if !gname.is_empty() && !gname.starts_with('_') {
                    groups.push(gname.to_string());
                }
            }
        }
    }
    groups.sort();
    groups.dedup();

    Ok(SystemAccounts { users, groups })
}

#[tauri::command]
pub fn debug_log(msg: String) {
    eprintln!("[JS_DEBUG] {}", msg);
}
