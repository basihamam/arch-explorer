// src/ui/open_with.rs — Rich categorized Open With modal inspired by another copy
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use crate::backend::open_with::{AppEntry, OpenWithInfo};

fn avatar_color(name: &str) -> Color32 {
    let colors = [
        Color32::from_rgb(0x7c, 0x6a, 0xf7),
        Color32::from_rgb(0xf7, 0x6a, 0x8c),
        Color32::from_rgb(0x6a, 0xf7, 0xc2),
        Color32::from_rgb(0xf7, 0xb4, 0x6a),
        Color32::from_rgb(0x6a, 0xc8, 0xf7),
        Color32::from_rgb(0xc4, 0x6a, 0xf7),
        Color32::from_rgb(0xf7, 0xe0, 0x6a),
        Color32::from_rgb(0x6a, 0xf7, 0x8c),
    ];
    let mut h: u32 = 0;
    for b in name.bytes() {
        h = (h.wrapping_mul(31).wrapping_add(b as u32)) & 0xffff;
    }
    colors[(h as usize) % colors.len()]
}

fn avatar_letter(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string())
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState, path: &str) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();

    let pw = 520.0_f32;
    let ph = 540.0_f32;
    let d_rect = Rect::from_center_size(screen.center(), Vec2::new(pw, ph));

    // ── Semi-transparent backdrop ─────────────────────────────────────────────
    egui::Area::new(Id::new("ow_backdrop"))
        .fixed_pos(screen.min)
        .order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
            let r = ui.allocate_rect(screen, Sense::click());
            let click_pos = ctx.input(|i| i.pointer.interact_pos());
            if r.clicked() && click_pos.map(|p| !d_rect.contains(p) && p.y > 36.0).unwrap_or(false) {
                state.panels.close();
            }
        });

    // ── Persistent/Cached UI state ────────────────────────────────────────────
    let q_id      = Id::new("ow_search_query");
    let sel_id    = Id::new("ow_selected_id");
    let cmd_id    = Id::new("ow_custom_cmd");
    let cache_id  = Id::new("ow_cached_info");
    let apps_id   = Id::new("ow_all_apps_cache");

    let mut search_q: String = ctx.data_mut(|d| d.get_temp(q_id).unwrap_or_default());
    let mut selected_id: Option<String> = ctx.data_mut(|d| d.get_temp(sel_id));
    let mut custom_cmd: String = ctx.data_mut(|d| d.get_temp(cmd_id).unwrap_or_default());

    // Cache OpenWithInfo per file path to avoid querying xdg-mime on every frame
    let cached_info: Option<(String, OpenWithInfo)> = ctx.data_mut(|d| d.get_temp(cache_id));
    let info = match cached_info {
        Some((ref p, ref inf)) if p == path => inf.clone(),
        _ => {
            match crate::backend::open_with::get_open_with_info(path.to_string()) {
                Ok(inf) => {
                    ctx.data_mut(|d| d.insert_temp(cache_id, (path.to_string(), inf.clone())));
                    if selected_id.is_none() {
                        selected_id = inf.default_app.clone().or_else(|| inf.apps.first().map(|a| a.id.clone()));
                    }
                    inf
                }
                Err(e) => {
                    OpenWithInfo {
                        mime_type: "application/octet-stream".into(),
                        default_app: None,
                        apps: vec![],
                    }
                }
            }
        }
    };

    // Cache all desktop apps
    let all_desktop_apps: Vec<AppEntry> = ctx.data_mut(|d| {
        d.get_temp(apps_id).unwrap_or_else(|| {
            let apps = crate::backend::open_with::get_all_apps();
            d.insert_temp(apps_id, apps.clone());
            apps
        })
    });

    let pw = 520.0_f32;
    let ph = 540.0_f32;

    let panel_fill = if t.effects.glass_panels {
        Color32::from_rgba_unmultiplied(
            c.bg_mantle().r(),
            c.bg_mantle().g(),
            c.bg_mantle().b(),
            (t.effects.panel_opacity * 255.0) as u8,
        )
    } else {
        c.bg_mantle()
    };

    egui::Area::new(Id::new("ow_dialog"))
        .fixed_pos(Pos2::new(screen.center().x - pw / 2.0, (screen.center().y - ph / 2.0).max(20.0)))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: panel_fill,
                rounding: Rounding::same(14.0),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.md),
                shadow: epaint::Shadow {
                    offset: Vec2::new(0.0, 12.0),
                    blur: 32.0,
                    spread: 0.0,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                },
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.set_width(pw - sp.md * 2.0);
                ui.set_height(ph - sp.md * 2.0);

                // ── 1. Header ─────────────────────────────────────────────
                let fname = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string());

                let mime_short = info.mime_type.split('/').nth(1).unwrap_or("FILE").to_uppercase();
                let mime_badge = if mime_short.len() > 5 { &mime_short[..5] } else { &mime_short };

                ui.horizontal(|ui| {
                    // Mime badge block
                    let badge_size = Vec2::splat(38.0);
                    let (badge_rect, _) = ui.allocate_exact_size(badge_size, Sense::hover());
                    ui.painter().rect_filled(badge_rect, Rounding::same(9.0), c.accent());
                    ui.painter().text(
                        badge_rect.center(),
                        Align2::CENTER_CENTER,
                        mime_badge,
                        FontId::new(9.0, FontFamily::Proportional),
                        Color32::WHITE,
                    );

                    ui.add_space(sp.xs);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Open With").color(c.text_primary()).size(14.0).strong());
                        });
                        ui.label(RichText::new(&fname).color(c.text_secondary()).size(11.0));
                        ui.label(RichText::new(&info.mime_type).color(c.text_hint()).size(9.5).monospace());
                    });

                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("✕").size(12.0).color(c.text_hint()))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                        ).clicked() {
                            state.panels.close();
                        }
                    });
                });

                ui.add_space(sp.xs);
                ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.xs);

                // ── 2. Search Box ─────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍").size(11.0).color(c.text_hint()));
                    ui.add(
                        egui::TextEdit::singleline(&mut search_q)
                            .hint_text("Uygulama ara…")
                            .font(FontId::new(12.0, FontFamily::Proportional))
                            .desired_width(ui.available_width() - 8.0)
                            .frame(false)
                    );
                });

                ui.add_space(sp.xs);
                ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.xs);

                // ── 3. Categorized App List ───────────────────────────────
                let search_lower = search_q.trim().to_lowercase();

                // Partition apps
                let mut default_app_entry: Option<AppEntry> = None;
                let mut recommended_apps: Vec<AppEntry> = vec![];
                let mut other_apps: Vec<AppEntry> = vec![];

                for app in &info.apps {
                    if Some(&app.id) == info.default_app.as_ref() {
                        default_app_entry = Some(app.clone());
                    } else {
                        recommended_apps.push(app.clone());
                    }
                }

                let rec_ids: std::collections::HashSet<String> = recommended_apps.iter().map(|a| a.id.clone()).collect();
                let def_id = default_app_entry.as_ref().map(|a| a.id.clone());

                for app in &all_desktop_apps {
                    if !rec_ids.contains(&app.id) && def_id.as_ref() != Some(&app.id) {
                        other_apps.push(app.clone());
                    }
                }

                let custom_commands = crate::backend::commands::load_user_commands();

                let mut triggered_open = false;

                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                        // Section: DEFAULT
                        if let Some(ref def_app) = default_app_entry {
                            if search_lower.is_empty() || def_app.name.to_lowercase().contains(&search_lower) {
                                ui.add_space(4.0);
                                section_tag(ui, t, "DEFAULT");
                                if render_app_row(ui, t, def_app, &mut selected_id, &mut custom_cmd, true) {
                                    triggered_open = true;
                                }
                            }
                        }

                        // Section: RECOMMENDED
                        let filtered_rec: Vec<_> = recommended_apps.iter()
                            .filter(|a| search_lower.is_empty() || a.name.to_lowercase().contains(&search_lower))
                            .collect();
                        if !filtered_rec.is_empty() {
                            ui.add_space(4.0);
                            section_tag(ui, t, "RECOMMENDED");
                            for app in filtered_rec {
                                if render_app_row(ui, t, app, &mut selected_id, &mut custom_cmd, false) {
                                    triggered_open = true;
                                }
                            }
                        }

                        // Section: OTHER APPLICATIONS
                        let filtered_other: Vec<_> = other_apps.iter()
                            .filter(|a| search_lower.is_empty() || a.name.to_lowercase().contains(&search_lower))
                            .collect();
                        if !filtered_other.is_empty() {
                            ui.add_space(4.0);
                            section_tag(ui, t, "OTHER APPLICATIONS");
                            for app in filtered_other {
                                if render_app_row(ui, t, app, &mut selected_id, &mut custom_cmd, false) {
                                    triggered_open = true;
                                }
                            }
                        }

                        // Section: COMMANDS & SCRIPTS
                        let filtered_cmds: Vec<_> = custom_commands.iter()
                            .filter(|cmd| search_lower.is_empty() || cmd.name.to_lowercase().contains(&search_lower))
                            .collect();
                        if !filtered_cmds.is_empty() {
                            ui.add_space(4.0);
                            section_tag(ui, t, "COMMANDS & SCRIPTS");
                            for cmd in filtered_cmds {
                                let app_equiv = AppEntry {
                                    id: format!("custom::{}", cmd.id),
                                    name: cmd.name.clone(),
                                    exec: cmd.exec.clone(),
                                    icon: None,
                                    comment: Some(cmd.description.clone()),
                                    categories: vec!["Custom".into()],
                                };
                                if render_app_row(ui, t, &app_equiv, &mut selected_id, &mut custom_cmd, false) {
                                    triggered_open = true;
                                }
                            }
                        }
                    });

                ui.add_space(sp.xs);
                ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.xs);

                // ── 4. Custom Executable Box ──────────────────────────────
                ui.label(RichText::new("Ya da doğrudan bir executable / komut:").color(c.text_hint()).size(10.0));
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let cmd_input_w = ui.available_width() - 36.0;
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut custom_cmd)
                            .hint_text("Executable veya komut…")
                            .font(FontId::new(11.0, FontFamily::Monospace))
                            .desired_width(cmd_input_w)
                    );
                    if te.changed() && !custom_cmd.is_empty() {
                        selected_id = None;
                    }

                    if ui.add_sized(
                        Vec2::new(30.0, 24.0),
                        egui::Button::new("📁")
                    ).on_hover_text("Executable dosya seç").clicked() {
                        if let Ok(out) = std::process::Command::new("zenity")
                            .args(["--file-selection", "--title=Select Executable"])
                            .output()
                        {
                            if out.status.success() {
                                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                                if !p.is_empty() {
                                    custom_cmd = p;
                                    selected_id = None;
                                }
                            }
                        }
                    }
                });

                ui.add_space(sp.xs);
                ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), Stroke::new(1.0_f32, c.border()));
                ui.add_space(sp.xs);

                // ── 5. Action Buttons (Footer) ────────────────────────────
                ui.horizontal(|ui| {
                    // Reset to system default button
                    if ui.add(
                        egui::Button::new(RichText::new("↺ Sistem Varsayılanı").color(c.danger()).size(11.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                    ).clicked() {
                        crate::backend::open_with::reset_default_for_mime(info.mime_type.clone()).ok();
                        state.notify.success("Sistem varsayılanına sıfırlandı".to_string());
                        ctx.data_mut(|d| d.remove::<(String, OpenWithInfo)>(cache_id));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Open button
                        let can_open = selected_id.is_some() || !custom_cmd.trim().is_empty();
                        let open_btn = ui.add_enabled(
                            can_open,
                            egui::Button::new(RichText::new("▶ Aç").color(Color32::WHITE).size(11.0).strong())
                                .fill(c.accent())
                                .rounding(Rounding::same(s.radius_sm))
                        );

                        if open_btn.clicked() || triggered_open {
                            if !custom_cmd.trim().is_empty() {
                                crate::backend::open_with::open_with_exec(path.to_string(), custom_cmd.trim().to_string()).ok();
                                state.panels.close();
                            } else if let Some(ref sel) = selected_id {
                                if sel.starts_with("custom::") {
                                    let cmd_id = sel.strip_prefix("custom::").unwrap();
                                    if let Some(cmd) = custom_commands.iter().find(|c| c.id == cmd_id) {
                                        let exec = cmd.exec.replace("{path}", path)
                                            .replace("{file}", path)
                                            .replace("{dir}", &state.nav.current_path);
                                        std::process::Command::new("bash").args(["-c", &exec]).spawn().ok();
                                    }
                                } else {
                                    crate::backend::open_with::open_with_app(path.to_string(), sel.clone()).ok();
                                }
                                state.panels.close();
                            }
                        }

                        // Set Default button
                        let can_set_default = selected_id.is_some() && !selected_id.as_ref().unwrap().starts_with("custom::");
                        if ui.add_enabled(
                            can_set_default,
                            egui::Button::new(RichText::new("★ Varsayılan Yap").size(11.0))
                                .fill(c.bg_surface0())
                                .rounding(Rounding::same(s.radius_sm))
                        ).clicked() {
                            if let Some(ref sel) = selected_id {
                                crate::backend::open_with::set_default_app(info.mime_type.clone(), sel.clone()).ok();
                                state.notify.success("Varsayılan uygulama ayarlandı".to_string());
                                ctx.data_mut(|d| d.remove::<(String, OpenWithInfo)>(cache_id));
                            }
                        }

                        // Cancel button
                        if ui.add(
                            egui::Button::new(RichText::new("İptal").color(c.text_secondary()).size(11.0))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                        ).clicked() {
                            state.panels.close();
                        }
                    });
                });
            });
        });

    // Save temporary state
    ctx.data_mut(|d| {
        d.insert_temp(q_id, search_q);
        d.insert_temp(sel_id, selected_id);
        d.insert_temp(cmd_id, custom_cmd);
    });

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.panels.close();
    }
}

