// mounts.rs — Full mount chain + akıllı fallback state + label-based mount points
//   Log: ~/.local/share/arch-explorer/mount_debug.log
//   State: ~/.local/share/arch-explorer/fallback_state.json
//
// Fallback mantığı:
//   - udisks2 VEYA pkexec 3+ kez başarısız olursa → 30 gün atla, direkt sudo şifresi iste
//   - 30 gün sonra sıfırla ve yeniden dene
//   - Her karar diske yazılır, uygulama yeniden başlayınca devam eder

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;
use zbus::proxy;
use zbus::zvariant::Value;
use zbus::Connection;

const ZBUS_TIMEOUT: u64 = 5;

// ---- Log ----

fn log_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("mount_debug.log")
}

fn mlog(step: &str, msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] [{}] {}\n", now, step, msg);
    eprint!("{}", line);
    let path = log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

// ---- Akıllı Fallback State ----
// udisks2 ve pkexec kaç kez başarısız oldu, ne zamana kadar atlayacağız

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MethodAttempts {
    pub failures: u32,
    /// Unix timestamp — bu zamana kadar bu metodu atla
    pub skip_until: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct FallbackState {
    pub udisks2: MethodAttempts,
    pub pkexec:  MethodAttempts,
}

fn fallback_state_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("fallback_state.json")
}

fn load_fallback_state() -> FallbackState {
    let path = fallback_state_path();
    if !path.exists() { return FallbackState::default(); }
    let data = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_fallback_state(state: &FallbackState) {
    let path = fallback_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(data) = serde_json::to_string_pretty(state) {
        fs::write(&path, data).ok();
    }
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Bu metod şu an atlanmalı mı?
fn should_skip(attempts: &MethodAttempts) -> bool {
    if attempts.failures < 3 { return false; }
    if let Some(until) = attempts.skip_until {
        return now_unix() < until;
    }
    false
}

/// 30 günlük süre dolmuşsa sıfırla
fn reset_if_expired(attempts: &mut MethodAttempts, name: &str) {
    if attempts.failures < 3 { return; }
    if let Some(until) = attempts.skip_until {
        if now_unix() >= until {
            mlog("FALLBACK", &format!("{}: 30 günlük bekleme doldu — sıfırlanıyor, yeniden denenecek", name));
            *attempts = MethodAttempts::default();
        }
    }
}

/// Başarısız → sayacı artır, 3'e ulaşınca 30 gün atla
fn record_failure(attempts: &mut MethodAttempts, name: &str) {
    attempts.failures += 1;
    mlog("FALLBACK", &format!("{}: başarısız sayısı = {}", name, attempts.failures));
    if attempts.failures >= 3 {
        let until = now_unix() + 30 * 24 * 3600;
        attempts.skip_until = Some(until);
        let until_dt = chrono::DateTime::from_timestamp(until, 0)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "30 gün sonra".to_string());
        mlog("FALLBACK", &format!(
            "{}: 3+ başarısız — {} tarihine kadar atlıyoruz, direkt sudo şifresi isteyeceğiz",
            name, until_dt
        ));
    }
}

// ---- Mount Point Adlandırma ----

/// Disk label'ından temiz bir mount point adı üret
/// "Windows 11" → "Windows_11"
/// "" (boş) veya tamamen özel karakter → "disk1", "disk2" ...
fn make_mount_name(label: &str) -> String {
    // Alphanumeric + tire + alt çizgi + boşluk→alt çizgi
    let clean: String = label
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();

    if clean.is_empty() {
        // Boş veya sadece özel karakter → disk1, disk2 ...
        next_disk_name()
    } else {
        clean
    }
}

/// /run/media/ içinde olmayan ilk diskN adını bul
fn next_disk_name() -> String {
    for i in 1..=99 {
        let path = format!("/run/media/disk{}", i);
        if !std::path::Path::new(&path).exists() {
            return format!("disk{}", i);
        }
    }
    "disk_misc".to_string()
}

fn make_mount_point(device: &str, label: &str) -> String {
    let name = if label.trim().is_empty() {
        // Label yok → device adından türet, olmuyorsa diskN
        let dev_suffix = device.trim_start_matches("/dev/");
        let clean: String = dev_suffix.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if clean.is_empty() { next_disk_name() } else { clean }
    } else {
        make_mount_name(label)
    };
    format!("/run/media/{}", name)
}

// ---- Structs ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockDevice {
    pub name:        String,
    pub device:      String,
    pub size:        String,
    pub fs_type:     String,
    pub label:       String,
    pub mount_point: Option<String>,
    pub is_mounted:  bool,
    pub is_windows:  bool,
    pub uuid:        String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MountResult {
    pub success:     bool,
    pub mount_point: String,
    pub message:     String,
}

// ---- UDisks2 D-Bus Proxy ----

#[proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisks2Filesystem {
    async fn mount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<String>;
    async fn unmount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

// ---- lsblk ----

pub fn list_block_devices() -> Vec<BlockDevice> {
    let out = Command::new("lsblk")
        .args(["-J", "-o", "NAME,SIZE,FSTYPE,LABEL,MOUNTPOINT,MOUNTPOINTS,UUID"])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(_) => return list_block_devices_fallback(),
    };
    if !out.status.success() { return list_block_devices_fallback(); }
    let text = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return list_block_devices_fallback(),
    };
    let mut devices = vec![];
    if let Some(bd) = json["blockdevices"].as_array() {
        for dev in bd { parse_lsblk_device(dev, &mut devices); }
    }
    devices
}

