//! Title menu screen.

use crate::data::GameData;
use crate::ui::{UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

pub fn draw(
    data: &GameData,
    ui: &VirtualUi,
    save_exists: bool,
    settings_open: bool,
    sfx_volume: f32,
) -> Vec<UiAction> {
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

    if settings_open {
        draw_settings_panel(mouse, sfx_volume, &mut actions);
    } else {
        let x = LOGICAL_WIDTH * 0.5 - 130.0;
        let mut y = 316.0;
        let entries: [(&str, bool, UiAction); 4] = [
            ("New Warren", true, UiAction::StartWarren),
            ("Continue", save_exists, UiAction::Load),
            ("Settings", true, UiAction::ToggleSettings),
            ("Exit Game", true, UiAction::ExitGame),
        ];
        for (label, enabled, action) in entries {
            if menu_button(Rect::new(x, y, 260.0, 48.0), label, enabled, mouse) {
                actions.push(action);
            }
            y += 58.0;
        }
    }

    let hint = "Feed the warren · forge with living furnaces · awaken the Colossal Worm";
    let hint_w = measure_text(hint, None, 17, 1.0).width;
    draw_ui_text_ex(
        hint,
        (LOGICAL_WIDTH - hint_w) * 0.5,
        596.0,
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

/// The volume stepper and a Done button, in place of the main menu stack.
fn draw_settings_panel(mouse: Vec2, sfx_volume: f32, actions: &mut Vec<UiAction>) {
    let panel = Rect::new(LOGICAL_WIDTH * 0.5 - 170.0, 316.0, 340.0, 168.0);
    draw_surface_with_title(
        panel,
        Some("Settings"),
        &SurfaceStyle::new(Color::new(0.07, 0.08, 0.10, 0.96))
            .with_border(1.0, Color::new(0.38, 0.45, 0.58, 0.55))
            .with_header(34.0, Color::new(0.09, 0.105, 0.13, 1.0))
            .with_header_divider(1.0, Color::new(0.38, 0.45, 0.58, 0.4)),
        TextStyle::new(17.0, dark::TEXT),
    );

    let y = panel.y + 56.0;
    draw_ui_text_ex(
        "Sound volume",
        panel.x + 20.0,
        y + 21.0,
        TextStyle::new(17.0, dark::TEXT).params(),
    );
    if menu_button(
        Rect::new(panel.right() - 152.0, y, 36.0, 30.0),
        "-",
        sfx_volume > 0.0,
        mouse,
    ) {
        actions.push(UiAction::AdjustVolume(-1));
    }
    draw_text_centered_in_box_ex(
        &format!("{:.0}%", sfx_volume * 100.0),
        panel.right() - 116.0,
        y,
        64.0,
        30.0,
        TextStyle::new(16.0, dark::TEXT_BRIGHT),
    );
    if menu_button(
        Rect::new(panel.right() - 52.0, y, 36.0, 30.0),
        "+",
        sfx_volume < 1.0,
        mouse,
    ) {
        actions.push(UiAction::AdjustVolume(1));
    }

    if menu_button(
        Rect::new(
            panel.x + (panel.w - 120.0) * 0.5,
            panel.bottom() - 46.0,
            120.0,
            32.0,
        ),
        "Done",
        true,
        mouse,
    ) {
        actions.push(UiAction::ToggleSettings);
    }
}

fn menu_button(rect: Rect, text: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let pressed = hovered && is_mouse_button_down(MouseButton::Left);
    let fill = if !enabled {
        Color::new(0.10, 0.13, 0.11, 1.0)
    } else if pressed {
        Color::new(0.14, 0.22, 0.16, 1.0)
    } else if hovered {
        Color::new(0.18, 0.30, 0.21, 1.0)
    } else {
        Color::new(0.12, 0.20, 0.14, 1.0)
    };
    let border = if enabled {
        dark::POSITIVE
    } else {
        Color::new(0.30, 0.36, 0.32, 0.6)
    };
    draw_surface(rect, &SurfaceStyle::new(fill).with_border(1.0, border));
    draw_text_centered_in_box_ex(
        text,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(
            (rect.h * 0.45).min(24.0),
            if enabled {
                dark::TEXT_BRIGHT
            } else {
                dark::TEXT_DIM
            },
        ),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}
