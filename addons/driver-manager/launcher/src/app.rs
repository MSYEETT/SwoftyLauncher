use std::path::PathBuf;

use serde_json::{Value, json};

use crate::game::{self, GameData};
use crate::host::Host;
use crate::loader::{self, Loading};
use crate::model::{Driver, Roster};
use crate::preview::{self, Building, Previews};

pub struct Edit {
    pub draft: Driver,
    pub form: String,
    pub tried: bool,
}

pub struct App {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub vanilla: bool,
    pub enabled: bool,
    pub game: Option<GameData>,
    pub game_error: Option<String>,
    pub loading: Option<Loading>,
    pub edit: Option<Edit>,
    pub forms: u32,
    pub game_dir: Option<PathBuf>,
    pub previews: Option<Previews>,
    pub building: Option<Building>,
    pub previews_tried: bool,
}

impl App {
    pub fn new(params: &Value) -> App {
        let path = |k: &str| PathBuf::from(params[k].as_str().unwrap_or_default());
        App {
            data_dir: path("data_dir"),
            cache_dir: path("cache_dir"),
            vanilla: params["profile"]["vanilla"].as_bool().unwrap_or(false),
            enabled: params["profile"]["enabled"].as_bool().unwrap_or(true),
            game: None,
            game_error: None,
            loading: None,
            edit: None,
            forms: 0,
            game_dir: None,
            previews: None,
            building: None,
            previews_tried: false,
        }
    }

    pub fn roster(&self) -> Result<Roster, String> {
        Roster::load(&self.data_dir)
    }

    pub fn save(&self, roster: &Roster) -> Result<(), String> {
        roster.save(&self.data_dir)
    }

    pub fn next_form(&mut self, base: &str) -> String {
        self.forms += 1;
        format!("{base}_{}", self.forms)
    }

    fn string(host: &mut Host, method: &str, params: Value) -> Result<Option<String>, String> {
        host.call(method, params).map(|v| v.as_str().map(str::to_owned))
    }

    pub fn ensure_game(&mut self, host: &mut Host) {
        self.poll();
        if self.game.is_some() || self.loading.is_some() || self.game_error.is_some() {
            return;
        }
        let key = match Self::string(host, "game_build_key", Value::Null) {
            Ok(Some(k)) => k,
            Ok(None) => {
                self.game_error =
                    Some("The launcher hasn't found F1 25. Locate the game on the Library page.".into());
                return;
            }
            Err(e) => {
                self.game_error = Some(e);
                return;
            }
        };
        let cache = loader::cache_path(&self.cache_dir);
        if let Some(data) = GameData::load_cached(&cache, &key) {
            self.game = Some(data);
            return;
        }
        let db = match Self::string(host, "game_original", json!({ "rel": game::DB_REL })) {
            Ok(Some(p)) => PathBuf::from(p),
            Ok(None) => {
                self.game_error = Some("The launcher returned no game database.".into());
                return;
            }
            Err(e) => {
                self.game_error = Some(format!("The game database isn't available: {e}"));
                return;
            }
        };
        let dir = Self::string(host, "game_dir", Value::Null)
            .ok()
            .flatten()
            .map(PathBuf::from);
        self.game_dir.clone_from(&dir);
        self.loading = Some(Loading::start(db, dir, key, cache));
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(done) = self.building.as_ref().and_then(Building::poll) {
            self.building = None;
            if let Some(p) = &done {
                eprintln!(
                    "previews: {} built, {} without art, {} ms, {} KiB",
                    p.index.ready.len(),
                    p.index.failed.len(),
                    p.index.millis,
                    p.index.bytes / 1024
                );
            }
            self.previews = done;
            changed = true;
        }
        let Some(result) = self.loading.as_ref().and_then(Loading::poll) else {
            return changed;
        };
        self.loading = None;
        match result {
            Ok(g) => self.game = Some(g),
            Err(e) => self.game_error = Some(e),
        }
        true
    }

    pub fn busy(&self) -> bool {
        self.loading.is_some() || self.building.is_some()
    }

    pub fn ensure_previews(&mut self, host: &mut Host) {
        let Some(game) = &self.game else {
            return;
        };
        if self.previews_tried {
            return;
        }
        self.previews_tried = true;
        if self.game_dir.is_none() {
            self.game_dir = Self::string(host, "game_dir", Value::Null)
                .ok()
                .flatten()
                .map(PathBuf::from);
        }
        let Some(dir) = self.game_dir.clone() else {
            return;
        };
        let root = self.cache_dir.join("previews");
        if let Some(p) = Previews::load(&root, &game.key) {
            self.previews = Some(p);
            return;
        }
        self.building = Some(Building::start(dir, root, game.key.clone(), preview::jobs(game)));
    }

    pub fn thumb(&self, list: game::List, value: &str) -> Option<String> {
        self.previews
            .as_ref()?
            .path(list, value)
            .map(|p| p.to_string_lossy().into_owned())
    }

    pub fn large(&mut self, list: game::List, value: &str) -> Option<String> {
        let (Some(previews), Some(game), Some(dir)) = (self.previews.as_mut(), &self.game, &self.game_dir)
        else {
            return None;
        };
        previews
            .large(dir, game, list, value)
            .map(|p| p.to_string_lossy().into_owned())
    }

    pub fn wait_game(&mut self, host: &mut Host) {
        self.ensure_game(host);
        if let Some(l) = self.loading.take() {
            match l.wait() {
                Ok(g) => self.game = Some(g),
                Err(e) => self.game_error = Some(e),
            }
        }
    }

    pub fn retry_game(&mut self) {
        if self.loading.is_none() {
            self.game_error = None;
        }
        if self.game.is_none() {
            self.previews_tried = false;
        }
    }
}
