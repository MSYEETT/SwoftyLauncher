use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::{Icon, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::install::Source;

use swofty_core::recovery::Blocked;
use swofty_core::stock;

use crate::discord::{BLURPLE, INVITE, LOGO};
use crate::pages::{game_art, label};
use crate::shell::{Page, Shell};
use crate::theme::{Palette, palette};

const TITLE: &str = "Your game files have been edited";

/// A blocked launch the player can fix: files that aren't Steam's copy, or
/// files Swofty did not put in the game folder.
#[derive(Clone)]
pub(crate) enum Edited {
    NotStock(stock::Report),
    Leftovers(Blocked),
}

impl Edited {
    fn files(&self) -> Vec<String> {
        match self {
            Self::NotStock(r) => r.files.clone(),
            Self::Leftovers(b) => b.files.iter().map(|f| f.path.clone()).collect(),
        }
    }

    fn leftovers(&self) -> bool {
        matches!(self, Self::Leftovers(_))
    }

    fn why(&self) -> &'static str {
        match self {
            Self::NotStock(_) => "It found game files that aren't the copy Steam installed",
            Self::Leftovers(_) => "It found files in the F1 25 folder that it didn't put there",
        }
    }
}

fn illustration(p: Palette) -> impl IntoElement {
    let badge = div()
        .absolute()
        .bottom(px(-5.))
        .right(px(-5.))
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(p.warning)
        .border_2()
        .border_color(p.surface)
        .child(Icon::new(IconName::Pencil).size(px(12.)).text_color(p.chrome));
    h_flex()
        .w_full()
        .justify_center()
        .py(px(7.))
        .rounded(px(10.))
        .bg(p.surface)
        .border_1()
        .border_color(p.border_variant)
        .child(div().relative().child(game_art(48., 48.)).child(badge))
}

fn heading(edited: &Edited, p: Palette) -> impl IntoElement {
    let why = format!(
        "Swofty keeps F1 25 fully restorable: it only changes files it owns and undoes them when \
         you quit. {}, so it stopped before touching anything. Files edited by another tool or \
         copied in by hand break that and can crash the game.",
        edited.why()
    );
    v_flex()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .child(
            div()
                .text_center()
                .text_size(px(18.))
                .line_height(px(23.))
                .font_weight(FontWeight::BOLD)
                .text_color(p.text)
                .child(TITLE),
        )
        .child(
            div()
                .text_center()
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(p.text_muted)
                .child(why),
        )
}

fn files_box(edited: &Edited, p: Palette) -> impl IntoElement {
    let files = edited.files();
    let shown = files.len().min(6);
    v_flex()
        .gap(px(6.))
        .child(label("THE FILES SWOFTY FOUND", p))
        .child(
            v_flex()
                .gap(px(2.))
                .p(px(9.))
                .rounded(px(6.))
                .bg(p.elevated)
                .border_1()
                .border_color(p.border)
                .text_size(px(12.))
                .children(files[..shown].iter().map(|f| {
                    h_flex()
                        .gap(px(7.))
                        .items_center()
                        .child(
                            Icon::new(IconName::FilePen)
                                .size(px(13.))
                                .flex_shrink_0()
                                .text_color(p.warning),
                        )
                        .child(div().min_w_0().truncate().child(f.clone()))
                }))
                .when(files.len() > shown, |d| {
                    d.child(
                        div()
                            .pl(px(21.))
                            .text_color(p.text_muted)
                            .child(format!("and {} more", files.len() - shown)),
                    )
                }),
        )
}

fn step(n: u32, title: &'static str, body: &'static str, p: Palette) -> impl IntoElement {
    h_flex()
        .gap(px(12.))
        .items_start()
        .child(
            div()
                .size(px(20.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(p.selected)
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.accent)
                .child(n.to_string()),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(p.text_muted)
                        .child(body),
                ),
        )
}

