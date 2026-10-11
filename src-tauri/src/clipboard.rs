/// clipboard.rs — Dosya kopyala/kes/yapıştır + çoklu batch Undo stack + çakışma ve üzerine yazma koruması
/// Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Z, Delete hepsini buradan yönetiyoruz
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use anyhow::Result;

// ---- Global state (Tauri app'te tek instance) ----

/// Clipboard: neyi kopyaladın/kestin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardState {
    pub op: ClipboardOp,
    pub paths: Vec<String>,
}

/// Tekil bir dosya/klasör işleminin geri alma kaydı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleUndoItem {
    pub op_type: String, // "move", "copy", "rename", "create", "delete"
    pub src: Option<String>,
    pub dst: Option<String>,
    /// Eğer işlem sırasında hedef dosyanın üzerine yazıldıysa,
    /// eski hedef dosya silinmeyip buraya yedeklenir. Geri alınınca yerine konur!
    pub overwritten_backup: Option<String>,
    pub was_dir: bool,
}

/// Kullanıcının tek bir hamlede (örneğin 40 dosya birden taşındığında) yaptığı tüm alt işlemleri
/// içeren tek bir mantıksal Undo adımı. Ctrl+Z'ye basınca bu paketin hepsi birden geri alınır!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoBatch {
    pub description: String,
    pub items: Vec<SingleUndoItem>,
}

// Thread-safe global state
static CLIPBOARD: Mutex<Option<ClipboardState>> = Mutex::new(None);
static UNDO_STACK: Mutex<Vec<UndoBatch>> = Mutex::new(Vec::new());

const MAX_UNDO: usize = 50;

/// Yedeklenen dosyaların güvenle saklandığı dizin (~/.local/share/arch-explorer/undo_backups)
pub fn undo_backups_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("undo_backups")
}

/// Üzerine yazılacak dosyayı yok etmek yerine güvenli yedek dizinine taşır
pub fn stash_overwritten_item(dst: &Path) -> Option<String> {
    if !dst.exists() {
        return None;
    }
    let backup_dir = undo_backups_dir();
    let _ = fs::create_dir_all(&backup_dir);

    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let orig_name = dst
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "backup".to_string());
    let backup_filename = format!("{:016x}_{}", now, orig_name);
    let backup_path = backup_dir.join(backup_filename);

    if fs::rename(dst, &backup_path).is_ok() {
        Some(backup_path.to_string_lossy().to_string())
    } else if copy_recursive(dst, &backup_path).is_ok() {
        if dst.is_dir() {
            let _ = fs::remove_dir_all(dst);
        } else {
            let _ = fs::remove_file(dst);
        }
        Some(backup_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// 50 sınırı aşıldığında eski undo paketindeki yedek dosyaları diskten temizle
fn cleanup_pruned_batch(batch: &UndoBatch) {
    for item in &batch.items {
        if let Some(ref backup_path) = item.overwritten_backup {
            let p = Path::new(backup_path);
            if p.is_dir() {
                let _ = fs::remove_dir_all(p);
            } else if p.exists() {
                let _ = fs::remove_file(p);
            }
        }
    }
}

// ---- Çakışma Kontrolü & Batch İşlemleri ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictItem {
    pub src: String,
    pub dst: String,
    pub name: String,
    pub is_dir: bool,
    pub src_size: u64,
    pub dst_size: u64,
    pub src_modified: String,
    pub dst_modified: String,
}

fn collect_conflicts_recursive(src: &Path, dst: &Path, conflicts: &mut Vec<ConflictItem>) {
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.filter_map(|e| e.ok()) {
            let child_src = entry.path();
            let name = child_src.file_name().unwrap_or_default().to_string_lossy().to_string();
            let child_dst = dst.join(&name);
            if child_dst.exists() {
                let src_meta = fs::symlink_metadata(&child_src).ok();
                let dst_meta = fs::symlink_metadata(&child_dst).ok();

                let is_dir = child_dst.is_dir();
                let src_size = src_meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let dst_size = dst_meta.as_ref().map(|m| m.len()).unwrap_or(0);

                let src_modified = src_meta
                    .and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
                    .unwrap_or_default();
                let dst_modified = dst_meta
                    .and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
                    .unwrap_or_default();

                conflicts.push(ConflictItem {
                    src: child_src.to_string_lossy().to_string(),
                    dst: child_dst.to_string_lossy().to_string(),
                    name,
                    is_dir,
                    src_size,
                    dst_size,
                    src_modified,
                    dst_modified,
                });

                if is_dir && child_src.is_dir() {
                    collect_conflicts_recursive(&child_src, &child_dst, conflicts);
                }
            }
        }
    }
}

