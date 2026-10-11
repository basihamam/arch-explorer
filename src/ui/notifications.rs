// ─────────────────────────────────────────────────────────────────────────────

use egui::*;
use crate::theme::types::AexTheme;
use crate::state::{AppState, NotifyKind};

pub fn render(ctx: &Context, t: &AexTheme, state: &mut AppState) {
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    state.notify.tick();

    if state.notify.items.is_empty() { return; }

    let screen = ctx.screen_rect();
    let toast_w = 280.0;
    let toast_h = 48.0;
    let gap     = 8.0;
    let margin  = 16.0;

    for (i, notif) in state.notify.items.iter().enumerate() {
        let y = screen.bottom() - margin - (toast_h + gap) * (i + 1) as f32;

        let (icon_str, accent) = match notif.kind {
            NotifyKind::Success => (super::icon(t, "success"), c.success()),
            NotifyKind::Error   => (super::icon(t, "error"),   c.danger()),
            NotifyKind::Warning => (super::icon(t, "warning"), c.warning()),
            NotifyKind::Info    => (super::icon(t, "info"),    c.info()),
        };

        // Fade out animasyonu
        let elapsed = notif.created.elapsed().as_millis() as f32;
        let fade_start = notif.duration_ms as f32 - 400.0;
        let alpha = if elapsed > fade_start {
            ((notif.duration_ms as f32 - elapsed) / 400.0).clamp(0.0, 1.0)
        } else {
            1.0
        };

        egui::Area::new(Id::new(format!("toast_{}", i)))
            .fixed_pos(Pos2::new(screen.right() - toast_w - margin, y))
            .order(Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame {
                    fill: Color32::from_rgba_unmultiplied(
                        c.bg_mantle().r(), c.bg_mantle().g(), c.bg_mantle().b(),
                        (220.0 * alpha) as u8
                    ),
                    rounding: Rounding::same(s.radius_md),
                    stroke: Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(
                        accent.r(), accent.g(), accent.b(), (100.0 * alpha) as u8
                    )),
                    shadow: epaint::Shadow {
                        offset: Vec2::new(0.0, 2.0),
                        blur: 8.0,
                        spread: 0.0,
                        color: Color32::from_rgba_unmultiplied(0,0,0,(40.0*alpha) as u8),
                    },
                    inner_margin: Margin::symmetric(sp.md, sp.sm),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.set_width(toast_w);
                    ui.horizontal(|ui| {
                        // Accent left strip
                        let strip_rect = Rect::from_min_size(
                            ui.min_rect().min,
                            Vec2::new(3.0, toast_h - sp.sm * 2.0),
                        );
                        ui.painter().rect_filled(
                            strip_rect,
                            Rounding::same(1.5),
                            Color32::from_rgba_unmultiplied(
                                accent.r(), accent.g(), accent.b(), (200.0 * alpha) as u8
                            ),
                        );

                        ui.add_space(sp.sm);

                        // Icon
                        ui.label(
                            RichText::new(icon_str)
                                .color(Color32::from_rgba_unmultiplied(
                                    accent.r(), accent.g(), accent.b(), (255.0 * alpha) as u8
                                ))
                                .size(t.icons.size_md)
                        );

                        // Message
                        ui.label(
                            RichText::new(&notif.message)
                                .color(Color32::from_rgba_unmultiplied(
                                    c.text_primary().r(), c.text_primary().g(), c.text_primary().b(),
                                    (255.0 * alpha) as u8
                                ))
                                .size(t.typography.size_sm)
                        );
                    });
                });
            });

        // Animasyon için repaint
        if alpha < 1.0 {
            ctx.request_repaint();
        }
    }
}
