use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState, NumberInput};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Map, Value};
use swofty_core::view::{
    Align, Block, Body, Card, Cell, Field, Form, Grid, Hero, Meter, Pill, Search, Table, Tabs, Tile, Tone,
    View,
};

use crate::pages::label;
use crate::sections::{art, icon, tone};
use crate::shell::{Page, Shell};
use crate::theme::Palette;

const GAP: f32 = 16.;
const GRID_COLUMN: f32 = 280.;
const TILE_COLUMN: f32 = 170.;

#[derive(Clone, Copy)]
pub(crate) struct At<'a> {
    pub addon: &'a str,
    pub section: &'a str,
    pub width: f32,
    pub nested: bool,
    pub fill: bool,
}

impl At<'_> {
    pub(crate) fn key(&self, id: &str) -> String {
        format!("{}/{}/{id}", self.addon, self.section)
    }

    fn inner(self, width: f32) -> Self {
        Self {
            width,
            nested: true,
            fill: false,
            ..self
        }
    }
}

pub(crate) enum Reset {
    No,
    Now,
    After(u64),
}

pub(crate) struct FormSlot {
    inputs: HashMap<String, Entity<InputState>>,
    sliders: HashMap<String, Entity<SliderState>>,
    picks: HashMap<String, Value>,
    synced: HashMap<String, Value>,
    pub(crate) reset: Reset,
    seen: bool,
    _subs: Vec<Subscription>,
}

pub(crate) struct SearchSlot {
    state: Entity<InputState>,
    pub(crate) query: String,
    generation: u64,
    seen: bool,
    _sub: Subscription,
}

pub(crate) struct CellEdit {
    key: String,
    addon: String,
    section: String,
    table: String,
    row: String,
    column: String,
    field: Field,
    original: Value,
    input: Entity<InputState>,
    _subs: Vec<Subscription>,
}

pub(crate) fn pill(p: &Pill, pal: Palette) -> Div {
    let color = tone(p.tone, pal);
    let (bg, fg) = match p.tone {
        Tone::Neutral => (pal.elevated, pal.text),
        Tone::Muted => (pal.hover, pal.text_muted),
        _ => (color.opacity(0.14), color),
    };
    h_flex()
        .h(px(22.))
        .px(px(9.))
        .gap(px(6.))
        .flex_shrink_0()
        .rounded(px(11.))
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .when(p.dot, |d| d.child(div().size(px(6.)).rounded_full().bg(fg)))
        .when_some(p.icon.filter(|_| !p.dot), |d, i| {
            d.child(Icon::new(icon(i)).size(px(12.)))
        })
        .child(p.text.clone())
}

fn pills(items: &[Pill], pal: Palette) -> Div {
    h_flex()
        .gap(px(6.))
        .flex_wrap()
        .children(items.iter().map(|p| pill(p, pal)))
}

fn bar(fraction: f32, color: Hsla, height: f32, pal: Palette) -> Div {
    div()
        .h(px(height))
        .w_full()
        .rounded_full()
        .bg(pal.elevated)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0., 1.)))
                .rounded_full()
                .bg(color),
        )
}

fn fill_tone(t: Option<Tone>, pal: Palette) -> Hsla {
    match t {
        None | Some(Tone::Neutral) => pal.accent,
        Some(t) => tone(t, pal),
    }
}

fn tile_columns(count: usize, width: f32) -> usize {
    let wanted = match count {
        0..=4 => count.max(1),
        5 | 6 => 3,
        _ => 4,
    };
    wanted.min(((width + GAP) / (TILE_COLUMN + GAP)).floor().max(1.) as usize)
}

fn surface(pal: Palette) -> Div {
    v_flex()
        .rounded(px(8.))
        .border_1()
        .border_color(pal.border_variant)
        .bg(pal.surface)
}

fn number_text(text: &str) -> Option<f64> {
    let cleaned: String = text.chars().filter(|c| !matches!(c, ',' | ' ' | '_')).collect();
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn field_text(field: &Field, value: &Value) -> String {
    match (field, value) {
        (Field::Number(f) | Field::Slider(f), Value::Number(n)) => {
            n.as_f64().map(|v| f.text(v)).unwrap_or_default()
        }
        (_, Value::String(s)) => s.clone(),
        (_, Value::Null) => String::new(),
        (_, v) => v.to_string(),
    }
}

fn sorted_rows(t: &Table, sort: Option<&(String, bool)>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..t.rows.len()).collect();
    let Some((column, desc)) = sort else {
        return order;
    };
    let Some(ci) = t.columns.iter().position(|c| c.key() == column) else {
        return order;
    };
    let key = |i: usize| t.rows[i].cells().get(ci).map(Cell::sort_key);
    order.sort_by(|a, b| match (key(*a), key(*b)) {
        (Some(x), Some(y)) => x.compare(&y),
        (x, y) => x.is_some().cmp(&y.is_some()).reverse(),
    });
    if *desc {
        order.reverse();
    }
    order
}

pub(crate) fn walk<'a>(blocks: &'a [Block], out: &mut Vec<&'a Block>) {
    for b in blocks {
        out.push(b);
        match b {
            Block::Card(c) => walk(&c.blocks, out),
            Block::Grid(g) => walk(&g.blocks, out),
            _ => {}
        }
    }
}

