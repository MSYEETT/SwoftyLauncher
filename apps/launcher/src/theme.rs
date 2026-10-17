use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, px, rgb};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Hsla,
    pub surface: Hsla,
    pub elevated: Hsla,
    pub chrome: Hsla,
    pub border: Hsla,
    pub border_variant: Hsla,
    pub hover: Hsla,
    pub switch_off: Hsla,
    pub selected: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_placeholder: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
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
            background: hex(0x282c33),
            surface: hex(0x2f343e),
            elevated: hex(0x353b45),
            chrome: hex(0x3b414d),
            border: hex(0x464b57),
            border_variant: hex(0x363c46),
            hover: hex(0x363c46),
            switch_off: hex(0x5f6573),
            selected: hex(0x454a56),
            text: hex(0xdce0e5),
            text_muted: hex(0xa9afbc),
            text_placeholder: hex(0x878a98),
            accent: hex(0x74ade8),
            accent_hover: hex(0x8bbcef),
            success: hex(0xa1c181),
            warning: hex(0xdec184),
            error: hex(0xd07277),
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
        t.font_size = px(16.);
        t.radius = px(6.);
        t.radius_lg = px(10.);
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
        t.primary_foreground = p.background;
        t.secondary = p.elevated;
        t.secondary_hover = p.hover;
        t.secondary_active = p.selected;
        t.secondary_foreground = p.text;
        t.button = p.elevated;
        t.button_hover = p.hover;
        t.button_active = p.selected;
        t.button_foreground = p.text;
        t.button_primary = p.accent;
        t.button_primary_hover = p.accent_hover;
        t.button_primary_active = p.accent;
        t.button_primary_foreground = p.background;
        t.popover = p.surface;
        t.popover_foreground = p.text;
        t.colors.list = p.background;
        t.list_hover = p.hover;
        t.list_active = p.selected;
        t.list_active_border = p.accent;
        t.sidebar = p.surface;
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