fn list_block_devices_fallback() -> Vec<BlockDevice> {
    let out = Command::new("lsblk")
        .args(["-J", "-o", "NAME,SIZE,FSTYPE,LABEL,MOUNTPOINT,UUID"])
        .output();
    let out = match out { Ok(o) => o, Err(_) => return vec![] };
    let text = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v, Err(_) => return vec![],
    };
    let mut devices = vec![];
    if let Some(bd) = json["blockdevices"].as_array() {
        for dev in bd { parse_lsblk_device(dev, &mut devices); }
    }
    devices
}

fn parse_lsblk_device(dev: &serde_json::Value, out: &mut Vec<BlockDevice>) {
    if let Some(children) = dev["children"].as_array() {
        for child in children { parse_lsblk_device(child, out); }
    }
    let fs_type = dev["fstype"].as_str().unwrap_or("").to_string();
    let name    = dev["name"].as_str().unwrap_or("").to_string();
    if name.is_empty() || fs_type.is_empty() { return; }
    let mount_point = extract_mount_point(dev);
    let is_windows  = matches!(fs_type.as_str(), "ntfs" | "ntfs-3g" | "exfat");
    let label_raw   = dev["label"].as_str().unwrap_or("").to_string();
    let label       = if label_raw.is_empty() { name.clone() } else { label_raw };
    out.push(BlockDevice {
        device: format!("/dev/{}", name),
        name: name.clone(),
        size: dev["size"].as_str().unwrap_or("?").to_string(),
        label,
        uuid: dev["uuid"].as_str().unwrap_or("").to_string(),
        is_mounted: mount_point.is_some(),
        mount_point,
        is_windows,
        fs_type,
    })
}

fn extract_mount_point(dev: &serde_json::Value) -> Option<String> {
    if let Some(arr) = dev["mountpoints"].as_array() {
        return arr.iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "[SWAP]")
            .map(|s| s.to_string())
            .next();
    }
    dev["mountpoint"].as_str()
        .filter(|s| !s.is_empty() && *s != "[SWAP]")
        .map(|s| s.to_string())
}

// ---- Tauri commands ----

pub async fn mount_device(device: String, label: Option<String>) -> Result<MountResult, String> {
    mlog("MOUNT", &format!("Başlatıldı: {} (label: {:?})", device, label));
    if let Some(mp) = get_mount_status(device.clone()) {
        mlog("MOUNT", &format!("Zaten mounted: {}", mp));
        return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("Zaten mounted: {}", mp) });
    }
    full_mount_chain(device, label.unwrap_or_default()).await
}

pub async fn mount_ntfs_windows_disk(device: String, label: Option<String>, _mount_point: Option<String>) -> Result<MountResult, String> {
    mlog("MOUNT_NTFS", &format!("Başlatıldı: {} (label: {:?})", device, label));
    if let Some(mp) = get_mount_status(device.clone()) {
        mlog("MOUNT_NTFS", &format!("Zaten mounted: {}", mp));
        return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("Zaten mounted: {}", mp) });
    }
    full_mount_chain(device, label.unwrap_or_default()).await
}

