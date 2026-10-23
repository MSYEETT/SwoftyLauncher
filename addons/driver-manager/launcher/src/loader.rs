use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use swofty_formats::gvdb::Gvdb;
use swofty_formats::lng::Lng;
use swofty_formats::nefs::Archive;

use crate::game::{self, GameData};

const GAME_EXE: &str = "F1_25.exe";
const ENG: &str = "localisation/2025_release/language_eng.lng";

pub struct Loading(Receiver<Result<GameData, String>>);

pub fn cache_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("game-lists.json")
}

pub fn english(game_dir: &Path) -> Result<Lng, String> {
    let exe_path = game_dir.join(GAME_EXE);
    let mut exe = std::fs::File::open(&exe_path).map_err(|e| format!("{}: {e}", exe_path.display()))?;
    let archive = Archive::open_exe(&mut exe).map_err(|e| e.to_string())?;
    let entry = archive.entry(ENG).ok_or("the game has no English text")?;
    let volume = game_dir.join(&archive.volume);
    let mut data = std::fs::File::open(&volume).map_err(|e| format!("{}: {e}", volume.display()))?;
    let bytes = archive.read(entry, &mut data).map_err(|e| e.to_string())?;
    Lng::parse(&bytes).map_err(|e| e.to_string())
}

pub fn read(db_path: &Path, game_dir: Option<&Path>, key: &str) -> Result<GameData, String> {
    let bytes = std::fs::read(db_path).map_err(|e| format!("{}: {e}", db_path.display()))?;
    let db = Gvdb::parse(&bytes).map_err(|e| format!("the game database is unreadable: {e}"))?;
    let lng = match game_dir.map(english) {
        Some(Ok(lng)) => Some(lng),
        Some(Err(e)) => {
            eprintln!("English text unavailable, showing internal names: {e}");
            None
        }
        None => None,
    };
    Ok(game::extract(&db, lng.as_ref(), key))
}

impl Loading {
    pub fn start(db_path: PathBuf, game_dir: Option<PathBuf>, key: String, cache: PathBuf) -> Loading {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let result = read(&db_path, game_dir.as_deref(), &key);
            if let Ok(data) = &result {
                data.save_cached(&cache);
            }
            tx.send(result).ok();
        });
        Loading(rx)
    }

    pub fn poll(&self) -> Option<Result<GameData, String>> {
        match self.0.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("reading the game's lists stopped".into())),
        }
    }

    pub fn wait(self) -> Result<GameData, String> {
        self.0
            .recv()
            .unwrap_or_else(|_| Err("reading the game's lists stopped".into()))
    }
}