/// Taşıma veya yapıştırma öncesi hedefte çakışan (aynı adlı) dosyaları listeler
#[tauri::command]
pub fn check_file_conflicts(sources: Vec<String>, dest_dir: String) -> Result<Vec<ConflictItem>, String> {
    if dest_dir.starts_with("adb://") {
        // ADB hedefinde çakışma kontrolü
        let mut conflicts = vec![];
        if let Ok(dir_listing) = crate::adb::adb_list_directory(&dest_dir, true) {
            let existing_names: HashMap<String, crate::fs_ops::FileEntry> = dir_listing.entries.into_iter().map(|e| (e.name.clone(), e)).collect();
            for src_path in sources {
                let name = if src_path.starts_with("adb://") {
                    Path::new(&src_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                } else {
                    PathBuf::from(&src_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                };
                if let Some(target) = existing_names.get(&name) {
                    let (src_size, src_mod, is_dir) = if src_path.starts_with("adb://") {
                        if let Ok(info) = crate::adb::adb_get_file_entry(&src_path) {
                            (info.size, info.modified, info.is_dir)
                        } else {
                            (0, "-".into(), false)
                        }
                    } else {
                        let m = fs::metadata(&src_path).ok();
                        let sz = m.as_ref().map(|meta| meta.len()).unwrap_or(0);
                        let mod_t = m.and_then(|meta| meta.modified().ok())
                            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
                            .unwrap_or_default();
                        (sz, mod_t, PathBuf::from(&src_path).is_dir())
                    };
                    conflicts.push(ConflictItem {
                        src: src_path,
                        dst: target.path.clone(),
                        name,
                        is_dir,
                        src_size,
                        dst_size: target.size,
                        src_modified: src_mod,
                        dst_modified: target.modified.clone(),
                    });
                }
            }
        }
        return Ok(conflicts);
    }

    let dest = PathBuf::from(&dest_dir);
    let mut conflicts = vec![];

    for src_path in sources {
        let name = if src_path.starts_with("adb://") {
            Path::new(&src_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        } else {
            match PathBuf::from(&src_path).file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            }
        };
        let src = PathBuf::from(&src_path);
        let dst = dest.join(&name);
        if dst.exists() {
            let src_meta = fs::symlink_metadata(&src).ok();
            let dst_meta = fs::symlink_metadata(&dst).ok();

            let is_dir = dst.is_dir();
            let src_size = src_meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let dst_size = dst_meta.as_ref().map(|m| m.len()).unwrap_or(0);

            let src_modified = src_meta
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default();
            let dst_modified = dst_meta
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default();

            conflicts.push(ConflictItem {
                src: src_path,
                dst: dst.to_string_lossy().to_string(),
                name,
                is_dir,
                src_size,
                dst_size,
                src_modified,
                dst_modified,
            });

            // Eğer her ikisi de klasör ise, içindeki alt dosyaları da tara
            if is_dir && src.is_dir() {
                collect_conflicts_recursive(&src, &dst, &mut conflicts);
            }
        }
    }

    Ok(conflicts)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemPlan {
    pub src: String,
    pub dst: String,
    /// "replace" | "rename" | "skip"
    pub conflict_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExecuteRequest {
    /// "move" | "copy"
    pub op: String,
    pub items: Vec<BatchItemPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExecuteResult {
    pub success_count: usize,
    pub skipped_count: usize,
    pub failed_items: Vec<(String, String)>,
    #[serde(default)]
    pub created_paths: Vec<String>,
}

/// Tüm sürükle-bırak, yapıştır ve taşıma işlemlerini yürüten ve TEK BİR UNDO BATCH'i oluşturan ana komut
#[tauri::command]
pub fn execute_file_batch(req: BatchExecuteRequest) -> Result<BatchExecuteResult, String> {
    let is_move = req.op == "move";
    let mut success_count = 0;
    let mut skipped_count = 0;
    let mut failed_items = vec![];
    let mut undo_items = vec![];
    let mut created_paths = vec![];

    for item in req.items {
        // ADB Transfer Handling (Phone <-> PC or Phone <-> Phone)
        if item.src.starts_with("adb://") || item.dst.starts_with("adb://") {
            if item.conflict_action == "skip" {
                skipped_count += 1;
                continue;
            }

            let res = if item.src.starts_with("adb://") && item.dst.starts_with("adb://") {
                if is_move {
                    crate::adb::adb_rename_item(&item.src, &item.dst)
                } else {
                    match crate::adb::adb_cache_file(&item.src) {
                        Ok(tmp) => crate::adb::adb_push(tmp.to_str().unwrap_or(""), &item.dst),
                        Err(e) => Err(e),
                    }
                }
            } else if item.src.starts_with("adb://") {
                // ADB -> PC (Pull)
                match crate::adb::adb_pull(&item.src, &item.dst) {
                    Ok(_) => {
                        if is_move {
                            let _ = crate::adb::adb_delete_item(&item.src);
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            } else {
                // PC -> ADB (Push)
                match crate::adb::adb_push(&item.src, &item.dst) {
                    Ok(_) => {
                        if is_move {
                            let p = PathBuf::from(&item.src);
                            if p.is_dir() { let _ = fs::remove_dir_all(&p); }
                            else { let _ = fs::remove_file(&p); }
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            };

            match res {
                Ok(_) => {
                    success_count += 1;
                    created_paths.push(item.dst.clone());
                }
                Err(e) => {
                    failed_items.push((item.src.clone(), e));
                }
            }
            continue;
        }

        let src = PathBuf::from(&item.src);
        if !src.exists() {
            // Linux'ta kısıtlı üst dizinlerde (ör. başka kullanıcının /home'u 0700) stat PermissionDenied döner ve exists() false verir.
            let is_permission_err = match fs::symlink_metadata(&src) {
                Err(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied => true,
                _ => false,
            };
            if is_permission_err {
                failed_items.push((item.src.clone(), "Permission denied (Yetki gerekli)".into()));
            } else {
                failed_items.push((item.src.clone(), "Kaynak dosya bulunamadı".into()));
            }
            continue;
        }

        if item.conflict_action == "skip" {
            skipped_count += 1;
            continue;
        }

        let orig_dst = PathBuf::from(&item.dst);
        let dst_parent = orig_dst.parent().unwrap_or_else(|| Path::new("/"));
        let filename = match orig_dst.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => {
                failed_items.push((item.src.clone(), "Geçersiz hedef yolu".into()));
                continue;
            }
        };

        // Eğer kullanıcı rename seçtiyse veya varsayılan kopya isteniyorsa yeni isim ver: "dosya (2).ext"
        let final_dst = if item.conflict_action == "rename" {
            resolve_conflict(dst_parent, &filename)
        } else if !is_move && src == orig_dst && item.conflict_action != "replace" {
            resolve_conflict(dst_parent, &filename)
        } else {
            orig_dst
        };

        // Eğer kaynak dosya hedef dosyanın tamamen aynısıysa (kendi üzerine yazma/taşıma) işlem yapmadan tamamla
        if src == final_dst {
            success_count += 1;
            continue;
        }

        // Eğer üzerine yazılıyorsa ve hedef kaynak dosyanın kendisi değilse güvenle yedekle
        let mut overwritten_backup = None;
        if final_dst.exists() && final_dst != src && item.conflict_action != "rename" {
            overwritten_backup = stash_overwritten_item(&final_dst);
        }

        let was_dir = src.is_dir();
        let res = if is_move {
            move_item_inner(&src, &final_dst)
        } else {
            copy_recursive(&src, &final_dst)
        };

        match res {
            Ok(_) => {
                success_count += 1;
                let final_dst_str = final_dst.to_string_lossy().to_string();
                created_paths.push(final_dst_str.clone());
                undo_items.push(SingleUndoItem {
                    op_type: if is_move { "move".into() } else { "copy".into() },
                    src: Some(item.src.clone()),
                    dst: Some(final_dst_str),
                    overwritten_backup,
                    was_dir,
                });
            }
            Err(e) => {
                // Eğer başarısız olduysa ve yedek almışsak yedeği geri getir
                if let Some(ref b) = overwritten_backup {
                    let _ = move_item_inner(Path::new(b), &final_dst);
                }
                failed_items.push((item.src.clone(), e.to_string()));
            }
        }
    }

    if !undo_items.is_empty() {
        let desc = format!(
            "{} ({} öğe)",
            if is_move { "Taşındı" } else { "Kopyalandı" },
            success_count
        );
        let _ = push_undo_batch(UndoBatch {
            description: desc,
            items: undo_items,
        });
    }

    Ok(BatchExecuteResult {
        success_count,
        skipped_count,
        failed_items,
        created_paths,
    })
}

// ---- Çoklu Silme (Toplu Undo Kayıtlı) ----

#[tauri::command]
pub fn batch_delete(paths: Vec<String>, permanent: bool) -> Result<BatchExecuteResult, String> {
    let mut success_count = 0;
    let mut failed_items = vec![];
    let mut undo_items = vec![];

    for path in paths {
        if path.starts_with("adb://") {
            match crate::adb::adb_delete_item(&path) {
                Ok(_) => { success_count += 1; }
                Err(e) => { failed_items.push((path, e)); }
            }
            continue;
        }

        let p = PathBuf::from(&path);
        let was_dir = p.is_dir();
        let res = if permanent {
            if was_dir {
                fs::remove_dir_all(&p).map_err(|e| e.to_string())
            } else {
                fs::remove_file(&p).map_err(|e| e.to_string())
            }
        } else {
            trash::delete(&path).map_err(|e| e.to_string())
        };

        match res {
            Ok(_) => {
                success_count += 1;
                undo_items.push(SingleUndoItem {
                    op_type: "delete".into(),
                    src: Some(path),
                    dst: None,
                    overwritten_backup: None,
                    was_dir,
                });
            }
            Err(e) => {
                failed_items.push((path, e));
            }
        }
    }

    if !undo_items.is_empty() {
        let _ = push_undo_batch(UndoBatch {
            description: format!("Silindi ({} öğe)", success_count),
            items: undo_items,
        });
    }

    Ok(BatchExecuteResult {
        success_count,
        skipped_count: 0,
        failed_items,
        created_paths: vec![],
    })
}

// ---- System OS Clipboard Synchronization (Wayland / X11 / GTK) ----

fn path_to_file_uri(p: &str) -> String {
    let clean = p.replace('\\', "/");
    let mut out = String::from("file://");
    for b in clean.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().unwrap_or(b'0');
            let h2 = chars.next().unwrap_or(b'0');
            let hex_str = [h1, h2];
            if let Ok(hex_s) = std::str::from_utf8(&hex_str) {
                if let Ok(val) = u8::from_str_radix(hex_s, 16) {
                    bytes.push(val);
                    continue;
                }
            }
            bytes.push(b);
            bytes.push(h1);
            bytes.push(h2);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn parse_uri_list(raw: &str) -> Vec<String> {
    let mut out = vec![];
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("file://") {
            let decoded = percent_decode(rest);
            if Path::new(&decoded).exists() {
                out.push(decoded);
            }
        }
    }
    out
}

fn read_system_clipboard_paths() -> Vec<String> {
    // 1. Wayland wl-paste
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(out) = std::process::Command::new("wl-paste")
            .args(["-t", "text/uri-list"])
            .output()
        {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    let paths = parse_uri_list(&s);
                    if !paths.is_empty() {
                        return paths;
                    }
                }
            }
        }
    }

    // 2. X11 xclip
    if std::env::var("DISPLAY").is_ok() {
        if let Ok(out) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "text/uri-list", "-o"])
            .output()
        {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    let paths = parse_uri_list(&s);
                    if !paths.is_empty() {
                        return paths;
                    }
                }
            }
        }
    }

    vec![]
}

pub fn sync_to_system_clipboard(paths: &[String], is_cut: bool) {
    if paths.is_empty() {
        return;
    }

    let mut uris = Vec::with_capacity(paths.len());
    for p in paths {
        uris.push(path_to_file_uri(p));
    }

    let uri_list = format!("{}\r\n", uris.join("\r\n"));
    let gnome_copied = format!(
        "{}\n{}",
        if is_cut { "cut" } else { "copy" },
        uris.join("\n")
    );
    let plain_text = paths.join("\n");

    // 1. GTK Clipboard: register multiple targets (text/uri-list, x-special/gnome-copied-files, text/plain)
    let uri_list_gtk = uri_list.clone();
    let gnome_gtk = gnome_copied.clone();
    let plain_gtk = plain_text.clone();

    let _ = gtk::glib::MainContext::default().invoke(move || {
        let cb = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        let targets = vec![
            gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::OTHER_APP, 0),
            gtk::TargetEntry::new("x-special/gnome-copied-files", gtk::TargetFlags::OTHER_APP, 1),
            gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::OTHER_APP, 2),
            gtk::TargetEntry::new("text/plain", gtk::TargetFlags::OTHER_APP, 3),
        ];
        let u = uri_list_gtk.clone();
        let g = gnome_gtk.clone();
        let p = plain_gtk.clone();
        cb.set_with_data(&targets, move |_cb, selection_data, info| {
            match info {
                0 => {
                    selection_data.set(&gdk::Atom::intern("text/uri-list"), 8, u.as_bytes());
                }
                1 => {
                    selection_data.set(&gdk::Atom::intern("x-special/gnome-copied-files"), 8, g.as_bytes());
                }
                2 | 3 => {
                    selection_data.set_text(&p);
                }
                _ => {}
            }
        });
    });

    // 2. Wayland / X11 process clipboard helpers
    let uri_list_bg = uri_list;
    std::thread::spawn(move || {
        use std::io::Write;
        use std::process::{Command, Stdio};

        // Wayland wl-copy
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Ok(mut child) = Command::new("wl-copy")
                .arg("-t")
                .arg("text/uri-list")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(uri_list_bg.as_bytes());
                }
            }
        }

        // X11 xclip
        if std::env::var("DISPLAY").is_ok() {
            if let Ok(mut child) = Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "text/uri-list"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(uri_list_bg.as_bytes());
                }
            }
        }
    });
}

