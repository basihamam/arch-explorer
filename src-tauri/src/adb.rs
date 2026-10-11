// adb.rs — Android Debug Bridge (ADB) device and file system integration
// Supports detecting connected devices, browsing /sdcard, pushing/pulling files,
// creating folders, deleting, renaming, and reading files.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::{Path, PathBuf};
use crate::fs_ops::{FileEntry, DirectoryListing};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdbDeviceInfo {
    pub serial: String,
    pub model: String,
    pub brand: String,
    pub name: String,
    pub state: String,
    pub total_size: String,
    pub used_size: String,
    pub avail_size: String,
    pub mount_point: String,
}

/// Parse `adb devices -l` and fetch storage + device properties
pub fn list_connected_adb_devices() -> Vec<AdbDeviceInfo> {
    let output = match Command::new("adb").args(["devices", "-l"]).output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() {
        return vec![];
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("List of devices") {
            continue;
        }

        // Example: R6CY901HCTL device usb:5-1 product:a26xtur model:SM_A266B device:a26x transport_id:1
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let serial = parts[0].to_string();
        let state = parts[1].to_string();

        if state != "device" {
            // unauthorized or offline device
            let model = parts.iter().find(|p| p.starts_with("model:")).map(|p| p.trim_start_matches("model:").replace('_', " ")).unwrap_or_else(|| serial.clone());
            devices.push(AdbDeviceInfo {
                serial: serial.clone(),
                model: model.clone(),
                brand: "Android".into(),
                name: format!("Android ({})", state),
                state,
                total_size: "?".into(),
                used_size: "?".into(),
                avail_size: "?".into(),
                mount_point: format!("adb://{}/sdcard", serial),
            });
            continue;
        }

        // Query model and brand directly
        let mut model = parts.iter().find(|p| p.starts_with("model:")).map(|p| p.trim_start_matches("model:").replace('_', " ")).unwrap_or_default();
        let mut brand = String::new();

        if let Ok(prop_out) = Command::new("adb").args(["-s", &serial, "shell", "getprop ro.product.model; getprop ro.product.brand"]).output() {
            if prop_out.status.success() {
                let prop_str = String::from_utf8_lossy(&prop_out.stdout);
                let lines: Vec<&str> = prop_str.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
                if let Some(m) = lines.get(0) {
                    if !m.is_empty() { model = m.to_string(); }
                }
                if let Some(b) = lines.get(1) {
                    if !b.is_empty() { brand = b.to_string(); }
                }
            }
        }

        if model.is_empty() {
            model = "Android Device".to_string();
        }

        let name = if !brand.is_empty() {
            let brand_cap = brand.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default() + &brand[1..];
            format!("{} {}", brand_cap, model)
        } else {
            model.clone()
        };

        // Query df -h /sdcard
        let mut total_size = "?".to_string();
        let mut used_size = "?".to_string();
        let mut avail_size = "?".to_string();

        if let Ok(df_out) = Command::new("adb").args(["-s", &serial, "shell", "df -h /sdcard"]).output() {
            if df_out.status.success() {
                let df_str = String::from_utf8_lossy(&df_out.stdout);
                // Header + row
                // Filesystem Size Used Avail Use% Mounted on
                // /dev/fuse   224G  98G  126G  44% /storage/emulated
                for df_line in df_str.lines().skip(1) {
                    let cols: Vec<&str> = df_line.split_whitespace().collect();
                    if cols.len() >= 4 {
                        total_size = cols[1].to_string();
                        used_size = cols[2].to_string();
                        avail_size = cols[3].to_string();
                        break;
                    }
                }
            }
        }

        devices.push(AdbDeviceInfo {
            serial: serial.clone(),
            model,
            brand,
            name,
            state,
            total_size,
            used_size,
            avail_size,
            mount_point: format!("adb://{}/sdcard", serial),
        });
    }

    devices
}

/// Convert AdbDeviceInfo into BlockDevice for seamless integration with sidebar and device views
pub fn get_adb_as_block_devices() -> Vec<crate::mounts::BlockDevice> {
    let adb_devices = list_connected_adb_devices();
    adb_devices.into_iter().map(|dev| {
        let size_display = if dev.total_size != "?" {
            format!("{} ({} boş)", dev.total_size, dev.avail_size)
        } else {
            "?".into()
        };
        crate::mounts::BlockDevice {
            name: dev.name.clone(),
            device: format!("adb:{}", dev.serial),
            size: size_display,
            fs_type: "adb".into(),
            label: dev.name,
            mount_point: Some(dev.mount_point),
            is_mounted: dev.state == "device",
            is_windows: false,
            uuid: dev.serial,
        }
    }).collect()
}

