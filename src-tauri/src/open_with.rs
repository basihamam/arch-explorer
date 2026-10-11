// open_with.rs — categorized app launcher with icon resolution + custom commands

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use once_cell::sync::Lazy;
use std::sync::Mutex;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppEntry {
    pub id:          String,
    pub name:        String,
    pub exec:        String,
    pub icon:        Option<String>,
    pub icon_path:   Option<String>,  // resolved fs path for frontend convertFileSrc
    pub comment:     Option<String>,
    pub categories:  Vec<String>,
    pub is_custom:   bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenWithInfo {
    pub mime_type:   String,
    pub default_app: Option<AppEntry>,
    pub recommended: Vec<AppEntry>,
    pub others:      Vec<AppEntry>,
    pub custom:      Vec<AppEntry>,
}

// ── Mime index cache — built once, lives for app session ──────────────────────
static MIME_INDEX: Lazy<Mutex<Option<HashMap<String, Vec<AppEntry>>>>> =
    Lazy::new(|| Mutex::new(None));

static ALL_APPS_CACHE: Lazy<Mutex<Option<Vec<AppEntry>>>> =
    Lazy::new(|| Mutex::new(None));

static ICON_CACHE: Lazy<Mutex<HashMap<String, Option<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn get_mime_index() -> HashMap<String, Vec<AppEntry>> {
    let mut guard = MIME_INDEX.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(build_mime_index());
    }
    guard.as_ref().unwrap().clone()
}

fn get_cached_all_apps() -> Vec<AppEntry> {
    let mut guard = ALL_APPS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(build_all_desktop_apps());
    }
    guard.as_ref().unwrap().clone()
}

// ── Icon resolution ───────────────────────────────────────────────────────────

fn get_gtk_icon_theme() -> String {
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout)
            .trim()
            .trim_matches('\'')
            .to_string();
        if !s.is_empty() && s != "@ms ''" {
            return s;
        }
    }
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = fs::read_to_string(cfg) {
            for line in content.lines() {
                if line.trim().starts_with("gtk-icon-theme-name") {
                    if let Some(v) = line.split('=').nth(1) {
                        return v.trim().to_string();
                    }
                }
            }
        }
    }
    "hicolor".to_string()
}

fn resolve_icon_path(name: &str) -> Option<String> {
    if name.is_empty() { return None; }

    {
        let guard = ICON_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = guard.get(name) {
            return cached.clone();
        }
    }

    let resolved = resolve_icon_path_uncached(name);
    {
        let mut guard = ICON_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(name.to_string(), resolved.clone());
    }
    resolved
}

fn resolve_icon_path_uncached(name: &str) -> Option<String> {
    // Already absolute path
    if name.starts_with('/') {
        let p = Path::new(name);
        if p.exists() { return Some(name.to_string()); }
        // might be without extension
        for ext in &["png", "svg", "xpm"] {
            let with_ext = format!("{}.{}", name, ext);
            if Path::new(&with_ext).exists() { return Some(with_ext); }
        }
    }

    let theme = get_gtk_icon_theme();
    let home  = dirs::home_dir();
    let sizes = ["48x48", "32x32", "64x64", "128x128", "scalable", "22x22", "16x16"];
    let exts  = ["png", "svg", "xpm"];

    let mut search_roots: Vec<PathBuf> = vec![];
    if let Some(ref h) = home {
        search_roots.push(h.join(".local/share/icons"));
        search_roots.push(h.join(".icons"));
    }
    search_roots.push(PathBuf::from("/usr/share/icons"));
    search_roots.push(PathBuf::from("/usr/local/share/icons"));

    let themes = [theme.as_str(), "hicolor", "Adwaita", "breeze"];

    for root in &search_roots {
        for t in &themes {
            for size in &sizes {
                for ext in &exts {
                    let p = root.join(t).join(size).join("apps")
                        .join(format!("{}.{}", name, ext));
                    if p.exists() { return Some(p.to_string_lossy().to_string()); }
                }
            }
        }
    }

    // pixmaps fallback
    for ext in &exts {
        let p = PathBuf::from(format!("/usr/share/pixmaps/{}.{}", name, ext));
        if p.exists() { return Some(p.to_string_lossy().to_string()); }
        let p2 = PathBuf::from(format!("/usr/share/pixmaps/{}", name));
        if p2.exists() { return Some(p2.to_string_lossy().to_string()); }
    }

    None
}

