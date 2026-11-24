use eframe::egui::{self, Color32, CornerRadius, FontData, FontFamily, FontId, Stroke, vec2};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

// Omarchy's Tokyo Night roles, as in the panel.
pub const BACKGROUND: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x26);
pub const FOREGROUND: Color32 = Color32::from_rgb(0xa9, 0xb1, 0xd6);
pub const DIM: Color32 = Color32::from_rgb(0x56, 0x5f, 0x89);
pub const ACCENT: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);
pub const RED: Color32 = Color32::from_rgb(0xf7, 0x76, 0x8e);
pub const BLUE: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);

// Omarchy's type scale and spacing at its default base size of 12.
pub const CAPTION: f32 = 10.0;
// The map's credit must be there, but should hardly be seen.
pub const CREDIT: f32 = 9.0;
// Opacity of everything drawn over the map: the map is guessed through it, and the
// text stays easy to read.
pub const SEE_THROUGH: f32 = 0.88;
pub const BODY: f32 = 12.0;
pub const ROW_HEIGHT: f32 = 28.0;
pub const ROW_PADDING: f32 = 12.0;
pub const GAP: f32 = 8.0;

pub fn omarchy_look(ctx: &egui::Context) {
    ctx.add_font(FontInsert::new(
        "JetBrains Mono",
        FontData::from_static(include_bytes!(
            "../../bar/ui/fonts/JetBrainsMono-Regular.ttf"
        )),
        vec![
            InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            },
            InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Highest,
            },
        ],
    ));

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = BACKGROUND;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.override_text_color = Some(FOREGROUND);
    visuals.selection.bg_fill = mix(0.35);
    visuals.selection.stroke = Stroke::new(1.0, FOREGROUND);
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    visuals.window_stroke = Stroke::new(2.0, ACCENT);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    // Save waits greyed until there is a summit and a name; at egui's 0.5, over the
    // see-through fill, it was hard to read.
    visuals.disabled_alpha = 0.7;
    // Omarchy's [controls]: fill and border strength for idle, hover and press. The
    // buttons sit on the map, so they let it show through.
    let widgets = &mut visuals.widgets;
    for (state, fill, border) in [
        (&mut widgets.inactive, 0.04, 0.4),
        (&mut widgets.hovered, 0.08, 0.25),
        (&mut widgets.active, 0.22, 0.25),
        (&mut widgets.open, 0.08, 0.25),
    ] {
        state.bg_fill = translucent(mix(fill), SEE_THROUGH);
        state.weak_bg_fill = translucent(mix(fill), SEE_THROUGH);
        state.bg_stroke = Stroke::new(1.0, translucent(FOREGROUND, border));
        state.fg_stroke = Stroke::new(1.0, FOREGROUND);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);

    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        for (text_style, size) in [
            (TextStyle::Small, CAPTION),
            (TextStyle::Body, BODY),
            (TextStyle::Button, BODY),
            (TextStyle::Monospace, BODY),
            (TextStyle::Heading, 16.0),
        ] {
            style
                .text_styles
                .insert(text_style, FontId::proportional(size));
        }
        style.spacing.button_padding = vec2(10.0, 6.0);
        style.spacing.interact_size.y = ROW_HEIGHT;
        style.spacing.item_spacing = vec2(GAP, GAP / 2.0);
    });
}

// The panel's popups: background with a 2 px accent border and square corners.
pub fn popup_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(translucent(BACKGROUND, SEE_THROUGH))
        .stroke(Stroke::new(2.0, ACCENT))
        .inner_margin(4.0)
}

pub fn translucent(color: Color32, alpha: f32) -> Color32 {
    let alpha = channel(alpha * 255.0);
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

// The foreground over the background at Omarchy's fill strengths.
pub fn mix(alpha: f32) -> Color32 {
    let blend = |back: u8, front: u8| {
        channel(f32::from(back) + (f32::from(front) - f32::from(back)) * alpha)
    };
    Color32::from_rgb(
        blend(BACKGROUND.r(), FOREGROUND.r()),
        blend(BACKGROUND.g(), FOREGROUND.g()),
        blend(BACKGROUND.b(), FOREGROUND.b()),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=255 first"
)]
fn channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_mix_foreground_into_background() {
        assert_eq!(mix(0.0), BACKGROUND);
        assert_eq!(mix(1.0), FOREGROUND);
    }
}
