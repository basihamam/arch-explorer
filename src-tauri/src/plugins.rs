// plugins.rs — Shell-based plugin sistemi
// Plugin dizini: ~/.config/arch-explorer/plugins/
// Her plugin: dizin içinde plugin.toml + çalıştırılabilir script (run.sh, run.py, vb.)
// Context env vars: ARCH_PATH, ARCH_DIR, ARCH_SELECTED, ARCH_CURRENT_DIR
// Stdout'ta JSON çıktı varsa parse et, yoksa notification olarak göster

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Plugin {
    pub id:          String,
    pub name:        String,
    pub description: String,
    pub icon:        String,
    pub exec:        String,   // script dosya yolu
    pub when:        String,   // "always" | "file" | "dir" | "archive"
    pub shortcut:    Option<String>,
    pub enabled:     bool,
    pub version:     String,
    pub author:      String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginMeta {
    pub name:        String,
    pub description: Option<String>,
    pub icon:        Option<String>,
    pub when:        Option<String>,
    pub shortcut:    Option<String>,
    pub version:     Option<String>,
    pub author:      Option<String>,
    pub exec:        Option<String>, // override exec script adı
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResult {
    pub success: bool,
    pub output:  String,
    pub action:  Option<PluginAction>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginAction {
    Notify { message: String, level: String },
    OpenFile { path: String },
    NavigateTo { path: String },
    Reload,
}

fn plugins_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("plugins")
}

fn enabled_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("plugins_enabled.json")
}

fn load_enabled() -> HashMap<String, bool> {
    let p = enabled_path();
    if !p.exists() { return HashMap::new(); }
    let data = fs::read_to_string(&p).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_enabled(map: &HashMap<String, bool>) {
    let p = enabled_path();
    if let Some(parent) = p.parent() { fs::create_dir_all(parent).ok(); }
    if let Ok(data) = serde_json::to_string_pretty(map) {
        fs::write(&p, data).ok();
    }
}

fn find_exec_script(plugin_dir: &Path, meta: &PluginMeta) -> Option<String> {
    // Explicit exec override
    if let Some(exec) = &meta.exec {
        let p = plugin_dir.join(exec);
        if p.exists() { return Some(p.to_string_lossy().to_string()); }
    }

    // Otomatik keşif: run.sh, run.py, run.lua, main.sh, main.py
    for name in &["run.sh", "run.py", "run.lua", "main.sh", "main.py", "plugin.sh"] {
        let p = plugin_dir.join(name);
        if p.exists() { return Some(p.to_string_lossy().to_string()); }
    }
    None
}

#[tauri::command]
pub fn list_plugins() -> Vec<Plugin> {
    let dir = plugins_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).ok();
        return vec![];
    }

    let enabled = load_enabled();
    let mut plugins = vec![];

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e, Err(_) => return vec![],
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let toml_path = path.join("plugin.toml");
        if !toml_path.exists() { continue; }

        let data = match fs::read_to_string(&toml_path) {
            Ok(d) => d, Err(_) => continue,
        };

        let meta: PluginMeta = match toml::from_str(&data) {
            Ok(m) => m, Err(_) => continue,
        };

        let id = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let exec = match find_exec_script(&path, &meta) {
            Some(e) => e, None => continue,
        };

        let is_enabled = enabled.get(&id).copied().unwrap_or(true);

        plugins.push(Plugin {
            id:          id.clone(),
            name:        meta.name,
            description: meta.description.unwrap_or_default(),
            icon:        meta.icon.unwrap_or_else(|| "⚡".to_string()),
            exec,
            when:        meta.when.unwrap_or_else(|| "always".to_string()),
            shortcut:    meta.shortcut,
            enabled:     is_enabled,
            version:     meta.version.unwrap_or_else(|| "1.0.0".to_string()),
            author:      meta.author.unwrap_or_else(|| "unknown".to_string()),
        });
    }

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