fn section_tag(ui: &mut Ui, t: &AexTheme, title: &str) {
    ui.label(
        RichText::new(title)
            .color(t.colors.text_hint())
            .size(9.0)
            .strong()
    );
}

pub fn get_or_load_app_icon(ctx: &Context, app: &AppEntry) -> Option<TextureHandle> {
    let icon_name = app.icon.as_deref().unwrap_or(&app.id);
    let tex_id = Id::new(format!("app_ico_tex_{}", icon_name));
    if let Some(h) = ctx.data_mut(|d| d.get_temp::<TextureHandle>(tex_id)) {
        return Some(h);
    }

    let icon_path = crate::backend::open_with::find_app_icon(icon_name)?;
    let ext = icon_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" {
        None
    } else {
        let bytes = std::fs::read(&icon_path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let color_img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
        let handle = ctx.load_texture(
            format!("app_tex_{}", icon_name),
            color_img,
            TextureOptions::LINEAR,
        );
        ctx.data_mut(|d| d.insert_temp(tex_id, handle.clone()));
        Some(handle)
    }
}

fn render_app_row(
    ui: &mut Ui,
    t: &AexTheme,
    app: &AppEntry,
    selected_id: &mut Option<String>,
    custom_cmd: &mut String,
    is_default: bool,
) -> bool {
    let c = &t.colors;
    let s = &t.shape;
    let is_sel = selected_id.as_deref() == Some(&app.id);

    let row_h = 42.0_f32;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());

    let bg = if is_sel {
        Color32::from_rgba_unmultiplied(c.accent().r(), c.accent().g(), c.accent().b(), 35)
    } else if resp.hovered() {
        c.bg_surface0()
    } else {
        Color32::TRANSPARENT
    };

    ui.painter().rect_filled(rect, Rounding::same(s.radius_sm), bg);

    if is_default {
        // Subtle accent bar on the left
        let bar = Rect::from_min_size(rect.min + Vec2::new(0.0, 6.0), Vec2::new(2.5, rect.height() - 12.0));
        ui.painter().rect_filled(bar, Rounding::same(1.0), c.accent());
    }

    // Avatar or real Icon
    let av_rect = Rect::from_center_size(
        Pos2::new(rect.min.x + 22.0, rect.center().y),
        Vec2::splat(30.0),
    );

    if let Some(tex) = get_or_load_app_icon(ui.ctx(), app) {
        ui.painter().rect_filled(av_rect, Rounding::same(6.0), Color32::from_rgba_unmultiplied(255, 255, 255, 10));
        ui.painter().image(
            tex.id(),
            av_rect.shrink(2.0),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let av_color = avatar_color(&app.name);
        ui.painter().rect_filled(av_rect, Rounding::same(6.0), av_color);
        ui.painter().text(
            av_rect.center(),
            Align2::CENTER_CENTER,
            avatar_letter(&app.name),
            FontId::new(13.0, FontFamily::Proportional),
            Color32::WHITE,
        );
    }

    // Text details
    let text_x = rect.min.x + 44.0;
    ui.painter().text(
        Pos2::new(text_x, rect.center().y - 7.0),
        Align2::LEFT_CENTER,
        &app.name,
        FontId::new(11.5, FontFamily::Proportional),
        c.text_primary(),
    );

    let sub_text = app.comment.as_deref().unwrap_or(app.exec.as_str());
    let sub_trunc = if sub_text.len() > 50 { format!("{}…", &sub_text[..48]) } else { sub_text.to_string() };
    ui.painter().text(
        Pos2::new(text_x, rect.center().y + 8.0),
        Align2::LEFT_CENTER,
        sub_trunc,
        FontId::new(9.5, FontFamily::Monospace),
        c.text_hint(),
    );

    // Default badge or checkmark
    let mut right_x = rect.max.x - 8.0;
    if is_sel {
        ui.painter().text(
            Pos2::new(right_x, rect.center().y),
            Align2::RIGHT_CENTER,
            "✓",
            FontId::new(14.0, FontFamily::Proportional),
            c.accent(),
        );
        right_x -= 16.0;
    }

    if is_default {
        let badge_rect = Rect::from_center_size(Pos2::new(right_x - 18.0, rect.center().y), Vec2::new(36.0, 14.0));
        ui.painter().rect_filled(badge_rect, Rounding::same(3.0), c.accent());
        ui.painter().text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            "default",
            FontId::new(8.0, FontFamily::Proportional),
            Color32::WHITE,
        );
    }

    if resp.clicked() {
        *selected_id = Some(app.id.clone());
        custom_cmd.clear();
    }

    resp.double_clicked()
}
