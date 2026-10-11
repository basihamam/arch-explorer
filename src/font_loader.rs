// src/font_loader.rs
// Sistem Nerd Font'larını tarayıp egui'ye yükler
// Yoksa built-in font + ASCII fallback icon'ları kullanır

use egui::{FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;

/// Nerd Font arama yolları (Arch Linux + genel)
fn nerd_font_paths() -> Vec<PathBuf> {
    let mut paths = vec![];

    // Sistem font dizinleri
    let sys_dirs = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/usr/share/fonts/nerd-fonts-complete",
        "/usr/share/fonts/TTF",
        "/usr/share/fonts/truetype",
    ];

    for dir in &sys_dirs {
        // JetBrainsMono Nerd Font & Symbols Nerd Font
        for name in &[
            "JetBrainsMonoNerdFont-Regular.ttf",
            "JetBrainsMono Nerd Font Regular.ttf",
            "JetBrainsMonoNLNerdFont-Regular.ttf",
            "JetBrainsMono-Regular.ttf",
            "SymbolsNerdFont-Regular.ttf",
            "SymbolsNerdFontMono-Regular.ttf",
        ] {
            let p = PathBuf::from(dir).join(name);
            if p.exists() { paths.push(p); }
        }

        // Diğer nerd font'lar (öncelik sırasıyla)
        if let Ok(read) = std::fs::read_dir(dir) {
            for entry in read.flatten() {
                let path = entry.path();
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if name.contains("nerd") && name.ends_with(".ttf")
                    && (name.contains("regular") || name.contains("mono"))
                {
                    paths.push(path);
                }
            }
        }
    }

    // ~/.local/share/fonts (kullanıcı kurulumu)
    if let Some(home) = dirs::home_dir() {
        let user_fonts = home.join(".local/share/fonts");
        if let Ok(read) = std::fs::read_dir(&user_fonts) {
            for entry in read.flatten() {
                let path = entry.path();
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if name.contains("nerd") && name.ends_with(".ttf") {
                    paths.push(path);
                }
            }
        }
    }

    paths
}

fn ui_font_paths() -> Vec<PathBuf> {
    let mut paths = vec![];
    let candidates = [
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            paths.push(p);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let user_fonts = home.join(".local/share/fonts");
        for name in &["NotoSans-Regular.ttf", "Inter-Regular.ttf", "DejaVuSans.ttf"] {
            let p = user_fonts.join(name);
            if p.exists() {
                paths.push(p);
            }
        }
    }
    paths
}

/// egui'ye font yükle
/// UI Font bulunursa: Proportional family'ye birincil olarak eklenir (net, bulanık olmayan yazılar)
/// Nerd Font bulunursa: Monospace family'ye ve proportional fallback'e eklenir (ikonlar)
pub fn load_fonts(ctx: &egui::Context) -> bool {
    let mut fonts = FontDefinitions::default();
    let paths = nerd_font_paths();
    let ui_paths = ui_font_paths();

    if paths.is_empty() && ui_paths.is_empty() {
        log::warn!(
            "Font bulunamadı! İkonlar eksik görünebilir.\n\
             Kur: sudo pacman -S ttf-jetbrains-mono-nerd noto-fonts\n\
             Sonra uygulamayı yeniden başlat."
        );
        ctx.set_fonts(fonts);
        return false;
    }

    let mut loaded_any = false;

    // 1. Birincil UI Fontu (Net ve yüksek çözünürlüklü arayüz metinleri için)
    if let Some(ui_path) = ui_paths.first() {
        match std::fs::read(ui_path) {
            Ok(data) => {
                fonts.font_data.insert(
                    "UIFont".to_owned(),
                    FontData::from_owned(data),
                );
                fonts.families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, "UIFont".to_owned());
                log::info!("UI Font yüklendi (Proportional): {}", ui_path.display());
                loaded_any = true;
            }
            Err(e) => {
                log::warn!("UI Font okunamadı {}: {}", ui_path.display(), e);
            }
        }
    }

    // 2. Nerd Font (İkonlar ve Monospace kod/terminal için)
    if let Some(font_path) = paths.first() {
        match std::fs::read(font_path) {
            Ok(data) => {
                fonts.font_data.insert(
                    "NerdFont".to_owned(),
                    FontData::from_owned(data),
                );

                // Monospace family'ye en başa ekle (kod & ikonlar)
                fonts.families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .insert(0, "NerdFont".to_owned());

                // Eğer UI font yoksa, Proportional'ın da başına NerdFont koy
                // Varsa, fallback olarak 2. sıraya koy ki metin içi ikonlar da açılsın
                let prop = fonts.families.entry(FontFamily::Proportional).or_default();
                if prop.is_empty() || prop[0] != "UIFont" {
                    prop.insert(0, "NerdFont".to_owned());
                } else {
                    prop.insert(1, "NerdFont".to_owned());
                }

                log::info!("Nerd Font yüklendi (Monospace & Icons): {}", font_path.display());
                loaded_any = true;
            }
            Err(e) => {
                log::error!("Nerd Font okunamadı {}: {}", font_path.display(), e);
            }
        }
    }

    ctx.set_fonts(fonts);
    loaded_any
}

/// Kullanıcıya gösterilecek font durumu mesajı
pub fn font_status_msg() -> Option<String> {
    if nerd_font_paths().is_empty() {
        Some("sudo pacman -S ttf-jetbrains-mono-nerd".to_string())
    } else {
        None
    }
}
