// src/backend/adb.rs
// Android Debug Bridge (ADB) entegrasyonu
// Telefon bağlandığında otomatik algılama, dosya listeleme, push/pull/delete/mkdir/rename desteği

use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::backend::fs_ops::{FileEntry, DirectoryListing};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AdbDevice {
    pub serial: String,
    pub model: String,
    pub manufacturer: String,
    pub state: String, // "device", "unauthorized", "offline", vb.
    pub is_authorized: bool,
    pub total_storage: u64,
    pub used_storage: u64,
}

impl AdbDevice {
    pub fn display_name(&self) -> String {
        if !self.model.is_empty() {
            if !self.manufacturer.is_empty() {
                format!("{} {}", capitalize(&self.manufacturer), self.model)
            } else {
                self.model.clone()
            }
        } else {
            format!("Android ({})", self.serial)
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Bağlı ADB cihazlarını listele
pub fn list_adb_devices() -> Vec<AdbDevice> {
    let output = match Command::new("adb").args(["devices", "-l"]).output() {
        Ok(out) => out,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') || line.starts_with("List of devices") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let serial = parts[0].to_string();
        let state = parts.get(1).unwrap_or(&"unknown").to_string();
        let is_authorized = state == "device";

        let mut model = String::new();
        for part in &parts[2..] {
            if let Some(m) = part.strip_prefix("model:") {
                model = m.replace('_', " ");
            }
        }

        let mut manufacturer = String::new();
        let mut total_storage = 0u64;
        let mut used_storage = 0u64;

        if is_authorized {
            if model.is_empty() {
                if let Ok(out) = Command::new("adb")
                    .args(["-s", &serial, "shell", "getprop", "ro.product.model"])
                    .output()
                {
                    model = String::from_utf8_lossy(&out.stdout).trim().to_string();
                }
            }

            if let Ok(out) = Command::new("adb")
                .args(["-s", &serial, "shell", "getprop", "ro.product.manufacturer"])
                .output()
            {
                manufacturer = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }

            // Depolama alanı bilgisi
            if let Ok(out) = Command::new("adb")
                .args(["-s", &serial, "shell", "df", "-k", "/sdcard"])
                .output()
            {
                let df_out = String::from_utf8_lossy(&out.stdout);
                for df_line in df_out.lines().skip(1) {
                    let cols: Vec<&str> = df_line.split_whitespace().collect();
                    if cols.len() >= 4 {
                        let total_kb: u64 = cols[1].parse().unwrap_or(0);
                        let used_kb: u64 = cols[2].parse().unwrap_or(0);
                        if total_kb > 0 {
                            total_storage = total_kb * 1024;
                            used_storage = used_kb * 1024;
                            break;
                        }
                    }
                }
            }
        }

        devices.push(AdbDevice {
            serial,
            model,
            manufacturer,
            state,
            is_authorized,
            total_storage,
            used_storage,
        });
    }

    devices
}

/// adb://serial/path formatını ayrıştır: (serial, device_path)
pub fn parse_adb_path(path: &str) -> Option<(String, String)> {
    let clean = path.strip_prefix("adb://")?;
    let mut parts = clean.splitn(2, '/');
    let serial = parts.next()?.to_string();
    if serial.is_empty() {
        return None;
    }
    let sub = parts.next().unwrap_or("");
    let device_path = if sub.is_empty() {
        "/sdcard".to_string()
    } else if !sub.starts_with('/') {
        format!("/{}", sub)
    } else {
        sub.to_string()
    };
    Some((serial, device_path))
}

pub fn make_adb_path(serial: &str, device_path: &str) -> String {
    let norm = device_path.trim_start_matches('/');
    format!("adb://{}/{}", serial, norm)
}

/// ADB cihazındaki bir dizini listele
pub fn list_adb_directory(serial: &str, device_path: &str, show_hidden: bool) -> Result<DirectoryListing, String> {
    // Normalizasyon: /sdcard ya da /sdcard/ gibi
    let target_dir = if device_path.is_empty() { "/sdcard" } else { device_path };

    // ls -lan <path>/
    let path_with_slash = if target_dir.ends_with('/') {
        target_dir.to_string()
    } else {
        format!("{}/", target_dir)
    };

    let escaped_path = path_with_slash.replace('\'', "'\\''");
    let cmd_str = format!("ls -lan '{}'", escaped_path);

    let output = Command::new("adb")
        .args(["-s", serial, "shell", &cmd_str])
        .output()
        .map_err(|e| format!("ADB çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            "ADB dizin okunamadı veya erişim engellendi".to_string()
        } else {
            err.trim().to_string()
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("total ") {
            continue;
        }

        // Parse: drwxrws--- 26 10291 1023 53248 2026-10-10 15:56 Download
        // veya symlink: lrw-r--r-- 1 0 0 21 2022-01-01 03:00 sdcard -> /storage/self/primary
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            continue;
        }

        let perms = parts[0];
        let mut is_dir = perms.starts_with('d');
        let is_symlink = perms.starts_with('l');
        let size: u64 = parts[4].parse().unwrap_or(0);
        let date = parts[5];
        let time = parts[6];
        let modified = format!("{} {}", date, time);

        let mut name = parts[7..].join(" ");
        if is_symlink {
            if let Some((link_name, target)) = name.clone().split_once(" -> ") {
                let target_trimmed = target.trim().to_string();
                name = link_name.trim().to_string();
                if target_trimmed.ends_with('/') || (target_trimmed.starts_with('/') && !target_trimmed.contains('.')) {
                    is_dir = true;
                }
            }
        }

        if name == "." || name == ".." {
            continue;
        }

        let hidden = name.starts_with('.');
        if hidden && !show_hidden {
            continue;
        }

        let full_device_path = if target_dir.ends_with('/') {
            format!("{}{}", target_dir, name)
        } else {
            format!("{}/{}", target_dir, name)
        };

        let aex_path = make_adb_path(serial, &full_device_path);

        let extension = if is_dir {
            String::new()
        } else {
            std::path::Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let mime_type = if is_dir {
            "inode/directory".to_string()
        } else {
            match extension.to_lowercase().as_str() {
                "png" => "image/png".to_string(),
                "jpg" | "jpeg" => "image/jpeg".to_string(),
                "gif" => "image/gif".to_string(),
                "webp" => "image/webp".to_string(),
                "mp4" => "video/mp4".to_string(),
                "mkv" => "video/x-matroska".to_string(),
                "mp3" => "audio/mpeg".to_string(),
                "apk" => "application/vnd.android.package-archive".to_string(),
                "pdf" => "application/pdf".to_string(),
                "zip" => "application/zip".to_string(),
                "txt" => "text/plain".to_string(),
                _ => "application/octet-stream".to_string(),
            }
        };

        entries.push(FileEntry {
            name,
            path: aex_path,
            is_dir,
            is_symlink,
            size: if is_dir { 0 } else { size },
            modified,
            created: String::new(),
            permissions: perms.to_string(),
            mime_type,
            extension,
            hidden,
            is_executable: perms.contains('x') && !is_dir,
        });
    }

    let parent_path = if target_dir == "/" || target_dir == "/sdcard" {
        None
    } else {
        std::path::Path::new(target_dir)
            .parent()
            .map(|p| make_adb_path(serial, &p.to_string_lossy()))
    };

    Ok(DirectoryListing {
        path: make_adb_path(serial, target_dir),
        total_items: entries.len(),
        entries,
        parent: parent_path,
    })
}

/// Telefona yerel dosyayı kopyala (adb push)
pub fn adb_push(serial: &str, local_path: &str, device_dest_dir: &str) -> Result<(), String> {
    let output = Command::new("adb")
        .args(["-s", serial, "push", local_path, device_dest_dir])
        .output()
        .map_err(|e| format!("adb push çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            err.trim().to_string()
        });
    }
    Ok(())
}

/// Telefondan yerel bilgisayara dosyayı çek (adb pull)
pub fn adb_pull(serial: &str, device_path: &str, local_dest_dir: &str) -> Result<(), String> {
    let output = Command::new("adb")
        .args(["-s", serial, "pull", device_path, local_dest_dir])
        .output()
        .map_err(|e| format!("adb pull çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            err.trim().to_string()
        });
    }
    Ok(())
}

/// Telefon içinde dosya / klasör sil (adb shell rm -rf)
pub fn adb_delete(serial: &str, device_path: &str) -> Result<(), String> {
    let escaped_path = device_path.replace('\'', "'\\''");
    let cmd_str = format!("rm -rf '{}'", escaped_path);
    let output = Command::new("adb")
        .args(["-s", serial, "shell", &cmd_str])
        .output()
        .map_err(|e| format!("adb rm çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

/// Telefon içinde yeni klasör oluştur (adb shell mkdir -p)
pub fn adb_mkdir(serial: &str, device_path: &str) -> Result<(), String> {
    let escaped_path = device_path.replace('\'', "'\\''");
    let cmd_str = format!("mkdir -p '{}'", escaped_path);
    let output = Command::new("adb")
        .args(["-s", serial, "shell", &cmd_str])
        .output()
        .map_err(|e| format!("adb mkdir çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

/// Telefon içinde yeni dosya oluştur (adb shell touch)
pub fn adb_touch(serial: &str, device_path: &str) -> Result<(), String> {
    let escaped_path = device_path.replace('\'', "'\\''");
    let cmd_str = format!("touch '{}'", escaped_path);
    let output = Command::new("adb")
        .args(["-s", serial, "shell", &cmd_str])
        .output()
        .map_err(|e| format!("adb touch çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

/// Telefon içinde dosya taşı / yeniden adlandır (adb shell mv)
pub fn adb_rename(serial: &str, old_device_path: &str, new_device_path: &str) -> Result<(), String> {
    let esc_old = old_device_path.replace('\'', "'\\''");
    let esc_new = new_device_path.replace('\'', "'\\''");
    let cmd_str = format!("mv '{}' '{}'", esc_old, esc_new);
    let output = Command::new("adb")
        .args(["-s", serial, "shell", &cmd_str])
        .output()
        .map_err(|e| format!("adb mv çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}