/// Parse an `adb://<serial>/<remote_path>` URI
pub fn parse_adb_path(path: &str) -> Option<(String, String)> {
    if !path.starts_with("adb://") {
        return None;
    }
    let rest = &path["adb://".len()..];
    let mut parts = rest.splitn(2, '/');
    let serial = parts.next()?.trim().to_string();
    if serial.is_empty() {
        return None;
    }
    let remote_path = match parts.next() {
        Some(p) => {
            if p.is_empty() || p == "/" {
                "/sdcard".to_string()
            } else if p.starts_with('/') {
                p.to_string()
            } else {
                format!("/{}", p)
            }
        }
        None => "/sdcard".to_string(),
    };
    Some((serial, remote_path))
}

/// List directory entries on the Android device via adb shell stat
pub fn adb_list_directory(path: &str, show_hidden: bool) -> Result<DirectoryListing, String> {
    let (serial, remote_path) = parse_adb_path(path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", path))?;

    // Run fast single stat command inside target directory with fallback to prevent glob errors
    let escaped_path = remote_path.replace('\'', "'\\''");
    let cmd = format!(
        "cd '{}' && {{ stat -c '%s|%Y|%F|%A|%n' * 2>/dev/null; stat -c '%s|%Y|%F|%A|%n' .* 2>/dev/null; true; }}",
        escaped_path
    );

    let output = Command::new("adb")
        .args(["-s", &serial, "shell", &cmd])
        .output()
        .map_err(|e| format!("ADB çalıştırılamadı: {}", e))?;

    let out_str = String::from_utf8_lossy(&output.stdout);
    let err_str = String::from_utf8_lossy(&output.stderr);

    // Sadece çıkış başarısız VE hiçbir dosya listelenmemişse hata ver
    if !output.status.success() && out_str.trim().is_empty() {
        return Err(format!("ADB dizin okunamadı: {}", if err_str.trim().is_empty() { "Erişim reddedildi veya bulunamadı" } else { &err_str }));
    }
    let mut entries = Vec::new();

    for line in out_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Format: %s|%Y|%F|%A|%n
        // Example: 3452|1780998549|directory|drwxrws---|DCIM
        let parts: Vec<&str> = line.splitn(5, '|').collect();
        if parts.len() < 5 {
            continue;
        }

        let size: u64 = parts[0].parse().unwrap_or(0);
        let timestamp: i64 = parts[1].parse().unwrap_or(0);
        let file_type = parts[2];
        let permissions = parts[3].to_string();
        let name = parts[4].to_string();

        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        let hidden = name.starts_with('.');
        if !show_hidden && hidden {
            continue;
        }

        let is_dir = file_type.contains("directory");
        let is_symlink = file_type.contains("symbolic link");

        let entry_remote_path = if remote_path.ends_with('/') {
            format!("{}{}", remote_path, name)
        } else {
            format!("{}/{}", remote_path, name)
        };
        let full_adb_path = format!("adb://{}/{}", serial, entry_remote_path.trim_start_matches('/'));

        let modified = if timestamp > 0 {
            chrono::DateTime::from_timestamp(timestamp, 0)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_else(|| "-".into())
        } else {
            "-".into()
        };

        let extension = if is_dir {
            String::new()
        } else {
            Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let mime_type = if is_dir {
            "inode/directory".to_string()
        } else {
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .to_string()
        };

        entries.push(FileEntry {
            name,
            path: full_adb_path,
            is_dir,
            is_symlink,
            size,
            modified: modified.clone(),
            created: modified,
            permissions,
            mime_type,
            extension,
            hidden,
            children_count: None,
        });
    }

    // Sort folders first, then alphabetically
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

    // Parent calculation
    let parent = if remote_path == "/sdcard" || remote_path == "/storage/emulated/0" || remote_path == "/" {
        None
    } else {
        let parent_remote = Path::new(&remote_path).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        if parent_remote.is_empty() {
            Some(format!("adb://{}/sdcard", serial))
        } else {
            Some(format!("adb://{}/{}", serial, parent_remote.trim_start_matches('/')))
        }
    };

    let total = entries.len();
    Ok(DirectoryListing {
        path: path.to_string(),
        entries,
        parent,
        total_items: total,
    })
}

/// Create a directory on the Android device
pub fn adb_create_dir(path: &str) -> Result<(), String> {
    let (serial, remote_path) = parse_adb_path(path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", path))?;
    let escaped = remote_path.replace('\'', "'\\''");
    let out = Command::new("adb")
        .args(["-s", &serial, "shell", &format!("mkdir -p '{}'", escaped)])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Create an empty file on the Android device
pub fn adb_create_file(path: &str) -> Result<(), String> {
    let (serial, remote_path) = parse_adb_path(path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", path))?;
    let escaped = remote_path.replace('\'', "'\\''");
    let out = Command::new("adb")
        .args(["-s", &serial, "shell", &format!("touch '{}'", escaped)])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Delete file or directory on the Android device
pub fn adb_delete_item(path: &str) -> Result<(), String> {
    let (serial, remote_path) = parse_adb_path(path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", path))?;
    let escaped = remote_path.replace('\'', "'\\''");
    let out = Command::new("adb")
        .args(["-s", &serial, "shell", &format!("rm -rf '{}'", escaped)])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Rename or move within the same Android device
pub fn adb_rename_item(old_path: &str, new_path: &str) -> Result<(), String> {
    let (serial_old, remote_old) = parse_adb_path(old_path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", old_path))?;
    let (serial_new, remote_new) = parse_adb_path(new_path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", new_path))?;

    if serial_old != serial_new {
        return Err("Farklı ADB cihazları arasında doğrudan yeniden adlandırma yapılamaz".into());
    }

    let esc_old = remote_old.replace('\'', "'\\''");
    let esc_new = remote_new.replace('\'', "'\\''");

    let out = Command::new("adb")
        .args(["-s", &serial_old, "shell", &format!("mv '{}' '{}'", esc_old, esc_new)])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Push local file or folder to Android device
pub fn adb_push(local_src: &str, adb_dst: &str) -> Result<(), String> {
    let (serial, remote_dst) = parse_adb_path(adb_dst)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", adb_dst))?;

    let out = Command::new("adb")
        .args(["-s", &serial, "push", local_src, &remote_dst])
        .output()
        .map_err(|e| format!("ADB push başlatılamadı: {}", e))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ADB push hatası: {}", err));
    }
    Ok(())
}

/// Pull file or folder from Android device to local filesystem
pub fn adb_pull(adb_src: &str, local_dst: &str) -> Result<(), String> {
    let (serial, remote_src) = parse_adb_path(adb_src)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", adb_src))?;

    let out = Command::new("adb")
        .args(["-s", &serial, "pull", &remote_src, local_dst])
        .output()
        .map_err(|e| format!("ADB pull başlatılamadı: {}", e))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ADB pull hatası: {}", err));
    }
    Ok(())
}

/// Cache an ADB file locally (e.g. for opening with default desktop app or generating thumbnail)
pub fn adb_cache_file(adb_path: &str) -> Result<PathBuf, String> {
    let (serial, remote_path) = parse_adb_path(adb_path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", adb_path))?;

    let file_name = Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("adb_cache")
        .join(&serial);

    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let target = cache_dir.join(&file_name);

    adb_pull(adb_path, target.to_str().unwrap_or_default())?;
    Ok(target)
}

/// Get file properties/stat for an adb path
pub fn adb_get_file_entry(path: &str) -> Result<FileEntry, String> {
    let (serial, remote_path) = parse_adb_path(path)
        .ok_or_else(|| format!("Geçersiz ADB yolu: {}", path))?;

    let escaped = remote_path.replace('\'', "'\\''");
    let cmd = format!("stat -c '%s|%Y|%F|%A|%n' '{}'", escaped);

    let output = Command::new("adb")
        .args(["-s", &serial, "shell", &cmd])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("Dosya bilgisi alınamadı".into());
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let line = out_str.lines().next().unwrap_or("").trim();
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    if parts.len() < 5 {
        return Err("Geçersiz stat çıktısı".into());
    }

    let size: u64 = parts[0].parse().unwrap_or(0);
    let timestamp: i64 = parts[1].parse().unwrap_or(0);
    let file_type = parts[2];
    let permissions = parts[3].to_string();
    let full_name = parts[4];
    let name = Path::new(full_name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| full_name.to_string());

    let is_dir = file_type.contains("directory");
    let is_symlink = file_type.contains("symbolic link");

    let modified = if timestamp > 0 {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string())
            .unwrap_or_else(|| "-".into())
    } else {
        "-".into()
    };

    let extension = if is_dir {
        String::new()
    } else {
        Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    let mime_type = if is_dir {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(&name)
            .first_or_octet_stream()
            .to_string()
    };

    Ok(FileEntry {
        name: name.clone(),
        path: path.to_string(),
        is_dir,
        is_symlink,
        size,
        modified: modified.clone(),
        created: modified,
        permissions,
        mime_type,
        extension,
        hidden: name.starts_with('.'),
        children_count: None,
    })
}
