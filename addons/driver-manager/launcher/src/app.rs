use std::path::PathBuf;

use serde_json::{Value, json};

use crate::game::{self, GameData};
use crate::host::Host;
use crate::loader::{self, Loading};
use crate::model::{Driver, Roster};

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
        self.loading = Some(Loading::start(db, dir, key, cache));
    }

    pub fn poll(&mut self) -> bool {
        let Some(result) = self.loading.as_ref().and_then(Loading::poll) else {
            return false;
        };
        self.loading = None;
        match result {
            Ok(g) => self.game = Some(g),
            Err(e) => self.game_error = Some(e),
        }
        true
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
    }
}
