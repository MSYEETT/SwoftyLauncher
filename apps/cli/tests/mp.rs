use std::io::{BufRead, BufReader, Lines};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

fn data_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-cli-mp-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    for dir in ["profiles", "addons/t.a/db", "game"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("config.json"),
        r#"{"data_version":3,"active_profile":"grid"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("profiles/grid.json"),
        r#"{"schema_version":2,"name":"grid","addons":[{"id":"t.a"}]}"#,
    )
    .unwrap();
    let manifest = json!({
        "schema_version": 1, "id": "t.a", "name": "Grid", "version": "1.2.0", "author": "a",
        "compat": { "game": "f1_25" }, "kind": ["database"],
        "operations": { "database": ["db/ops.json"] },
        "options": [{ "id": "laps", "type": "slider", "min": 1, "max": 10, "step": 1, "default": 5, "binds": "db" }],
    });
    std::fs::write(root.join("addons/t.a/swofty.json"), manifest.to_string()).unwrap();
    std::fs::write(
        root.join("addons/t.a/db/ops.json"),
        r#"{"schema_version":1,"ops":[]}"#,
    )
    .unwrap();
    std::fs::write(root.join("game/F1_25.exe"), b"exe").unwrap();
    root
}

fn swofty(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_swofty"))
        .args(args)
        .env("SWOFTY_DATA_DIR", root)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn prints_the_fingerprint_and_compares_two() {
    let root = data_dir("compare");
    let game = root.join("game");
    let game = game.to_str().unwrap();
    let out = swofty(&root, &["mp", "fingerprint", "--profile", "grid", "--game", game]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let f: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(f["version"], 1);
    assert_eq!(f["dlc"], Value::Null);
    assert!(f["game_build"]["build_id"].as_str().unwrap().starts_with("exe-"));
    let addons: Vec<&str> = f["addons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(addons, ["swofty.core", "t.a"]);
    assert_eq!(f["addons"][1]["version"], "1.2.0");
    assert_eq!(f["addons"][1]["source"]["content"].as_str().unwrap().len(), 64);
    assert_eq!(f["options"], json!({ "t.a": { "laps": 5 } }));
    assert_eq!(f["digest"].as_str().unwrap().len(), 64);

    let mut host = f.clone();
    host["options"]["t.a"]["laps"] = json!(9);
    let (a, b) = (root.join("a.json"), root.join("b.json"));
    std::fs::write(&a, f.to_string()).unwrap();
    std::fs::write(&b, host.to_string()).unwrap();
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

    let same = swofty(&root, &["mp", "compare", a, a]);
    assert!(same.status.success());
    assert_eq!(stdout(&same), "the fingerprints match\n");

    let differ = swofty(&root, &["mp", "compare", a, b]);
    assert_eq!(differ.status.code(), Some(1));
    assert_eq!(stdout(&differ), "t.a.laps is 5; the host has 9 (fixable)\n");

    let listed = swofty(&root, &["mp", "compare", "--json", a, b]);
    assert_eq!(listed.status.code(), Some(1));
    let found: Value = serde_json::from_str(&stdout(&listed)).unwrap();
    assert_eq!(
        found,
        json!([{ "kind": "option_differs", "addon": "t.a", "option": "laps", "local": 5, "host": 9, "fixable": true }])
    );
    std::fs::remove_dir_all(&root).ok();
}

struct Running {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
}

impl Running {
    fn start(root: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_swofty"))
            .args(args)
            .env("SWOFTY_DATA_DIR", root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        Self { child, lines }
    }

    fn line_with(&mut self, needle: &str) -> String {
        for line in self.lines.by_ref() {
            let line = line.unwrap();
            if line.contains(needle) {
                return line;
            }
        }
        panic!("swofty exited before printing {needle:?}");
    }

    fn base(&mut self) -> u16 {
        let line = self.line_with("SWOFTY_NETPLAY=");
        line.rsplit_once("base=").unwrap().1.parse().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn fake_game() -> (UdpSocket, String) {
    let game = UdpSocket::bind("127.0.0.1:0").unwrap();
    game.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let port = game.local_addr().unwrap().port().to_string();
    (game, port)
}

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

fn recv(game: &UdpSocket) -> (Vec<u8>, SocketAddr) {
    let mut buf = [0u8; 2048];
    let (n, from) = game.recv_from(&mut buf).unwrap();
    (buf[..n].to_vec(), from)
}

#[test]
fn echo_prints_what_the_game_sends_and_injects_a_captured_datagram() {
    let root = data_dir("echo");
    let (game, game_port) = fake_game();
    let capture = root.join("capture.bin");
    std::fs::write(&capture, b"captured datagram").unwrap();
    let inject = format!("3:{}", capture.display());
    let saved = root.join("saved");
    let mut echo = Running::start(
        &root,
        &[
            "mp",
            "echo",
            "--self",
            "2",
            "--members",
            "1,2,3",
            "--game-port",
            &game_port,
            "--inject-file",
            &inject,
            "--save",
            saved.to_str().unwrap(),
        ],
    );
    let base = echo.base();
    let (payload, from) = recv(&game);
    assert_eq!(payload, b"captured datagram");
    assert_eq!(from, loopback(base + 3));
    echo.line_with("injected 17 bytes as member 3");

    game.send_to(b"hello host", loopback(base + 1)).unwrap();
    let line = echo.line_with("to member 1 from");
    assert!(line.ends_with(", 10 bytes: 68656c6c6f20686f7374"), "{line}");
    assert_eq!(
        std::fs::read(saved.join("000000-to-1.bin")).unwrap(),
        b"hello host"
    );
    drop(echo);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn two_bridges_relay_between_fake_games() {
    let root = data_dir("bridge");
    let key = "a5".repeat(32);
    let free = || {
        UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };
    let (listen_a, listen_b) = (free(), free());
    let (game_a, port_a) = fake_game();
    let (game_b, port_b) = fake_game();
    let bridge = |me: &str, them: &str, listen: u16, peer: u16, game_port: &str| {
        Running::start(
            &root,
            &[
                "mp",
                "bridge",
                "--self",
                me,
                "--members",
                "1,2",
                "--key-hex",
                &key,
                "--peer",
                &format!("{them}=127.0.0.1:{peer}"),
                "--listen",
                &format!("127.0.0.1:{listen}"),
                "--game-port",
                game_port,
            ],
        )
    };
    let mut a = bridge("1", "2", listen_a, listen_b, &port_a);
    let mut b = bridge("2", "1", listen_b, listen_a, &port_b);
    let (base_a, base_b) = (a.base(), b.base());

    game_a.send_to(b"from one", loopback(base_a + 2)).unwrap();
    let (payload, from) = recv(&game_b);
    assert_eq!(payload, b"from one");
    assert_eq!(from, loopback(base_b + 1));
    game_b.send_to(b"from two", loopback(base_b + 1)).unwrap();
    let (payload, from) = recv(&game_a);
    assert_eq!(payload, b"from two");
    assert_eq!(from, loopback(base_a + 2));

    let bad = swofty(
        &root,
        &[
            "mp",
            "bridge",
            "--self",
            "1",
            "--members",
            "1,2",
            "--key-hex",
            "zz",
            "--peer",
            "2=127.0.0.1:9",
        ],
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("64 hex digits"));
    drop((a, b));
    std::fs::remove_dir_all(&root).ok();
}