// ── .desktop parser ───────────────────────────────────────────────────────────

fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.insert(0, home.join(".local/share/applications"));
    }
    dirs
}

fn parse_desktop_file(path: &Path) -> Option<(AppEntry, Vec<String>)> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_entry   = false;
    let mut name       = String::new();
    let mut exec       = String::new();
    let mut icon: Option<String>    = None;
    let mut comment: Option<String> = None;
    let mut categories  = vec![];
    let mut mime_types  = vec![];
    let mut no_display  = false;
    let mut hidden      = false;
    let mut entry_type  = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" { in_entry = true; continue; }
        if line.starts_with('[') && line != "[Desktop Entry]" {
            if in_entry { break; }
            continue;
        }
        if !in_entry || line.starts_with('#') || line.is_empty() { continue; }

        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Name"       => { if name.is_empty()  { name  = v.trim().to_string(); } }
                "Exec"       => { if exec.is_empty()  { exec  = v.trim().to_string(); } }
                "Type"       => { entry_type = v.trim().to_string(); }
                "Icon"       => { icon    = Some(v.trim().to_string()); }
                "Comment"    => { comment = Some(v.trim().to_string()); }
                "Categories" => {
                    categories = v.trim().split(';')
                        .filter(|s| !s.is_empty()).map(String::from).collect();
                }
                "MimeType"   => {
                    mime_types = v.trim().split(';')
                        .filter(|s| !s.is_empty()).map(String::from).collect();
                }
                "NoDisplay"  => { no_display = v.trim().eq_ignore_ascii_case("true"); }
                "Hidden"     => { hidden     = v.trim().eq_ignore_ascii_case("true"); }
                _ => {}
            }
        }
    }

    if name.is_empty() || exec.is_empty() { return None; }
    if entry_type != "Application" && !entry_type.is_empty() { return None; }
    if no_display || hidden { return None; }

    let icon_path = icon.as_deref().and_then(resolve_icon_path);
    let id = path.file_name()?.to_string_lossy().to_string();

    Some((
        AppEntry { id, name, exec, icon, icon_path, comment, categories, is_custom: false },
        mime_types,
    ))
}

fn build_mime_index() -> HashMap<String, Vec<AppEntry>> {
    let mut index: HashMap<String, Vec<AppEntry>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();

    for dir in app_dirs() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") { continue; }
            let Some((app, mimes)) = parse_desktop_file(&path) else { continue };
            if seen.contains(&app.id) { continue; }
            seen.insert(app.id.clone());
            for mime in mimes {
                index.entry(mime).or_default().push(app.clone());
            }
        }
    }
    index
}

// ── All installed desktop apps ────────────────────────────────────────────────
fn build_all_desktop_apps() -> Vec<AppEntry> {
    let mut apps = vec![];
    let mut seen = std::collections::HashSet::new();
    for dir in app_dirs() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") { continue; }
            let Some((app, _)) = parse_desktop_file(&path) else { continue };
            if seen.insert(app.id.clone()) { apps.push(app); }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps
}

// ── Custom commands as AppEntry ───────────────────────────────────────────────

