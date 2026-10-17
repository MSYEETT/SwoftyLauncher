#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod game;
mod library;
mod pages;
mod play_bar;
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
                    size(px(1440.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(1024.), px(680.))),
                app_id: Some("net.swofty.ModLauncher".into()),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |_, cx| cx.new(shell::Shell::new))
                .expect("failed to open window");
            cx.activate(true);
        });
}
