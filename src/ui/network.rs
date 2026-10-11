// src/ui/network.rs
// Network Shares — SMB/NFS mount/unmount paneli
use egui::*;
use serde_json;
use crate::theme::types::AexTheme;
use crate::state::AppState;
use super::icon;

#[derive(Clone, PartialEq)]
enum NetworkTab { Smb, Nfs, Connected }

#[derive(Clone)]
struct NetworkState {
    tab:        NetworkTab,
    smb_host:   String,
    smb_shares: Vec<SmbShare>,
    smb_user:   String,
    smb_pass:   String,
    smb_share:  String, // seçili share
    smb_loading: bool,
    smb_error:  Option<String>,
    nfs_host:   String,
    nfs_path:   String,
    nfs_error:  Option<String>,
    connected:  Vec<MountedShare>,
    status_msg: Option<String>,
}

#[derive(Clone)]
struct SmbShare { name: String, comment: String }

#[derive(Clone)]
struct MountedShare { host: String, share: String, mount_point: String, fs_type: String }

impl NetworkState {
    fn new() -> Self {
        Self {
            tab: NetworkTab::Smb,
            smb_host: String::new(), smb_shares: Vec::new(),
            smb_user: String::new(), smb_pass: String::new(),
            smb_share: String::new(), smb_loading: false,
            smb_error: None, nfs_host: String::new(),
            nfs_path: String::new(), nfs_error: None,
            connected: list_mounted_shares(),
            status_msg: None,
        }
    }
}

fn list_mounted_shares() -> Vec<MountedShare> {
    let out = std::process::Command::new("findmnt")
        .args(["-t", "cifs,nfs,nfs4", "-J", "--output", "TARGET,SOURCE,FSTYPE"])
        .output();
    let Ok(out) = out else { return vec![]; };
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let Some(arr) = json["filesystems"].as_array() else { return vec![]; };
    arr.iter().filter_map(|e| {
        let mp  = e["target"].as_str()?.to_string();
        let src = e["source"].as_str().unwrap_or("").to_string();
        let fs  = e["fstype"].as_str().unwrap_or("").to_string();
        let (host, share) = src.split_once('/').unwrap_or((&src, ""));
        Some(MountedShare {
            host: host.to_string(), share: share.to_string(),
            mount_point: mp, fs_type: fs,
        })
    }).collect()
}

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("net_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,140));
            if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
        });

    let pw = 520.0_f32; let ph = 480.0_f32;
    let state_id = Id::new("network_state");
    let mut ns: NetworkState = ctx.data_mut(|d|
        d.get_temp(state_id).unwrap_or_else(NetworkState::new));

    egui::Area::new(Id::new("network_panel"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, screen.center().y - ph/2.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::ZERO,
                shadow: epaint::Shadow { offset: Vec2::new(0.0, 8.0), blur: 24.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw);

                // Header
                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md),
                    fill: Color32::TRANSPARENT, ..Default::default() }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} Network Shares", icon(t, "server")))
                            .color(c.accent()).size(t.typography.size_lg).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("✕").clicked() { state.panels.close(); }
                        });
                    });
                });

                // Tab bar
                ui.horizontal(|ui| {
                    ui.add_space(sp.xl);
                    for (tab, label) in [
                        (NetworkTab::Smb,       "SMB / Windows"),
                        (NetworkTab::Nfs,       "NFS / Linux"),
                        (NetworkTab::Connected, "Connected"),
                    ] {
                        let active = ns.tab == tab;
                        if ui.add(Button::new(RichText::new(label)
                            .color(if active { c.accent() } else { c.text_secondary() })
                            .size(t.typography.size_sm))
                            .fill(if active { Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::NONE)).clicked() {
                            ns.tab = tab;
                            if ns.tab == NetworkTab::Connected {
                                ns.connected = list_mounted_shares();
                            }
                        }
                    }
                });

                let sep_r = ui.min_rect();
                ui.painter().hline(sep_r.left()..=sep_r.right(), sep_r.bottom(),
                    Stroke::new(1.0_f32, c.border()));

                egui::Frame { inner_margin: Margin::symmetric(sp.xl, sp.md),
                    fill: Color32::TRANSPARENT, ..Default::default() }
                .show(ui, |ui| {
                    ui.set_width(pw - sp.xl * 2.0);
                    match ns.tab {
                        NetworkTab::Smb       => tab_smb(ui, t, state, &mut ns),
                        NetworkTab::Nfs       => tab_nfs(ui, t, state, &mut ns),
                        NetworkTab::Connected => tab_connected(ui, t, state, &mut ns),
                    }
                });
            });
        });

    ctx.data_mut(|d| d.insert_temp(state_id, ns));
    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

