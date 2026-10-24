use std::path::Path;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::compat;
use swofty_core::install::Source;

use crate::game::Install;
use crate::shell::Shell;

const TITLE: &str = "F1 25 needs administrator rights";

/// Where the player moves the game, by the store it came from.
fn move_hint(source: Source) -> &'static str {
    match source {
        Source::EaApp => " in the EA app",
        Source::Steam => " in Steam (Properties > Installed Files > Move install folder)",
        Source::Unknown => "",
    }
}

pub(crate) fn text(dir: &Path, source: Source, windows: bool) -> String {
    if !windows {
        return format!("{}.", compat::write_blocked_text_for(dir, false));
    }
    let mut text = format!(
        "F1 25 is installed in {}, a folder Windows lets only administrators change, so Swofty can't \
         add your mods there.\n\nRestart Swofty Launcher as administrator to play with mods, or move \
         F1 25 to a folder outside Program Files{}.",
        dir.display(),
        move_hint(source)
    );
    if source == Source::EaApp {
        text.push_str(" Close the EA app before you restart, so it opens as administrator too.");
    }
    text
}

impl Shell {
    /// The game folder is one this launcher can't write in: say so and offer
    /// to restart as administrator, on Windows.
    pub(crate) fn show_admin_needed(
        &mut self,
        install: &Install,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let windows = cfg!(windows);
        let text = text(&install.dir, install.source, windows);
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .width(px(560.))
                .title(TITLE)
                .description(text.clone())
                .map(|a| {
                    if !windows {
                        return a.ok_text("OK");
                    }
                    a.ok_text("Restart as administrator")
                        .show_cancel(true)
                        .on_ok(|_, window, cx| {
                            match compat::relaunch_elevated() {
                                Ok(()) => cx.quit(),
                                Err(why) => window.push_notification(
                                    Notification::warning(format!(
                                        "Windows didn't restart Swofty Launcher: {why}. It keeps running \
                                         without administrator rights."
                                    ))
                                    .title("Not restarted as administrator"),
                                    cx,
                                ),
                            }
                            true
                        })
                })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::text;
    use std::path::Path;
    use swofty_core::install::Source;

    #[test]
    fn names_the_folder_and_both_ways_out() {
        let ea = text(Path::new(r"C:\Program Files\EA Games\F1 25"), Source::EaApp, true);
        assert!(
            ea.starts_with(r"F1 25 is installed in C:\Program Files\EA Games\F1 25, a folder"),
            "{ea}"
        );
        assert!(ea.contains("Restart Swofty Launcher as administrator"), "{ea}");
        assert!(ea.contains("outside Program Files in the EA app."), "{ea}");
        assert!(ea.ends_with("Close the EA app before you restart, so it opens as administrator too."));
        let steam = text(
            Path::new(r"C:\Program Files\Steam\steamapps\common\F1 25"),
            Source::Steam,
            true,
        );
        assert!(steam.contains("Move install folder"), "{steam}");
        assert!(!steam.contains("EA app"), "{steam}");
        let linux = text(Path::new("/games/F1 25"), Source::Steam, false);
        assert!(
            linux.contains("/games/F1 25") && !linux.contains("administrator"),
            "{linux}"
        );
    }
}
