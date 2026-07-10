//! Title menu screen.

use crate::data::GameData;
use crate::ui::{UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

pub fn draw(data: &GameData, ui: &VirtualUi) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    let title = &data.config.display_name;
    let title_size = 72.0;
    let title_w = measure_text(title, None, title_size as u16, 1.0).width;
    draw_ui_text_ex(
        title,
        (LOGICAL_WIDTH - title_w) * 0.5,
        220.0,
        TextStyle::new(title_size, dark::TEXT_BRIGHT).params(),
    );

    let tagline = "Every conveyor belt is a creature with needs.";
    let tagline_w = measure_text(tagline, None, 22, 1.0).width;
    draw_ui_text_ex(
        tagline,
        (LOGICAL_WIDTH - tagline_w) * 0.5,
        262.0,
        TextStyle::new(22.0, dark::TEXT_DIM).params(),
    );

    let btn = Rect::new(LOGICAL_WIDTH * 0.5 - 130.0, 340.0, 260.0, 54.0);
    if menu_button(btn, "New Warren", mouse) {
        actions.push(UiAction::StartWarren);
    }

    let footer = format!("v{} — a WebHatchery game", data.config.version);
    let footer_w = measure_text(&footer, None, 16, 1.0).width;
    draw_ui_text_ex(
        &footer,
        (LOGICAL_WIDTH - footer_w) * 0.5,
        LOGICAL_HEIGHT - 32.0,
        TextStyle::new(16.0, dark::TEXT_DIM).params(),
    );

    actions
}

fn menu_button(rect: Rect, text: &str, mouse: Vec2) -> bool {
    let hovered = rect.contains_point(mouse);
    let pressed = hovered && is_mouse_button_down(MouseButton::Left);
    let fill = if pressed {
        Color::new(0.14, 0.22, 0.16, 1.0)
    } else if hovered {
        Color::new(0.18, 0.30, 0.21, 1.0)
    } else {
        Color::new(0.12, 0.20, 0.14, 1.0)
    };
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(1.0, dark::POSITIVE),
    );
    draw_text_centered_in_box_ex(
        text,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(24.0, dark::TEXT_BRIGHT),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}