// ---- Clipboard commands ----

/// Seçili dosyaları clipboard'a kopyala (Ctrl+C)
#[tauri::command]
pub fn clipboard_copy(paths: Vec<String>) -> Result<(), String> {
    sync_to_system_clipboard(&paths, false);
    let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
    *cb = Some(ClipboardState {
        op: ClipboardOp::Copy,
        paths,
    });
    Ok(())
}

/// Seçili dosyaları clipboard'a kes (Ctrl+X)
#[tauri::command]
pub fn clipboard_cut(paths: Vec<String>) -> Result<(), String> {
    sync_to_system_clipboard(&paths, true);
    let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
    *cb = Some(ClipboardState {
        op: ClipboardOp::Cut,
        paths,
    });
    Ok(())
}

/// Clipboard içeriğini görüntüle
#[tauri::command]
pub fn clipboard_get() -> Option<ClipboardState> {
    CLIPBOARD.lock().ok()?.clone()
}

/// Clipboard'ı temizle
#[tauri::command]
pub fn clipboard_clear() -> Result<(), String> {
    let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
    *cb = None;
    Ok(())
}

/// Clipboard'daki dosyaları hedef dizine yapıştır (Ctrl+V)
/// Çakışma kontrolü yapılmamışsa doğrudan resolve_conflict ile yapıştırır ve tek bir UndoBatch üretir
#[tauri::command]
pub fn clipboard_paste(dest_dir: String) -> Result<Vec<String>, String> {
    let state = {
        let cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        if let Some(ref s) = *cb {
            if !s.paths.is_empty() {
                s.clone()
            } else {
                let ext_paths = read_system_clipboard_paths();
                if !ext_paths.is_empty() {
                    ClipboardState {
                        op: ClipboardOp::Copy,
                        paths: ext_paths,
                    }
                } else {
                    return Err("Clipboard boş".to_string());
                }
            }
        } else {
            let ext_paths = read_system_clipboard_paths();
            if !ext_paths.is_empty() {
                ClipboardState {
                    op: ClipboardOp::Copy,
                    paths: ext_paths,
                }
            } else {
                return Err("Clipboard boş".to_string());
            }
        }
    };

    let dest = PathBuf::from(&dest_dir);
    if !dest.is_dir() {
        return Err(format!("Hedef dizin bulunamadı: {}", dest_dir));
    }

    let mut plans = vec![];
    for src_path in &state.paths {
        let src = PathBuf::from(src_path);
        if !src.exists() {
            continue;
        }
        let file_name = match src.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let dst_path = dest.join(&file_name).to_string_lossy().to_string();
        plans.push(BatchItemPlan {
            src: src_path.clone(),
            dst: dst_path,
            conflict_action: "rename".into(), // Varsayılan otomatik koruma
        });
    }

    let op = if state.op == ClipboardOp::Cut { "move" } else { "copy" };
    let res = execute_file_batch(BatchExecuteRequest {
        op: op.into(),
        items: plans,
    })?;

    if state.op == ClipboardOp::Cut {
        let mut cb = CLIPBOARD.lock().map_err(|e| e.to_string())?;
        *cb = None;
    }

    Ok(vec![format!("{} dosya başarıyla yapıştırıldı", res.success_count)])
}

