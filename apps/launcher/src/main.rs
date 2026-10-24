#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod addons;
mod blocks;
mod browser;
mod console;
mod db;
mod discord;
mod ea_app;
mod game;
mod game_edited;
mod importer;
mod library;
mod news;
mod pages;
mod play_bar;
mod reveal;
mod sections;
mod send_logs;
mod setup;
mod shell;
mod tasks;
mod theme;
mod tools;
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
            let (main, shell) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| shell::Shell::new(window, cx))
            })
            .expect("failed to open window");
            cx.on_window_closed(move |cx, id| {
                if id == main.window_id() {
                    cx.quit();
                }
            })
            .detach();
            shell.update(cx, |s, cx| s.restore_console(cx));
            main.update(cx, |_, window, cx| {
                shell.update(cx, |s, cx| s.run_startup(window, cx))
            })
            .ok();
            cx.activate(true);
        });
}
