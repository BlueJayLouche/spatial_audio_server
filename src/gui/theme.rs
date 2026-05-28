use egui::{Color32, Stroke, Style, Visuals};

pub const DARK_BG: Color32 = Color32::from_rgb(18, 18, 22);
pub const PANEL_BG: Color32 = Color32::from_rgb(28, 28, 34);
pub const ACCENT: Color32 = Color32::from_rgb(70, 150, 210);
pub const TEXT: Color32 = Color32::from_rgb(210, 210, 215);
pub const DIM_TEXT: Color32 = Color32::from_rgb(110, 110, 125);
pub const LEVEL_GREEN: Color32 = Color32::from_rgb(60, 200, 80);
pub const LEVEL_YELLOW: Color32 = Color32::from_rgb(230, 190, 40);
pub const LEVEL_RED: Color32 = Color32::from_rgb(220, 60, 60);

pub const SIDE_PANEL_WIDTH: f32 = 360.0;
pub const LOG_MAX: usize = 256;
pub const FLOORPLAN_SPEAKER_RADIUS: f32 = 8.0;
pub const FLOORPLAN_SOUND_RADIUS: f32 = 5.0;

pub fn apply(ctx: &egui::Context) {
    let mut style = Style::default();
    let mut visuals = Visuals::dark();
    visuals.window_fill = PANEL_BG;
    visuals.panel_fill = PANEL_BG;
    visuals.extreme_bg_color = DARK_BG;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, DIM_TEXT);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(70, 150, 210, 70);
    visuals.hyperlink_color = ACCENT;
    style.visuals = visuals;
    ctx.set_global_style(style);
}

/// Map a normalised 0..=1 level to the appropriate meter colour.
pub fn level_color(level: f32) -> Color32 {
    if level > 0.85 { LEVEL_RED }
    else if level > 0.6 { LEVEL_YELLOW }
    else { LEVEL_GREEN }
}
