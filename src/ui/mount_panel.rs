// src/ui/mount_panel.rs
// Mount paneli — tam şifre akışı, PASSWORD_REQUIRED state machine
use egui::*;
use std::sync::mpsc::Sender;
use crate::app::BackendMsg;
use crate::backend::mounts::{BlockDevice, list_block_devices};
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::icon;

#[derive(Clone, PartialEq, Default)]
enum MountFlow {
    #[default]
    DeviceList,
    Mounting(String),
    NeedsPassword { device: String, label: String, error: String },
    WrongPassword(String, String),
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState, tx: &Sender<BackendMsg>, _rt: &tokio::runtime::Runtime) {
    let c = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let screen = ctx.screen_rect();

    egui::Area::new(Id::new("mount_backdrop")).fixed_pos(screen.min).order(Order::Background).show(ctx, |ui| {
        ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0,0,0,130));
        if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
    });

    let (pw, ph) = (560.0_f32, 520.0_f32);
    let flow_id = Id::new("mount_flow");
    let pass_id = Id::new("mount_pass");
    let show_id = Id::new("mount_show");
    
    let mut flow: MountFlow = ctx.data_mut(|d| d.get_temp(flow_id).unwrap_or_default());
    let mut password: String = ctx.data_mut(|d| d.get_temp::<String>(pass_id).unwrap_or_default());
    let mut show_pw: bool = ctx.data_mut(|d| d.get_temp(show_id).unwrap_or(false));

    // Backend'den gelen pending event'leri oku ve UI akışına aktar
    if let Some((device, label, error)) = state.pending_mount_password.take() {
        flow = MountFlow::NeedsPassword { device, label, error };
    }
    if let Some((device, label)) = state.pending_mount_wrong.take() {
        flow = MountFlow::WrongPassword(device, label);
    }
    // Mount işlemi bitti — flow'u sıfırla
    if state.mount_flow_reset {
        state.mount_flow_reset = false;
        flow = MountFlow::DeviceList;
    }

    let dev_cache_id = Id::new("mount_dev_cache");
    // list_block_devices her frame değil — sadece DeviceList modunda ve 2s'de bir yükle
    let dev_time_id = Id::new("mount_dev_time");
    let devices: Vec<BlockDevice> = {
        let last_load: Option<std::time::Instant> = ctx.data_mut(|d| d.get_temp(dev_time_id));
        let stale = last_load.map(|t| t.elapsed().as_secs() > 2).unwrap_or(true);
        if stale && matches!(flow, MountFlow::DeviceList) {
            let devs = list_block_devices();
            ctx.data_mut(|d| {
                d.insert_temp(dev_cache_id, devs.clone());
                d.insert_temp(dev_time_id, std::time::Instant::now());
            });
            devs
        } else {
            ctx.data_mut(|d| d.get_temp(dev_cache_id).unwrap_or_default())
        }
    };

    egui::Area::new(Id::new("mount_panel"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, screen.center().y - ph/2.0))
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
                let title = match &flow { 
                    MountFlow::NeedsPassword{..} | MountFlow::WrongPassword(..) => "Authentication Required", 
                    MountFlow::Mounting(_) => "Mounting…", 
                    _ => "Drive Manager" 
                };
                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}  {}", icon(t,"drive"), title)).color(c.text_primary()).size(t.typography.size_lg).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("✕").color(c.text_hint())).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() { state.panels.close(); }
                        });
                    });
                });
                let r = ui.min_rect(); ui.painter().hline(r.left()..=r.right(), r.bottom(), Stroke::new(1.0_f32, c.border())); ui.add_space(sp.xs);

                // Body
                match flow.clone() {
                    MountFlow::DeviceList => {
                        egui::ScrollArea::vertical().max_height(ph - 120.0).show(ui, |ui| {
                            egui::Frame { inner_margin: Margin::symmetric(sp.md, sp.sm), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                                if devices.is_empty() {
                                    ui.add_space(40.0); ui.vertical_centered(|ui| { ui.label(RichText::new(icon(t,"drive")).color(c.text_disabled()).size(32.0)); ui.label(RichText::new("No block devices").color(c.text_hint()).size(t.typography.size_sm)); });
                                } else {
                                    for dev in &devices {
                                        if let Some(action) = device_row_v2(ui, t, dev) {
                                            match action {
                                                RowAct::Mount => {
                                                    let d2 = dev.clone(); let tx2 = tx.clone();
                                                    flow = MountFlow::Mounting(d2.device.clone());
                                                    std::thread::spawn(move || {
                                                        let r = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
                                                            .block_on(crate::backend::mounts::mount_device(d2.device.clone(), if d2.label.is_empty() { None } else { Some(d2.label.clone()) }));
                                                        match r {
                                                            Ok(mr) if mr.success => { tx2.send(BackendMsg::MountDone { success: true, message: mr.message, mount_point: mr.mount_point }).ok(); }
                                                            Ok(mr) if mr.message.starts_with("PASSWORD_REQUIRED") => { tx2.send(BackendMsg::MountNeedsPassword { device: d2.device, label: d2.label, error: mr.message }).ok(); }
                                                            Ok(mr) => { tx2.send(BackendMsg::MountDone { success: false, message: mr.message, mount_point: String::new() }).ok(); }
                                                            Err(e) => { tx2.send(BackendMsg::MountDone { success: false, message: e, mount_point: String::new() }).ok(); }
                                                        }
                                                    });
                                                }
                                                RowAct::Unmount(mp) => {
                                                    let tx2 = tx.clone();
                                                    let mp_clone = mp.clone();
                                                    std::thread::spawn(move || {
                                                        let r = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(crate::backend::mounts::unmount_device(mp_clone.clone()));
                                                        match r {
                                                            Ok(m) => { tx2.send(BackendMsg::UnmountDone { success: true, message: m, target: mp_clone }).ok(); }
                                                            Err(e) => { tx2.send(BackendMsg::UnmountDone { success: false, message: e, target: mp_clone }).ok(); }
                                                        }
                                                    });
                                                }
                                                RowAct::Open(mp) => { state.navigate(mp); state.panels.close(); }
                                            }
                                        }
                                        ui.add_space(sp.xs);
                                    }
                                }
                            });
                        });
                    }
                    MountFlow::Mounting(ref dev) => {
                        ui.add_space(60.0); ui.vertical_centered(|ui| {
                            let fr = (ui.ctx().input(|i| i.time) * 8.0) as usize % 8;
                            ui.label(RichText::new(["⣾","⣽","⣻","⢿","⡿","⣟","⣯","⣷"][fr]).color(c.accent()).size(28.0));
                            ui.add_space(8.0); ui.label(RichText::new(format!("Mounting {}…", dev)).color(c.text_secondary()).size(t.typography.size_sm));
                            ui.label(RichText::new("zbus → ntfs3 → ntfs-3g → pkexec").color(c.text_hint()).size(t.typography.size_xs));
                        });
                        ctx.request_repaint();
                    }
                    MountFlow::NeedsPassword { ref device, ref label, ref error } => {
                        let d2 = device.clone();
                        let l2 = label.clone();
                        if let Some(nf) = password_view(ui, t, &d2, &l2, error, false, &mut password, &mut show_pw, tx) {
                            flow = nf;
                        }
                    }
                    MountFlow::WrongPassword(ref device, ref label) => {
                        let error_msg = "Yanlış şifre — tekrar dene".to_string();
                        let d2 = device.clone();
                        let l2 = label.clone();
                        password.clear();
                        if let Some(nf) = password_view(ui, t, &d2, &l2, &error_msg, true, &mut password, &mut show_pw, tx) {
                            flow = nf;
                        }
                    }
                }

                // Footer
                let r = ui.available_rect_before_wrap(); ui.painter().hline(r.left()..=r.right(), r.top(), Stroke::new(1.0_f32, c.border()));
                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.sm), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} Log: ~/.local/share/arch-explorer/mount_debug.log", icon(t,"info"))).color(c.text_hint()).size(t.typography.size_xs));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Close").color(c.text_secondary()).size(t.typography.size_sm)).fill(c.bg_surface0()).stroke(Stroke::new(1.0_f32, c.border())).rounding(Rounding::same(s.radius_sm))).clicked() { state.panels.close(); }
                        });
                    });
                });
            });
        });

    ctx.data_mut(|d| { d.insert_temp(flow_id, flow); d.insert_temp(pass_id, password); d.insert_temp(show_id, show_pw); });
    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

