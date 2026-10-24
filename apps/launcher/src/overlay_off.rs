use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::overlay_guard::{self, OverlayOff};

use crate::game_edited::discord_link;
use crate::pages::{game_art, label};
use crate::shell::Shell;
use crate::theme::{Palette, palette};

const TITLE: &str = "The in-game overlay is off";

/// The `config.news_seen` key that records Keep it off for one GPU-removed record.
pub(crate) fn seen_key(off: &OverlayOff) -> String {
    format!("overlay-off-{}", off.at as u64)
}

pub(crate) fn why(off: &OverlayOff) -> String {
    let when = off
        .removal
        .as_ref()
        .map(|r| match (r.after_draw_ms + 500) / 1000 {
            0 | 1 => "about a second".to_owned(),
            n => format!("about {n} seconds"),
        });
    format!(
        "Your graphics card stopped responding{} after an addon drew over the game, so Swofty turned the \
         in-game overlay off. Your addons still run, but nothing they draw over the game shows.",
        when.map_or_else(String::new, |w| format!(" {w}"))
    )
}

pub(crate) fn advice() -> String {
    format!(
        "Turn it back on to try again. If the game stops again, keep it off and press {}.",
        crate::send_logs::label()
    )
}

pub(crate) fn addons_text(painters: &[(String, String)]) -> Vec<String> {
    if painters.is_empty() {
        return vec!["None of the addons on in this profile draw over the game.".to_owned()];
    }
    painters.iter().map(|(_, name)| name.clone()).collect()
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
        .child(Icon::new(IconName::Layers).size(px(12.)).text_color(p.chrome));
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

fn addons_box(painters: &[(String, String)], p: Palette) -> impl IntoElement {
    let none = painters.is_empty();
    v_flex()
        .gap(px(6.))
        .child(label("ADDONS THAT DRAW OVER THE GAME", p))
        .child(
            v_flex()
                .gap(px(2.))
                .p(px(9.))
                .rounded(px(6.))
                .bg(p.elevated)
                .border_1()
                .border_color(p.border)
                .text_size(px(12.))
                .children(addons_text(painters).into_iter().map(move |name| {
                    h_flex()
                        .gap(px(7.))
                        .items_center()
                        .when(!none, |d| {
                            d.child(
                                Icon::new(IconName::Layers)
                                    .size(px(13.))
                                    .flex_shrink_0()
                                    .text_color(p.warning),
                            )
                        })
                        .when(none, |d| d.text_color(p.text_muted))
                        .child(div().min_w_0().truncate().child(name))
                })),
        )
}

impl Shell {
    pub(crate) fn show_overlay_off(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(off) = self.overlay_off.clone() else {
            return;
        };
        let painters = overlay_guard::painters(&self.store, &self.store.active_profile());
        let shell = cx.entity().downgrade();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, window, _| {
            let vh = f32::from(window.viewport_size().height);
            let top = ((vh - 520.) / 2.).max(12.);
            let (keep, reset) = (shell.clone(), shell.clone());
            let (keep_off, reset_off) = (off.clone(), off.clone());
            alert
                .width(px(560.))
                .margin_top(px(top))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .pt(px(2.))
                        .child(illustration(p))
                        .child(
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
                                        .child(why(&off)),
                                ),
                        )
                        .child(addons_box(&painters, p))
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(p.text_muted)
                                .child(advice()),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .w_full()
                        .justify_between()
                        .child(discord_link("overlay-off-discord", p))
                        .child(
                            h_flex()
                                .gap(px(8.))
                                .child(
                                    Button::new("overlay-off-keep")
                                        .ghost()
                                        .h(px(36.))
                                        .px(px(14.))
                                        .label("Keep it off")
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let off = keep_off.clone();
                                            keep.update(cx, |s, _| s.keep_overlay_off(&off)).ok();
                                        }),
                                )
                                .child(
                                    Button::new("overlay-off-reset")
                                        .primary()
                                        .h(px(36.))
                                        .px(px(14.))
                                        .icon(IconName::Power)
                                        .label("Turn the overlay back on")
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let off = reset_off.clone();
                                            reset
                                                .update(cx, |s, cx| s.turn_overlay_on(&off, window, cx))
                                                .ok();
                                        }),
                                ),
                        ),
                )
        });
    }

    pub(crate) fn keep_overlay_off(&mut self, off: &OverlayOff) {
        let mut config = self.store.config();
        if config.news_seen.insert(seen_key(off)) {
            self.store.save_config(&config).ok();
        }
    }

    pub(crate) fn turn_overlay_on(&mut self, off: &OverlayOff, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = off
            .path
            .parent()
            .and_then(|d| d.parent())
            .map(|d| d.to_path_buf())
        else {
            return;
        };
        match overlay_guard::reset(&game) {
            Ok(_) => {
                self.overlay_off = None;
                window.push_notification(
                    Notification::success("Addons can draw over the game again from your next launch.")
                        .title("The in-game overlay is back on"),
                    cx,
                );
            }
            Err(e) => window.push_notification(
                Notification::error(e.to_string()).title("The overlay could not be turned back on"),
                cx,
            ),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{addons_text, advice, seen_key, why};
    use swofty_core::overlay_guard::{OverlayOff, Removal};

    fn off(after_draw_ms: Option<u64>) -> OverlayOff {
        OverlayOff {
            path: "/games/F1 25/swofty/overlay-gpu-removed.json".into(),
            at: 1_792_800_000.5,
            removal: after_draw_ms.map(|ms| Removal {
                after_draw_ms: ms,
                ..Removal::default()
            }),
        }
    }

    #[test]
    fn the_notice_says_what_happened_in_plain_words() {
        assert_eq!(
            why(&off(Some(4812))),
            "Your graphics card stopped responding about 5 seconds after an addon drew over the game, so \
             Swofty turned the in-game overlay off. Your addons still run, but nothing they draw over the game \
             shows."
        );
        assert!(why(&off(None)).starts_with("Your graphics card stopped responding after an addon drew"));
        assert!(why(&off(Some(1200))).contains("responding about a second after"));
    }

    #[test]
    fn the_notice_names_the_addons_that_draw_over_the_game() {
        let painters = [
            ("kizi.pitlane".to_owned(), "Manual Pit Lane".to_owned()),
            ("kizi.canned".to_owned(), "Canned DNFs + Safety Car".to_owned()),
        ];
        assert_eq!(
            addons_text(&painters),
            ["Manual Pit Lane", "Canned DNFs + Safety Car"]
        );
        assert_eq!(
            addons_text(&[]),
            ["None of the addons on in this profile draw over the game."]
        );
    }

    #[test]
    fn the_advice_names_the_send_logs_button() {
        assert!(advice().ends_with(&format!("keep it off and press {}.", crate::send_logs::label())));
    }

    #[test]
    fn keep_it_off_is_remembered_per_record() {
        assert_eq!(seen_key(&off(None)), "overlay-off-1792800000");
    }
}
