use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::asset::{AssetOp, Features};
use crate::launcher::ProfileInfo;
use crate::view::Event;

pub use crate::run_helper::{claim_word, run_helper};

pub const API_VERSION: u32 = 1;
pub const RESTART: &str = "swofty.restart";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request<C> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub call: C,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum Call {
    Hello(Hello),
    Profile(ProfileInfo),
    View {
        section: String,
    },
    Event {
        section: String,
        event: Event,
    },
    Import {
        paths: Vec<PathBuf>,
    },
    Build {
        options: BTreeMap<String, Value>,
        features: Features,
    },
    Command {
        path: Vec<String>,
        args: Vec<String>,
    },
    Summary,
    Tick {
        section: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub api: u32,
    pub addon: String,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub profile: ProfileInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum HostCall {
    Progress { done: f32, text: String },
    GetState,
    SetState(Value),
    GameBuildKey,
    GameExeVersion,
    GameIsStock { rel: String },
    GameOriginal { rel: String },
    GameOriginalLang { code: String },
    GameDir,
    AddonGameDir,
    GameStatus,
    SteamSaveDir,
    ProfileTracks,
    Invalidate(#[serde(default)] Invalidate),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invalidate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reply {
    Ok { id: u64, result: Value },
    Err { id: u64, error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Reply(Reply),
    Call(Request<HostCall>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub api: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Built {
    #[serde(default)]
    pub ops: Vec<AssetOp>,
    // The item each op comes from, by position in ops, such as one mod of the
    // addon's library. Core builds one runtime overlay archive per item.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Option<String>>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    // Field names for add-row, keyed `<table>.<row>` as in an op file's `fields`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Vec<String>>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::asset::{FileOp, Kind, Source};

    fn profile() -> ProfileInfo {
        ProfileInfo {
            name: "default".into(),
            vanilla: false,
            enabled: true,
        }
    }

    fn round<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: T,
        wire: Value,
    ) {
        assert_eq!(serde_json::to_value(&value).unwrap(), wire);
        let line = serde_json::to_string(&value).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<T>(&line).unwrap(), value);
    }

    #[test]
    fn requests_round_trip() {
        round(
            Request {
                id: Some(0),
                call: Call::Hello(Hello {
                    api: API_VERSION,
                    addon: "example.addon".into(),
                    data_dir: "/d".into(),
                    cache_dir: "/c".into(),
                    profile: profile(),
                    launcher: Some("1.0.10".into()),
                }),
            },
            json!({"id": 0, "method": "hello", "params": {"api": 1, "addon": "example.addon",
                "data_dir": "/d", "cache_dir": "/c",
                "profile": {"name": "default", "vanilla": false, "enabled": true},
                "launcher": "1.0.10"}}),
        );
        round(
            Request {
                id: None,
                call: Call::Profile(profile()),
            },
            json!({"method": "profile", "params": {"name": "default", "vanilla": false, "enabled": true}}),
        );
        round(
            Request {
                id: Some(2),
                call: Call::Event {
                    section: "items".into(),
                    event: Event::Toggle {
                        id: "a".into(),
                        on: true,
                    },
                },
            },
            json!({"id": 2, "method": "event", "params": {"section": "items",
                "event": {"toggle": {"id": "a", "on": true}}}}),
        );
        round(
            Request {
                id: Some(3),
                call: Call::Build {
                    options: BTreeMap::from([("size".into(), json!(24))]),
                    features: Features {
                        stable: [Kind::File].into(),
                        experimental: Default::default(),
                    },
                },
            },
            json!({"id": 3, "method": "build", "params": {"options": {"size": 24},
                "features": {"stable": ["file"], "experimental": []}}}),
        );
        round(
            Request {
                id: Some(4),
                call: Call::Summary,
            },
            json!({"id": 4, "method": "summary"}),
        );
    }

    #[test]
    fn helper_messages_round_trip() {
        round(
            Incoming::Reply(Reply::Ok {
                id: 1,
                result: Value::Null,
            }),
            json!({"id": 1, "result": null}),
        );
        round(
            Incoming::Reply(Reply::Err {
                id: 1,
                error: "no".into(),
            }),
            json!({"id": 1, "error": "no"}),
        );
        round(
            Incoming::Call(Request {
                id: None,
                call: HostCall::Progress {
                    done: 0.5,
                    text: "half".into(),
                },
            }),
            json!({"method": "progress", "params": {"done": 0.5, "text": "half"}}),
        );
        round(
            Incoming::Call(Request {
                id: Some(7),
                call: HostCall::GetState,
            }),
            json!({"id": 7, "method": "get_state"}),
        );
        round(
            Incoming::Call(Request {
                id: Some(8),
                call: HostCall::SetState(json!({"on": ["a"]})),
            }),
            json!({"id": 8, "method": "set_state", "params": {"on": ["a"]}}),
        );
        round(
            Incoming::Call(Request {
                id: Some(9),
                call: HostCall::GameBuildKey,
            }),
            json!({"id": 9, "method": "game_build_key"}),
        );
        round(
            Incoming::Call(Request {
                id: Some(10),
                call: HostCall::GameOriginal {
                    rel: "ui/a.erp".into(),
                },
            }),
            json!({"id": 10, "method": "game_original", "params": {"rel": "ui/a.erp"}}),
        );
        let built: Built = serde_json::from_value(json!({"ops": [
            {"op": "file.put", "source": {"root": "data", "path": "a.dds"}, "target": "b.dds"}
        ]}))
        .unwrap();
        assert!(matches!(
            &built.ops[0],
            AssetOp::File(FileOp::Put { source: Source { path, .. }, .. }) if path == "a.dds"
        ));
        assert!(built.items.is_empty());
        let built: Built = serde_json::from_value(json!({"ops": [
            {"op": "file.put", "source": {"root": "data", "path": "a.dds"}, "target": "b.dds"},
            {"op": "file.put", "source": {"root": "data", "path": "c.dds"}, "target": "d.dds"}
        ], "items": ["mod.a", null]}))
        .unwrap();
        assert_eq!(built.items, [Some("mod.a".to_owned()), None]);
        assert_eq!(
            serde_json::to_value(&built).unwrap()["items"],
            json!(["mod.a", null])
        );
        assert!(
            serde_json::to_value(Built::default())
                .unwrap()
                .get("items")
                .is_none()
        );
        assert!(built.fields.is_empty());
        let built: Built = serde_json::from_value(json!({"ops": [
            {"op": "db.add-row", "table": "t", "from": "t.r_0"}
        ], "fields": {"t.r": ["a", "b"]}}))
        .unwrap();
        assert_eq!(built.fields["t.r"], ["a", "b"]);
        assert!(
            serde_json::to_value(Built::default())
                .unwrap()
                .get("fields")
                .is_none()
        );
        assert_eq!(
            serde_json::from_str::<Incoming>(r#"{"id": 7, "method": "get_state", "params": null}"#).unwrap(),
            Incoming::Call(Request {
                id: Some(7),
                call: HostCall::GetState,
            })
        );
        assert!(serde_json::from_str::<Incoming>(r#"{"result": 1}"#).is_err());
        round(
            Request {
                id: Some(5),
                call: Call::Tick {
                    section: "overview".into(),
                },
            },
            json!({"id": 5, "method": "tick", "params": {"section": "overview"}}),
        );
        for (method, call) in [
            ("game_dir", HostCall::GameDir),
            ("addon_game_dir", HostCall::AddonGameDir),
            ("game_status", HostCall::GameStatus),
            ("steam_save_dir", HostCall::SteamSaveDir),
            ("profile_tracks", HostCall::ProfileTracks),
        ] {
            round(
                Incoming::Call(Request { id: Some(11), call }),
                json!({"id": 11, "method": method}),
            );
        }
        round(
            Incoming::Call(Request {
                id: None,
                call: HostCall::Invalidate(Invalidate {
                    section: Some("overview".into()),
                }),
            }),
            json!({"method": "invalidate", "params": {"section": "overview"}}),
        );
        assert_eq!(
            serde_json::from_str::<Incoming>(r#"{"method": "invalidate", "params": {}}"#).unwrap(),
            Incoming::Call(Request {
                id: None,
                call: HostCall::Invalidate(Invalidate::default()),
            })
        );
        let status: crate::launcher::GameStatus = serde_json::from_str(
            r#"{"running": true, "pid": 48211, "since": 1760941234.5, "session": "2026-10-20T14-22-33"}"#,
        )
        .unwrap();
        assert_eq!(status.pid, Some(48211));
        assert_eq!(status.session.as_deref(), Some("2026-10-20T14-22-33"));
        assert!(serde_json::from_str::<Incoming>(r#"{"id": 1, "method": "launch"}"#).is_err());
    }
}