// ---- Undo ----

/// Son işlemi geri al (Ctrl+Z) — İster 1 dosya olsun ister 40 dosya, tek hamlede geri alır!
/// Eğer bir dosyanın üzerine yazılmışsa, yedekten çıkarıp eski yerine koyar!
#[tauri::command]
pub fn undo_last() -> Result<String, String> {
    let batch = {
        let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
        stack.pop().ok_or_else(|| "Geri alınacak işlem yok".to_string())?
    };

    let mut restored_count = 0;
    // Ters sıra ile geri alma: en son yapılan işlem ilk geri alınır
    for item in batch.items.into_iter().rev() {
        match item.op_type.as_str() {
            "move" => {
                if let (Some(ref src), Some(ref dst)) = (&item.src, &item.dst) {
                    let d = PathBuf::from(dst);
                    let s = PathBuf::from(src);
                    // Taşınan dosyayı eski yerine geri taşı
                    if d.exists() {
                        let _ = move_item_inner(&d, &s);
                    }
                    // Üzerine yazılan eski hedef varsa yedekten geri getir!
                    if let Some(ref backup) = item.overwritten_backup {
                        let b = PathBuf::from(backup);
                        if b.exists() {
                            let _ = move_item_inner(&b, &d);
                        }
                    }
                    restored_count += 1;
                }
            }
            "copy" => {
                if let Some(ref dst) = item.dst {
                    let d = PathBuf::from(dst);
                    // Kopyalanan dosyayı/klasörü hedeften kaldır
                    if d.is_dir() {
                        let _ = fs::remove_dir_all(&d);
                    } else if d.exists() {
                        let _ = fs::remove_file(&d);
                    }
                    // Üzerine yazılan eski hedef varsa yedekten geri getir!
                    if let Some(ref backup) = item.overwritten_backup {
                        let b = PathBuf::from(backup);
                        if b.exists() {
                            let _ = move_item_inner(&b, &d);
                        }
                    }
                    restored_count += 1;
                }
            }
            "rename" => {
                if let (Some(ref src), Some(ref dst)) = (&item.src, &item.dst) {
                    let _ = fs::rename(dst, src);
                    restored_count += 1;
                }
            }
            "create" => {
                if let Some(ref path) = item.dst {
                    let p = PathBuf::from(path);
                    if p.is_dir() {
                        let _ = fs::remove_dir_all(&p);
                    } else if p.exists() {
                        let _ = fs::remove_file(&p);
                    }
                    restored_count += 1;
                }
            }
            "delete" => {
                // trash-restore kuruluysa geri getirmeyi dene
                let _ = std::process::Command::new("trash-restore")
                    .stdin(std::process::Stdio::null())
                    .output();
                restored_count += 1;
            }
            _ => {}
        }
    }

    Ok(format!("{} ({} öğe geri alındı)", batch.description, restored_count))
}

