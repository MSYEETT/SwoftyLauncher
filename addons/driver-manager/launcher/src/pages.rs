use serde_json::{Value, json};

use crate::app::{App, Edit};
use crate::check::{self, Issue, Level};
use crate::game::{GameData, List};
use crate::host::Host;
use crate::model::{
    Driver, FIRST_DRIVER_ID, LAST_DRIVER_ID, MAX_DRIVERS, MONTHS, Rating, Roster, STATS, TRAITS,
};

pub const SECTION: &str = "drivers";
const FOCUS_HELP: &str =
    "The game sets the starting focus itself and moves it with form, so this value is only a guide.";

fn notice(text: &str, tone: &str) -> Value {
    json!({ "text": text, "tone": tone })
}

fn button(id: &str, label: &str, icon: &str, style: &str) -> Value {
    let mut b = json!({ "id": id, "label": label, "style": style });
    if !icon.is_empty() {
        b["icon"] = json!(icon);
    }
    b
}

fn confirm(mut b: Value, title: &str, text: &str) -> Value {
    b["kind"] = json!({ "confirm": { "title": title, "text": text } });
    b
}

fn fact(label: &str, value: impl Into<String>) -> Value {
    json!({ "label": label, "value": value.into() })
}

fn text(t: &str, tone: &str) -> Value {
    json!({ "text": { "text": t, "tone": tone } })
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn status_tag(d: &Driver) -> Value {
    if d.retired {
        json!({ "text": "Retired", "tone": "muted" })
    } else {
        json!({ "text": "Free agent", "tone": "accent" })
    }
}

fn check_tag(issues: &[Issue]) -> Value {
    let errors = check::errors(issues);
    let warns = issues.len() - errors;
    if errors > 0 {
        json!({ "text": format!("{} · left out", plural(errors, "error", "errors")), "tone": "bad" })
    } else if warns > 0 {
        json!({ "text": plural(warns, "warning", "warnings"), "tone": "warn" })
    } else {
        json!({ "text": "Ready", "tone": "good" })
    }
}

fn issue_blocks(issues: &[Issue]) -> Vec<Value> {
    issues
        .iter()
        .map(|i| match i.level {
            Level::Error => text(&format!("Error: {}", i.text), "bad"),
            Level::Warn => text(&format!("Warning: {}", i.text), "warn"),
        })
        .collect()
}

fn nationality(game: Option<&GameData>, d: &Driver) -> String {
    game.map_or_else(
        || d.nationality.clone(),
        |g| g.label(List::Nationality, &d.nationality),
    )
}

fn scope_notices(app: &App) -> Vec<Value> {
    let mut n = Vec::new();
    if app.vanilla {
        n.push(notice("This is the vanilla profile, which takes no addons. Your drivers are added in modded profiles with Driver Manager on.", "warn"));
    } else if !app.enabled {
        n.push(json!({
            "text": "Driver Manager is off in this profile, so its drivers aren't added here.",
            "tone": "warn",
            "action": { "id": "enable", "label": "Turn on" }
        }));
    }
    n.push(notice(
        "New drivers join careers you start after adding them, as free agents you can sign. Ratings apply to new careers.",
        "accent",
    ));
    n.push(notice(
        "Retiring a driver keeps careers that have the driver loading. Removing Driver Manager breaks careers that contain its drivers.",
        "warn",
    ));
    if app.loading.is_some() {
        n.push(notice(
            "Reading the game's lists of heads, kit and names…",
            "muted",
        ));
    } else if app.building.is_some() {
        n.push(notice(
            "Making previews of the heads, kit, poses and emotes…",
            "muted",
        ));
    }
    if let Some(e) = &app.game_error {
        n.push(json!({ "text": e, "tone": "bad", "action": { "id": "retry", "label": "Try again" } }));
    }
    n
}

pub fn view(app: &mut App, host: &mut Host) -> Value {
    if app.game.is_none() {
        app.retry_game();
    }
    app.ensure_game(host);
    app.ensure_previews(host);
    let roster = match app.roster() {
        Ok(r) => r,
        Err(e) => {
            return json!({
                "subtitle": "Driver list unreadable",
                "notices": [notice(&e, "bad")],
                "body": { "empty": { "title": "The driver list can't be read", "text": "Fix or move drivers.json in the addon's data folder.", "action": { "id": "open_data", "label": "Open data folder" } } }
            });
        }
    };
    let mut v = if let Some(e) = &app.edit {
        let (head, helmet) = (e.draft.head.clone(), e.draft.helmet.clone());
        let large = Looks {
            head: app.large(List::Head, &head),
            helmet: app.large(List::Helmet, &helmet),
        };
        edit_view(app, &roster, &large)
    } else {
        list_view(app, &roster)
    };
    if app.busy() {
        v["tick"] = json!(true);
    }
    v
}

fn art(app: &App, d: &Driver, fallback: &str) -> Value {
    match app.thumb(List::Head, &d.head) {
        Some(image) => json!({ "image": image }),
        None => {
            json!({ "monogram": d.code.chars().next().map(String::from).unwrap_or_else(|| fallback.into()) })
        }
    }
}

pub struct Looks {
    pub head: Option<String>,
    pub helmet: Option<String>,
}

fn looks_card(game: Option<&GameData>, d: &Driver, looks: &Looks) -> Option<Value> {
    let label = |list: List, value: &str| game.map_or_else(|| value.to_owned(), |g| g.label(list, value));
    let mut blocks = Vec::new();
    if let Some(path) = &looks.head {
        blocks.push(json!({ "image": { "path": path, "width": 144, "height": 144, "caption": label(List::Head, &d.head), "fit": "cover" } }));
    }
    if let Some(path) = &looks.helmet {
        blocks.push(json!({ "image": { "path": path, "width": 288, "height": 144, "caption": label(List::Helmet, &d.helmet), "fit": "contain" } }));
    }
    if blocks.is_empty() {
        return None;
    }
    Some(json!({ "card": {
        "title": "Looks",
        "subtitle": "The saved head and the helmet's design. Customisable kit is shown in neutral colours.",
        "blocks": [ { "grid": { "columns": [1, 2], "blocks": blocks } } ]
    } }))
}

fn list_view(app: &App, roster: &Roster) -> Value {
    let game = app.game.as_ref();
    let active = roster.drivers.iter().filter(|d| !d.retired).count();
    let retired = roster.drivers.len() - active;
    let left = game.map_or(MAX_DRIVERS.saturating_sub(roster.drivers.len()), |g| {
        roster.free_ids(g)
    });
    let subtitle = format!(
        "{} free in new careers · {} retired · {} ids left",
        plural(active, "driver", "drivers"),
        retired,
        left
    );
    let can_add = game.is_some() && left > 0;
    let actions = if can_add {
        vec![button("add", "Add driver", "plus", "primary")]
    } else {
        Vec::new()
    };
    let footer = format!(
        "Drivers are kept in {} and shared by every profile. Ids {FIRST_DRIVER_ID} to {LAST_DRIVER_ID} are never reused.",
        crate::model::Roster::path(&app.data_dir).display()
    );
    if roster.drivers.is_empty() {
        return json!({
            "subtitle": subtitle,
            "actions": actions,
            "notices": scope_notices(app),
            "body": { "empty": {
                "title": "No drivers yet",
                "text": "Create a driver: name, number, nationality, looks, commentary name and ratings. New drivers are free agents in careers you start afterwards.",
                "action": if can_add { json!({ "id": "add", "label": "Add driver" }) } else { Value::Null }
            } },
            "footer": footer
        });
    }
    let rows: Vec<Value> = roster
        .drivers
        .iter()
        .map(|d| {
            let issues = check::check(d, roster, game);
            let id = d.stat_id;
            let mut detail = vec![json!({ "facts": [
                fact("Driver id", d.stat_id.to_string()),
                fact("Look id", d.preset_id.to_string()),
                fact("Head", game.map_or(d.head.clone(), |g| g.label(List::Head, &d.head))),
                fact("Helmet", game.map_or(d.helmet.clone(), |g| g.label(List::Helmet, &d.helmet))),
                fact("Commentary name", game.map_or(d.voice.clone(), |g| g.label(List::Voice, &d.voice))),
                fact("Seasons completed", d.seasons.to_string()),
                fact("Born", format!("{} {} {}", d.birth.day, d.birth.month_name(), d.birth.year)),
            ] })];
            detail.push(json!({ "facts": STATS.iter().zip(d.ratings.iter()).map(|((_, name), r)| fact(name, format!("{} → {}", r.start, r.peak))).collect::<Vec<_>>() }));
            detail.extend(issue_blocks(&issues));
            let toggle = if d.retired {
                button(&format!("restore:{id}"), "Restore", "undo", "outline")
            } else {
                confirm(
                    button(&format!("retire:{id}"), "Retire", "", "danger"),
                    &format!("Retire {}?", d.display_name()),
                    "The driver stays in the game, so careers that have the driver keep loading, but new careers don't offer the driver. You can restore the driver later.",
                )
            };
            let mut buttons = Vec::new();
            if game.is_some_and(|g| roster.free_ids(g) > 0) {
                buttons.push(button(&format!("dup:{id}"), "Duplicate", "copy", "outline"));
            }
            buttons.push(toggle);
            detail.push(json!({ "buttons": buttons }));
            json!({
                "id": id.to_string(),
                "title": d.display_name(),
                "subtitle": format!("{} · #{} · {}", if d.code.is_empty() { "---" } else { &d.code }, d.number, nationality(game, d)),
                "art": art(app, d, "?"),
                "tags": [status_tag(d), check_tag(&issues)],
                "trailing": { "button": button(&format!("edit:{id}"), "Edit", "pencil", "outline") },
                "detail": detail
            })
        })
        .collect();
    json!({
        "subtitle": subtitle,
        "actions": actions,
        "notices": scope_notices(app),
        "body": { "list": { "rows": rows, "reorder": false, "search": { "id": "drivers", "placeholder": "Search drivers" } } },
        "footer": footer
    })
}

fn option(app: &App, list: List, value: &str, label: String) -> Value {
    let mut o = json!({ "value": value, "label": label });
    if let Some(image) = app.thumb(list, value) {
        o["image"] = json!(image);
    }
    o
}

fn options(app: &App, list: List, current: &str) -> Value {
    let game = app.game.as_ref();
    let mut opts: Vec<Value> = game
        .map(|g| {
            g.choices(list)
                .map(|c| option(app, list, &c.value, c.label.clone()))
                .collect()
        })
        .unwrap_or_default();
    if !opts.iter().any(|o| o["value"] == current) {
        let label = match game {
            Some(g) if g.has(list, current) => format!("{} (not installed)", g.label(list, current)),
            _ => format!("{current} (not in the game)"),
        };
        opts.insert(0, option(app, list, current, label));
    }
    Value::Array(opts)
}

fn select(id: &str, label: &str, value: &str, options: Value, search: bool, help: Option<&str>) -> Value {
    let mut f = json!({ "id": id, "label": label, "value": value, "options": options });
    if search {
        f["search"] = json!(true);
    }
    if let Some(h) = help {
        f["help"] = json!(h);
    }
    json!({ "select": f })
}

fn number(id: &str, label: &str, value: u32, min: u32, max: u32, help: Option<&str>) -> Value {
    let mut f = json!({ "id": id, "label": label, "value": value, "min": min, "max": max, "step": 1 });
    if let Some(h) = help {
        f["help"] = json!(h);
    }
    json!({ "number": f })
}

fn text_field(id: &str, label: &str, value: &str, placeholder: &str, max: usize) -> Value {
    json!({ "text": { "id": id, "label": label, "value": value, "placeholder": placeholder, "max_len": max } })
}

fn edit_view(app: &App, roster: &Roster, looks: &Looks) -> Value {
    let edit = app.edit.as_ref().expect("edit view");
    let d = &edit.draft;
    let game = app.game.as_ref();
    let issues = check::check(d, roster, game);
    let (title, status) = if d.is_new() {
        (
            if d.full_name().is_empty() {
                "New driver".to_owned()
            } else {
                d.display_name()
            },
            json!({ "text": "New", "tone": "accent" }),
        )
    } else {
        (d.display_name(), status_tag(d))
    };
    let subtitle = if d.is_new() {
        "The driver id and look id are given when you save.".to_owned()
    } else if d.retired {
        format!(
            "Driver id {} · look id {} · retired: careers that have the driver keep loading; new careers don't offer the driver.",
            d.stat_id, d.preset_id
        )
    } else {
        format!(
            "Driver id {} · look id {} · a free agent in careers started after the driver was added.",
            d.stat_id, d.preset_id
        )
    };
    let mut hero_buttons = vec![button("back", "Back to drivers", "", "ghost")];
    if !d.is_new() {
        if game.is_some_and(|g| roster.free_ids(g) > 0) {
            hero_buttons.push(button(
                &format!("dup:{}", d.stat_id),
                "Duplicate",
                "copy",
                "outline",
            ));
        }
        hero_buttons.push(if d.retired {
            button(&format!("restore:{}", d.stat_id), "Restore", "undo", "outline")
        } else {
            confirm(
                button(&format!("retire:{}", d.stat_id), "Retire", "", "danger"),
                &format!("Retire {}?", d.display_name()),
                "The driver stays in the game, so careers that have the driver keep loading, but new careers don't offer the driver. You can restore the driver later.",
            )
        });
    }
    let mut pills = vec![status];
    if !d.is_new() {
        pills.push(check_tag(&issues));
    } else if edit.tried && !check::is_valid(&issues) {
        pills.push(json!({ "text": format!("{} to fix", plural(check::errors(&issues), "error", "errors")), "tone": "bad" }));
    }
    let mut blocks = vec![json!({ "hero": {
        "title": title,
        "subtitle": subtitle,
        "art": art(app, d, "+"),
        "pills": pills,
        "buttons": hero_buttons
    } })];
    let shown: Vec<Issue> = if edit.tried || !d.is_new() {
        issues.clone()
    } else {
        issues
            .iter()
            .filter(|i| i.level == Level::Warn)
            .cloned()
            .collect()
    };
    if !shown.is_empty() {
        let title = if check::errors(&shown) > 0 {
            "Fix these before saving"
        } else {
            "Check these"
        };
        blocks.push(json!({ "card": { "title": title, "blocks": issue_blocks(&shown) } }));
    }
    blocks.extend(looks_card(game, d, looks));
    let ids = if d.is_new() {
        "Given when you save".to_owned()
    } else {
        format!("Driver {} · look {}", d.stat_id, d.preset_id)
    };
    let mut fields = vec![
        text_field("first", "First name", &d.first, "First name", check::NAME_MAX),
        text_field("last", "Last name", &d.last, "Last name", check::NAME_MAX),
        json!({ "toggle": { "id": "reversed", "label": "Surname first", "value": d.reversed, "help": "Shows the name as \"LAST First\", as for Zhou Guanyu." } }),
        text_field("code", "Code", &d.code, "ABC", 3),
        number("number", "Number", d.number, 1, 99, None),
        select(
            "nationality",
            "Nationality",
            &d.nationality,
            options(app, List::Nationality, &d.nationality),
            true,
            None,
        ),
        number(
            "seasons",
            "F1 seasons completed",
            d.seasons,
            0,
            check::SEASONS_MAX,
            None,
        ),
        select(
            "garage",
            "Garage slot preference",
            &d.garage.to_string(),
            json!([{ "value": "0", "label": "Slot 1" }, { "value": "1", "label": "Slot 2" }]),
            false,
            None,
        ),
        select(
            "head",
            "Head",
            &d.head,
            options(app, List::Head, &d.head),
            true,
            Some("A generic head, or one that looks like a real driver."),
        ),
        select(
            "voice",
            "Commentary name",
            &d.voice,
            options(app, List::Voice, &d.voice),
            true,
            Some("The surname the commentators say."),
        ),
        select(
            "helmet",
            "Helmet",
            &d.helmet,
            options(app, List::Helmet, &d.helmet),
            true,
            None,
        ),
        select(
            "suit",
            "Suit",
            &d.suit,
            options(app, List::Suit, &d.suit),
            true,
            None,
        ),
        select(
            "gloves",
            "Gloves",
            &d.gloves,
            options(app, List::Gloves, &d.gloves),
            true,
            None,
        ),
        select(
            "pose",
            "Pose",
            &d.pose,
            options(app, List::Pose, &d.pose),
            true,
            None,
        ),
        select(
            "emote",
            "Podium emote",
            &d.emote,
            options(app, List::Emote, &d.emote),
            true,
            None,
        ),
        json!({ "text": { "id": "ids", "label": "Ids", "value": ids, "disabled": true } }),
        number(
            "birth_year",
            "Birth year",
            d.birth.year,
            check::BIRTH_YEARS.0,
            check::BIRTH_YEARS.1,
            None,
        ),
        select(
            "birth_month",
            "Birth month",
            &d.birth.month.to_string(),
            Value::Array(
                MONTHS
                    .iter()
                    .enumerate()
                    .map(|(i, m)| json!({ "value": (i + 1).to_string(), "label": m }))
                    .collect(),
            ),
            false,
            None,
        ),
        number("birth_day", "Birth day", d.birth.day, 1, 31, None),
        json!({ "text": { "id": "age", "label": "Age at the start of 2026", "value": format!("{}", 2026i64 - i64::from(d.birth.year) - 1 + i64::from(d.birth.month == 1 && d.birth.day == 1)), "disabled": true } }),
    ];
    for ((key, name), v) in TRAITS.iter().zip(d.traits.iter()) {
        fields.push(json!({ "slider": {
            "id": format!("trait_{key}"), "label": format!("AI {}", name.to_lowercase()), "value": v,
            "min": 0, "max": 1, "step": 0.01,
            "help": if *key == "mistakes" { "Higher means fewer mistakes. Stock drivers sit between 0.2 and 1." } else { "Higher is stronger. Stock drivers sit between 0.2 and 1." }
        } }));
    }
    for ((key, name), r) in STATS.iter().zip(d.ratings.iter()) {
        let guide = (*key == "focus").then_some(FOCUS_HELP);
        fields.push(number(
            &format!("{key}_start"),
            &format!("{name} at start"),
            r.start,
            0,
            100,
            guide,
        ));
        fields.push(number(
            &format!("{key}_peak"),
            &format!("{name} at peak"),
            r.peak,
            0,
            100,
            guide,
        ));
    }
    blocks.push(json!({ "card": {
        "title": "Driver",
        "subtitle": "Ratings grow from the start value to the peak as the career goes on.",
        "blocks": [ { "form": {
            "id": edit.form,
            "columns": 2,
            "fields": fields,
            "apply": if d.is_new() { "Add driver" } else { "Save driver" },
            "disabled": game.is_none()
        } } ]
    } }));
    json!({
        "subtitle": if d.is_new() { "New driver".to_owned() } else { format!("Editing {}", d.display_name()) },
        "notices": scope_notices(app),
        "body": { "blocks": blocks }
    })
}

fn str_of(values: &Value, key: &str) -> Option<String> {
    values.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn u32_of(values: &Value, key: &str) -> Option<u32> {
    let v = values.get(key)?;
    v.as_u64()
        .or_else(|| v.as_f64().map(|f| f.round().max(0.) as u64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

pub fn apply_values(d: &mut Driver, values: &Value) {
    if let Some(v) = str_of(values, "first") {
        d.first = v.trim().to_owned();
    }
    if let Some(v) = str_of(values, "last") {
        d.last = v.trim().to_owned();
    }
    if let Some(v) = values.get("reversed").and_then(Value::as_bool) {
        d.reversed = v;
    }
    if let Some(v) = str_of(values, "code") {
        d.code = v.trim().to_uppercase();
    }
    if let Some(v) = u32_of(values, "number") {
        d.number = v;
    }
    if let Some(v) = u32_of(values, "seasons") {
        d.seasons = v;
    }
    if let Some(v) = u32_of(values, "garage") {
        d.garage = v;
    }
    for (key, slot) in [
        ("nationality", &mut d.nationality),
        ("head", &mut d.head),
        ("voice", &mut d.voice),
        ("helmet", &mut d.helmet),
        ("suit", &mut d.suit),
        ("gloves", &mut d.gloves),
        ("pose", &mut d.pose),
        ("emote", &mut d.emote),
    ] {
        if let Some(v) = str_of(values, key) {
            *slot = v;
        }
    }
    if let Some(v) = u32_of(values, "birth_year") {
        d.birth.year = v;
    }
    if let Some(v) = u32_of(values, "birth_month") {
        d.birth.month = v;
    }
    if let Some(v) = u32_of(values, "birth_day") {
        d.birth.day = v;
    }
    for (i, (key, _)) in TRAITS.iter().enumerate() {
        if let Some(v) = values.get(format!("trait_{key}")).and_then(Value::as_f64) {
            d.traits[i] = (v * 100.).round() / 100.;
        }
    }
    for (i, (key, _)) in STATS.iter().enumerate() {
        let r: &mut Rating = &mut d.ratings[i];
        if let Some(v) = u32_of(values, &format!("{key}_start")) {
            r.start = v;
        }
        if let Some(v) = u32_of(values, &format!("{key}_peak")) {
            r.peak = v;
        }
    }
}

fn parse_id(id: &str, prefix: &str) -> Option<u32> {
    id.strip_prefix(prefix)?.parse().ok()
}

fn open_editor(app: &mut App, draft: Driver) {
    let base = if draft.is_new() {
        "new".to_owned()
    } else {
        format!("edit_{}", draft.stat_id)
    };
    let form = app.next_form(&base);
    app.edit = Some(Edit {
        draft,
        form,
        tried: false,
    });
}

fn set_retired(app: &mut App, id: u32, retired: bool) -> Result<Value, String> {
    let mut roster = app.roster()?;
    let d = roster
        .get_mut(id)
        .ok_or_else(|| format!("No driver with id {id}"))?;
    d.retired = retired;
    let name = d.display_name();
    app.save(&roster)?;
    if let Some(e) = app.edit.as_mut().filter(|e| e.draft.stat_id == id) {
        e.draft.retired = retired;
    }
    Ok(json!({ "notice": if retired {
        format!("Retired {name}. Careers that have the driver keep loading; new careers don't offer the driver.")
    } else {
        format!("Restored {name}. New careers offer the driver as a free agent again.")
    } }))
}

pub fn event(app: &mut App, host: &mut Host, event: &Value) -> Result<Value, String> {
    app.poll();
    if let Some(click) = event.get("click") {
        let id = click["id"].as_str().unwrap_or_default();
        return click_event(app, host, id);
    }
    if let Some(submit) = event.get("submit") {
        let form = submit["form"].as_str().unwrap_or_default();
        let Some(edit) = app.edit.as_mut().filter(|e| e.form == form) else {
            return Ok(json!("reload"));
        };
        apply_values(&mut edit.draft, &submit["values"]);
        edit.tried = true;
        let mut d = edit.draft.clone();
        let mut roster = app.roster()?;
        let issues = check::check(&d, &roster, app.game.as_ref());
        if !check::is_valid(&issues) {
            return Ok(json!("reload"));
        }
        let name = d.display_name();
        let notice = if d.is_new() {
            let game = app.game.as_ref().ok_or("The game's lists aren't loaded yet.")?;
            let (id, preset) = roster.allocate(game)?;
            d.stat_id = id;
            d.preset_id = preset;
            roster.drivers.push(d);
            format!("Added {name}. A free agent in careers you start from now on.")
        } else {
            let slot = roster
                .get_mut(d.stat_id)
                .ok_or_else(|| format!("No driver with id {}", d.stat_id))?;
            d.retired = slot.retired;
            *slot = d;
            format!("Saved {name}.")
        };
        app.save(&roster)?;
        app.edit = None;
        return Ok(json!({ "notice": notice }));
    }
    Ok(json!("none"))
}

fn click_event(app: &mut App, host: &mut Host, id: &str) -> Result<Value, String> {
    match id {
        "add" => {
            app.wait_game(host);
            let roster = app.roster()?;
            let game = app.game.as_ref().ok_or("The game's lists couldn't be read.")?;
            if roster.free_ids(game) == 0 {
                return Err(format!("All {MAX_DRIVERS} driver ids are used."));
            }
            let draft = Driver::blank(Some(game), roster.free_number(Some(game)));
            open_editor(app, draft);
            Ok(json!("reload"))
        }
        "back" => {
            app.edit = None;
            Ok(json!("reload"))
        }
        "retry" => {
            app.retry_game();
            app.ensure_game(host);
            Ok(json!("reload"))
        }
        "enable" => Ok(json!({ "set_addon_enabled": true })),
        "open_data" => Ok(json!({ "open": app.data_dir.to_string_lossy() })),
        _ => {
            if let Some(n) = parse_id(id, "edit:") {
                let roster = app.roster()?;
                let d = roster
                    .get(n)
                    .ok_or_else(|| format!("No driver with id {n}"))?
                    .clone();
                open_editor(app, d);
                return Ok(json!("reload"));
            }
            if let Some(n) = parse_id(id, "dup:") {
                let roster = app.roster()?;
                let d = roster
                    .get(n)
                    .ok_or_else(|| format!("No driver with id {n}"))?
                    .duplicate();
                open_editor(app, d);
                return Ok(json!("reload"));
            }
            if let Some(n) = parse_id(id, "retire:") {
                return set_retired(app, n, true);
            }
            if let Some(n) = parse_id(id, "restore:") {
                return set_retired(app, n, false);
            }
            Ok(json!("none"))
        }
    }
}

pub fn summary(app: &App) -> Value {
    let Ok(roster) = app.roster() else {
        return Value::Null;
    };
    let names: Vec<String> = roster
        .drivers
        .iter()
        .filter(|d| !d.retired)
        .map(Driver::display_name)
        .collect();
    if names.is_empty() {
        Value::Null
    } else {
        json!({ "label": "DRIVERS", "items": names })
    }
}

pub fn build(app: &mut App, host: &mut Host, params: &Value) -> Value {
    app.retry_game();
    app.wait_game(host);
    let roster = match app.roster() {
        Ok(r) => r,
        Err(e) => return json!({ "ops": [], "warnings": [e] }),
    };
    crate::ops::build(&roster, app.game.as_ref(), &params["features"])
}
