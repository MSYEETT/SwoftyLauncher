use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::asset::{AssetOp, Features};
use crate::launcher::ProfileInfo;
use crate::view::Event;

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub api: u32,
    pub addon: String,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub profile: ProfileInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum HostCall {
    Progress { done: f32, text: String },
    GetState,
    SetState(Value),
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
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
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
                }),
            },
            json!({"id": 0, "method": "hello", "params": {"api": 1, "addon": "example.addon",
                "data_dir": "/d", "cache_dir": "/c",
                "profile": {"name": "default", "vanilla": false, "enabled": true}}}),
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
        let built: Built = serde_json::from_value(json!({"ops": [
            {"op": "file.put", "source": {"root": "data", "path": "a.dds"}, "target": "b.dds"}
        ]}))
        .unwrap();
        assert!(matches!(
            &built.ops[0],
            AssetOp::File(FileOp::Put { source: Source { path, .. }, .. }) if path == "a.dds"
        ));
        assert_eq!(
            serde_json::from_str::<Incoming>(r#"{"id": 7, "method": "get_state", "params": null}"#).unwrap(),
            Incoming::Call(Request {
                id: Some(7),
                call: HostCall::GetState,
            })
        );
        assert!(serde_json::from_str::<Incoming>(r#"{"result": 1}"#).is_err());
        assert!(serde_json::from_str::<Incoming>(r#"{"id": 1, "method": "launch"}"#).is_err());
    }
}