pub async fn mount_ntfs_force_fix(device: String, label: Option<String>) -> Result<MountResult, String> {
    mlog("NTFSFIX", &format!("Hiberfil temizleniyor: {}", device));
    match run_cmd(&["ntfsfix", "-d", &device]) {
        Ok(o) => {
            mlog("NTFSFIX", &format!("OK: {}", o.trim()));
            full_mount_chain(device, label.unwrap_or_default()).await
        }
        Err(e) => {
            mlog("NTFSFIX", &format!("HATA: {}", e.trim()));
            Err(format!("ntfsfix başarısız: {}", e))
        }
    }
}

pub async fn mount_ntfs_with_password(
    device: String,
    password: String,
    label: Option<String>,
    mount_point: Option<String>,
) -> Result<MountResult, String> {
    mlog("SUDO_MOUNT", &format!("Başlatıldı: {}", device));

    if let Some(mp) = get_mount_status(device.clone()) {
        mlog("SUDO_MOUNT", &format!("Zaten mounted: {}", mp));
        return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("Zaten mounted: {}", mp) });
    }

    // Mount point: explicit > label-based > device-based
    let mp = mount_point.unwrap_or_else(|| {
        make_mount_point(&device, &label.unwrap_or_default())
    });
    mlog("SUDO_MOUNT", &format!("Mount noktası: {}", mp));

    if let Err(e) = sudo_mkdir(&mp, &password) {
        mlog("SUDO_MOUNT", &format!("mkdir HATA: {}", e));
        if e == "WRONG_PASSWORD" {
            return Ok(MountResult { success: false, mount_point: String::new(), message: "WRONG_PASSWORD".into() });
        }
        return Ok(MountResult { success: false, mount_point: String::new(), message: format!("Mount noktası oluşturulamadı: {}", e) });
    }

    // ntfs3 kernel
    mlog("SUDO_MOUNT", "Deneniyor: sudo mount -t ntfs3");
    match sudo_mount(&device, &mp, "ntfs3", &password) {
        Ok(true) => {
            mlog("SUDO_MOUNT", "BAŞARILI: ntfs3");
            return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("sudo+ntfs3: {}", mp) });
        }
        Ok(false) => mlog("SUDO_MOUNT", "ntfs3 başarısız (false)"),
        Err(e) if e == "WRONG_PASSWORD" => {
            mlog("SUDO_MOUNT", "YANLIŞ ŞİFRE (ntfs3 adımında)");
            return Ok(MountResult { success: false, mount_point: String::new(), message: "WRONG_PASSWORD".into() });
        }
        Err(e) => mlog("SUDO_MOUNT", &format!("ntfs3 HATA: {}", e)),
    }

    // ntfs-3g
    mlog("SUDO_MOUNT", "Deneniyor: sudo mount -t ntfs-3g");
    match sudo_mount(&device, &mp, "ntfs-3g", &password) {
        Ok(true) => {
            mlog("SUDO_MOUNT", "BAŞARILI: ntfs-3g");
            return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("sudo+ntfs-3g: {}", mp) });
        }
        Ok(false) => mlog("SUDO_MOUNT", "ntfs-3g başarısız (false)"),
        Err(e) if e == "WRONG_PASSWORD" => {
            mlog("SUDO_MOUNT", "YANLIŞ ŞİFRE (ntfs-3g adımında)");
            return Ok(MountResult { success: false, mount_point: String::new(), message: "WRONG_PASSWORD".into() });
        }
        Err(e) => mlog("SUDO_MOUNT", &format!("ntfs-3g HATA: {}", e)),
    }

    // ext4 / vfat / auto (Windows olmayan diskler için)
    mlog("SUDO_MOUNT", "Deneniyor: sudo mount -t auto");
    match sudo_mount(&device, &mp, "auto", &password) {
        Ok(true) => {
            mlog("SUDO_MOUNT", "BAŞARILI: auto");
            return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("sudo+auto: {}", mp) });
        }
        Ok(false) => mlog("SUDO_MOUNT", "auto başarısız (false)"),
        Err(e) if e == "WRONG_PASSWORD" => {
            mlog("SUDO_MOUNT", "YANLIŞ ŞİFRE (auto adımında)");
            return Ok(MountResult { success: false, mount_point: String::new(), message: "WRONG_PASSWORD".into() });
        }
        Err(e) => mlog("SUDO_MOUNT", &format!("auto HATA: {}", e)),
    }

    mlog("SUDO_MOUNT", "TÜM YOLLAR BAŞARISIZ");
    Ok(MountResult {
        success: false,
        mount_point: String::new(),
        message: "Yanlış şifre veya mount başarısız — log: ~/.local/share/arch-explorer/mount_debug.log".into(),
    })
}

