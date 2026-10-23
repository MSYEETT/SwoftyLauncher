use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::App;
use crate::game::{GameData, List, StockDriver};

const SCHEMA: u32 = 1;
pub const DRIVERS: &str = "entrants.drivers.driver";
pub const LOOKS: &str = "entrants.driver_customisations.folder";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helmet: Option<String>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.head.is_none() && self.helmet.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub schema: u32,
    #[serde(default)]
    pub drivers: BTreeMap<String, Change>,
}

impl Changes {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("stock.json")
    }

    pub fn load(data_dir: &Path) -> Result<Changes, String> {
        let path = Self::path(data_dir);
        match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("{} is unreadable: {e}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Changes {
                schema: SCHEMA,
                drivers: BTreeMap::new(),
            }),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| format!("{}: {e}", data_dir.display()))?;
        let path = Self::path(data_dir);
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn set(&mut self, label: &str, change: Change) {
        if change.is_empty() {
            self.drivers.remove(label);
        } else {
            self.drivers.insert(label.to_owned(), change);
        }
    }
}

pub fn helmet(s: &StockDriver) -> &str {
    s.looks.last().map_or("", |l| l.helmet.as_str())
}

pub fn ops(changes: &Changes, game: &GameData) -> (Vec<Value>, Vec<String>) {
    let mut ops = Vec::new();
    let mut warnings = Vec::new();
    for (label, c) in &changes.drivers {
        let Some(s) = game.stock_driver(label) else {
            warnings.push(format!(
                "{label} isn't in the game any more, so its head and helmet are left as they are."
            ));
            continue;
        };
        if let Some(head) = &c.head {
            if game.available(List::Head, head) {
                ops.push(json!({ "op": "db.set", "select": format!("{DRIVERS}[_metadata_={label}]"), "field": "head", "value": head }));
            } else {
                warnings.push(format!(
                    "{}: head {} isn't installed, so the driver keeps the stock head.",
                    s.name,
                    game.label(List::Head, head)
                ));
            }
        }
        if let Some(helmet) = &c.helmet {
            if game.available(List::Helmet, helmet) {
                for look in &s.looks {
                    ops.push(json!({
                        "op": "db.set",
                        "select": format!("{LOOKS}[_metadata_={}].preloaded_driver_customisation_properties[_metadata_={}]", look.folder, look.label),
                        "field": "helmet",
                        "value": helmet
                    }));
                }
            } else {
                warnings.push(format!(
                    "{}: helmet {} isn't installed, so the driver keeps the stock helmet.",
                    s.name,
                    game.label(List::Helmet, helmet)
                ));
            }
        }
    }
    (ops, warnings)
}

pub struct Editing {
    pub label: Option<String>,
    pub form: String,
}

fn button(id: &str, label: &str, icon: &str, style: &str) -> Value {
    let mut b = json!({ "id": id, "label": label, "style": style });
    if !icon.is_empty() {
        b["icon"] = json!(icon);
    }
    b
}

fn art(app: &App, head: &str, code: &str) -> Value {
    match app.thumb(List::Head, head) {
        Some(image) => json!({ "image": image }),
        None => json!({ "monogram": code.chars().next().map_or_else(|| "?".to_owned(), String::from) }),
    }
}

pub fn view(app: &mut App, notices: Vec<Value>) -> Value {
    let changes = match Changes::load(&app.data_dir) {
        Ok(c) => c,
        Err(e) => {
            return json!({
                "subtitle": "Game drivers",
                "notices": [{ "text": e, "tone": "bad" }],
                "body": { "empty": { "title": "The changes to game drivers can't be read", "text": "Fix or move stock.json in the addon's data folder.", "action": { "id": "open_data", "label": "Open data folder" } } }
            });
        }
    };
    let label = app.stock.as_ref().and_then(|e| e.label.clone());
    match label {
        Some(label) => edit_view(app, &changes, &label, notices),
        None => list_view(app, &changes, notices),
    }
}

