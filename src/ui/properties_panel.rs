// ─────────────────────────────────────────────────────────────────────────────
// src/ui/properties.rs — Dosya/klasör özellikleri paneli
// ─────────────────────────────────────────────────────────────────────────────
use egui::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::{icon, human_size};

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState, path: &str) {
    let c = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let screen = ctx.screen_rect();

    egui::Area::new(Id::new("prop_backdrop")).fixed_pos(screen.min).order(Order::Background).show(ctx, |ui| {
        ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0,0,0,130));
        if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
    });

    let (pw, ph) = (420.0_f32, 480.0_f32);
    let px = screen.center().x - pw / 2.0;
    let py = screen.center().y - ph / 2.0;

    let meta = fs::metadata(path);
    let symlink_meta = fs::symlink_metadata(path);

    egui::Area::new(Id::new("properties_panel"))
        .fixed_pos(Pos2::new(px, py))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(), rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 24.0, spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                inner_margin: Margin::ZERO, ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw);

                // Header
                let filename = std::path::Path::new(path)
                    .file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string());

                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let is_sym = symlink_meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);

                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(if is_dir { icon(t,"folder") } else { icon(t,"file") }).color(c.accent()).size(28.0));
                        ui.add_space(sp.sm);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&filename).color(c.text_primary()).size(t.typography.size_base).strong());
                            ui.label(RichText::new(if is_sym { "Symbolic link" } else if is_dir { "Directory" } else { "File" }).color(c.text_hint()).size(t.typography.size_xs));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("✕").color(c.text_hint()).size(12.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() { state.panels.close(); }
                        });
                    });
                });

                let r = ui.min_rect(); ui.painter().hline(r.left()..=r.right(), r.bottom(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.sm);

                // Properties
                egui::ScrollArea::vertical().max_height(ph - 120.0).show(ui, |ui| {
                    egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.xs), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                        // Location
                        prop_row(ui, t, "Location", path);

                        // Size
                        if let Ok(ref m) = meta {
                            if m.is_dir() {
                                // Klasör boyutunu hesapla (recursive)
                                let size = dir_size(path);
                                prop_row(ui, t, "Size", &format!("{} ({})", human_size(size), count_items(path)));
                            } else {
                                prop_row(ui, t, "Size", &human_size(m.len()));
                            }
                        }

                        // Mime type
                        let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
                        prop_row(ui, t, "Type", &mime);

                        // Symlink target
                        if is_sym {
                            if let Ok(target) = fs::read_link(path) {
                                prop_row(ui, t, "Target", &target.to_string_lossy());
                            }
                        }

                        // Modified / Created
                        if let Ok(ref m) = meta {
                            if let Ok(modified) = m.modified() {
                                let dt: chrono::DateTime<chrono::Local> = modified.into();
                                prop_row(ui, t, "Modified", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            }
                            if let Ok(created) = m.created() {
                                let dt: chrono::DateTime<chrono::Local> = created.into();
                                prop_row(ui, t, "Created", &dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            }
                        }

                        ui.add_space(sp.md);
                        section_label(ui, t, "PERMISSIONS");
                        ui.add_space(sp.xs);

                        if let Ok(ref m) = meta {
                            let mode = m.permissions().mode();
                            prop_row(ui, t, "Mode (octal)", &format!("{:o}", mode & 0o777));
                            prop_row(ui, t, "Readable",    if mode & 0o400 != 0 { "Yes" } else { "No" });
                            prop_row(ui, t, "Writable",    if mode & 0o200 != 0 { "Yes" } else { "No" });
                            prop_row(ui, t, "Executable",  if mode & 0o100 != 0 { "Yes" } else { "No" });

                            // Permission string rwxrwxrwx
                            let perm_str = perm_string(mode);
                            prop_row(ui, t, "Permission", &perm_str);
                        }

                        ui.add_space(sp.md);
                        section_label(ui, t, "DETAILS");
                        ui.add_space(sp.xs);

                        // Hidden
                        let is_hidden = filename.starts_with('.');
                        prop_row(ui, t, "Hidden", if is_hidden { "Yes" } else { "No" });

                        // Inode (Unix)
                        if let Ok(ref m) = symlink_meta {
                            use std::os::unix::fs::MetadataExt;
                            prop_row(ui, t, "Inode",       &m.ino().to_string());
                            prop_row(ui, t, "Hard links",  &m.nlink().to_string());
                            prop_row(ui, t, "Device",      &m.dev().to_string());
                        }
                    });
                });

                // Footer
                let r = ui.available_rect_before_wrap(); ui.painter().hline(r.left()..=r.right(), r.top(), Stroke::new(1.0_f32, c.border()));
                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.sm), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // chmod shortcut
                        if ui.add(egui::Button::new(RichText::new(format!("{} chmod…", icon(t,"settings"))).color(c.text_hint()).size(t.typography.size_xs)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).hovered() {
                            ui.label(RichText::new(format!("chmod +x {}", path)).color(c.text_hint()).size(t.typography.size_xs).monospace());
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Close").color(c.text_secondary()).size(t.typography.size_sm)).fill(c.bg_surface0()).stroke(Stroke::new(1.0_f32, c.border())).rounding(Rounding::same(s.radius_sm))).clicked() { state.panels.close(); }
                        });
                    });
                });
            });
        });

    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