pub async fn unmount_device(device: String) -> Result<String, String> {
    mlog("UNMOUNT", &format!("Başlatıldı: {}", device));

    // 1. UDisks2 / zbus (polkit agent varsa şifresiz)
    let res = timeout(Duration::from_secs(ZBUS_TIMEOUT), zbus_unmount(device.clone())).await;
    match res {
        Ok(Ok(msg)) => { mlog("UNMOUNT", &format!("zbus OK: {}", msg)); return Ok(msg); }
        Ok(Err(e))  => { mlog("UNMOUNT", &format!("zbus HATA: {} → pkexec deneniyor", e)); }
        Err(_)      => { mlog("UNMOUNT", &format!("zbus timeout ({}s) → pkexec deneniyor", ZBUS_TIMEOUT)); }
    }

    // 2. pkexec umount (polkit ile ayrıcalıklı — şifresiz GUI dialog)
    match run_cmd(&["pkexec", "umount", &device]) {
        Ok(_) => { mlog("UNMOUNT", "pkexec OK"); return Ok(format!("{} pkexec ile ayrıldı.", device)); }
        Err(e) => { mlog("UNMOUNT", &format!("pkexec HATA: {} → sudo şifresi gerekiyor", e)); }
    }

    // 3. plain umount (rootsa çalışır)
    match run_cmd(&["umount", &device]) {
        Ok(_)  => { mlog("UNMOUNT", "umount OK"); return Ok(format!("{} umount ile ayrıldı.", device)); }
        Err(e) => { mlog("UNMOUNT", &format!("umount HATA: {}", e)); }
    }

    // Hepsi başarısız → UI'a sudo şifresi iste sinyali gönder
    Err(format!("NEEDS_PASSWORD:{}", device))
}

pub async fn unmount_with_password(device: String, password: String) -> Result<String, String> {
    mlog("SUDO_UNMOUNT", &format!("Başlatıldı: {}", device));
    let mut child = Command::new("sudo")
        .args(["-S", "umount", &device])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| { mlog("SUDO_UNMOUNT", &format!("spawn HATA: {}", e)); e.to_string() })?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{}\n", password).as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    if out.status.success() {
        mlog("SUDO_UNMOUNT", "OK");
        Ok(format!("{} ayrıldı.", device))
    } else if is_wrong_password(&stderr) {
        mlog("SUDO_UNMOUNT", &format!("YANLIŞ ŞİFRE — stderr: {}", stderr.trim()));
        Err("WRONG_PASSWORD".into())
    } else {
        mlog("SUDO_UNMOUNT", &format!("HATA — stderr: {}", stderr.trim()));
        Err(format!("Unmount hatası: {}", stderr))
    }
}

pub fn get_mount_status(device: String) -> Option<String> {
    let out = Command::new("findmnt")
        .args(["-n", "-o", "TARGET", &device])
        .output().ok()?;
    let m = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if m.is_empty() { None } else { Some(m) }
}

/// Mevcut fallback state'i frontend'e göster (debug/bilgi için)
pub fn get_fallback_state() -> FallbackState {
    load_fallback_state()
}

/// Fallback state'i manuel sıfırla (settings'ten kullanılabilir)
pub fn reset_fallback_state() -> Result<(), String> {
    save_fallback_state(&FallbackState::default());
    mlog("FALLBACK", "State manuel sıfırlandı");
    Ok(())
}

// ---- Full mount chain — Akıllı fallback ile ----

