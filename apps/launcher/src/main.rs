#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod data;
mod shell;
mod theme;

use gpui_kit::component::TitleBar;
use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1120.), px(720.)),
                    cx,
                ))),
                window_min_size: Some(size(px(760.), px(480.))),
                app_id: Some("net.swofty.ModLauncher".into()),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| shell::Shell::new()))
                .expect("failed to open window");
            cx.activate(true);
        });
}
