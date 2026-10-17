use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Background, BoxShadow, Global, Hsla, linear_color_stop, linear_gradient, px, rgb};

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
    pub on_color: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub brand: Hsla,
    pub brand_end: Hsla,
    pub mods: Hsla,
    pub maps: Hsla,
    pub profiles: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
}

impl Global for Palette {}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

const HUES: [u32; 8] = [
    0x8b7cff, 0x4cc9f0, 0x3ddc97, 0xffc857, 0xff6b9d, 0x5b8cff, 0xff9f43, 0x2ed3b7,
];

pub fn hue(key: &str) -> Hsla {
    let h = key.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    hex(HUES[h as usize % HUES.len()])
}

pub fn deepen(color: Hsla) -> Hsla {
    Hsla {
        h: (color.h + 0.06).fract(),
        l: color.l * 0.78,
        ..color
    }
}

pub fn gradient(from: Hsla, to: Hsla) -> Background {
    linear_gradient(135., linear_color_stop(from, 0.), linear_color_stop(to, 1.))
}

pub fn glow(color: Hsla, strength: f32) -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.), px(8.), color.opacity(strength)).blur_radius(px(24.))]
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            background: hex(0x0d0f15),
            surface: hex(0x151822),
            elevated: hex(0x1c2030),
            chrome: hex(0x10121a),
            border: hex(0x2c3247),
            border_variant: hex(0x1f2434),
            hover: hex(0x1b1f2d),
            switch_off: hex(0x3a4158),
            selected: hex(0x242a3c),
            text: hex(0xeceef6),
            text_muted: hex(0xa3a9c0),
            text_placeholder: hex(0x6c7391),
            on_color: hex(0xffffff),
            accent: hex(0x8b7cff),
            accent_hover: hex(0xa396ff),
            brand: hex(0xff4757),
            brand_end: hex(0xff9f43),
            mods: hex(0x8b7cff),
            maps: hex(0x2ed3b7),
            profiles: hex(0xff6b9d),
            success: hex(0x3ddc97),
            warning: hex(0xffc857),
            error: hex(0xff6b6b),
        }
    }

    pub fn brand_gradient(&self) -> Background {
        linear_gradient(
            100.,
            linear_color_stop(self.brand, 0.),
            linear_color_stop(self.brand_end, 1.),
        )
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
        t.radius = px(8.);
        t.radius_lg = px(12.);
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
        t.primary_foreground = p.on_color;
        t.secondary = p.elevated;
        t.secondary_hover = p.hover;
        t.secondary_active = p.selected;
        t.secondary_foreground = p.text;
        t.button = p.elevated;
        t.button_hover = p.selected;
        t.button_active = p.selected;
        t.button_foreground = p.text;
        t.button_primary = p.accent;
        t.button_primary_hover = p.accent_hover;
        t.button_primary_active = p.accent;
        t.button_primary_foreground = p.on_color;
        t.popover = p.elevated;
        t.popover_foreground = p.text;
        t.colors.list = p.background;
        t.list_hover = p.hover;
        t.list_active = p.selected;
        t.list_active_border = p.accent;
        t.sidebar = p.chrome;
        t.sidebar_border = p.border_variant;
        t.sidebar_foreground = p.text;
        t.title_bar = p.chrome;
        t.title_bar_border = p.border_variant;
        t.status_bar = p.chrome;
        t.status_bar_border = p.border_variant;
        t.window_border = p.border;
        t.switch = p.switch_off;
        t.switch_thumb = p.on_color;
        t.success = p.success;
        t.warning = p.warning;
        t.danger = p.error;
    });
    cx.set_global(p);
}