enum RowAct { Mount, Unmount(String), Open(String) }

fn device_row_v2(ui: &mut Ui, t: &AexTheme, dev: &BlockDevice) -> Option<RowAct> {
    let c = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let mut act: Option<RowAct> = None;
    egui::Frame { fill: c.bg_surface0(), rounding: Rounding::same(s.radius_md), stroke: Stroke::new(1.0_f32, c.border()), inner_margin: Margin::symmetric(sp.md, sp.sm), ..Default::default() }.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(if dev.is_windows { icon(t,"ntfs") } else { icon(t,"drive") }).color(if dev.is_mounted { c.accent() } else { c.text_disabled() }).size(t.icons.size_lg));
            ui.add_space(sp.sm);
            ui.vertical(|ui| {
                ui.label(RichText::new(if dev.label.is_empty() { dev.name.clone() } else { format!("{} ({})", dev.label, dev.name) }).color(c.text_primary()).size(t.typography.size_base).strong());
                ui.label(RichText::new(if dev.is_mounted { format!("{}  •  {}  •  {}", dev.fs_type, dev.size, dev.mount_point.as_deref().unwrap_or("?")) } else { format!("{}  •  {}  •  not mounted", dev.fs_type, dev.size) }).color(c.text_hint()).size(t.typography.size_xs));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(sp.sm);
                if dev.is_mounted {
                    if let Some(ref mp) = dev.mount_point {
                        if ui.add(egui::Button::new(RichText::new(format!("{}  Open", icon(t,"folder_open"))).color(c.accent()).size(t.typography.size_sm)).fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20)).stroke(Stroke::new(1.0_f32, c.accent())).rounding(Rounding::same(s.radius_sm))).clicked() { act = Some(RowAct::Open(mp.clone())); }
                        ui.add_space(sp.xs);
                        if ui.add(egui::Button::new(RichText::new("Unmount").color(c.warning()).size(t.typography.size_sm)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, c.warning())).rounding(Rounding::same(s.radius_sm))).clicked() { act = Some(RowAct::Unmount(mp.clone())); }
                    }
                } else {
                    if ui.add(egui::Button::new(RichText::new(format!("{}  Mount", icon(t,"mount"))).color(Color32::WHITE).size(t.typography.size_sm)).fill(c.accent()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() { act = Some(RowAct::Mount); }
                }
            });
        });
        if dev.is_windows && !dev.is_mounted { ui.horizontal(|ui| { ui.add_space(sp.lg*2.0); ui.label(RichText::new(format!("{} Windows partition — may need sudo password", icon(t,"warning"))).color(c.warning()).size(t.typography.size_xs)); }); }
    });
    act
}