#[tauri::command]
pub fn run_plugin(
    plugin_id: String,
    selected_path: Option<String>,
    current_dir: String,
) -> Result<PluginResult, String> {
    let plugins = list_plugins();
    let plugin = plugins.iter()
        .find(|p| p.id == plugin_id)
        .ok_or_else(|| format!("Plugin bulunamadı: {}", plugin_id))?;

    if !plugin.enabled {
        return Err(format!("Plugin devre dışı: {}", plugin.name));
    }

    let exec_path = PathBuf::from(&plugin.exec);
    let selected  = selected_path.clone().unwrap_or_else(|| current_dir.clone());
    let sel_path  = PathBuf::from(&selected);

    let dir = if sel_path.is_dir() {
        selected.clone()
    } else {
        sel_path.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| current_dir.clone())
    };

    // Script yorumlayıcısını belirle
    let (interpreter, args) = detect_interpreter(&exec_path);

    let mut cmd = Command::new(&interpreter);
    cmd.args(&args)
        .args([exec_path.to_string_lossy().as_ref()])
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Context env vars
        .env("ARCH_PATH",        &selected)
        .env("ARCH_DIR",         &dir)
        .env("ARCH_CURRENT_DIR", &current_dir)
        .env("ARCH_SELECTED",    selected_path.as_deref().unwrap_or(""));

    let out = cmd.output().map_err(|e| format!("Script çalıştırılamadı: {}", e))?;

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    // JSON action var mı?
    let action = if let Ok(act) = serde_json::from_str::<PluginAction>(stdout.trim()) {
        Some(act)
    } else {
        None
    };

    let output = if action.is_some() {
        stdout.clone()
    } else {
        format!("{}{}", stdout, stderr).trim().to_string()
    };

    Ok(PluginResult {
        success: out.status.success(),
        output,
        action,
    })
}

fn detect_interpreter(exec: &Path) -> (String, Vec<String>) {
    let ext = exec.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "py"   => ("python3".to_string(), vec![]),
        "lua"  => ("lua".to_string(),     vec![]),
        "rb"   => ("ruby".to_string(),    vec![]),
        "js"   => ("node".to_string(),    vec![]),
        "sh" | "" => {
            // Shebang kontrol et
            if let Ok(content) = fs::read_to_string(exec) {
                let first = content.lines().next().unwrap_or("");
                if first.starts_with("#!") {
                    let shebang = first.trim_start_matches("#!").trim();
                    let parts: Vec<&str> = shebang.split_whitespace().collect();
                    if !parts.is_empty() {
                        return (parts[0].to_string(), parts[1..].iter().map(|s| s.to_string()).collect());
                    }
                }
            }
            ("sh".to_string(), vec![])
        }
        _ => ("sh".to_string(), vec![]),
    }
}

#[tauri::command]
pub fn set_plugin_enabled(plugin_id: String, enabled: bool) -> Result<(), String> {
    let mut map = load_enabled();
    map.insert(plugin_id, enabled);
    save_enabled(&map);
    Ok(())
}

#[tauri::command]
pub fn get_plugins_dir() -> String {
    let dir = plugins_dir();
    fs::create_dir_all(&dir).ok();
    dir.to_string_lossy().to_string()
}

/// Örnek plugin oluştur (yeni başlayanlar için)
#[tauri::command]
pub fn create_example_plugin() -> Result<String, String> {
    let dir = plugins_dir().join("hello-world");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let toml = r#"name = "Hello World"
description = "Örnek plugin — ARCH_PATH'i ekrana yazar"
icon = "👋"
when = "always"
version = "1.0.0"
author = "sen"
"#;

    let script = r#"#!/usr/bin/env bash
# ARCH_PATH: seçili dosya/klasör
# ARCH_DIR: çalışma dizini
# ARCH_CURRENT_DIR: dosya yöneticisinin bulunduğu dizin

echo "Seçili: $ARCH_PATH"
echo "Dizin:  $ARCH_DIR"
"#;

    fs::write(dir.join("plugin.toml"), toml).map_err(|e| e.to_string())?;
    fs::write(dir.join("run.sh"), script).map_err(|e| e.to_string())?;

    // run.sh'yi çalıştırılabilir yap
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir.join("run.sh"))
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.join("run.sh"), perms).map_err(|e| e.to_string())?;
    }

    Ok(dir.to_string_lossy().to_string())
}