async fn full_mount_chain(device: String, label: String) -> Result<MountResult, String> {
    mlog("CHAIN", &format!("=== Mount zinciri başlatıldı: {} (label: '{}') ===", device, label));

    // Fallback state'i yükle, süresi dolmuşları sıfırla
    let mut fb = load_fallback_state();
    reset_if_expired(&mut fb.udisks2, "udisks2");
    reset_if_expired(&mut fb.pkexec,  "pkexec");

    let mut last_error = String::new();

    // 1. UDisks2/zbus — 3+ başarısız ve 30 gün dolmamışsa atla
    if should_skip(&fb.udisks2) {
        let days_left = fb.udisks2.skip_until
            .map(|u| (u - now_unix()) / 86400)
            .unwrap_or(0);
        mlog("CHAIN", &format!(
            "1/4 ATLANDI — udisks2 son 30 günde 3+ kez başarısız oldu (~{} gün daha atlanacak)",
            days_left
        ));
    } else {
        mlog("CHAIN", "1/4 → zbus/UDisks2 deneniyor");
        match timeout(Duration::from_secs(ZBUS_TIMEOUT), zbus_mount(device.clone())).await {
            Ok(Ok(mp)) => {
                mlog("CHAIN", &format!("1/4 BAŞARILI: UDisks2 → {}", mp));
                return Ok(MountResult { success: true, mount_point: mp.clone(), message: format!("UDisks2: {}", mp) });
            }
            Ok(Err(ref e)) if e == "HIBERNATION_WARNING" => {
                mlog("CHAIN", "1/4 HIBERNATION tespit edildi");
                return Ok(MountResult { success: false, mount_point: String::new(), message: "HIBERNATION_WARNING".into() });
            }
            Ok(Err(e)) => {
                mlog("CHAIN", &format!("1/4 HATA: {}", e));
                record_failure(&mut fb.udisks2, "udisks2");
                save_fallback_state(&fb);
                let _ = e;
            }
            Err(_) => {
                mlog("CHAIN", &format!("1/4 TIMEOUT ({}s)", ZBUS_TIMEOUT));
                record_failure(&mut fb.udisks2, "udisks2");
                save_fallback_state(&fb);
                last_error = "UDisks2 timeout (polkit agent yok)".to_string();
            }
        }
    }

    let mp_dir = make_mount_point(&device, &label);
    mlog("CHAIN", &format!("Mount noktası: {}", mp_dir));
    std::fs::create_dir_all(&mp_dir).ok();

    // 2. kernel ntfs3
    mlog("CHAIN", &format!("2/4 → ntfs3 kernel: mount -t ntfs3 {} {}", device, mp_dir));
    match run_cmd(&["mount", "-t", "ntfs3", &device, &mp_dir]) {
        Ok(_) if get_mount_status(device.clone()).is_some() => {
            mlog("CHAIN", "2/4 BAŞARILI: ntfs3");
            return Ok(MountResult { success: true, mount_point: mp_dir.clone(), message: format!("ntfs3: {}", mp_dir) });
        }
        Ok(o) | Err(o) => {
            if is_hibernation(&o) {
                mlog("CHAIN", "2/4 HIBERNATION tespit edildi");
                return Ok(MountResult { success: false, mount_point: String::new(), message: "HIBERNATION_WARNING".into() });
            }
            mlog("CHAIN", &format!("2/4 HATA: {}", o.trim()));
            let _ = o;
        }
    }

    // 3. ntfs-3g FUSE
    mlog("CHAIN", &format!("3/4 → ntfs-3g: ntfs-3g {} {}", device, mp_dir));
    match run_cmd(&["ntfs-3g", &device, &mp_dir]) {
        Ok(_) if get_mount_status(device.clone()).is_some() => {
            mlog("CHAIN", "3/4 BAŞARILI: ntfs-3g");
            return Ok(MountResult { success: true, mount_point: mp_dir.clone(), message: format!("ntfs-3g: {}", mp_dir) });
        }
        Ok(o) | Err(o) => {
            if is_hibernation(&o) {
                mlog("CHAIN", "3/4 HIBERNATION tespit edildi");
                return Ok(MountResult { success: false, mount_point: String::new(), message: "HIBERNATION_WARNING".into() });
            }
            mlog("CHAIN", &format!("3/4 HATA: {}", o.trim()));
            let _ = o;
        }
    }

    // 4. pkexec — 3+ başarısız ve süre dolmamışsa atla
    if should_skip(&fb.pkexec) {
        let days_left = fb.pkexec.skip_until
            .map(|u| (u - now_unix()) / 86400)
            .unwrap_or(0);
        mlog("CHAIN", &format!(
            "4/4 ATLANDI — pkexec son 30 günde 3+ kez başarısız (~{} gün daha atlanacak)",
            days_left
        ));
    } else {
        mlog("CHAIN", &format!("4/4 → pkexec: pkexec mount {} {}", device, mp_dir));
        match run_cmd(&["pkexec", "mount", &device, &mp_dir]) {
            Ok(_) if get_mount_status(device.clone()).is_some() => {
                mlog("CHAIN", "4/4 BAŞARILI: pkexec");
                return Ok(MountResult { success: true, mount_point: mp_dir.clone(), message: format!("pkexec: {}", mp_dir) });
            }
            Ok(o) | Err(o) => {
                let is_auth_fail = o.contains("Not authorized") || o.contains("not authorized")
                    || o.contains("Authentication failed") || o.contains("No authentication agent");
                if is_auth_fail {
                    mlog("CHAIN", "4/4 pkexec: auth başarısız → failure kaydediliyor");
                    record_failure(&mut fb.pkexec, "pkexec");
                    save_fallback_state(&fb);
                } else {
                    mlog("CHAIN", &format!("4/4 HATA: {}", o.trim()));
                    let _ = o;
                }
            }
        }
    }

    // Hepsi başarısız → şifre iste
    mlog("CHAIN", &format!("=== Tüm zincir başarısız → PASSWORD_REQUIRED | son hata: {} ===", last_error.trim()));
    Ok(MountResult {
        success: false,
        mount_point: String::new(),
        message: format!("PASSWORD_REQUIRED|{}", last_error),
    })
}

