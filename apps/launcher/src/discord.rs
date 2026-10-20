use std::sync::{Arc, LazyLock};

use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::store::Store;

use crate::shell::Shell;
use crate::theme::{Palette, palette};

pub const INVITE: &str = "https://discord.gg/STWsgyJKZS";
pub const LABEL: &str = "Join the Discord";

const NEWS: &str = "discord";
const BODY: &str = "Join the Swofty Developments Discord for support and updates. Ask the developers \
                    for help when something goes wrong, and hear about new releases and addons first.";
const LOGO: &[u8] = include_bytes!("../assets/discord.svg");
const BLURPLE: u32 = 0x5865f2;
const BLURPLE_HOVER: u32 = 0x4752c4;

fn server_icon(size: f32) -> Div {
    static ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/discord-server.png").to_vec(),
        ))
    });
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.28))
        .overflow_hidden()
        .child(img(ICON.clone()).size_full())
}

fn banner() -> Stateful<Div> {
    let white = |alpha: f32| hsla(0., 0., 1., alpha);
    h_flex()
        .id("news-banner")
        .relative()
        .w_full()
        .h(px(176.))
        .px(px(28.))
        .gap(px(20.))
        .items_center()
        .rounded(px(10.))
        .bg(rgb(BLURPLE))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(BLURPLE_HOVER)))
        .on_click(|_, _, cx| cx.open_url(INVITE))
        .child(
            svg()
                .data(LOGO)
                .absolute()
                .top(px(24.))
                .right(px(28.))
                .size(px(128.))
                .text_color(white(0.16)),
        )
        .child(server_icon(76.))
        .child(
            v_flex()
                .min_w_0()
                .gap(px(4.))
                .text_color(white(1.))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(white(0.8))
                        .child(svg().data(LOGO).size(px(14.)).text_color(white(0.8)))
                        .child("DISCORD SERVER"),
                )
                .child(
                    div()
                        .text_size(px(24.))
                        .font_weight(FontWeight::BOLD)
                        .child("Swofty Developments"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(white(0.75))
                        .child(INVITE.trim_start_matches("https://")),
                ),
        )
}

fn mark_seen(store: &Store) {
    let mut config = store.config();
    if config.news_seen.insert(NEWS.to_owned())
        && let Err(e) = store.save_config(&config)
    {
        eprintln!("swofty-launcher: could not remember that the news was closed: {e}");
    }
}

impl Shell {
    pub(crate) fn show_news(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.store.config().news_seen.contains(NEWS) {
            return;
        }
        let store = self.store.clone();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let store = store.clone();
            alert
                .width(px(600.))
                .title("News")
                .child(
                    v_flex()
                        .gap(px(14.))
                        .child(banner())
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(BODY)),
                )
                .ok_text(LABEL)
                .cancel_text("Close")
                .show_cancel(true)
                .on_ok(|_, _, cx| {
                    cx.open_url(INVITE);
                    true
                })
                .on_close(move |_, _, _| mark_seen(&store))
        });
    }

    pub(crate) fn discord_button(&self, p: Palette) -> impl IntoElement {
        h_flex()
            .id("discord")
            .min_h(px(36.))
            .px(px(10.))
            .py(px(6.))
            .gap(px(9.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .text_color(p.text_muted)
            .hover(move |s| s.bg(p.hover).border_color(p.border).text_color(p.text))
            .on_click(|_, _, cx| cx.open_url(INVITE))
            .child(
                svg()
                    .data(LOGO)
                    .size(px(15.))
                    .flex_shrink_0()
                    .text_color(rgb(BLURPLE)),
            )
            .child(div().flex_1().min_w_0().child(LABEL))
    }
}