/// Undo stack'i görüntüle
#[tauri::command]
pub fn get_undo_stack() -> Vec<String> {
    let stack = UNDO_STACK.lock().unwrap_or_else(|e| e.into_inner());
    stack.iter().map(|b| format!("{}: {} alt öğe", b.description, b.items.len())).collect()
}

/// Undo stack'i temizle ve diskteki tüm yedek dosyaları sil
#[tauri::command]
pub fn clear_undo_stack() -> Result<(), String> {
    let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
    for batch in stack.iter() {
        cleanup_pruned_batch(batch);
    }
    stack.clear();
    let dir = undo_backups_dir();
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
    }
    Ok(())
}

// ---- Tracked file operations (tekil operasyonlar) ----

#[tauri::command]
pub fn tracked_rename(old_path: String, new_path: String) -> Result<(), String> {
    if old_path.starts_with("adb://") {
        return crate::adb::adb_rename_item(&old_path, &new_path);
    }
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())?;
    push_undo_batch(UndoBatch {
        description: format!("Yeniden adlandırıldı: {}", new_path),
        items: vec![SingleUndoItem {
            op_type: "rename".into(),
            src: Some(old_path),
            dst: Some(new_path),
            overwritten_backup: None,
            was_dir: false,
        }],
    })?;
    Ok(())
}