fn list_view(app: &App, changes: &Changes, notices: Vec<Value>) -> Value {
    let Some(game) = app.game.as_ref() else {
        return json!({
            "subtitle": "Game drivers",
            "actions": [button("stock_back", "Back to drivers", "", "ghost")],
            "notices": notices,
            "body": { "empty": { "title": "Reading the game's drivers", "text": "The game's drivers show once its lists are read." } }
        });
    };
    let mut drivers: Vec<&StockDriver> = game.stock.iter().collect();
    drivers.sort_by_key(|s| s.name.to_lowercase());
    let rows: Vec<Value> = drivers
        .iter()
        .map(|s| {
            let change = changes.drivers.get(&s.label).cloned().unwrap_or_default();
            let head = change.head.clone().unwrap_or_else(|| s.head.clone());
            let helm = change.helmet.clone().unwrap_or_else(|| helmet(s).to_owned());
            let mut tags = Vec::new();
            if !change.is_empty() {
                tags.push(json!({ "text": "Changed", "tone": "accent" }));
            }
            json!({
                "id": format!("stock:{}", s.label),
                "title": s.name,
                "subtitle": format!("{} · #{} · {}", if s.code.is_empty() { "---" } else { &s.code }, s.number, game.label(List::Head, &head)),
                "art": art(app, &head, &s.code),
                "tags": tags,
                "trailing": { "button": button(&format!("stock_edit:{}", s.label), "Edit", "pencil", "outline") },
                "detail": [ { "facts": [
                    { "label": "Head", "value": game.label(List::Head, &head) },
                    { "label": "Helmet", "value": game.label(List::Helmet, &helm) },
                    { "label": "Looks", "value": s.looks.len().to_string() },
                ] } ]
            })
        })
        .collect();
    let changed = changes.drivers.len();
    json!({
        "subtitle": format!("{} drivers in the game · {changed} changed", drivers.len()),
        "actions": [button("stock_back", "Back to drivers", "", "ghost")],
        "notices": notices,
        "body": { "list": { "rows": rows, "reorder": false, "search": { "id": "stock_drivers", "placeholder": "Search game drivers" } } },
        "footer": format!("Changes are kept in {} and shared by every profile.", Changes::path(&app.data_dir).display())
    })
}

fn edit_view(app: &App, changes: &Changes, label: &str, notices: Vec<Value>) -> Value {
    let Some(s) = app.game.as_ref().and_then(|g| g.stock_driver(label)) else {
        return list_view(app, changes, notices);
    };
    let change = changes.drivers.get(label).cloned().unwrap_or_default();
    let head = change.head.clone().unwrap_or_else(|| s.head.clone());
    let helm = change.helmet.clone().unwrap_or_else(|| helmet(s).to_owned());
    let form = app.stock.as_ref().map_or_else(String::new, |e| e.form.clone());
    let mut buttons = vec![button("stock_list", "Back to game drivers", "", "ghost")];
    if !change.is_empty() {
        buttons.push(button(
            &format!("stock_reset:{label}"),
            "Use the stock head and helmet",
            "undo",
            "outline",
        ));
    }
    let fields = vec![
        json!({ "select": { "id": "head", "label": "Head", "value": head, "options": crate::pages::options(app, List::Head, &head), "search": true,
            "help": "Heads that addons or files in the game folder add are listed with the rest." } }),
        json!({ "select": { "id": "helmet", "label": "Helmet", "value": helm, "options": crate::pages::options(app, List::Helmet, &helm), "search": true,
            "help": "Every look the driver has (each season and series) gets this helmet." } }),
    ];
    json!({
        "subtitle": format!("Editing {}", s.name),
        "notices": notices,
        "body": { "blocks": [
            { "hero": {
                "title": s.name,
                "subtitle": format!("A driver in the game · {} look(s)", s.looks.len()),
                "art": art(app, &head, &s.code),
                "buttons": buttons
            } },
            { "card": {
                "title": "Head and helmet",
                "subtitle": "Applies the next time you play with Driver Manager on.",
                "blocks": [ { "form": { "id": form, "columns": 2, "fields": fields, "apply": "Save" } } ]
            } }
        ] }
    })
}

pub fn submit(app: &mut App, form: &str, values: &Value) -> Result<Value, String> {
    let Some(label) = app
        .stock
        .as_ref()
        .filter(|e| e.form == form)
        .and_then(|e| e.label.clone())
    else {
        return Ok(json!("reload"));
    };
    let game = app.game.as_ref().ok_or("The game's lists aren't loaded yet.")?;
    let s = game
        .stock_driver(&label)
        .ok_or_else(|| format!("{label} isn't in the game"))?;
    let pick = |key: &str, stock: &str| {
        values
            .get(key)
            .and_then(Value::as_str)
            .filter(|v| *v != stock)
            .map(str::to_owned)
    };
    let change = Change {
        head: pick("head", &s.head),
        helmet: pick("helmet", helmet(s)),
    };
    let name = s.name.clone();
    let mut changes = Changes::load(&app.data_dir)?;
    changes.set(&label, change);
    changes.save(&app.data_dir)?;
    app.stock = Some(Editing {
        label: None,
        form: String::new(),
    });
    Ok(json!({ "notice": format!("Saved {name}'s head and helmet.") }))
}

