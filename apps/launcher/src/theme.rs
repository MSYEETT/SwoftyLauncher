use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, px, rgb};

#[derive(Clone, Copy)]
pub struct Palette {
    pub chrome: Hsla,
    pub background: Hsla,
    pub surface: Hsla,
    pub hover: Hsla,
    pub elevated: Hsla,
    pub selected: Hsla,
    pub border_variant: Hsla,
    pub border: Hsla,
    pub switch_off: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_placeholder: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub on_accent: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
}

impl Global for Palette {}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            chrome: hex(0x181613),
            background: hex(0x1e1b19),
            surface: hex(0x24211e),
            hover: hex(0x282521),
            elevated: hex(0x2d2a26),
            selected: hex(0x383530),
            border_variant: hex(0x302d29),
            border: hex(0x403c38),
            switch_off: hex(0x4b4742),
            text: hex(0xeae7e4),
            text_muted: hex(0xa9a49d),
            text_placeholder: hex(0x7c7670),
            accent: hex(0xfaab35),
            accent_hover: hex(0xffbe5c),
            on_accent: hex(0x150e06),
            success: hex(0x61cb7c),
            warning: hex(0xeab532),
            error: hex(0xf66d67),
        }
    }
}

pub fn palette(cx: &App) -> Palette {
    *cx.global::<Palette>()
}

pub fn init(cx: &mut App) {
    let p = Palette::dark();
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        t.font_size = px(14.);
        t.radius = px(5.);
        t.radius_lg = px(6.);
        t.shadow = false;
        t.background = p.background;
        t.foreground = p.text;
        t.border = p.border;
        t.input = p.border;
        t.ring = p.accent;
        t.caret = p.accent;
        t.selection = p.selected;
        t.muted = p.elevated;
        t.muted_foreground = p.text_muted;
        t.accent = p.hover;
        t.accent_foreground = p.text;
        t.primary = p.accent;
        t.primary_hover = p.accent_hover;
        t.primary_active = p.accent;
        t.primary_foreground = p.on_accent;
        t.secondary = p.elevated;
        t.secondary_hover = p.selected;
        t.secondary_active = p.selected;
        t.secondary_foreground = p.text;
        t.button = p.elevated;
        t.button_hover = p.selected;
        t.button_active = p.selected;
        t.button_foreground = p.text;
        t.button_primary = p.accent;
        t.button_primary_hover = p.accent_hover;
        t.button_primary_active = p.accent;
        t.button_primary_foreground = p.on_accent;
        t.link = p.text;
        t.link_hover = p.accent;
        t.link_active = p.accent;
        t.popover = p.elevated;
        t.popover_foreground = p.text;
        t.colors.list = p.background;
        t.list_hover = p.hover;
        t.list_active = p.selected;
        t.list_active_border = p.border;
        t.sidebar = p.chrome;
        t.sidebar_border = p.border_variant;
        t.sidebar_foreground = p.text;
        t.title_bar = p.chrome;
        t.title_bar_border = p.border_variant;
        t.status_bar = p.chrome;
        t.status_bar_border = p.border_variant;
        t.window_border = p.border;
        t.switch = p.switch_off;
        t.switch_thumb = p.text;
        t.success = p.success;
        t.warning = p.warning;
        t.danger = p.error;
    });
    cx.set_global(p);
}
