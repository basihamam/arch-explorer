// archive.rs — zip/tar.gz/tar.bz2/7z extract + compress
// extract: hedef dizin = arşiv yanı (veya "here" subdirectory)
// compress: seçili dosyalar → hedef arşiv

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub success: bool,
    pub output_path: String,
    pub message: String,
}

/// Arşiv tipini uzantıdan tahmin et
fn archive_kind(path: &Path) -> &'static str {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") { return "tar.gz"; }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") { return "tar.bz2"; }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") { return "tar.xz"; }
    if name.ends_with(".tar.zst") { return "tar.zst"; }
    if name.ends_with(".tar") { return "tar"; }
    if name.ends_with(".zip") { return "zip"; }
    if name.ends_with(".7z") { return "7z"; }
    if name.ends_with(".rar") { return "rar"; }
    "unknown"
}

/// Çakışma varsa yeni isim üret — "foo" → "foo (2)" → "foo (3)" …
fn unique_path(base: &Path) -> PathBuf {
    if !base.exists() { return base.to_path_buf(); }
    let stem = base.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext  = base.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dir  = base.parent().unwrap_or(Path::new("."));
    for i in 2..=999 {
        let candidate = dir.join(format!("{} ({}){}", stem, i, ext));
        if !candidate.exists() { return candidate; }
    }
    dir.join(format!("{}_copy{}", stem, ext))
}

/// İsimdeki sondaki kopya/indirme eklerini (örn: "dosya (1)", "dosya(1)", "dosya (2)") temizle
fn clean_copy_suffix(name: &str) -> String {
    let trimmed = name.trim();
    if let Some(open_idx) = trimmed.rfind('(') {
        if let Some(close_idx) = trimmed.rfind(')') {
            if close_idx == trimmed.len() - 1 && open_idx < close_idx {
                let inside = &trimmed[open_idx + 1..close_idx];
                if inside.chars().all(|c| c.is_ascii_digit()) {
                    let base = trimmed[..open_idx].trim_end();
                    if !base.is_empty() {
                        return base.to_string();
                    }
                }
            }
        }
    }
    trimmed.to_string()
}

/// Eğer çıkarılan arşiv içinde tek bir klasör varsa (ör. dest/dest/...), gereksiz çift klasörü düzleştir
fn flatten_single_root_dir(dest: &Path) {
    let entries = match std::fs::read_dir(dest) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect::<Vec<_>>(),
        Err(_) => return,
    };

    if entries.len() == 1 && entries[0].path().is_dir() {
        let single_child = entries[0].path();
        let tmp_parent = dest.parent().unwrap_or(dest);
        let tmp_move = tmp_parent.join(format!(".extract_tmp_{}", std::process::id()));

        // İçteki klasörü geçici yere taşı, sonra içeriğini dest'e koy
        if std::fs::rename(&single_child, &tmp_move).is_ok() {
            if let Ok(inner_entries) = std::fs::read_dir(&tmp_move) {
                for item in inner_entries.filter_map(|e| e.ok()) {
                    let target = dest.join(item.file_name());
                    let _ = std::fs::rename(item.path(), target);
                }
            }
            let _ = std::fs::remove_dir_all(&tmp_move);
        }
    }
}