fn password_view(ui: &mut Ui, t: &AexTheme, device: &str, label: &str, error: &str, wrong: bool, password: &mut String, show_pw: &mut bool, tx: &Sender<BackendMsg>) -> Option<MountFlow> {
    let c = &t.colors; let s = &t.shape; let sp = &t.spacing;
    let mut next: Option<MountFlow> = None;
    egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md), fill: Color32::TRANSPARENT, ..Default::default() }.show(ui, |ui| {
        if wrong { egui::Frame { fill: Color32::from_rgba_unmultiplied(c.danger().r(),c.danger().g(),c.danger().b(),15), rounding: Rounding::same(s.radius_sm), stroke: Stroke::new(1.0_f32, c.danger()), inner_margin: Margin::same(sp.sm), ..Default::default() }.show(ui, |ui| { ui.label(RichText::new(format!("{} {}", icon(t,"error"), error)).color(c.danger()).size(t.typography.size_sm)); }); ui.add_space(sp.sm); }
        ui.vertical_centered(|ui| {
            if !wrong { ui.label(RichText::new(icon(t,"mount")).color(c.accent()).size(32.0)); ui.add_space(sp.sm); ui.label(RichText::new("Sudo password required").color(c.text_primary()).size(t.typography.size_base).strong()); ui.label(RichText::new(format!("Mounting {} requires elevated permissions", device)).color(c.text_hint()).size(t.typography.size_xs)); ui.add_space(sp.md); }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Password:").color(c.text_secondary()).size(t.typography.size_sm));
            ui.add_space(sp.sm);
            let resp = ui.add(egui::TextEdit::singleline(password).password(!*show_pw).desired_width(f32::INFINITY).font(FontId::new(t.typography.size_base, FontFamily::Proportional)).hint_text("Enter sudo password…"));
            // request_focus sadece ilk frame'de — her frame çağrılırsa click interrupt oluyor
            let focused_id = Id::new("mount_pw_focused");
            let already_focused: bool = ui.ctx().data_mut(|d| d.get_temp(focused_id).unwrap_or(false));
            if !already_focused {
                resp.request_focus();
                ui.ctx().data_mut(|d| d.insert_temp(focused_id, true));
            }
            // Focus kaybedilince flag'i sıfırla
            if resp.lost_focus() {
                ui.ctx().data_mut(|d| d.insert_temp(focused_id, false));
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) { next = Some(do_sudo_mount(device, label, password, tx)); }
        });
        ui.add_space(sp.xs);
        ui.horizontal(|ui| { ui.add_space(80.0); ui.checkbox(show_pw, RichText::new("Show password").color(c.text_hint()).size(t.typography.size_xs)); });
        ui.add_space(sp.md);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Mount").color(Color32::WHITE).size(t.typography.size_sm)).fill(c.accent()).stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() { next = Some(do_sudo_mount(device, label, password, tx)); }
                ui.add_space(sp.xs);
                if ui.add(egui::Button::new(RichText::new("Cancel").color(c.text_secondary()).size(t.typography.size_sm)).fill(c.bg_surface0()).stroke(Stroke::new(1.0_f32, c.border())).rounding(Rounding::same(s.radius_sm))).clicked() { password.clear(); next = Some(MountFlow::DeviceList); }
            });
        });
        if !wrong { ui.add_space(sp.sm); egui::Frame { fill: c.bg_crust(), rounding: Rounding::same(s.radius_sm), inner_margin: Margin::same(sp.sm), ..Default::default() }.show(ui, |ui| { ui.label(RichText::new("Tip: sudoers kuralı ile şifresiz mount (install.sh otomatik yapar)").color(c.text_hint()).size(t.typography.size_xs)); }); }
    });
    next
}

fn do_sudo_mount(device: &str, label: &str, password: &str, tx: &Sender<BackendMsg>) -> MountFlow {
    let d = device.to_string(); let l = label.to_string(); let p = password.to_string(); let tx = tx.clone();
    std::thread::spawn(move || {
        let r = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
            .block_on(crate::backend::mounts::mount_ntfs_with_password(d.clone(), p, if l.is_empty() { None } else { Some(l.clone()) }, None));
        match r {
            Ok(mr) if mr.success => { tx.send(BackendMsg::MountDone { success: true, message: mr.message, mount_point: mr.mount_point }).ok(); }
            Ok(mr) if mr.message == "WRONG_PASSWORD" => { tx.send(BackendMsg::MountWrongPassword { device: d, label: l }).ok(); }
            Ok(mr) => { tx.send(BackendMsg::MountDone { success: false, message: mr.message, mount_point: String::new() }).ok(); }
            Err(e) => { tx.send(BackendMsg::MountDone { success: false, message: e, mount_point: String::new() }).ok(); }
        }
    });
    MountFlow::Mounting(device.to_string())
}