pub fn click(app: &mut App, id: &str) -> Option<Result<Value, String>> {
    if id == "stock" || id == "stock_list" {
        app.stock = Some(Editing {
            label: None,
            form: String::new(),
        });
        return Some(Ok(json!("reload")));
    }
    if id == "stock_back" {
        app.stock = None;
        return Some(Ok(json!("reload")));
    }
    if let Some(label) = id.strip_prefix("stock_edit:") {
        let form = app.next_form("stock");
        app.stock = Some(Editing {
            label: Some(label.to_owned()),
            form,
        });
        return Some(Ok(json!("reload")));
    }
    if let Some(label) = id.strip_prefix("stock_reset:") {
        let result = Changes::load(&app.data_dir).and_then(|mut c| {
            c.set(label, Change::default());
            c.save(&app.data_dir)
        });
        let form = app.next_form("stock");
        app.stock = Some(Editing {
            label: Some(label.to_owned()),
            form,
        });
        return Some(result.map(|()| json!({ "notice": "Back to the stock head and helmet." })));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Choice, StockLook};

    fn game() -> GameData {
        let choice = |v: &str| Choice {
            value: v.into(),
            label: v.into(),
        };
        let mut g = GameData {
            heads: vec![
                choice("sebastian_vettel"),
                choice("daniil_kvyat"),
                choice("nikita_mazepin"),
            ],
            helmets: vec![choice("2025-ferrari_x"), choice("modded_helmet")],
            ..GameData::default()
        };
        g.missing.insert(List::Head.key("nikita_mazepin"));
        g.stock.push(StockDriver {
            label: "pierre_gasly".into(),
            name: "Pierre Gasly".into(),
            head: "pierre_gasly".into(),
            looks: vec![
                StockLook {
                    folder: "official_driver_customisations".into(),
                    label: "2025-alpine_pierre_gasly".into(),
                    helmet: "2025-alpine_pierre_gasly".into(),
                },
                StockLook {
                    folder: "official_driver_customisations".into(),
                    label: "2026-alpine_pierre_gasly".into(),
                    helmet: "2026-alpine_pierre_gasly".into(),
                },
            ],
            ..StockDriver::default()
        });
        g
    }

    #[test]
    fn changes_become_sets_on_the_driver_and_every_look() {
        let g = game();
        let mut c = Changes::default();
        c.set(
            "pierre_gasly",
            Change {
                head: Some("daniil_kvyat".into()),
                helmet: Some("modded_helmet".into()),
            },
        );
        let (ops, warnings) = ops(&c, &g);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            ops,
            [
                json!({ "op": "db.set", "select": "entrants.drivers.driver[_metadata_=pierre_gasly]", "field": "head", "value": "daniil_kvyat" }),
                json!({ "op": "db.set", "select": "entrants.driver_customisations.folder[_metadata_=official_driver_customisations].preloaded_driver_customisation_properties[_metadata_=2025-alpine_pierre_gasly]", "field": "helmet", "value": "modded_helmet" }),
                json!({ "op": "db.set", "select": "entrants.driver_customisations.folder[_metadata_=official_driver_customisations].preloaded_driver_customisation_properties[_metadata_=2026-alpine_pierre_gasly]", "field": "helmet", "value": "modded_helmet" }),
            ]
        );
    }

    #[test]
    fn missing_heads_and_gone_drivers_are_left_out() {
        let g = game();
        let mut c = Changes::default();
        c.set(
            "pierre_gasly",
            Change {
                head: Some("nikita_mazepin".into()),
                helmet: None,
            },
        );
        c.set(
            "gone_driver",
            Change {
                head: Some("daniil_kvyat".into()),
                helmet: None,
            },
        );
        let (ops, warnings) = ops(&c, &g);
        assert!(ops.is_empty());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("keeps the stock head")));
        c.set("pierre_gasly", Change::default());
        assert!(!c.drivers.contains_key("pierre_gasly"));
    }
}
