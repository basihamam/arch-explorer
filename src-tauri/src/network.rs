// network.rs — SMB/NFS ağ paylaşımı mount
// Gereksinimler: smbclient (SMB keşif), cifs-utils (SMB mount), nfs-utils (NFS mount)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmbShare {
    pub host:    String,
    pub name:    String,
    pub comment: String,
    pub share_type: String, // "Disk" | "Printer" | "IPC"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMount {
    pub source:      String, // //host/share veya host:/path
    pub mount_point: String,
    pub fs_type:     String, // "cifs" | "nfs"
    pub options:     String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkMountResult {
    pub success:     bool,
    pub mount_point: String,
    pub message:     String,
}

fn network_mount_base() -> PathBuf {
    PathBuf::from("/run/media/network")
}

fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

// ── SMB ──

/// SMB host'undaki paylaşımları listele
#[tauri::command]
pub fn list_smb_shares(host: String) -> Result<Vec<SmbShare>, String> {
    let out = Command::new("smbclient")
        .args(["-L", &host, "-N", "--no-pass"])
        .output()
        .map_err(|e| format!("smbclient bulunamadı: {}\n'sudo pacman -S samba' ile kur.", e))?;

    let text = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);

    let mut shares = vec![];
    let mut in_section = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("Sharename") { in_section = true; continue; }
        if line.starts_with("-----") { continue; }
        if line.is_empty() { in_section = false; continue; }
        if !in_section { continue; }

        // Format: "ShareName   Type    Comment"
        let parts: Vec<&str> = line.splitn(3, char::is_whitespace).collect();
        if parts.len() < 2 { continue; }

        let name       = parts[0].trim().to_string();
        let share_type = parts[1].trim().to_string();
        let comment    = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default();

        if name == "IPC$" || share_type == "IPC" { continue; } // IPC share'leri atla

        shares.push(SmbShare { host: host.clone(), name, comment, share_type });
    }

    if shares.is_empty() && !out.status.success() {
        return Err(format!(
            "Host'a erişilemedi: {}\nGüvenlik duvarını kontrol et veya host adını doğrula.",
            host
        ));
    }

    Ok(shares)
}

/// SMB paylaşımını mount et
#[tauri::command]
pub fn mount_smb(
    host:       String,
    share:      String,
    username:   Option<String>,
    password:   Option<String>,
    mount_point: Option<String>,
) -> Result<NetworkMountResult, String> {
    let mp = mount_point.unwrap_or_else(|| {
        let name = format!("{}_{}", safe_name(&host), safe_name(&share));
        network_mount_base().join(name).to_string_lossy().to_string()
    });

    fs::create_dir_all(&mp).map_err(|e| format!("Mount noktası oluşturulamadı: {}", e))?;

    let source = format!("//{}/{}", host, share);

    // Options oluştur
    let mut options = vec!["uid=1000".to_string(), "gid=1000".to_string()];
    if let Some(user) = &username {
        options.push(format!("username={}", user));
    } else {
        options.push("guest".to_string());
    }
    if let Some(pass) = &password {
        options.push(format!("password={}", pass));
    }
    options.push("vers=3.0".to_string());
    options.push("iocharset=utf8".to_string());

    let opts_str = options.join(",");

    let out = Command::new("mount")
        .args(["-t", "cifs", &source, &mp, "-o", &opts_str])
        .output()
        .map_err(|e| format!("mount çalıştırılamadı: {}", e))?;

    if out.status.success() {
        Ok(NetworkMountResult {
            success: true,
            mount_point: mp.clone(),
            message: format!("Mounted: {} → {}", source, mp),
        })
    } else {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        // Mount noktasını temizle (başarısız ise)
        fs::remove_dir(&mp).ok();
        Err(format!(
            "SMB mount başarısız: {}\ncifs-utils kurulu değilse: sudo pacman -S cifs-utils",
            err.trim()
        ))
    }
}

// ── NFS ──

/// NFS mount et
#[tauri::command]
pub fn mount_nfs(
    host:        String,
    remote_path: String,
    mount_point: Option<String>,
) -> Result<NetworkMountResult, String> {
    let mp = mount_point.unwrap_or_else(|| {
        let name = format!("nfs_{}_{}", safe_name(&host), safe_name(&remote_path));
        network_mount_base().join(name).to_string_lossy().to_string()
    });

    fs::create_dir_all(&mp).map_err(|e| format!("Mount noktası oluşturulamadı: {}", e))?;

    let source = format!("{}:{}", host, remote_path);

    let out = Command::new("mount")
        .args(["-t", "nfs", &source, &mp, "-o", "vers=4,rw,hard,intr"])
        .output()
        .map_err(|e| format!("mount çalıştırılamadı: {}", e))?;

    if out.status.success() {
        Ok(NetworkMountResult {
            success: true,
            mount_point: mp.clone(),
            message: format!("NFS mounted: {} → {}", source, mp),
        })
    } else {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        fs::remove_dir(&mp).ok();
        Err(format!(
            "NFS mount başarısız: {}\nnfs-utils kurulu değilse: sudo pacman -S nfs-utils",
            err.trim()
        ))
    }
}

/// Bağlı ağ paylaşımlarını listele
#[tauri::command]
pub fn list_network_mounts() -> Vec<NetworkMount> {
    let out = Command::new("findmnt")
        .args(["-t", "cifs,nfs,nfs4", "-J", "-o", "SOURCE,TARGET,FSTYPE,OPTIONS"])
        .output();

    let out = match out {
        Ok(o) => o,
        Err(_) => return fallback_network_mounts(),
    };

    if !out.status.success() { return fallback_network_mounts(); }

    let text = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v, Err(_) => return fallback_network_mounts(),
    };

    json["filesystems"].as_array()
        .map(|arr| arr.iter().filter_map(|fs| {
            Some(NetworkMount {
                source:      fs["source"].as_str()?.to_string(),
                mount_point: fs["target"].as_str()?.to_string(),
                fs_type:     fs["fstype"].as_str()?.to_string(),
                options:     fs["options"].as_str().unwrap_or("").to_string(),
            })
        }).collect())
        .unwrap_or_default()
}

fn fallback_network_mounts() -> Vec<NetworkMount> {
    // /proc/mounts'tan parse et
    let content = match fs::read_to_string("/proc/mounts") {
        Ok(c) => c, Err(_) => return vec![],
    };

    content.lines().filter_map(|line| {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { return None; }
        let fs_type = parts[2];
        if !matches!(fs_type, "cifs" | "nfs" | "nfs4") { return None; }
        Some(NetworkMount {
            source:      parts[0].to_string(),
            mount_point: parts[1].to_string(),
            fs_type:     fs_type.to_string(),
            options:     parts[3].to_string(),
        })
    }).collect()
}

/// Ağ paylaşımını unmount et
#[tauri::command]
pub fn unmount_network(mount_point: String) -> Result<String, String> {
    let out = Command::new("umount")
        .arg(&mount_point)
        .output()
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        // Mount noktası boşsa kaldır
        fs::remove_dir(&mount_point).ok();
        Ok(format!("{} ayrıldı", mount_point))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// Bağlantı testi — host'a ping at
#[tauri::command]
pub fn ping_host(host: String) -> bool {
    Command::new("ping")
        .args(["-c", "1", "-W", "2", &host])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
