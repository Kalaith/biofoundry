//! Title menu screen.

use crate::data::GameData;
use crate::ui::{UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

pub fn draw(data: &GameData, ui: &VirtualUi) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    draw_backdrop();

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

    let hint = "Feed the warren · forge with living furnaces · awaken the Colossal Worm";
    let hint_w = measure_text(hint, None, 17, 1.0).width;
    draw_ui_text_ex(
        hint,
        (LOGICAL_WIDTH - hint_w) * 0.5,
        432.0,
        TextStyle::new(17.0, dark::TEXT_DIM).params(),
    );

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

/// A quiet cavern tableau behind the title: a worm silhouette arcing
/// through the dark, mushroom clusters along the floor.
fn draw_backdrop() {
    // The worm: a broad arc of dim segments across the upper screen.
    for i in 0..14 {
        let t = i as f32 / 13.0;
        let x = LOGICAL_WIDTH * (0.08 + 0.84 * t);
        let y = 120.0 - (t * std::f32::consts::PI).sin() * 70.0 + 430.0;
        let r = 46.0 - (t - 0.5).abs() * 30.0;
        draw_circle(x, y, r, Color::new(0.16, 0.13, 0.19, 1.0));
        draw_circle_lines(x, y, r, 2.0, Color::new(0.30, 0.24, 0.38, 0.8));
    }

    // Mushroom clusters along the cave floor.
    let clusters = [
        (90.0, 690.0, 1.2),
        (240.0, 705.0, 0.8),
        (1050.0, 695.0, 1.1),
        (1180.0, 708.0, 0.7),
        (620.0, 712.0, 0.9),
    ];
    for (cx, cy, s) in clusters {
        for (dx, h, r) in [(-18.0, 26.0, 13.0), (2.0, 40.0, 18.0), (22.0, 20.0, 10.0)] {
            let stem = Color::new(0.30, 0.27, 0.22, 1.0);
            let cap = Color::new(0.52, 0.44, 0.30, 1.0);
            draw_rectangle(cx + dx * s - 3.0 * s, cy - h * s, 6.0 * s, h * s, stem);
            draw_circle(cx + dx * s, cy - h * s, r * s, cap);
        }
    }

    // Drifting spores.
    for i in 0..24 {
        let x = (i as f32 * 157.3) % LOGICAL_WIDTH;
        let y = 80.0 + (i as f32 * 97.7) % 560.0;
        draw_circle(x, y, 2.0, Color::new(0.55, 0.60, 0.45, 0.20));
    }
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