fn build_custom_entries() -> Vec<AppEntry> {
    crate::commands::load_user_commands()
        .into_iter()
        .map(|cmd| {
            AppEntry {
                id:         format!("custom::{}", cmd.id),
                name:       cmd.name,
                exec:       cmd.exec,
                icon:       Some(cmd.icon.clone()),
                icon_path:  None,
                comment:    Some(cmd.description),
                categories: vec!["Custom".to_string()],
                is_custom:  true,
            }
        })
        .collect()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_mime(path: &str) -> Result<String, String> {
    if let Ok(out) = Command::new("xdg-mime").args(["query", "filetype", path]).output() {
        let mime = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !mime.is_empty() { return Ok(mime); }
    }
    let guessed = mime_guess::from_path(path).first_or_octet_stream().to_string();
    Ok(guessed)
}

fn find_app_by_id(id: &str) -> Option<AppEntry> {
    for dir in app_dirs() {
        let p = dir.join(id);
        if p.exists() {
            if let Some((app, _)) = parse_desktop_file(&p) {
                return Some(app);
            }
        }
        if !id.ends_with(".desktop") {
            let p2 = dir.join(format!("{}.desktop", id));
            if p2.exists() {
                if let Some((app, _)) = parse_desktop_file(&p2) {
                    return Some(app);
                }
            }
        }
    }
    None
}

fn get_default(mime: &str) -> Option<String> {
    // 1. Try xdg-mime query default
    if let Ok(out) = Command::new("xdg-mime").args(["query", "default", mime]).output() {
        let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !id.is_empty() { return Some(id); }
    }
    // 2. Try gio mime query
    if let Ok(out) = Command::new("gio").args(["mime", mime]).output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("Default application for") {
                if let Some((_, def)) = line.split_once(':') {
                    let d = def.trim().to_string();
                    if !d.is_empty() { return Some(d); }
                }
            }
        }
    }
    // 3. Try mimeapps.list
    let mut config_paths = vec![
        PathBuf::from("/etc/xdg/mimeapps.list"),
        PathBuf::from("/usr/share/applications/mimeapps.list"),
    ];
    if let Some(h) = dirs::home_dir() {
        config_paths.insert(0, h.join(".config/mimeapps.list"));
        config_paths.insert(1, h.join(".local/share/applications/mimeapps.list"));
    }
    for cfg in config_paths {
        if let Ok(content) = fs::read_to_string(&cfg) {
            let mut in_def = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "[Default Applications]" { in_def = true; continue; }
                if line.starts_with('[') { in_def = false; continue; }
                if in_def && line.starts_with(mime) && line.contains('=') {
                    if let Some((_, val)) = line.split_once('=') {
                        let first = val.split(';').next().unwrap_or("").trim();
                        if !first.is_empty() { return Some(first.to_string()); }
                    }
                }
            }
        }
    }
    None
}