// ---- zbus helpers ----

async fn zbus_mount(device: String) -> Result<String, String> {
    let conn = Connection::system().await.map_err(|e| e.to_string())?;
    let dev_name = device.trim_start_matches("/dev/");
    let path = format!("/org/freedesktop/UDisks2/block_devices/{}", dev_name);
    let proxy = UDisks2FilesystemProxy::builder(&conn)
        .path(path).map_err(|e| e.to_string())?
        .build().await.map_err(|e| e.to_string())?;
    let opts: HashMap<&str, Value<'_>> = HashMap::new();
    match proxy.mount(opts).await {
        Ok(mp) => Ok(mp),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("AlreadyMounted") { Ok(get_mount_status(device).unwrap_or_default()) }
            else if is_hibernation(&msg) { Err("HIBERNATION_WARNING".into()) }
            else { Err(msg) }
        }
    }
}

async fn zbus_unmount(device: String) -> Result<String, String> {
    let conn = Connection::system().await.map_err(|e| e.to_string())?;
    let dev_name = device.trim_start_matches("/dev/");
    let path = format!("/org/freedesktop/UDisks2/block_devices/{}", dev_name);
    let proxy = UDisks2FilesystemProxy::builder(&conn)
        .path(path).map_err(|e| e.to_string())?
        .build().await.map_err(|e| e.to_string())?;
    let opts: HashMap<&str, Value<'_>> = HashMap::new();
    match proxy.unmount(opts).await {
        Ok(_) => Ok(format!("{} ayrıldı.", device)),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("NotMounted") { Ok(format!("{} zaten unmounted.", device)) }
            else { Err(msg) }
        }
    }
}

// ---- sudo -S ----

fn sudo_mkdir(mp: &str, password: &str) -> Result<(), String> {
    let mkdir_cmd = format!("mkdir -p '{}'", mp);
    let mut child = Command::new("sudo")
        .args(["-S", "sh", "-c", &mkdir_cmd])
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

    mlog("SUDO", &format!(
        "sudo mkdir -p {} → exit:{} stdout:'{}' stderr:'{}'",
        mp, out.status.code().unwrap_or(-1), stdout.trim(), stderr.trim()
    ));

    if is_wrong_password(&stderr) { return Err("WRONG_PASSWORD".into()); }
    if !out.status.success() { return Err(format!("mkdir başarısız: {}", stderr.trim())); }
    Ok(())
}

fn sudo_mount(device: &str, mp: &str, fs: &str, password: &str) -> Result<bool, String> {
    let mut child = Command::new("sudo")
        .args(["-S", "mount", "-t", fs, device, mp])
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

    mlog("SUDO", &format!(
        "sudo mount -t {} {} {} → exit:{} stdout:'{}' stderr:'{}'",
        fs, device, mp,
        out.status.code().unwrap_or(-1),
        stdout.trim(), stderr.trim()
    ));

    if is_wrong_password(&stderr) { Err("WRONG_PASSWORD".into()) }
    else { Ok(out.status.success()) }
}

// ---- helpers ----

fn run_cmd(args: &[&str]) -> Result<String, String> {
    let out = Command::new(args[0])
        .args(&args[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{}: {}", args[0], e))?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() { Ok(combined) } else { Err(combined) }
}

fn is_hibernation(s: &str) -> bool {
    s.contains("hiberfil") || s.contains("hiberfile") || s.contains("hibernat")
}

fn is_wrong_password(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("incorrect password")
        || l.contains("authentication failure")
        || l.contains("sorry, try again")
        || l.contains("wrong password")
}

fn sanitize_dev(device: &str) -> String {
    device.trim_start_matches("/dev/").replace('/', "_")
}
