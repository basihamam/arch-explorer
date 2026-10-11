#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn detect_system_cursor_size() -> u32 {
    // 1. Ortam değişkeninde zaten varsa
    if let Ok(val) = env::var("XCURSOR_SIZE") {
        if let Ok(size) = val.parse::<u32>() {
            return size;
        }
    }

    // 2. GTK 3.0 yapılandırmasından oku
    if let Some(home) = env::var_os("HOME") {
        let gtk_config = PathBuf::from(home).join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = fs::read_to_string(gtk_config) {
            for line in content.lines() {
                if line.trim().starts_with("gtk-cursor-theme-size") {
                    if let Some(val) = line.split('=').nth(1) {
                        if let Ok(size) = val.trim().parse::<u32>() {
                            return size;
                        }
                    }
                }
            }
        }
    }

    // 3. GNOME / KDE / Cinnamon okuma dene
    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "cursor-size"])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            if let Ok(size) = text.trim().parse::<u32>() {
                return size;
            }
        }
    }

    24
}

fn detect_system_cursor_theme() -> String {
    if let Ok(theme) = env::var("XCURSOR_THEME") {
        return theme;
    }

    if let Some(home) = env::var_os("HOME") {
        let gtk_config = PathBuf::from(home).join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = fs::read_to_string(gtk_config) {
            for line in content.lines() {
                if line.trim().starts_with("gtk-cursor-theme-name") {
                    if let Some(val) = line.split('=').nth(1) {
                        return val.trim().replace('"', "");
                    }
                }
            }
        }
    }

    "Adwaita".to_string()
}

fn main() {
    // 1. WebKitGTK grafik şeffaflık hatasını engelle
    env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    // 2. Sistem imleç boyutu ve temasını tespit et
    let cursor_size = detect_system_cursor_size();
    let cursor_theme = detect_system_cursor_theme();

    // 3. GTK ve X11/Wayland için çift ölçeklemeyi engelleyen tüm bayrakları ayarla
    env::set_var("XCURSOR_SIZE", cursor_size.to_string());
    env::set_var("GDK_CURSOR_SIZE", cursor_size.to_string());
    env::set_var("XCURSOR_THEME", &cursor_theme);

    // Wayland/HiDPI üzerinde çift ölçeklemeyi engellemek için varsayılan ölçek 1 değilse sabitle
    if env::var("GDK_SCALE").is_err() {
        env::set_var("GDK_SCALE", "1");
    }
    std::env::set_var("XCURSOR_PATH", "/usr/share/icons:~/.local/share/icons:~/.icons");
    std::env::set_var("XCURSOR_SIZE", "24");
    std::env::set_var("GDK_CURSOR_SIZE", "24");
    // 4. Tauri uygulamasını başlat
    arch_explorer_lib::run();
}