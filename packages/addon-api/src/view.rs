use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionSpec {
    pub id: String,
    pub title: String,
    pub icon: Icon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub subtitle: String,
    #[serde(default)]
    pub actions: Vec<Button>,
    #[serde(default)]
    pub notices: Vec<Notice>,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    List {
        rows: Vec<Row>,
        reorder: bool,
    },
    Empty {
        title: String,
        text: String,
        action: Option<Button>,
    },
    Blocks(Vec<Block>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    #[serde(default)]
    pub art: Art,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub trailing: Option<Control>,
    #[serde(default)]
    pub detail: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    Stats(Vec<Fact>),
    Facts(Vec<Fact>),
    Label(String),
    Text {
        text: String,
        tone: Tone,
    },
    Names {
        label: String,
        items: Vec<String>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Buttons(Vec<Button>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Switch {
        id: String,
        on: bool,
        enabled: bool,
    },
    Choice {
        id: String,
        options: Vec<String>,
        current: usize,
    },
    Button(Button),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<Icon>,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub kind: ButtonKind,
}

impl Button {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            style: Style::default(),
            kind: ButtonKind::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonKind {
    #[default]
    Event,
    Confirm {
        title: String,
        text: String,
    },
    Pick {
        title: String,
        extensions: Vec<String>,
        folders: bool,
        multiple: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Click { id: String },
    Toggle { id: String, on: bool },
    Choose { id: String, index: usize },
    Move { row: String, to: usize },
    Picked { id: String, paths: Vec<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    None,
    Reload,
    Notice(String),
    SetAddonEnabled(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

impl Fact {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
    #[serde(default)]
    pub action: Option<Button>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Muted,
    Accent,
    Good,
    Warn,
    Bad,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Primary,
    #[default]
    Outline,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Art {
    #[default]
    None,
    Monogram(String),
    Image(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Icon {
    Activity,
    ArrowRightLeft,
    ChevronDown,
    ChevronUp,
    CircleAlert,
    CircleCheck,
    Copy,
    Eraser,
    FolderOpen,
    Gamepad,
    Layers,
    Lock,
    Map,
    Package,
    Pencil,
    Play,
    Plus,
    Puzzle,
    Settings,
    ShieldCheck,
    Trash,
    Upload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_round_trip_through_json() {
        let view = View {
            subtitle: "1 in library".into(),
            actions: vec![Button {
                kind: ButtonKind::Pick {
                    title: "Import".into(),
                    extensions: vec!["serps".into(), "zip".into()],
                    folders: true,
                    multiple: true,
                },
                icon: Some(Icon::Upload),
                ..Button::new("import", "Import…")
            }],
            notices: vec![Notice {
                text: "Team Simplified is off in this profile.".into(),
                tone: Tone::Warn,
                action: Some(Button::new("enable", "Turn on")),
            }],
            body: Body::List {
                rows: vec![Row {
                    id: "rm.alphatauri".into(),
                    title: "Scuderia AlphaTauri".into(),
                    subtitle: "RM Designs · 1.0".into(),
                    art: Art::Monogram("AT".into()),
                    tags: vec![Tag {
                        text: "Database".into(),
                        tone: Tone::Muted,
                    }],
                    trailing: Some(Control::Switch {
                        id: "rm.alphatauri".into(),
                        on: true,
                        enabled: true,
                    }),
                    detail: vec![Block::Stats(vec![Fact::new("Version", "1.0")])],
                }],
                reorder: true,
            },
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["body"]["list"]["rows"][0]["trailing"]["switch"]["on"], true);
        assert_eq!(json["actions"][0]["icon"], "upload");
        assert_eq!(serde_json::from_value::<View>(json).unwrap(), view);
        let event: Event = serde_json::from_str(r#"{"move":{"row":"a","to":2}}"#).unwrap();
        assert_eq!(
            event,
            Event::Move {
                row: "a".into(),
                to: 2
            }
        );
        assert_eq!(serde_json::to_value(Effect::Reload).unwrap(), "reload");
    }
}