fn clean_exec(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' { chars.next(); } else { out.push(c); }
    }
    out.trim().to_string()
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_open_with_info(path: String) -> Result<OpenWithInfo, String> {
    let mime       = detect_mime(&path)?;
    let index      = get_mime_index();
    let default_id = get_default(&mime);

    // 1. Apps from .desktop MimeType index
    let mut mime_apps = index.get(&mime).cloned().unwrap_or_default();
    let mut seen_ids: std::collections::HashSet<String> = mime_apps.iter().map(|a| a.id.clone()).collect();

    // 2. Also query gio mime to catch inherited / registered / recommended apps
    if let Ok(out) = Command::new("gio").args(["mime", &mime]).output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let line = line.trim();
            if line.ends_with(".desktop") && !line.contains(':') {
                if !seen_ids.contains(line) {
                    if let Some(app) = find_app_by_id(line) {
                        seen_ids.insert(app.id.clone());
                        mime_apps.push(app);
                    }
                }
            }
        }
    }

    // 3. Also check mimeinfo.cache
    let cache_paths = [
        PathBuf::from("/usr/share/applications/mimeinfo.cache"),
        PathBuf::from("/usr/local/share/applications/mimeinfo.cache"),
    ];
    for cp in &cache_paths {
        if let Ok(content) = fs::read_to_string(cp) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with(&format!("{}=", mime)) {
                    if let Some((_, list)) = line.split_once('=') {
                        for item in list.split(';') {
                            let item = item.trim();
                            if item.ends_with(".desktop") && !seen_ids.contains(item) {
                                if let Some(app) = find_app_by_id(item) {
                                    seen_ids.insert(app.id.clone());
                                    mime_apps.push(app);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let custom = build_custom_entries();

    // Resolve default app
    let mut default_app: Option<AppEntry> = None;
    if let Some(ref def_id) = default_id {
        // First try to find in mime_apps
        if let Some(pos) = mime_apps.iter().position(|a| a.id == *def_id || a.id == format!("{}.desktop", def_id)) {
            default_app = Some(mime_apps.remove(pos));
        } else if let Some(app) = find_app_by_id(def_id) {
            default_app = Some(app);
        }
    }

    // All remaining mime_apps are recommended
    let recommended = mime_apps;

    let rec_ids: std::collections::HashSet<String> = recommended.iter().map(|a| a.id.clone()).collect();
    let def_id_val = default_app.as_ref().map(|a| a.id.clone());
    let others = get_cached_all_apps()
        .into_iter()
        .filter(|a| !rec_ids.contains(&a.id) && def_id_val.as_ref() != Some(&a.id))
        .collect::<Vec<_>>();

    Ok(OpenWithInfo { mime_type: mime, default_app, recommended, others, custom })
}

#[tauri::command]
pub fn get_mime_type(path: String) -> Result<String, String> {
    detect_mime(&path)
}

#[tauri::command]
pub fn open_with_app(path: String, desktop_id: String) -> Result<(), String> {
    // custom command
    if let Some(cmd_id) = desktop_id.strip_prefix("custom::") {
        let cmds = crate::commands::load_user_commands();
        if let Some(cmd) = cmds.iter().find(|c| c.id == cmd_id) {
            let p   = PathBuf::from(&path);
            let dir = if p.is_dir() { path.clone() }
                      else { p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or(path.clone()) };
            let exec = cmd.exec
                .replace("{path}", &path)
                .replace("{file}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default().as_str())
                .replace("{dir}", &dir)
                .replace("{name}", p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default().as_str());
            std::process::Command::new("sh").args(["-c", &exec]).spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }

    // normal desktop app
    std::process::Command::new("gtk-launch")
        .arg(&desktop_id).arg(&path)
        .spawn()
        .map_err(|e| format!("gtk-launch failed: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn set_default_app(mime_type: String, desktop_id: String) -> Result<(), String> {
    Command::new("xdg-mime")
        .args(["default", &desktop_id, &mime_type])
        .status()
        .map_err(|e| format!("xdg-mime default failed: {e}"))?;
    // invalidate index so next call reflects new default
    let mut guard = MIME_INDEX.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
    Ok(())
}

#[tauri::command]
pub fn reset_default_for_mime(mime_type: String) -> Result<(), String> {
    let mimeapps = dirs::config_dir()
        .ok_or("no config dir")?
        .join("mimeapps.list");
    let content = fs::read_to_string(&mimeapps).unwrap_or_default();
    let filtered: Vec<&str> = content.lines()
        .filter(|l| !l.trim().starts_with(&format!("{}=", mime_type)))
        .collect();
    fs::write(&mimeapps, filtered.join("\n") + "\n")
        .map_err(|e| format!("Failed to write mimeapps.list: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn open_with_custom_cmd(path: String, custom_cmd: String) -> Result<(), String> {
    let exec = custom_cmd.trim();
    if exec.is_empty() { return Err("Komut boş olamaz".into()); }
    let full = if exec.contains("%f") || exec.contains("%F") {
        exec.replace("%f", &path).replace("%F", &path)
    } else {
        format!("{} \"{}\"", exec, path)
    };
    std::process::Command::new("sh").args(["-c", &full]).spawn()
        .map_err(|e| format!("Uygulama başlatılamadı: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn add_app_association(mime_type: String, desktop_id: String) -> Result<(), String> {
    set_default_app(mime_type, desktop_id)
}

#[tauri::command]
pub fn get_all_apps() -> Vec<AppEntry> {
    let index = get_mime_index();
    let mut seen = std::collections::HashSet::new();
    let mut apps = vec![];
    for list in index.values() {
        for app in list {
            if seen.insert(app.id.clone()) {
                apps.push(app.clone());
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps
}

#[tauri::command]
pub fn open_file_default(path: String) -> Result<bool, String> {
    let target_path = if path.starts_with("adb://") {
        crate::adb::adb_cache_file(&path)?.to_string_lossy().to_string()
    } else {
        path
    };

    let p = Path::new(&target_path);
    if !p.exists() {
        return Err("Dosya bulunamadı".into());
    }

    let mime = detect_mime(&target_path).unwrap_or_else(|_| "application/octet-stream".to_string());
    let default_id = get_default(&mime);

    // 1. xdg-mime üzerinde tanımlı varsayılan bir .desktop uygulaması var mı?
    if let Some(id) = default_id {
        if !id.is_empty() {
            // gtk-launch ile doğrudan aç
            if let Ok(_child) = Command::new("gtk-launch").arg(&id).arg(&target_path).spawn() {
                // Hata vermeden başladıysa başarılı
                return Ok(true);
            }
        }
    }

    // 2. gio open veya xdg-open dene
    if let Ok(status) = Command::new("gio").args(["open", &target_path]).status() {
        if status.success() {
            return Ok(true);
        }
    }

    if let Ok(status) = Command::new("xdg-open").arg(&target_path).status() {
        if status.success() {
            return Ok(true);
        }
    }

    // Varsayılan uygulama bulunamadı veya açılamadı -> frontend OpenWithPanel açacak
    Ok(false)
}