fn tab_smb(ui: &mut Ui, t: &AexTheme, state: &mut AppState, ns: &mut NetworkState) {
    let c  = &t.colors; let sp = &t.spacing; let s = &t.shape;

    ui.label(RichText::new("Host / IP Address").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.xs);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut ns.smb_host)
            .hint_text("192.168.1.1 or hostname")
            .desired_width(280.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Monospace)));
        ui.add_space(sp.xs);

        // Ping
        if ui.add(Button::new(RichText::new("Ping").color(c.text_secondary()).size(t.typography.size_sm))
            .fill(c.bg_surface0()).stroke(Stroke::new(1.0_f32, c.border()))
            .rounding(Rounding::same(s.radius_sm))).clicked() {
            let host = ns.smb_host.clone();
            let ok = std::process::Command::new("ping")
                .args(["-c", "1", "-W", "2", &host]).status()
                .map(|s| s.success()).unwrap_or(false);
            ns.status_msg = Some(if ok { format!("{} is reachable", host) }
                                 else { format!("{} unreachable", host) });
        }

        // Scan
        if ui.add(Button::new(RichText::new(format!("{} Scan", icon(t, "search"))).color(c.accent()).size(t.typography.size_sm))
            .fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20))
            .stroke(Stroke::new(1.0_f32, c.accent())).rounding(Rounding::same(s.radius_sm))).clicked() {
            ns.smb_loading = true; ns.smb_error = None;
            let host = ns.smb_host.clone();
            let out = std::process::Command::new("smbclient")
                .args(["-L", &host, "-N"]).output();
            ns.smb_loading = false;
            match out {
                Ok(o) => {
                    let txt = String::from_utf8_lossy(&o.stdout);
                    ns.smb_shares = txt.lines()
                        .filter(|l| l.trim_start().starts_with("Disk") || l.contains("Disk"))
                        .filter_map(|l| {
                            let mut parts = l.split_whitespace();
                            let name = parts.next()?.to_string();
                            let comment = parts.collect::<Vec<_>>().join(" ");
                            Some(SmbShare { name, comment })
                        })
                        .collect();
                    if ns.smb_shares.is_empty() {
                        ns.smb_error = Some("No shares found (try with credentials below)".into());
                    }
                }
                Err(e) => ns.smb_error = Some(format!("smbclient not found: {} — install cifs-utils", e)),
            }
        }
    });

    if let Some(ref e) = ns.smb_error.clone() {
        ui.label(RichText::new(e).color(c.warning()).size(t.typography.size_xs));
    }
    if let Some(ref msg) = ns.status_msg.clone() {
        ui.label(RichText::new(msg).color(c.info()).size(t.typography.size_xs));
    }

    // Share listesi
    if !ns.smb_shares.is_empty() {
        ui.add_space(sp.sm);
        ui.label(RichText::new("Shares").color(c.text_hint()).size(t.typography.size_xs));
        ui.add_space(sp.xs);
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            for share in &ns.smb_shares.clone() {
                let is_sel = ns.smb_share == share.name;
                let resp = ui.add(Button::new(
                    RichText::new(format!("{} {}", icon(t, "drive"), share.name))
                        .color(if is_sel { c.accent() } else { c.text_secondary() })
                        .size(t.typography.size_sm))
                    .fill(if is_sel { Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20) } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE));
                if resp.clicked() { ns.smb_share = share.name.clone(); }
            }
        });
    }

    // Credentials
    ui.add_space(sp.sm);
    ui.label(RichText::new("Credentials (optional — leave blank for guest)").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.xs);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut ns.smb_user).hint_text("Username").desired_width(150.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
        ui.add_space(sp.xs);
        ui.add(egui::TextEdit::singleline(&mut ns.smb_pass).hint_text("Password")
            .password(true).desired_width(150.0)
            .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
    });

    ui.add_space(sp.md);
    let can_mount = !ns.smb_host.is_empty() && !ns.smb_share.is_empty();
    if ui.add_enabled(can_mount, Button::new(
        RichText::new(format!("{} Mount //{}{}{}",
            icon(t, "mount"), ns.smb_host,
            if ns.smb_share.is_empty() { "" } else { "/" },
            ns.smb_share))
            .color(Color32::WHITE).size(t.typography.size_sm))
        .fill(if can_mount { c.accent() } else { c.bg_surface1() })
        .rounding(Rounding::same(s.radius_sm)).stroke(Stroke::NONE)).clicked() {
        let host  = ns.smb_host.clone();
        let share = ns.smb_share.clone();
        let user  = if ns.smb_user.is_empty() { None } else { Some(ns.smb_user.clone()) };
        let pass  = if ns.smb_pass.is_empty() { None } else { Some(ns.smb_pass.clone()) };
        let mp    = format!("/run/media/network/{}_{}", host, share);
        std::fs::create_dir_all(&mp).ok();
        let src_path = format!("//{}/{}", host, share);
        let mut args = vec!["-t", "cifs", src_path.as_str(), mp.as_str()];
        let opt = format!("{}{}",
            user.as_ref().map(|u| format!("username={},", u)).unwrap_or_default(),
            pass.as_ref().map(|p| format!("password={}", p)).unwrap_or("password=".into()),
        );
        args.extend_from_slice(&["-o", opt.as_str()]);
        let result = std::process::Command::new("sudo").args(&args).status();
        match result {
            Ok(s) if s.success() => {
                ns.status_msg = Some(format!("Mounted at {}", mp));
                state.navigate(mp);
                state.panels.close();
            }
            _ => { ns.smb_error = Some("Mount failed. Check credentials or install cifs-utils".into()); }
        }
    }
}