#[tauri::command]
pub fn tracked_create_file(path: String) -> Result<(), String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_create_file(&path);
    }
    fs::File::create(&path).map_err(|e| e.to_string())?;
    push_undo_batch(UndoBatch {
        description: format!("Dosya oluşturuldu: {}", path),
        items: vec![SingleUndoItem {
            op_type: "create".into(),
            src: None,
            dst: Some(path),
            overwritten_backup: None,
            was_dir: false,
        }],
    })?;
    Ok(())
}

#[tauri::command]
pub fn tracked_create_dir(path: String) -> Result<(), String> {
    if path.starts_with("adb://") {
        return crate::adb::adb_create_dir(&path);
    }
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    push_undo_batch(UndoBatch {
        description: format!("Klasör oluşturuldu: {}", path),
        items: vec![SingleUndoItem {
            op_type: "create".into(),
            src: None,
            dst: Some(path),
            overwritten_backup: None,
            was_dir: true,
        }],
    })?;
    Ok(())
}

#[tauri::command]
pub fn tracked_delete(path: String, permanent: bool) -> Result<(), String> {
    batch_delete(vec![path], permanent).map(|_| ())
}

// ---- Internal helpers ----

pub fn push_undo_batch(batch: UndoBatch) -> Result<(), String> {
    let mut stack = UNDO_STACK.lock().map_err(|e| e.to_string())?;
    while stack.len() >= MAX_UNDO {
        let pruned = stack.remove(0);
        cleanup_pruned_batch(&pruned);
    }
    stack.push(batch);
    Ok(())
}

/// Çakışma varsa otomatik "(2)", "(3)" ekle
pub fn resolve_conflict(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }

    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 2..=999 {
        let new_name = format!("{} ({}){}", stem, i, ext);
        let candidate = dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    dir.join(format!("{}_copy{}", stem, ext))
}

pub fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let dst_path = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &dst_path)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

pub fn move_item_inner(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // Aynı filesystem: rename yeterli
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Cross-device: kopyala + sil
    copy_recursive(src, dst)?;
    if src.is_dir() {
        fs::remove_dir_all(src)?;
    } else {
        fs::remove_file(src)?;
    }
    Ok(())
}
