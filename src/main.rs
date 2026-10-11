// src/main.rs
// Arch Explorer v2 — egui native, webkit'siz, AEX theming

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;
mod state;
mod ui;
mod backend;
pub mod font_loader;

fn get_ipc_socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| dirs::runtime_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")));
    runtime_dir.join("arch-explorer-ipc.sock")
}

fn try_forward_to_existing_instance(target_path: &str) -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let sock_path = get_ipc_socket_path();
    if !sock_path.exists() {
        return false;
    }

    if let Ok(mut stream) = UnixStream::connect(&sock_path) {
        if writeln!(stream, "{}", target_path).is_ok() {
            log::info!("Mevcut Arch Explorer sürecine yol iletildi: {}", target_path);
            return true;
        }
    } else {
        // Stale socket file
        let _ = std::fs::remove_file(&sock_path);
    }
    false
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Warn)
        .init();

    // --new-tab parametresi verilmedikçe her başlatma bağımsız bir yeni pencere açar
    let args: Vec<String> = std::env::args().collect();
    let force_new_tab = args.iter().any(|a| a == "--new-tab");
    let target_path = args.iter().skip(1).find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/".into())
    });

    if force_new_tab && try_forward_to_existing_instance(&target_path) {
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Arch Explorer")
            .with_inner_size([1200.0, 750.0])
            .with_min_inner_size([600.0, 400.0])
            .with_transparent(true)          // Hyprland blur için şeffaflık
            .with_decorations(false)         // custom titlebar
            .with_app_id("arch-explorer"),   // Hyprland windowrulev2 için

        renderer: eframe::Renderer::Wgpu,    // Wayland'da wgpu şart

        // Persistent egui state (panel boyutları, scroll pozisyonları vs)
        persist_window: true,

        ..Default::default()
    };

    eframe::run_native(
        "Arch Explorer",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ArchExplorer::new(cc)))),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