fn tab_nfs(ui: &mut Ui, t: &AexTheme, state: &mut AppState, ns: &mut NetworkState) {
    let c  = &t.colors; let sp = &t.spacing; let s = &t.shape;

    ui.label(RichText::new("Server Address").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.xs);
    ui.add(egui::TextEdit::singleline(&mut ns.nfs_host)
        .hint_text("192.168.1.1 or hostname").desired_width(f32::INFINITY)
        .font(FontId::new(t.typography.size_sm, FontFamily::Monospace)));

    ui.add_space(sp.sm);
    ui.label(RichText::new("Remote Path").color(c.text_hint()).size(t.typography.size_xs));
    ui.add_space(sp.xs);
    ui.add(egui::TextEdit::singleline(&mut ns.nfs_path)
        .hint_text("/exports/data").desired_width(f32::INFINITY)
        .font(FontId::new(t.typography.size_sm, FontFamily::Monospace)));

    if let Some(ref e) = ns.nfs_error.clone() {
        ui.add_space(sp.xs);
        ui.label(RichText::new(e).color(c.danger()).size(t.typography.size_xs));
    }

    ui.add_space(sp.md);
    ui.label(RichText::new("Requirements: sudo pacman -S nfs-utils")
        .color(c.text_disabled()).size(t.typography.size_xs).italics());
    ui.add_space(sp.sm);

    let can = !ns.nfs_host.is_empty() && !ns.nfs_path.is_empty();
    if ui.add_enabled(can, Button::new(
        RichText::new(format!("{} NFS Mount", icon(t, "mount")))
            .color(Color32::WHITE).size(t.typography.size_sm))
        .fill(if can { c.accent() } else { c.bg_surface1() })
        .rounding(Rounding::same(s.radius_sm)).stroke(Stroke::NONE)).clicked() {
        let src = format!("{}:{}", ns.nfs_host, ns.nfs_path);
        let mp  = format!("/run/media/network/{}_{}",
            ns.nfs_host, ns.nfs_path.replace('/', "_"));
        std::fs::create_dir_all(&mp).ok();
        let result = std::process::Command::new("sudo")
            .args(["mount", "-t", "nfs", &src, &mp]).status();
        match result {
            Ok(s) if s.success() => { state.navigate(mp); state.panels.close(); }
            _ => { ns.nfs_error = Some("NFS mount failed. Check server and nfs-utils".into()); }
        }
    }
}

fn tab_connected(ui: &mut Ui, t: &AexTheme, state: &mut AppState, ns: &mut NetworkState) {
    let c  = &t.colors; let sp = &t.spacing; let s = &t.shape;

    if ns.connected.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new(icon(t, "server")).color(c.text_disabled()).size(32.0));
            ui.add_space(sp.sm);
            ui.label(RichText::new("No network shares connected")
                .color(c.text_hint()).size(t.typography.size_sm));
        });
        return;
    }

    for share in &ns.connected.clone() {
        ui.add_space(sp.xs);
        egui::Frame {
            fill: c.bg_surface0(), rounding: Rounding::same(s.radius_md),
            inner_margin: Margin::same(sp.sm), ..Default::default()
        }.show(ui, |ui| {
            ui.set_width(480.0 - sp.xl * 2.0 - sp.sm * 2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon(t, "server")).color(c.accent()).size(16.0));
                ui.add_space(sp.xs);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("//{}/{}", share.host, share.share))
                        .color(c.text_primary()).size(t.typography.size_sm));
                    ui.label(RichText::new(format!("{} · {}", share.mount_point, share.fs_type))
                        .color(c.text_hint()).size(t.typography.size_xs).monospace());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mp = share.mount_point.clone();
                    if ui.add(Button::new(RichText::new("Unmount").color(c.danger()).size(t.typography.size_xs))
                        .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, c.danger()))
                        .rounding(Rounding::same(s.radius_sm))).clicked() {
                        std::process::Command::new("sudo")
                            .args(["umount", &mp]).status().ok();
                        ns.connected = list_mounted_shares();
                    }
                    ui.add_space(sp.xs);
                    if ui.add(Button::new(RichText::new("Open").color(c.accent()).size(t.typography.size_xs))
                        .fill(Color32::from_rgba_unmultiplied(c.accent().r(),c.accent().g(),c.accent().b(),20))
                        .stroke(Stroke::NONE).rounding(Rounding::same(s.radius_sm))).clicked() {
                        state.navigate(mp.clone());
                        state.panels.close();
                    }
                });
            });
        });
    }

    ui.add_space(sp.md);
    if ui.add(Button::new(RichText::new("Refresh").color(c.text_hint()).size(t.typography.size_xs))
        .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
        ns.connected = list_mounted_shares();
    }
}
