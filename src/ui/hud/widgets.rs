//! The HUD's shared look: the panel surface style and the one button.

use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;

pub(super) fn panel_style() -> SurfaceStyle {
    SurfaceStyle::new(Color::new(0.07, 0.08, 0.10, 0.94))
        .with_border(1.0, Color::new(0.38, 0.45, 0.58, 0.55))
        .with_header(34.0, Color::new(0.09, 0.105, 0.13, 1.0))
        .with_header_divider(1.0, Color::new(0.38, 0.45, 0.58, 0.4))
}

pub(super) fn hud_button(rect: Rect, text: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let fill = if !enabled {
        Color::new(0.10, 0.11, 0.13, 1.0)
    } else if hovered {
        Color::new(0.20, 0.22, 0.28, 1.0)
    } else {
        Color::new(0.13, 0.145, 0.18, 1.0)
    };
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(1.0, Color::new(0.5, 0.55, 0.65, 0.5)),
    );
    draw_text_centered_in_box_ex(
        text,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(15.0, if enabled { dark::TEXT } else { dark::TEXT_DIM }),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}