fn view_blocks(view: &View) -> Vec<&Block> {
    let mut out = Vec::new();
    match &view.body {
        Body::Blocks(blocks) => walk(blocks, &mut out),
        Body::List { rows, .. } => {
            for r in rows {
                walk(&r.detail, &mut out);
            }
        }
        Body::Empty { .. } => {}
    }
    out
}

pub(crate) fn search_id(search: &Search, fallback: &str) -> String {
    search.id.clone().unwrap_or_else(|| fallback.to_owned())
}

impl Shell {
    fn current_view(&self) -> Option<(String, String, View, u64)> {
        let Page::Section(i) = self.page else {
            return None;
        };
        let s = self.library.sections.get(i)?;
        let key = format!("{}/{}", s.addon, s.spec.id);
        let view = self.views.get(&key)?.as_ref().ok()?.clone();
        let generation = self.view_gens.get(&key).copied().unwrap_or(0);
        Some((s.addon.clone(), s.spec.id.clone(), view, generation))
    }

    pub(crate) fn find_form(&self, addon: &str, section: &str, id: &str) -> Option<Form> {
        let view = self.views.get(&format!("{addon}/{section}"))?.as_ref().ok()?;
        view_blocks(view).into_iter().find_map(|b| match b {
            Block::Form(f) if f.id == id => Some(f.clone()),
            _ => None,
        })
    }