fn steps(edited: &Edited, source: Source, p: Palette) -> impl IntoElement {
    let (verify, how) = verify_steps(source);
    v_flex()
        .gap(px(9.))
        .child(label("WHAT TO DO", p))
        .child(step(
            1,
            "A mod was copied into the game folder",
            "If you put mod files in the F1 25 folder by hand, take them back out.",
            p,
        ))
        .child(step(2, verify, how, p))
        .child(step(
            3,
            "Add mods the safe way",
            "Use the Simplified Importer below to add a mod as an addon Swofty installs and removes cleanly.",
            p,
        ))
        .when(edited.leftovers(), |d| {
            d.child(step(
                4,
                "Or restore the game folder",
                "Restore game folder below moves the files aside and puts the game's own files back.",
                p,
            ))
        })
}

/// How the active copy's store checks the game files.
pub(crate) fn verify_steps(source: Source) -> (&'static str, &'static str) {
    match source {
        Source::EaApp => (
            "Repair the game in the EA app",
            "EA app > Library > F1 25 > the three dots > Repair, then press Play again.",
        ),
        _ => (
            "Verify the game files in Steam",
            "Steam > right-click F1 25 > Properties > Installed Files > Verify integrity of game \
             files, then press Play again.",
        ),
    }
}

fn discord_link(p: Palette) -> Stateful<Div> {
    h_flex()
        .id("game-edited-discord")
        .h(px(36.))
        .px(px(10.))
        .gap(px(8.))
        .items_center()
        .rounded(px(8.))
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(p.text_muted)
        .hover(|s| s.bg(p.hover).text_color(p.text))
        .child(svg().data(LOGO).size(px(15.)).text_color(rgb(BLURPLE)))
        .child("Get help on Discord")
        .on_click(|_, _, cx| cx.open_url(INVITE))
}

fn footer(shell: WeakEntity<Shell>, leftovers: bool, p: Palette) -> DialogFooter {
    let importer = shell.clone();
    let restore = shell.clone();
    DialogFooter::new()
        .w_full()
        .justify_between()
        .child(discord_link(p))
        .child(
            h_flex()
                .gap(px(8.))
                .child(
                    Button::new("game-edited-importer")
                        .outline()
                        .h(px(36.))
                        .px(px(14.))
                        .label("Go to Simplified Importer")
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            importer.update(cx, |s, cx| s.open_page(Page::Importer, cx)).ok();
                        }),
                )
                .when(leftovers, |d| {
                    d.child(
                        Button::new("game-edited-restore")
                            .primary()
                            .h(px(36.))
                            .px(px(14.))
                            .icon(IconName::RotateCcw)
                            .label("Restore game folder")
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                restore.update(cx, |s, cx| s.restore_game_folder(window, cx)).ok();
                            }),
                    )
                })
                .child(
                    Button::new("game-edited-close")
                        .map(|b| if leftovers { b.ghost() } else { b.primary() })
                        .h(px(36.))
                        .px(px(14.))
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
        )
}

impl Shell {
    /// The modal for a launch stopped because the game files were edited: files
    /// that aren't Steam's copy, or files Swofty did not put in the folder. It
    /// lists the files and what to do; the Leftovers case also offers Restore
    /// game folder. The play bar reopens it from [`Shell::game_edited`].
    pub(crate) fn show_game_edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edited) = self.game_edited.clone() else {
            return;
        };
        let shell = cx.entity().downgrade();
        let p = palette(cx);
        let source = self.install.as_ref().map_or(Source::Steam, |i| i.source);
        window.open_alert_dialog(cx, move |alert, window, _| {
            let (shell, edited) = (shell.clone(), edited.clone());
            let vh = f32::from(window.viewport_size().height);
            let top = ((vh - 660.) / 2.).max(12.);
            alert
                .width(px(620.))
                .margin_top(px(top))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .pt(px(2.))
                        .child(illustration(p))
                        .child(heading(&edited, p))
                        .child(files_box(&edited, p))
                        .child(steps(&edited, source, p)),
                )
                .footer(footer(shell, edited.leftovers(), p))
        });
    }
}