fn prop_row(ui: &mut Ui, t: &AexTheme, label: &str, value: &str) {
    let c = &t.colors; let sp = &t.spacing;
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        let lw = 100.0_f32;
        ui.add_sized(Vec2::new(lw, 20.0), egui::Label::new(RichText::new(label).color(c.text_hint()).size(t.typography.size_xs)));
        ui.add_space(sp.sm);
        // Seçilebilir value (copy için)
        ui.add(egui::Label::new(RichText::new(value).color(c.text_secondary()).size(t.typography.size_xs)).selectable(true));
    });
    ui.add_space(2.0);
}

fn section_label(ui: &mut Ui, t: &AexTheme, label: &str) {
    let c = &t.colors;
    ui.label(RichText::new(label).color(c.text_hint()).size(t.typography.size_xs).strong());
    let r = ui.min_rect();
    ui.painter().hline(r.left()..=r.right(), r.bottom(), Stroke::new(1.0_f32, c.border()));
}

fn perm_string(mode: u32) -> String {
    let bits = [(0o400,"r"),(0o200,"w"),(0o100,"x"),(0o040,"r"),(0o020,"w"),(0o010,"x"),(0o004,"r"),(0o002,"w"),(0o001,"x")];
    bits.iter().map(|(b,c)| if mode & b != 0 { *c } else { "-" }).collect::<Vec<_>>().join("")
}

fn dir_size(path: &str) -> u64 {
    walkdir::WalkDir::new(path).into_iter().flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

fn count_items(path: &str) -> String {
    let n = fs::read_dir(path).map(|r| r.count()).unwrap_or(0);
    format!("{} items", n)
}

// ─────────────────────────────────────────────────────────────────────────────
// BackendMsg enum update — app.rs'e eklenecekler
// ─────────────────────────────────────────────────────────────────────────────

// src/app.rs'de BackendMsg enum'una ekle:
//
// pub enum BackendMsg {
//     DirectoryLoaded(crate::backend::fs_ops::DirectoryListing),
//     DirectoryError(String),
//     BookmarksLoaded(Vec<crate::backend::fs_ops::Bookmark>),
//     DrivesLoaded(Vec<crate::backend::fs_ops::DriveInfo>),
//     BlockDevicesLoaded(Vec<crate::backend::mounts::BlockDevice>),
//
//     // YENİ — mount panel için:
//     MountDone { success: bool, message: String, mount_point: String },
//     MountNeedsPassword { device: String, label: String, error: String },
//     MountWrongPassword { device: String, label: String },
// }

// process_messages'a ekle:
//
// BackendMsg::MountDone { success, message, mount_point } => {
//     if success {
//         self.state.notify.success(format!("Mounted: {}", mount_point));
//         self.load_sidebar_data(); // drives güncelle
//     } else {
//         self.state.notify.error(&message);
//     }
//     // mount flow sıfırla
//     self.state.ctx_data_reset_mount();
// }
// BackendMsg::MountNeedsPassword { device, label, error } => {
//     // egui memory'deki mount flow'u güncelle — repaint ile gelir
//     // (Context yok burada — flag kullan)
//     self.state.mount_needs_password = Some((device, label, error));
// }
// BackendMsg::MountWrongPassword { device, label } => {
//     self.state.mount_wrong_password = Some((device, label));
// }