    pub(crate) fn sync_blocks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((addon, section, view, generation)) = self.current_view() else {
            self.forms.clear();
            self.searches.clear();
            self.cell = None;
            return;
        };
        let prefix = format!("{addon}/{section}/");
        for slot in self.forms.values_mut() {
            slot.seen = false;
        }
        for slot in self.searches.values_mut() {
            slot.seen = false;
        }
        if let Body::List { search: Some(s), .. } = &view.body {
            let key = format!("{prefix}{}", search_id(s, "list"));
            self.sync_search(&key, &addon, &section, s, window, cx);
        }
        for b in view_blocks(&view) {
            match b {
                Block::Form(f) => self.sync_form(&addon, &section, f, generation, window, cx),
                Block::Table(t) => {
                    if let Some(s) = &t.search {
                        let key = format!("{prefix}{}", search_id(s, t.id()));
                        self.sync_search(&key, &addon, &section, s, window, cx);
                    }
                }
                _ => {}
            }
        }
        self.forms.retain(|k, s| k.starts_with(&prefix) && s.seen);
        self.searches.retain(|k, s| k.starts_with(&prefix) && s.seen);
        if self.cell.as_ref().is_some_and(|c| !c.key.starts_with(&prefix)) {
            self.cell = None;
        }
    }

    fn sync_search(
        &mut self,
        key: &str,
        addon: &str,
        section: &str,
        search: &Search,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.searches.get_mut(key) {
            slot.seen = true;
            return;
        }
        let placeholder = search.placeholder.clone().unwrap_or_else(|| "Search".into());
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let (key_owned, addon, section, id, event) = (
            key.to_owned(),
            addon.to_owned(),
            section.to_owned(),
            search.id.clone(),
            search.event,
        );
        let fallback = key.rsplit('/').next().unwrap_or("list").to_owned();
        let sub = cx.subscribe_in(&state, window, move |this, state, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::Change) {
                return;
            }
            let query = state.read(cx).value().to_string();
            let Some(slot) = this.searches.get_mut(&key_owned) else {
                return;
            };
            if slot.query == query {
                return;
            }
            slot.query = query.clone();
            slot.generation += 1;
            cx.notify();
            if !event {
                return;
            }
            let (generation, key, addon, section) =
                (slot.generation, key_owned.clone(), addon.clone(), section.clone());
            let id = id.clone().unwrap_or_else(|| fallback.clone());
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                this.update_in(cx, |s, window, cx| {
                    if s.searches
                        .get(&key)
                        .is_some_and(|slot| slot.generation == generation)
                    {
                        let event = swofty_core::view::Event::Search { id, query };
                        s.send(&addon, &section, key.clone(), event, window, cx);
                    }
                })
                .ok();
            })
            .detach();
        });
        self.searches.insert(
            key.to_owned(),
            SearchSlot {
                state,
                query: String::new(),
                generation: 0,
                seen: true,
                _sub: sub,
            },
        );
    }

    pub(crate) fn query(&self, key: &str) -> Option<&str> {
        self.searches
            .get(key)
            .map(|s| s.query.as_str())
            .filter(|q| !q.trim().is_empty())
    }

    pub(crate) fn search_box(&self, key: &str, p: Palette) -> Option<AnyElement> {
        let slot = self.searches.get(key)?;
        Some(
            div()
                .w(px(280.))
                .child(
                    Input::new(&slot.state).small().cleanable(true).prefix(
                        Icon::new(IconName::Search)
                            .size(px(14.))
                            .text_color(p.text_placeholder),
                    ),
                )
                .into_any_element(),
        )
    }

    fn sync_form(
        &mut self,
        addon: &str,
        section: &str,
        form: &Form,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{addon}/{section}/{}", form.id);
        if !self.forms.contains_key(&key) {
            self.forms.insert(
                key.clone(),
                FormSlot {
                    inputs: HashMap::new(),
                    sliders: HashMap::new(),
                    picks: HashMap::new(),
                    synced: HashMap::new(),
                    reset: Reset::No,
                    seen: true,
                    _subs: Vec::new(),
                },
            );
        }
        let force = match self.forms[&key].reset {
            Reset::Now => true,
            Reset::After(g) => generation > g,
            Reset::No => false,
        };
        let mut subs = Vec::new();
        for field in &form.fields {
            let id = field.id().to_owned();
            let value = field.value();
            let current = self.field_value(&key, field, cx);
            let slot = self.forms.get_mut(&key).expect("inserted above");
            let synced = slot.synced.get(&id).cloned();
            let edited = synced.as_ref().is_some_and(|s| current.as_ref() != Some(s));
            let apply = force || synced.is_none() || (!edited && synced.as_ref() != Some(&value));
            match field {
                Field::Number(_) | Field::Text(_) if !slot.inputs.contains_key(&id) => {
                    let text = field_text(field, &value);
                    let state = cx.new(|cx| {
                        let mut s = InputState::new(window, cx).default_value(text);
                        if let Field::Number(nf) = field {
                            s = s.step(nf.step());
                            if let Some(min) = nf.min {
                                s = s.min(min);
                            }
                            if let Some(max) = nf.max {
                                s = s.max(max);
                            }
                        }
                        if let Field::Text(tf) = field
                            && let Some(ph) = tf.placeholder.clone()
                        {
                            s = s.placeholder(ph);
                        }
                        s
                    });
                    let (field_id, addon, section, form_id) =
                        (id.clone(), addon.to_owned(), section.to_owned(), form.id.clone());
                    subs.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, _, ev: &InputEvent, window, cx| {
                            let commit = matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur);
                            if matches!(ev, InputEvent::Change) {
                                cx.notify();
                            }
                            if !commit {
                                return;
                            }
                            let Some(form) = this.find_form(&addon, &section, &form_id) else {
                                return;
                            };
                            if form.live {
                                this.submit_form(&addon, &section, &form, Some(&field_id), window, cx);
                            } else if matches!(ev, InputEvent::PressEnter { .. }) {
                                this.submit_form(&addon, &section, &form, None, window, cx);
                            }
                        },
                    ));
                    slot.inputs.insert(id.clone(), state);
                }
                Field::Number(_) | Field::Text(_) => {
                    if apply && let Some(state) = slot.inputs.get(&id).cloned() {
                        let text = field_text(field, &value);
                        state.update(cx, |s, cx| s.set_value(text, window, cx));
                    }
                }
                Field::Slider(nf) => {
                    let at = value.as_f64().unwrap_or(0.) as f32;
                    if let Some(state) = slot.sliders.get(&id).cloned() {
                        if apply {
                            state.update(cx, |s, cx| s.set_value(at, window, cx));
                        }
                    } else {
                        let state = cx.new(|_| {
                            SliderState::new()
                                .min(nf.min.unwrap_or(0.) as f32)
                                .max(nf.max.unwrap_or(100.) as f32)
                                .step(nf.step() as f32)
                                .default_value(at)
                        });
                        let (addon, section, form_id, field_id) =
                            (addon.to_owned(), section.to_owned(), form.id.clone(), id.clone());
                        subs.push(cx.subscribe_in(
                            &state,
                            window,
                            move |this, _, ev: &SliderEvent, window, cx| {
                                cx.notify();
                                if !matches!(ev, SliderEvent::Release(_)) {
                                    return;
                                }
                                if let Some(form) =
                                    this.find_form(&addon, &section, &form_id).filter(|f| f.live)
                                {
                                    this.submit_form(&addon, &section, &form, Some(&field_id), window, cx);
                                }
                            },
                        ));
                        slot.sliders.insert(id.clone(), state);
                    }
                }
                Field::Select(_) | Field::Toggle(_) => {
                    if force || slot.picks.get(&id) == Some(&value) {
                        slot.picks.remove(&id);
                    }
                }
            }
            let slot = self.forms.get_mut(&key).expect("inserted above");
            if apply {
                slot.synced.insert(id, value);
            }
        }
        let slot = self.forms.get_mut(&key).expect("inserted above");
        if force {
            slot.reset = Reset::No;
        }
        slot.seen = true;
        slot._subs.extend(subs);
    }

    fn field_value(&self, key: &str, field: &Field, cx: &App) -> Option<Value> {
        let slot = self.forms.get(key)?;
        let id = field.id();
        match field {
            Field::Number(nf) => {
                let text = slot.inputs.get(id)?.read(cx).value().to_string();
                if text.trim().is_empty() {
                    return nf.value.is_none().then_some(Value::Null);
                }
                number_text(&text).map(|v| nf.json(nf.clamp(v)))
            }
            Field::Slider(nf) => {
                let v = slot.sliders.get(id)?.read(cx).value().start() as f64;
                Some(nf.json(nf.clamp(v)))
            }
            Field::Text(tf) => {
                let mut text = slot.inputs.get(id)?.read(cx).value().to_string();
                if let Some(max) = tf.max_len {
                    text = text.chars().take(max).collect();
                }
                Some(Value::String(text))
            }
            Field::Select(_) | Field::Toggle(_) => {
                Some(slot.picks.get(id).cloned().unwrap_or_else(|| field.value()))
            }
        }
    }

    fn form_changes(&self, key: &str, form: &Form, cx: &App) -> (Vec<String>, bool) {
        let mut changed = Vec::new();
        let mut valid = true;
        for f in &form.fields {
            match self.field_value(key, f, cx) {
                Some(v) if v != f.value() => changed.push(f.id().to_owned()),
                Some(_) => {}
                None if self
                    .forms
                    .get(key)
                    .is_some_and(|s| !s.inputs.contains_key(f.id()) && !s.sliders.contains_key(f.id())) => {}
                None => {
                    valid = false;
                    changed.push(f.id().to_owned());
                }
            }
        }
        (changed, valid)
    }

    pub(crate) fn submit_form(
        &mut self,
        addon: &str,
        section: &str,
        form: &Form,
        only: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{addon}/{section}/{}", form.id);
        if self.busy.contains(&key) || form.disabled {
            return;
        }
        let mut values = Map::new();
        let mut changed = Vec::new();
        for f in form.fields.iter().filter(|f| only.is_none_or(|o| o == f.id())) {
            let Some(v) = self.field_value(&key, f, cx) else {
                let name = f.label().unwrap_or(f.id()).to_owned();
                window.push_notification(Notification::error(format!("{name} is not a number")), cx);
                return;
            };
            if v != f.value() {
                changed.push(f.id().to_owned());
            }
            values.insert(f.id().to_owned(), v);
        }
        if changed.is_empty() {
            return;
        }
        let event = swofty_core::view::Event::Submit {
            form: form.id.clone(),
            row: None,
            values,
            changed,
        };
        self.send(addon, section, key, event, window, cx);
    }

    fn reset_form(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(slot) = self.forms.get_mut(key) {
            slot.reset = Reset::Now;
        }
        cx.notify();
    }

    pub(crate) fn settled(&mut self, control: &str, effect: &Result<swofty_core::view::Effect, String>) {
        use swofty_core::view::Effect;
        self.pending_cells.remove(control);
        if let Some(slot) = self.forms.get_mut(control) {
            slot.reset = match effect {
                Ok(Effect::Reload | Effect::Notice(_)) => Reset::After(self.view_gen),
                Ok(_) => Reset::Now,
                Err(_) => Reset::No,
            };
        }
    }

    fn pick(
        &mut self,
        key: &str,
        id: &str,
        value: Value,
        live: Option<(String, String, Form)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.forms.get_mut(key) {
            slot.picks.insert(id.to_owned(), value);
        }
        if let Some((addon, section, form)) = live {
            self.submit_form(&addon, &section, &form, Some(id), window, cx);
        }
        cx.notify();
    }

    pub(crate) fn rich_block(
        &self,
        at: At,
        b: &Block,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        Some(match b {
            Block::Hero(h) => self.hero(at, h, p, cx),
            Block::Pills(items) => pills(items, p).into_any_element(),
            Block::Tiles(tiles) => self.tiles(at, tiles, p),
            Block::Meters(meters) => self.meters(meters, p),
            Block::Card(c) => self.card(at, c, p, cx),
            Block::Grid(g) => self.grid(at, g, p, cx),
            Block::Form(f) => self.form(at, f, p, cx),
            Block::Tabs(t) => self.tabs(at, t, p, cx),
            Block::Table(t) if !t.is_plain() => self.table(at, t, p, cx),
            _ => return None,
        })
    }

    fn tabs(&self, at: At, t: &Tabs, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let active = t.active.clone().or_else(|| t.items.first().map(|i| i.id.clone()));
        let items = t.items.iter().map(|item| {
            let on = active.as_deref() == Some(item.id.as_str());
            let control = format!("{}/{}/{}:{}", at.addon, at.section, t.id, item.id);
            let busy = self.busy.contains(&control);
            let fg = if item.disabled {
                p.text_placeholder
            } else if on {
                p.on_accent
            } else {
                p.text_muted
            };
            let mut tab = div()
                .id(SharedString::from(format!("tab-{control}")))
                .h(px(32.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(7.))
                .rounded(px(7.))
                .text_size(px(13.))
                .font_weight(if on {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(fg)
                .whitespace_nowrap()
                .when(on, |d| d.bg(p.accent))
                .when_some(item.icon, |d, i| d.child(Icon::new(icon(i)).size(px(14.))))
                .child(item.label.clone())
                .when_some(item.badge.clone(), |d, badge| {
                    d.child(
                        div()
                            .h(px(18.))
                            .min_w(px(18.))
                            .px(px(5.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(9.))
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .bg(if on { p.on_accent.opacity(0.22) } else { p.elevated })
                            .text_color(if on { p.on_accent } else { p.text })
                            .child(badge),
                    )
                })
                .when(busy, |d| d.opacity(0.6));
            if !on && !item.disabled && !busy {
                let (addon, section, id, tab_id) = (
                    at.addon.to_owned(),
                    at.section.to_owned(),
                    t.id.clone(),
                    item.id.clone(),
                );
                tab = tab
                    .cursor_pointer()
                    .hover(|d| d.bg(p.hover).text_color(p.text))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        let event = swofty_core::view::Event::Tab {
                            id: id.clone(),
                            tab: tab_id.clone(),
                        };
                        this.send(&addon, &section, control.clone(), event, window, cx);
                    }));
            }
            tab
        });
        h_flex()
            .p(px(4.))
            .gap(px(4.))
            .flex_wrap()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .children(items)
            .into_any_element()
    }

    fn hero(&self, at: At, h: &Hero, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let buttons: Vec<AnyElement> = h
            .buttons
            .iter()
            .map(|b| self.button(at.addon, at.section, b, true, cx))
            .collect();
        let facts = (!h.facts.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_x(px(28.))
                .gap_y(px(10.))
                .pt(px(14.))
                .border_t_1()
                .border_color(p.border_variant)
                .children(h.facts.iter().map(|f| {
                    v_flex()
                        .gap(px(3.))
                        .child(label(f.label.to_uppercase(), p))
                        .child(div().text_size(px(13.)).text_color(p.text).child(f.value.clone()))
                }))
        });
        surface(p)
            .p(px(20.))
            .gap(px(16.))
            .child(
                h_flex()
                    .gap(px(14.))
                    .items_center()
                    .when(!h.art.is_none(), |d| d.child(art(&h.art, 48., p)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.))
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .flex_wrap()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.text)
                                            .child(h.title.clone()),
                                    )
                                    .child(pills(&h.pills, p)),
                            )
                            .when_some(h.subtitle.clone(), |d, s| {
                                d.child(div().text_size(px(13.)).text_color(p.text_muted).child(s))
                            }),
                    )
                    .when(!buttons.is_empty(), |d| {
                        d.child(h_flex().gap(px(6.)).children(buttons))
                    }),
            )
            .children(facts)
            .into_any_element()
    }

    fn tiles(&self, at: At, tiles: &[Tile], p: Palette) -> AnyElement {
        let cols = tile_columns(tiles.len(), at.width);
        let tile = |t: &Tile| {
            let value_color = tone(t.tone, p);
            v_flex()
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .px(px(16.))
                .pt(px(14.))
                .pb(px(if t.progress.is_some() { 12. } else { 14. }))
                .gap(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border_variant)
                .bg(p.surface)
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(label(t.label.to_uppercase(), p).flex_1().min_w_0().truncate())
                        .when_some(t.icon, |d, i| {
                            d.child(Icon::new(icon(i)).size(px(14.)).text_color(p.text_placeholder))
                        }),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_baseline()
                        .child(
                            div()
                                .text_size(px(24.))
                                .line_height(px(30.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(value_color)
                                .truncate()
                                .child(t.value.clone()),
                        )
                        .when_some(t.delta.clone(), |d, delta| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(tone(t.delta_tone(), p))
                                    .whitespace_nowrap()
                                    .child(delta),
                            )
                        }),
                )
                .when_some(t.hint.clone(), |d, h| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(h),
                    )
                })
                .when_some(t.progress, |d, f| {
                    let color = fill_tone(Some(t.tone), p);
                    d.child(div().pt(px(4.)).child(bar(f, color, 3., p)))
                })
                .into_any_element()
        };
        v_flex()
            .gap(px(12.))
            .children(tiles.chunks(cols).map(|row| {
                h_flex()
                    .items_stretch()
                    .gap(px(12.))
                    .children(row.iter().map(tile))
                    .children((row.len()..cols).map(|_| div().flex_1().flex_basis(px(0.))))
            }))
            .into_any_element()
    }

    fn meters(&self, meters: &[Meter], p: Palette) -> AnyElement {
        v_flex()
            .gap(px(14.))
            .children(meters.iter().map(|m| {
                v_flex()
                    .gap(px(7.))
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(p.text)
                                    .child(m.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .whitespace_nowrap()
                                    .child(m.text()),
                            ),
                    )
                    .child(bar(m.fraction(), fill_tone(m.tone, p), 6., p))
            }))
            .into_any_element()
    }

    fn card(&self, at: At, c: &Card, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let inner = at.inner(at.width - 34.);
        let head = c.title.is_some() || c.subtitle.is_some() || !c.pills.is_empty() || !c.buttons.is_empty();
        let buttons: Vec<AnyElement> = c
            .buttons
            .iter()
            .map(|b| self.button(at.addon, at.section, b, true, cx))
            .collect();
        surface(p)
            .when(at.fill, |d| d.flex_1())
            .p(px(16.))
            .gap(px(14.))
            .when(head, |d| {
                d.child(
                    h_flex()
                        .gap(px(10.))
                        .min_h(px(24.))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .when_some(c.title.clone(), |d, t| {
                                    d.child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(p.text)
                                            .truncate()
                                            .child(t),
                                    )
                                })
                                .when_some(c.subtitle.clone(), |d, s| {
                                    d.child(div().text_size(px(12.)).text_color(p.text_muted).child(s))
                                }),
                        )
                        .child(pills(&c.pills, p))
                        .children(buttons),
                )
            })
            .children(c.blocks.iter().map(|b| self.block_at(inner, b, p, cx)))
            .into_any_element()
    }

    fn grid(&self, at: At, g: &Grid, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut weights = g.columns.weights();
        let fits = ((at.width + GAP) / (GRID_COLUMN + GAP)).floor().max(1.) as usize;
        if weights.len() > fits {
            weights = vec![1.];
        }
        let cols = weights.len();
        let total: f32 = weights.iter().sum();
        let usable = at.width - GAP * (cols as f32 - 1.);
        v_flex()
            .gap(px(GAP))
            .children(g.blocks.chunks(cols).map(|row| {
                h_flex()
                    .items_stretch()
                    .gap(px(GAP))
                    .children(row.iter().zip(&weights).map(|(b, w)| {
                        let cell = At {
                            width: usable * w / total,
                            fill: true,
                            ..at
                        };
                        v_flex()
                            .flex_basis(px(0.))
                            .flex_grow(*w)
                            .min_w_0()
                            .child(self.block_at(cell, b, p, cx))
                    }))
                    .children(
                        weights[row.len()..]
                            .iter()
                            .map(|w| div().flex_basis(px(0.)).flex_grow(*w)),
                    )
            }))
            .into_any_element()
    }

    fn form(&self, at: At, f: &Form, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let key = at.key(&f.id);
        let busy = self.busy.contains(&key);
        let cols = f.columns.unwrap_or(1).clamp(1, 3);
        let cols = cols.min(((at.width + GAP) / (220. + GAP)).floor().max(1.) as usize);
        let (changed, valid) = self.form_changes(&key, f, cx);
        let fields: Vec<AnyElement> = f
            .fields
            .iter()
            .map(|field| {
                let dirty = changed.iter().any(|c| c == field.id());
                self.field(at, &key, f, field, dirty, busy, p, cx)
            })
            .collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut fields = fields.into_iter().peekable();
        while fields.peek().is_some() {
            let row: Vec<AnyElement> = fields.by_ref().take(cols).collect();
            let pad = cols - row.len();
            rows.push(
                h_flex()
                    .items_start()
                    .gap(px(20.))
                    .children(row)
                    .children((0..pad).map(|_| div().flex_1().flex_basis(px(0.))))
                    .into_any_element(),
            );
        }
        let footer = (!f.live).then(|| {
            let (reset_key, apply_form) = (key.clone(), f.clone());
            let (addon, section) = (at.addon.to_owned(), at.section.to_owned());
            let count = changed.len();
            h_flex()
                .gap(px(8.))
                .pt(px(4.))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(p.text_muted)
                        .child(match count {
                            0 => String::new(),
                            1 => "1 unsaved change".to_owned(),
                            n => format!("{n} unsaved changes"),
                        }),
                )
                .when(count > 0 && !busy, |d| {
                    d.child(
                        Button::new(SharedString::from(format!("reset-{key}")))
                            .ghost()
                            .small()
                            .label("Reset")
                            .on_click(cx.listener(move |this, _, _, cx| this.reset_form(&reset_key, cx))),
                    )
                })
                .child(
                    Button::new(SharedString::from(format!("apply-{key}")))
                        .primary()
                        .small()
                        .label(f.apply.clone().unwrap_or_else(|| "Apply".into()))
                        .loading(busy)
                        .disabled(busy || f.disabled || count == 0 || !valid)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.submit_form(&addon, &section, &apply_form, None, window, cx)
                        })),
                )
        });
        v_flex()
            .gap(px(16.))
            .when(cols == 1, |d| d.max_w(px(520.)))
            .children(rows)
            .children(footer)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn field(
        &self,
        at: At,
        key: &str,
        form: &Form,
        field: &Field,
        dirty: bool,
        busy: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = field.id().to_owned();
        let disabled = form.disabled || field.disabled() || busy;
        let slot = self.forms.get(key);
        let control_id = SharedString::from(format!("field-{key}/{id}"));
        let live = form
            .live
            .then(|| (at.addon.to_owned(), at.section.to_owned(), form.clone()));
        let control: AnyElement = match field {
            Field::Number(nf) => match slot.and_then(|s| s.inputs.get(&id)) {
                Some(state) => NumberInput::new(state)
                    .small()
                    .disabled(disabled)
                    .when_some(nf.unit.clone(), |n, u| {
                        n.suffix(div().text_size(px(12.)).text_color(p.text_placeholder).child(u))
                    })
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Field::Text(_) => match slot.and_then(|s| s.inputs.get(&id)) {
                Some(state) => Input::new(state).small().disabled(disabled).into_any_element(),
                None => div().into_any_element(),
            },
            Field::Slider(nf) => match slot.and_then(|s| s.sliders.get(&id)) {
                Some(state) => {
                    let v = state.read(cx).value().start() as f64;
                    let text = format!(
                        "{}{}",
                        nf.text(nf.clamp(v)),
                        nf.unit.as_ref().map(|u| format!(" {u}")).unwrap_or_default()
                    );
                    h_flex()
                        .h(px(28.))
                        .gap(px(12.))
                        .child(Slider::new(state).disabled(disabled).flex_1())
                        .child(
                            div()
                                .min_w(px(64.))
                                .flex()
                                .justify_end()
                                .text_size(px(13.))
                                .text_color(p.text)
                                .whitespace_nowrap()
                                .child(text),
                        )
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            Field::Select(sf) => {
                let current = slot
                    .and_then(|s| s.picks.get(&id))
                    .cloned()
                    .unwrap_or_else(|| field.value());
                let current = current.as_str().unwrap_or_default().to_owned();
                let label_text = sf
                    .options
                    .iter()
                    .find(|o| o.value() == current)
                    .map_or(current.clone(), |o| o.label().to_owned());
                let shell = cx.entity().downgrade();
                let (options, key, id) = (sf.options.clone(), key.to_owned(), id.clone());
                Button::new(control_id)
                    .outline()
                    .small()
                    .w_full()
                    .label(label_text)
                    .dropdown_caret(true)
                    .disabled(disabled)
                    .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                        for o in &options {
                            let (shell, key, id, live) =
                                (shell.clone(), key.clone(), id.clone(), live.clone());
                            let value = Value::String(o.value().to_owned());
                            menu = menu.item(
                                PopupMenuItem::new(o.label().to_owned())
                                    .checked(o.value() == current)
                                    .on_click(move |_, window, cx| {
                                        let (value, live) = (value.clone(), live.clone());
                                        shell
                                            .update(cx, |this, cx| {
                                                this.pick(&key, &id, value, live, window, cx)
                                            })
                                            .ok();
                                    }),
                            );
                        }
                        menu
                    })
                    .into_any_element()
            }
            Field::Toggle(_) => {
                let on = slot
                    .and_then(|s| s.picks.get(&id))
                    .cloned()
                    .unwrap_or_else(|| field.value())
                    == Value::Bool(true);
                let (key, id) = (key.to_owned(), id.clone());
                h_flex()
                    .h(px(28.))
                    .gap(px(10.))
                    .child(
                        Switch::new(control_id)
                            .checked(on)
                            .small()
                            .disabled(disabled)
                            .on_change(cx.listener(move |this, on: &bool, window, cx| {
                                this.pick(&key, &id, Value::Bool(*on), live.clone(), window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(if on { p.text } else { p.text_muted })
                            .child(if on { "On" } else { "Off" }),
                    )
                    .into_any_element()
            }
        };
        v_flex()
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .gap(px(6.))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text_muted)
                            .child(field.label().unwrap_or(field.id()).to_owned()),
                    )
                    .when(dirty, |d| d.child(div().size(px(6.)).rounded_full().bg(p.accent))),
            )
            .child(control)
            .when_some(field.help().map(str::to_owned), |d, h| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(p.text_placeholder)
                        .child(h),
                )
            })
            .into_any_element()
    }

    fn table(&self, at: At, t: &Table, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let table_key = at.key(&format!("table:{}", t.id()));
        let sort = self
            .sorts
            .get(&table_key)
            .cloned()
            .or_else(|| t.sort.as_ref().map(|s| (s.column.clone(), s.desc)));
        let search_key = t.search.as_ref().map(|s| at.key(&search_id(s, t.id())));
        let query = search_key
            .as_deref()
            .filter(|_| t.search.as_ref().is_some_and(|s| s.filter))
            .and_then(|k| self.query(k));
        let order: Vec<usize> = sorted_rows(t, sort.as_ref())
            .into_iter()
            .filter(|i| query.is_none_or(|q| Search::matches(q, t.rows[*i].cells().iter().map(Cell::text))))
            .collect();
        let (row_h, text) = if t.compact { (30., 12.) } else { (36., 13.) };
        let specs: Vec<_> = t.columns.iter().map(|c| c.spec()).collect();
        let sized = |d: Div, i: usize| -> Div {
            let spec = &specs[i];
            let d = match spec.width {
                Some(w) => d.w(px(w)).flex_shrink_0(),
                None => d.flex_1().flex_basis(px(0.)).min_w_0(),
            };
            match spec.align {
                Align::Left => d.justify_start(),
                Align::Right => d.justify_end(),
                Align::Center => d.justify_center(),
            }
        };
        let header = h_flex()
            .h(px(row_h - 2.))
            .px(px(12.))
            .gap(px(12.))
            .border_b_1()
            .border_color(p.border_variant)
            .children(t.columns.iter().enumerate().map(|(i, c)| {
                let spec = &specs[i];
                let column = c.key().to_owned();
                let active = sort.as_ref().filter(|(k, _)| *k == column).map(|(_, d)| *d);
                let editable = t.rows.iter().any(|r| {
                    r.cells()
                        .get(i)
                        .is_some_and(|c| matches!(c, Cell::Spec(s) if s.edit.is_some()))
                });
                let head = sized(h_flex().gap(px(4.)), i)
                    .id(SharedString::from(format!("{table_key}/h/{i}")))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if active.is_some() {
                        p.text
                    } else {
                        p.text_placeholder
                    })
                    .child(div().truncate().child(spec.title.to_uppercase()))
                    .when(editable, |d| {
                        d.child(
                            Icon::new(IconName::Pencil)
                                .size(px(10.))
                                .text_color(p.text_placeholder),
                        )
                    })
                    .when_some(active, |d, desc| {
                        d.child(
                            Icon::new(if desc {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            })
                            .size(px(12.))
                            .text_color(p.accent),
                        )
                    });
                if spec.sort {
                    let key = table_key.clone();
                    head.cursor_pointer()
                        .hover(move |s| s.text_color(p.text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let desc = active.is_some_and(|d| !d);
                            this.sorts.insert(key.clone(), (column.clone(), desc));
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    head.into_any_element()
                }
            }));
        let rows: Vec<AnyElement> = order
            .iter()
            .enumerate()
            .map(|(n, &ri)| {
                let row = &t.rows[ri];
                let row_id = row.id(ri);
                let row_tone = row.tone();
                h_flex()
                    .h(px(row_h))
                    .px(px(12.))
                    .gap(px(12.))
                    .when(n > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .children(
                        row.cells()
                            .iter()
                            .take(specs.len())
                            .enumerate()
                            .map(|(ci, cell)| {
                                let content = self.cell(
                                    at,
                                    t,
                                    &table_key,
                                    &row_id,
                                    &specs[ci],
                                    t.columns[ci].key(),
                                    cell,
                                    row_tone,
                                    text,
                                    p,
                                    cx,
                                );
                                sized(h_flex(), ci).child(content).into_any_element()
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        let empty = order.is_empty().then(|| {
            div()
                .px(px(12.))
                .py(px(14.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(t.empty.clone().unwrap_or_else(|| "No matches".into()))
        });
        let body = v_flex()
            .w_full()
            .overflow_hidden()
            .when(!at.nested, |d| {
                d.rounded(px(6.)).border_1().border_color(p.border_variant)
            })
            .child(header)
            .children(rows)
            .children(empty);
        v_flex()
            .gap(px(10.))
            .children(search_key.and_then(|k| self.search_box(&k, p)))
            .child(body)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn cell(
        &self,
        at: At,
        t: &Table,
        table_key: &str,
        row_id: &str,
        spec: &swofty_core::view::ColumnSpec,
        column: &str,
        cell: &Cell,
        row_tone: Option<Tone>,
        size: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = cell.spec();
        let key = format!("{table_key}/{row_id}/{column}");
        let color = c.tone.or(row_tone).map_or(p.text, |t| tone(t, p));
        if let Some(edit) = self.cell.as_ref().filter(|e| e.key == key) {
            let input = edit.input.clone();
            return div()
                .id(SharedString::from(format!("edit-{key}")))
                .w_full()
                .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    if ev.keystroke.key == "escape" {
                        this.cell = None;
                        cx.notify();
                    }
                }))
                .child(Input::new(&input).xsmall())
                .into_any_element();
        }
        let shown = self
            .pending_cells
            .get(&key)
            .cloned()
            .unwrap_or_else(|| c.text.clone());
        let pending = self.pending_cells.contains_key(&key);
        let text_el = div()
            .min_w_0()
            .truncate()
            .text_size(px(size))
            .text_color(if pending { p.text_muted } else { color })
            .child(shown.clone());
        let mut content = h_flex().gap(px(8.)).min_w_0();
        if spec.align == Align::Right {
            content = content.justify_end();
        }
        if let Some(f) = c.bar {
            content = content.child(div().w(px(56.)).flex_shrink_0().child(bar(
                f,
                fill_tone(c.tone, p),
                4.,
                p,
            )));
        }
        content = if c.pill {
            content.child(pill(
                &Pill {
                    text: shown,
                    tone: c.tone.unwrap_or_default(),
                    ..Pill::default()
                },
                p,
            ))
        } else {
            content.child(text_el)
        };
        let Some(field) = c.edit.clone() else {
            return content.into_any_element();
        };
        let (addon, section, table, row, column) = (
            at.addon.to_owned(),
            at.section.to_owned(),
            t.id().to_owned(),
            row_id.to_owned(),
            column.to_owned(),
        );
        let original = match field.value() {
            Value::Null => match &field {
                Field::Number(nf) => number_text(&c.text).map_or(Value::Null, |v| nf.json(v)),
                _ => Value::String(c.text.clone()),
            },
            v => v,
        };
        let busy = self.busy.contains(&key);
        content
            .id(SharedString::from(format!("cell-{key}")))
            .px(px(6.))
            .mx(px(-6.))
            .h(px(size + 12.))
            .rounded(px(4.))
            .cursor_pointer()
            .when(!busy, |d| d.hover(move |s| s.bg(p.hover)))
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.busy.contains(&key) {
                    return;
                }
                this.edit_cell(
                    CellTarget {
                        key: key.clone(),
                        addon: addon.clone(),
                        section: section.clone(),
                        table: table.clone(),
                        row: row.clone(),
                        column: column.clone(),
                    },
                    field.clone(),
                    original.clone(),
                    window,
                    cx,
                )
            }))
            .into_any_element()
    }

    fn edit_cell(
        &mut self,
        target: CellTarget,
        field: Field,
        original: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = field_text(&field, &original);
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).default_value(text);
            if let Field::Number(nf) = &field {
                s = s.step(nf.step());
            }
            if let Field::Text(tf) = &field
                && let Some(ph) = tf.placeholder.clone()
            {
                s = s.placeholder(ph);
            }
            s
        });
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let key = target.key.clone();
        let sub = cx.subscribe_in(&input, window, move |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                && this.cell.as_ref().is_some_and(|c| c.key == key)
            {
                this.commit_cell(window, cx);
            }
        });
        self.cell = Some(CellEdit {
            key: target.key,
            addon: target.addon,
            section: target.section,
            table: target.table,
            row: target.row,
            column: target.column,
            field,
            original,
            input,
            _subs: vec![sub],
        });
        cx.notify();
    }

    fn commit_cell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.cell.take() else {
            return;
        };
        let text = edit.input.read(cx).value().to_string();
        let value = match &edit.field {
            Field::Number(nf) => match number_text(&text) {
                Some(v) => nf.json(nf.clamp(v)),
                None => {
                    window.push_notification(Notification::error(format!("{text} is not a number")), cx);
                    cx.notify();
                    return;
                }
            },
            Field::Text(tf) => Value::String(match tf.max_len {
                Some(max) => text.chars().take(max).collect(),
                None => text,
            }),
            _ => Value::Null,
        };
        cx.notify();
        if value == edit.original || value.is_null() {
            return;
        }
        self.pending_cells
            .insert(edit.key.clone(), field_text(&edit.field, &value));
        let mut values = Map::new();
        values.insert(edit.column.clone(), value);
        let event = swofty_core::view::Event::Submit {
            form: edit.table.clone(),
            row: Some(edit.row.clone()),
            values,
            changed: vec![edit.column.clone()],
        };
        self.send(&edit.addon, &edit.section, edit.key.clone(), event, window, cx);
    }
}

pub(crate) struct CellTarget {
    key: String,
    addon: String,
    section: String,
    table: String,
    row: String,
    column: String,
}
