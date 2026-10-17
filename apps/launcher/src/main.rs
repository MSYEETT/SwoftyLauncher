#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod console;
mod game;
mod library;
mod pages;
mod play_bar;
mod sections;
mod shell;
mod theme;
mod update;

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
                titlebar: Some(TitlebarOptions {
                    title: Some("Swofty Launcher for F1 25".into()),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            };
            let (main, shell) = gpui_kit::open_window(options, cx, |_, cx| cx.new(shell::Shell::new))
                .expect("failed to open window");
            cx.on_window_closed(move |cx, id| {
                if id == main.window_id() {
                    cx.quit();
                }
            })
            .detach();
            shell.update(cx, |s, cx| s.restore_console(cx));
            cx.activate(true);
        });
}