/// Arşivi çıkar — "Buraya çıkar" (arşiv yanındaki dizin)
#[tauri::command]
pub fn extract_archive(archive_path: String) -> Result<ArchiveResult, String> {
    let src  = PathBuf::from(&archive_path);
    if !src.exists() {
        return Err(format!("Arşiv bulunamadı: {}", archive_path));
    }

    let parent  = src.parent().unwrap_or(Path::new("."));
    let kind    = archive_kind(&src);

    // Hedef klasör: arşiv ismi (uzantısız)
    let raw_name = src.file_name()
        .map(|n| {
            let n = n.to_string_lossy();
            // çift uzantıları temizle (.tar.gz, .tar.bz2)
            for suf in &[".tar.gz",".tgz",".tar.bz2",".tbz2",".tar.xz",".txz",".tar.zst",".tar",".zip",".7z",".rar"] {
                if let Some(stem) = n.strip_suffix(suf) { return stem.to_string(); }
            }
            // tekli uzantı
            Path::new(n.as_ref())
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| n.to_string())
        })
        .unwrap_or_else(|| "extracted".to_string());

    // Eğer isimde "(1)" gibi indirme/kopyalama eki varsa ve temiz isim hedef dizinde yoksa temiz ismi kullan!
    let cleaned = clean_copy_suffix(&raw_name);
    let chosen_folder_name = if !parent.join(&cleaned).exists() {
        cleaned
    } else {
        raw_name
    };

    let dest = unique_path(&parent.join(&chosen_folder_name));
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let dest_str = dest.to_string_lossy().to_string();

    let result = match kind {
        "tar.gz" | "tgz" | "tar.bz2" | "tbz2" | "tar.xz" | "txz" | "tar.zst" | "tar" => {
            Command::new("tar")
                .args(["xf", &archive_path, "-C", &dest_str])
                .output()
        }
        "zip" => {
            Command::new("unzip")
                .args(["-q", &archive_path, "-d", &dest_str])
                .output()
        }
        "7z" => {
            Command::new("7z")
                .args(["x", &archive_path, &format!("-o{}", dest_str), "-y"])
                .output()
        }
        "rar" => {
            // RAR için önce 7z (RAR5 dahil evrensel ve hatasız), sonra unrar trailing slash ile
            let res_7z = Command::new("7z")
                .args(["x", &archive_path, &format!("-o{}", dest_str), "-y"])
                .output();

            match res_7z {
                Ok(out) if out.status.success() => Ok(out),
                _ => {
                    // 7z yoksa veya hata verdiyse unrar ile dene (UNRAR için hedef yol / ile bitmeli!)
                    let unrar_dest = if dest_str.ends_with('/') { dest_str.clone() } else { format!("{}/", dest_str) };
                    Command::new("unrar")
                        .args(["x", "-y", &archive_path, &unrar_dest])
                        .output()
                }
            }
        }
        _ => {
            // Bilinmeyen — 7z ile dene (evrensel)
            Command::new("7z")
                .args(["x", &archive_path, &format!("-o{}", dest_str), "-y"])
                .output()
        }
    };

    match result {
        Ok(out) if out.status.success() => {
            // Eğer arşiv içinde sadece 1 klasör açıldıysa çift iç içe klasörü düzleştir
            flatten_single_root_dir(&dest);

            let final_name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(chosen_folder_name);
            Ok(ArchiveResult {
                success: true,
                output_path: dest_str,
                message: format!("Çıkarıldı: {}", final_name),
            })
        }
        Ok(out) => {
            let _ = std::fs::remove_dir_all(&dest);
            Err(format!(
                "Çıkarma başarısız: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dest);
            Err(format!("Komut başlatılamadı: {}", e))
        }
    }
}

/// Seçili dosyaları sıkıştır
/// format: "zip" | "tar.gz" | "tar.bz2" | "tar.xz" | "7z"
#[tauri::command]
pub fn compress_files(
    paths: Vec<String>,
    dest_dir: String,
    archive_name: String,  // uzantısız isim
    format: String,        // "zip" | "tar.gz" | "tar.bz2" | "tar.xz" | "7z"
) -> Result<ArchiveResult, String> {
    if paths.is_empty() {
        return Err("Sıkıştırılacak dosya seçilmedi".to_string());
    }

    let dir = PathBuf::from(&dest_dir);
    if !dir.is_dir() {
        return Err(format!("Hedef dizin bulunamadı: {}", dest_dir));
    }

    // Arşiv dosya yolu
    let ext = match format.as_str() {
        "zip"    => "zip",
        "tar.gz" => "tar.gz",
        "tar.bz2"=> "tar.bz2",
        "tar.xz" => "tar.xz",
        "7z"     => "7z",
        _        => "zip",
    };
    let archive_path = unique_path(&dir.join(format!("{}.{}", archive_name, ext)));
    let archive_str  = archive_path.to_string_lossy().to_string();

    // File args
    let file_args: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

    let result = match format.as_str() {
        "zip" => {
            Command::new("zip")
                .arg("-r")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
        "tar.gz" => {
            Command::new("tar")
                .arg("czf")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
        "tar.bz2" => {
            Command::new("tar")
                .arg("cjf")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
        "tar.xz" => {
            Command::new("tar")
                .arg("cJf")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
        "7z" => {
            Command::new("7z")
                .arg("a")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
        _ => {
            Command::new("zip")
                .arg("-r")
                .arg(&archive_str)
                .args(&file_args)
                .output()
        }
    };

    match result {
        Ok(out) if out.status.success() => Ok(ArchiveResult {
            success: true,
            output_path: archive_str,
            message: format!("Arşiv oluşturuldu: {}.{}", archive_name, ext),
        }),
        Ok(out) => Err(format!(
            "Sıkıştırma başarısız: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => Err(format!("Komut başlatılamadı: {}", e)),
    }
}

/// Arşiv mi? (context menu'da "Extract" göstermek için)
#[tauri::command]
pub fn is_archive(path: String) -> bool {
    archive_kind(Path::new(&path)) != "unknown"
}

/// MD5 + SHA256 checksum
#[tauri::command]
pub fn get_checksum(path: String) -> Result<serde_json::Value, String> {
    let p = PathBuf::from(&path);
    if !p.exists() || !p.is_file() {
        return Err(format!("Dosya bulunamadı: {}", path));
    }

    let md5 = run_hash("md5sum", &path);
    let sha256 = run_hash("sha256sum", &path);

    Ok(serde_json::json!({
        "md5":    md5,
        "sha256": sha256,
        "path":   path,
    }))
}

fn run_hash(tool: &str, path: &str) -> String {
    Command::new(tool)
        .arg(path)
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o) } else { None })
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.split_whitespace().next().map(|h| h.to_string())
        })
        .unwrap_or_else(|| format!("{} bulunamadı", tool))
}
