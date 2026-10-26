use std::collections::HashMap;
use std::io::{BufRead, IsTerminal};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use swofty_core::catalog;
use swofty_core::mpauth;
use swofty_core::multiplayer::bridge::{self, Bridge, BridgeConfig, Echoed, Stats};
use swofty_core::multiplayer::direct::DirectUdp;
use swofty_core::multiplayer::fingerprint::{self, Fingerprint};
use swofty_core::multiplayer::game::{self as mp_game, Category, Command as GameCommand};
use swofty_core::multiplayer::rooms::{Api, RoomSettings};
use swofty_core::multiplayer::session::{
    self, Local, MemberState, Phase, Relay, Session, SessionState, Setup,
};
use swofty_core::multiplayer::transport::{MAX_DATAGRAM, Transport};
use swofty_core::multiplayer::turn::TurnTransport;
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum MpCommand {
    #[command(
        about = "Print the profile's multiplayer fingerprint as JSON: the game build, its gameplay addons and their settings"
    )]
    Fingerprint {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
    },
    #[command(
        about = "Compare two fingerprints and list what the first must change to match the second, the host's; exits 1 when they differ"
    )]
    Compare {
        #[arg(help = "This player's fingerprint")]
        local: PathBuf,
        #[arg(help = "The host's fingerprint")]
        host: PathBuf,
        #[arg(long, help = "Print the differences as JSON")]
        json: bool,
    },
    #[command(
        about = "Run the netplay bridge over plain UDP to each member's address (a LAN or one machine); prints SWOFTY_NETPLAY for the game, then stats until Ctrl-C"
    )]
    Bridge(BridgeArgs),
    #[command(
        about = "Run the netplay bridge with no relay: print what the game sends to each member, and send the game captured datagrams"
    )]
    Echo(EchoArgs),
    #[command(
        about = "Allocate a TURN relay and print its address; with peers, ping them through their relays and print round trips"
    )]
    TurnProbe {
        #[arg(
            long,
            help = "The server's /v1/mp/turn answer (iceServers with username and credential)"
        )]
        ice_json: PathBuf,
        #[arg(long = "self", help = "This probe's member number, for the printout")]
        self_member: Option<u8>,
        #[arg(
            long,
            value_name = "N=IP:PORT",
            help = "A peer probe's member number and relayed address; repeatable. Without any, they're read from stdin, one per line until an empty line"
        )]
        peer: Vec<String>,
        #[arg(long, default_value_t = 10, help = "Round trips to measure per peer")]
        count: usize,
        #[arg(long, default_value_t = 30, help = "Give up after this many seconds")]
        wait: u64,
    },
    #[command(
        about = "Drive the running game's LAN session: status, room settings, host from the main menu, join a host, or leave"
    )]
    Game {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
        #[command(subcommand)]
        cmd: GameCmd,
    },
    #[command(about = "List the open multiplayer rooms")]
    Rooms {
        #[arg(long, value_name = "URL", help = SERVER_HELP)]
        server: Option<String>,
        #[arg(long, help = "Print the rooms as JSON")]
        json: bool,
    },
    #[command(
        about = "Create or join a multiplayer room and drive the game into its LAN session; prints the room until Ctrl-C, then leaves it"
    )]
    Room {
        #[command(subcommand)]
        cmd: RoomCmd,
    },
}

const SERVER_HELP: &str = "Lobby server (default: the launcher's server, SWOFTY_REGISTRY_URL)";

#[derive(Subcommand)]
pub enum RoomCmd {
    #[command(about = "Create a room and host it; the game must be on its main menu")]
    Create {
        #[arg(long, help = "Room name (default: <your name>'s room)")]
        name: Option<String>,
        #[arg(
            long,
            help = "Leave the room out of the room list; players join with its code"
        )]
        private: bool,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u8).range(2..=20), help = "Most players, 2 to 20")]
        max: u8,
        #[arg(long, default_value = "f1-2026", help = "Car category: f1-2026 or f1-2025")]
        category: Category,
        #[command(flatten)]
        common: RoomArgs,
    },
    #[command(about = "Join a room by its invite code once your addons match the host's")]
    Join {
        #[arg(help = "The invite code, like F1-7KQ2MX")]
        code: String,
        #[command(flatten)]
        common: RoomArgs,
    },
}

#[derive(Args)]
pub struct RoomArgs {
    #[arg(long, value_name = "URL", help = SERVER_HELP)]
    server: Option<String>,
    #[arg(long, help = crate::GAME_HELP)]
    game: Option<PathBuf>,
    #[arg(long, default_value_t = 10, help = "Seconds between traffic stats")]
    interval: u64,
    #[arg(
        long,
        value_name = "IP:PORT",
        hide = true,
        help = "Relay over plain UDP on this address instead of TURN, for a lobby server on this machine"
    )]
    direct_udp: Option<SocketAddr>,
}

#[derive(Subcommand)]
pub enum GameCmd {
    #[command(
        about = "Print the game's screen (main_menu, lan_lobby, other), whether it is in a LAN session, its role and room settings"
    )]
    Status,
    #[command(about = "Set the room: this player's member number and the bridge's loopback base port")]
    Configure {
        #[arg(
            long = "self",
            value_name = "N",
            help = "This player's member number, 1 to 254 (the host is 1)"
        )]
        self_member: u8,
        #[arg(long, value_name = "B", help = "The bridge's loopback base port, 1 to 65281")]
        base: u16,
    },
    #[command(about = "Forget the room settings")]
    Clear,
    #[command(
        about = "Host a LAN game with a car category; the game must be on its main menu with a room set"
    )]
    Host {
        #[arg(long, default_value = "f1-2026", help = "Car category: f1-2026 or f1-2025")]
        category: Category,
    },
    #[command(
        about = "Join the LAN game hosted at an IPv4 address; the game must be on its main menu with a room set"
    )]
    Join {
        #[arg(help = "The host's IPv4 address")]
        ip: std::net::Ipv4Addr,
    },
    #[command(about = "Leave the LAN session for the main menu")]
    Leave,
}

#[derive(Args)]
pub struct LobbyArgs {
    #[arg(
        long = "self",
        value_name = "N",
        help = "This player's lobby number (the host is 1)"
    )]
    self_member: u8,
    #[arg(
        long,
        value_delimiter = ',',
        required = true,
        help = "Every lobby member's number, comma-separated"
    )]
    members: Vec<u8>,
    #[arg(long, default_value_t = bridge::DEFAULT_GAME_PORT, help = "The game's socket port on loopback")]
    game_port: u16,
    #[arg(
        long,
        help = "Game folder: take the game port from swofty/netplay.json once the core writes it"
    )]
    game: Option<PathBuf>,
}

#[derive(Args)]
pub struct BridgeArgs {
    #[command(flatten)]
    lobby: LobbyArgs,
    #[arg(
        long,
        env = "SWOFTY_NETPLAY_KEY",
        hide_env_values = true,
        help = "The lobby key, 64 hex digits"
    )]
    key_hex: String,
    #[arg(
        long = "peer",
        value_name = "N=HOST:PORT",
        required = true,
        help = "Where member N's bridge listens; repeat for each member"
    )]
    peers: Vec<String>,
    #[arg(
        long,
        default_value = "0.0.0.0:21480",
        help = "Where this bridge listens for its peers"
    )]
    listen: String,
    #[arg(long, default_value_t = 5, help = "Seconds between stats")]
    interval: u64,
}

#[derive(Args)]
pub struct EchoArgs {
    #[command(flatten)]
    lobby: LobbyArgs,
    #[arg(
        long = "inject-file",
        value_name = "M:PATH",
        help = "Send the datagram in PATH to the game as member M; repeatable. Sent at start (with --game, once the core reports the bridge), on each Enter, and every --every seconds"
    )]
    inject: Vec<String>,
    #[arg(
        long,
        value_name = "SECS",
        help = "Send the --inject-file datagrams again every SECS seconds"
    )]
    every: Option<u64>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Save each datagram the game sends as DIR/<seq>-to-<member>.bin"
    )]
    save: Option<PathBuf>,
}

impl LobbyArgs {
    fn config(&self, key: [u8; 32]) -> BridgeConfig {
        BridgeConfig {
            self_member: self.self_member,
            members: self.members.clone(),
            key,
            game_port: self.game_port,
        }
    }
}

pub fn command(store: &Store, profile: &str, cmd: MpCommand) -> Result<ExitCode> {
    match cmd {
        MpCommand::Fingerprint { game } => {
            let g = crate::build::game(store, game)?;
            let p = store.profile(profile)?;
            let f = fingerprint::fingerprint(store, &p, &g.identity)?;
            println!("{}", serde_json::to_string_pretty(&f)?);
            Ok(ExitCode::SUCCESS)
        }
        MpCommand::Compare { local, host, json } => {
            let (local, host) = (read(&local)?, read(&host)?);
            let listing = catalog::cached(&store.root).unwrap_or_default();
            let found = fingerprint::compare(&local, &host, &listing);
            if json {
                println!("{}", serde_json::to_string_pretty(&found)?);
            } else if found.is_empty() {
                println!("the fingerprints match");
            } else {
                found.iter().for_each(|m| println!("{}", m.line()));
            }
            Ok(if found.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        MpCommand::Bridge(args) => run_bridge(args),
        MpCommand::Echo(args) => run_echo(args),
        MpCommand::TurnProbe {
            ice_json,
            self_member,
            peer,
            count,
            wait,
        } => turn_probe(&ice_json, self_member, &peer, count, Duration::from_secs(wait)),
        MpCommand::Game { game, cmd } => run_game(store, game, cmd),
        MpCommand::Rooms { server, json } => list_rooms(store, server.as_deref(), json),
        MpCommand::Room { cmd } => run_room(store, profile, cmd),
    }
}

fn api(store: &Store, server: Option<&str>) -> Result<Api> {
    let session = mpauth::load(&store.root)
        .context("not signed in to multiplayer; sign in on the launcher's Multiplayer page first")?;
    Ok(match server {
        Some(url) => Api::with_server(url, &session.token),
        None => Api::new(&session.token),
    })
}

fn list_rooms(store: &Store, server: Option<&str>, json: bool) -> Result<ExitCode> {
    let rooms = api(store, server)?.rooms()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rooms)?);
        return Ok(ExitCode::SUCCESS);
    }
    if rooms.is_empty() {
        println!("no open rooms");
    }
    for r in &rooms {
        let addons = r.addons.as_ref().map_or_else(String::new, |a| {
            let tracks = if a.tracks.is_empty() {
                String::new()
            } else {
                format!(" (tracks: {})", a.tracks.join(", "))
            };
            format!(", {} addons{tracks}", a.count)
        });
        println!(
            "{}  {}  host {}  {}/{}  {}{addons}",
            r.code, r.name, r.host.name, r.members, r.max_members, r.category
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn run_room(store: &Store, profile: &str, cmd: RoomCmd) -> Result<ExitCode> {
    let common = match &cmd {
        RoomCmd::Create { common, .. } | RoomCmd::Join { common, .. } => common,
    };
    let api = api(store, common.server.as_deref())?;
    let g = crate::build::game(store, common.game.clone())?;
    let local = Local::read(store, &store.profile(profile)?, &g.identity)?;
    let mut setup = Setup::new(api, &g.dir, local);
    if let Some(addr) = common.direct_udp {
        setup.relay = Relay::Direct(addr);
    }
    let interval = Duration::from_secs(common.interval.max(1));
    let session = match cmd {
        RoomCmd::Create {
            name,
            private,
            max,
            category,
            ..
        } => Session::create(
            setup,
            RoomSettings {
                name,
                public: !private,
                max_members: max,
                category,
            },
        ),
        RoomCmd::Join { code, .. } => Session::join(setup, &code),
    };
    crate::run::end_on_ctrl_c();
    let started = Instant::now();
    let mut next_stats = started + interval;
    let mut shown: Option<SessionState> = None;
    while !crate::run::ending() {
        let state = session.state();
        show_changes(shown.as_ref(), &state);
        let over = state.phase.is_over();
        if Instant::now() >= next_stats {
            if let Some(stats) = &state.bridge {
                print_stats(stats, started.elapsed(), None);
            }
            next_stats += interval;
        }
        shown = Some(state);
        if over {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if !crate::run::ending() {
        println!("leaving the room");
    }
    let last = session.leave();
    show_changes(shown.as_ref(), &last);
    Ok(match last.phase {
        Phase::Failed(_) => ExitCode::FAILURE,
        _ => ExitCode::SUCCESS,
    })
}

fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn show_changes(before: Option<&SessionState>, now: &SessionState) {
    let changed = |f: &dyn Fn(&SessionState) -> String| before.is_none_or(|b| f(b) != f(now));
    if changed(&|s| s.phase.to_string()) {
        println!("{}", now.phase);
    }
    if let Some(room) = &now.room
        && changed(&|s| format!("{:?}", s.room.as_ref().map(|r| (&r.code, &r.name, r.public))))
    {
        let listed = if room.public { "listed" } else { "private" };
        println!(
            "room {} \"{}\" ({listed}, {}, up to {} players)",
            room.code, room.name, room.category, room.max_members
        );
    }
    if changed(&|s| format!("{:?}{:?}{:?}", s.self_member, s.base, s.relay)) {
        if let (Some(me), Some(base)) = (now.self_member, now.base) {
            println!(
                "this player is member {me} (10.77.0.{me}); bridge base {base}; relay {}",
                now.relay.as_deref().unwrap_or("none")
            );
        } else if let Some(relay) = &now.relay {
            println!("relay {relay}");
        }
    }
    let line = |m: &MemberState| {
        let mut tags = Vec::new();
        if m.host {
            tags.push("host");
        }
        if m.me {
            tags.push("you");
        }
        if !m.connected {
            tags.push("offline");
        }
        let tags = if tags.is_empty() {
            String::new()
        } else {
            format!(" ({})", tags.join(", "))
        };
        format!(
            "member {} {}{tags}: {}, addons {}, relay {}",
            m.number,
            m.name,
            m.ip,
            word(&m.fingerprint),
            word(&m.relay)
        )
    };
    let lines = |s: &SessionState| s.members.iter().map(line).collect::<Vec<_>>();
    let old = before.map(lines).unwrap_or_default();
    for l in lines(now).iter().filter(|l| !old.contains(l)) {
        println!("{l}");
    }
    let numbers = |s: &SessionState| {
        s.members
            .iter()
            .map(|m| (m.number, m.name.clone()))
            .collect::<Vec<_>>()
    };
    if let Some(b) = before {
        for (n, name) in numbers(b).into_iter().filter(|m| !numbers(now).contains(m)) {
            println!("member {n} {name} left");
        }
    }
    if changed(&|s| format!("{:?}", s.mismatches)) && !now.mismatches.is_empty() {
        println!("your addons differ from the host's:");
        for m in &now.mismatches {
            println!("  {}", m.line());
        }
        for step in session::fix_plan(&now.mismatches) {
            println!("  fix: {}", serde_json::to_string(&step).unwrap_or_default());
        }
    }
    if changed(&|s| format!("{:?}", s.game.as_ref().map(|g| (g.screen, g.in_session, g.role))))
        && let Some(g) = &now.game
    {
        println!(
            "game: {:?}, {}",
            g.screen,
            if g.in_session {
                "in a LAN session"
            } else {
                "not in a LAN session"
            }
        );
    }
    if changed(&|s| format!("{:?}", s.last_error))
        && let Some(e) = &now.last_error
    {
        eprintln!("swofty: {e}");
    }
}

fn run_game(store: &Store, game: Option<PathBuf>, cmd: GameCmd) -> Result<ExitCode> {
    let dir = crate::build::game(store, game)?.dir;
    let cmd = match cmd {
        GameCmd::Status => GameCommand::Status,
        GameCmd::Configure { self_member, base } => GameCommand::Configure { self_member, base },
        GameCmd::Clear => GameCommand::Clear,
        GameCmd::Host { category } => GameCommand::Host { category },
        GameCmd::Join { ip } => GameCommand::Join { ip },
        GameCmd::Leave => GameCommand::Leave,
    };
    match mp_game::send(&dir, &cmd, mp_game::REPLY_TIMEOUT) {
        Ok(reply) => {
            println!("{}", serde_json::to_string_pretty(&reply.status)?);
            Ok(ExitCode::SUCCESS)
        }
        Err(mp_game::Error::Refused { op, why, status }) => {
            eprintln!("swofty: the game refused {op}: {why}");
            println!("{}", serde_json::to_string_pretty(&status)?);
            Ok(ExitCode::FAILURE)
        }
        Err(e) => Err(e.into()),
    }
}

fn run_bridge(args: BridgeArgs) -> Result<ExitCode> {
    let key = parse_key(&args.key_hex)?;
    let transport = DirectUdp::bind(&args.listen).with_context(|| format!("binding {}", args.listen))?;
    let transport = Arc::new(transport);
    let mut peers = HashMap::new();
    for spec in &args.peers {
        let (member, addr) = parse_peer(spec, transport.local_addr())?;
        if member == args.lobby.self_member || !args.lobby.members.contains(&member) {
            bail!("--peer {spec}: member {member} is not another lobby member");
        }
        peers.insert(member, addr);
    }
    transport.set_peers(&peers)?;
    let bridge = Bridge::start(args.lobby.config(key), transport.clone()).context("starting the bridge")?;
    println!("listening on {}", transport.local_addr());
    announce(&bridge);
    for m in bridge.members() {
        match peers.get(&m) {
            Some(addr) => println!("member {m}: relayed to {addr}"),
            None => eprintln!("swofty: member {m} has no --peer; what the game sends it is dropped"),
        }
    }
    crate::run::end_on_ctrl_c();
    let interval = Duration::from_secs(args.interval.max(1));
    let started = Instant::now();
    let mut next = started + interval;
    let mut game_port = None;
    let mut hinted = false;
    while !crate::run::ending() {
        std::thread::sleep(Duration::from_millis(100));
        sync_game_port(&bridge, args.lobby.game.as_deref(), &mut game_port);
        hint_foreign(&bridge, &mut hinted);
        if Instant::now() >= next {
            print_stats(
                &bridge.stats(),
                started.elapsed(),
                Some(transport.unknown_dropped()),
            );
            next += interval;
        }
    }
    let stats = bridge.stats();
    bridge.stop();
    print_stats(&stats, started.elapsed(), Some(transport.unknown_dropped()));
    Ok(ExitCode::SUCCESS)
}

fn run_echo(args: EchoArgs) -> Result<ExitCode> {
    let mut injects = Vec::new();
    for spec in &args.inject {
        let (member, path) = spec
            .split_once(':')
            .and_then(|(m, p)| Some((m.parse::<u8>().ok()?, PathBuf::from(p))))
            .ok_or_else(|| anyhow!("--inject-file {spec}: expected M:PATH"))?;
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        injects.push((member, bytes));
    }
    let bridge = Bridge::start_echo(args.lobby.config([0; 32])).context("starting the bridge")?;
    if let Some((m, _)) = injects.iter().find(|(m, _)| bridge.port(*m).is_none()) {
        bail!("--inject-file: member {m} is not another lobby member");
    }
    if let Some(dir) = &args.save {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    announce(&bridge);
    for m in bridge.members() {
        println!("member {m}: kept, not relayed");
    }
    crate::run::end_on_ctrl_c();
    let enter = (!injects.is_empty()).then(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                if line.is_err() || tx.send(()).is_err() {
                    break;
                }
            }
        });
        rx
    });
    let every = args.every.map(|s| Duration::from_secs(s.max(1)));
    let started = SystemTime::now();
    let clock = Instant::now();
    let mut game_port = None;
    let mut printed = 0;
    let mut last_inject: Option<Instant> = None;
    while !crate::run::ending() {
        std::thread::sleep(Duration::from_millis(100));
        sync_game_port(&bridge, args.lobby.game.as_deref(), &mut game_port);
        let fresh: Vec<Echoed> = bridge.echoed().into_iter().filter(|e| e.seq >= printed).collect();
        for e in fresh {
            if let Some(dir) = &args.save {
                let path = dir.join(format!("{:06}-to-{}.bin", e.seq, e.member));
                std::fs::write(&path, &e.payload).with_context(|| format!("writing {}", path.display()))?;
            }
            print_echoed(&e, started);
            printed = e.seq + 1;
        }
        let pressed = enter.as_ref().is_some_and(|rx| rx.try_recv().is_ok());
        let ready = args.lobby.game.is_none() || game_port.is_some();
        let due = match last_inject {
            None => ready,
            Some(at) => every.is_some_and(|d| at.elapsed() >= d),
        };
        if !injects.is_empty() && (pressed || due) {
            for (m, bytes) in &injects {
                match bridge.inject(*m, bytes) {
                    Ok(()) => println!(
                        "injected {} bytes as member {m}: 127.0.0.1:{} -> 127.0.0.1:{}",
                        bytes.len(),
                        bridge.port(*m).unwrap_or_default(),
                        bridge.delivery_port()
                    ),
                    Err(e) => eprintln!("swofty: injecting as member {m}: {e}"),
                }
            }
            last_inject = Some(Instant::now());
        }
    }
    let stats = bridge.stats();
    bridge.stop();
    print_stats(&stats, clock.elapsed(), None);
    Ok(ExitCode::SUCCESS)
}

fn announce(bridge: &Bridge) {
    println!("BASE={}", bridge.base());
    println!("{}={}", bridge::ENV, bridge.env_value());
    println!("game port 127.0.0.1:{}", bridge.game_port());
    for m in bridge.members() {
        println!(
            "member {m}: the game sends to 127.0.0.1:{}",
            bridge.port(m).unwrap_or_default()
        );
    }
}

fn sync_game_port(bridge: &Bridge, game: Option<&Path>, last: &mut Option<u16>) {
    let Some(port) = game.and_then(|g| bridge.sync_game_port(g)) else {
        return;
    };
    if *last != Some(port) {
        println!("the core reports game port {port} (swofty/netplay.json)");
        *last = Some(port);
    }
}

fn hint_foreign(bridge: &Bridge, hinted: &mut bool) {
    if *hinted {
        return;
    }
    let Some(from) = bridge.stats().foreign_from else {
        return;
    };
    *hinted = true;
    eprintln!(
        "swofty: dropping datagrams from {from}, which isn't the game's socket 127.0.0.1:{}; if it is \
         the game, rerun with --game-port {} or --game <game folder>",
        bridge.game_port(),
        from.port()
    );
}

fn print_echoed(e: &Echoed, started: SystemTime) {
    let at = e.at.duration_since(started).unwrap_or_default().as_secs_f64();
    let shown = e.payload.len().min(32);
    let hex: String = e.payload[..shown].iter().map(|b| format!("{b:02x}")).collect();
    let more = if e.payload.len() > shown { "..." } else { "" };
    let foreign = if e.foreign { " (not the game's socket)" } else { "" };
    println!(
        "[+{at:.3}s] to member {} from {}{foreign}, {} bytes: {hex}{more}",
        e.member,
        e.from,
        e.payload.len()
    );
}

fn print_stats(stats: &Stats, elapsed: Duration, unknown: Option<u64>) {
    let at = elapsed.as_secs();
    for (m, s) in &stats.members {
        println!(
            "[{at}s] member {m}: out {} ({} B), in {} ({} B)",
            s.packets_out, s.bytes_out, s.packets_in, s.bytes_in
        );
    }
    let mut drops: Vec<String> = stats
        .drops
        .nonzero()
        .into_iter()
        .map(|(reason, n)| format!("{reason} {n}"))
        .collect();
    if let Some(n) = unknown.filter(|n| *n > 0) {
        drops.push(format!("unknown_address {n}"));
    }
    let drops = if drops.is_empty() {
        "none".into()
    } else {
        drops.join(", ")
    };
    println!(
        "[{at}s] drops: {drops}; over {} bytes: {}",
        bridge::LARGE_PAYLOAD,
        stats.large
    );
}

fn parse_key(hex: &str) -> Result<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.is_ascii() {
        bail!("--key-hex: expected 64 hex digits");
    }
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| anyhow!("--key-hex: expected 64 hex digits"))?;
    }
    Ok(key)
}

fn parse_peer(spec: &str, local: SocketAddr) -> Result<(u8, SocketAddr)> {
    let (member, host) = spec
        .split_once('=')
        .ok_or_else(|| anyhow!("--peer {spec}: expected N=HOST:PORT"))?;
    let member: u8 = member
        .parse()
        .map_err(|_| anyhow!("--peer {spec}: {member} is not a lobby number"))?;
    let addrs: Vec<SocketAddr> = host
        .to_socket_addrs()
        .with_context(|| format!("--peer {spec}"))?
        .collect();
    let addr = addrs
        .iter()
        .find(|a| a.is_ipv4() == local.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| anyhow!("--peer {spec}: {host} has no address"))?;
    Ok((member, addr))
}

fn ice_server(path: &Path) -> Result<(Vec<String>, String, String)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    let servers = json["iceServers"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    for server in servers {
        let urls: Vec<String> = match &server["urls"] {
            serde_json::Value::String(u) => vec![u.clone()],
            serde_json::Value::Array(a) => a.iter().filter_map(|u| u.as_str().map(String::from)).collect(),
            _ => continue,
        };
        if let (Some(user), Some(credential)) = (server["username"].as_str(), server["credential"].as_str())
            && urls.iter().any(|u| u.starts_with("turn:"))
        {
            return Ok((urls, user.to_owned(), credential.to_owned()));
        }
    }
    bail!(
        "{}: no iceServers entry with turn: URLs and credentials",
        path.display()
    )
}

fn parse_probe_peer(spec: &str) -> Result<(u8, SocketAddr)> {
    let (n, addr) = spec
        .split_once('=')
        .with_context(|| format!("{spec:?}: expected N=IP:PORT"))?;
    Ok((
        n.trim()
            .parse()
            .with_context(|| format!("{spec:?}: bad member number"))?,
        addr.trim()
            .parse()
            .with_context(|| format!("{spec:?}: bad address"))?,
    ))
}

fn turn_probe(
    ice_json: &Path,
    self_member: Option<u8>,
    specs: &[String],
    count: usize,
    wait: Duration,
) -> Result<ExitCode> {
    let (urls, user, credential) = ice_server(ice_json)?;
    let turn = TurnTransport::connect(&urls, &user, &credential).context("allocating a TURN relay")?;
    println!("server  {}", turn.server_addr());
    if let Some(mapped) = turn.mapped_addr() {
        println!("mapped  {mapped}");
    }
    println!("relayed {}", turn.local_addr());

    let mut peers: HashMap<u8, SocketAddr> =
        specs.iter().map(|s| parse_probe_peer(s)).collect::<Result<_>>()?;
    if peers.is_empty() {
        if std::io::stdin().is_terminal() {
            eprintln!("enter peers as N=IP:PORT, one per line, then an empty line:");
        }
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                break;
            }
            let (n, addr) = parse_probe_peer(&line)?;
            peers.insert(n, addr);
        }
    }
    if let Some(me) = self_member
        && peers.remove(&me).is_some()
    {
        eprintln!("ignoring member {me}: that's this probe");
    }
    if peers.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    turn.set_peers(&peers).context("creating permissions")?;

    let start = Instant::now();
    let mut rtts: HashMap<u8, Vec<Duration>> = peers.keys().map(|m| (*m, Vec::new())).collect();
    let mut buf = [0u8; MAX_DATAGRAM];
    let (mut seq, mut next_ping, mut done) = (0u64, start, None::<Instant>);
    while start.elapsed() < wait && done.is_none_or(|d| d.elapsed() < Duration::from_secs(3)) {
        if done.is_none() && Instant::now() >= next_ping {
            for (m, got) in &rtts {
                if got.len() < count {
                    let stamp = start.elapsed().as_micros();
                    let _ = turn.send(*m, format!("ping {seq} {stamp}").as_bytes());
                }
            }
            seq += 1;
            next_ping = Instant::now() + Duration::from_millis(500);
        }
        if let Some((from, n)) = turn.recv(&mut buf, Duration::from_millis(50))? {
            let text = String::from_utf8_lossy(&buf[..n]).into_owned();
            let mut words = text.split(' ');
            match (
                words.next(),
                words.next(),
                words.next().and_then(|w| w.parse::<u64>().ok()),
            ) {
                (Some("ping"), Some(s), Some(stamp)) => {
                    let _ = turn.send(from, format!("pong {s} {stamp}").as_bytes());
                }
                (Some("pong"), Some(s), Some(stamp)) => {
                    let rtt = start.elapsed().saturating_sub(Duration::from_micros(stamp));
                    println!("member {from} seq {s} rtt {:.1} ms", rtt.as_secs_f64() * 1000.0);
                    if let Some(got) = rtts.get_mut(&from) {
                        got.push(rtt);
                    }
                }
                _ => println!("member {from}: {n} bytes"),
            }
        }
        if done.is_none() && rtts.values().all(|got| got.len() >= count) {
            done = Some(Instant::now());
        }
    }

    let stats = turn.stats();
    println!(
        "sent {} as ChannelData, {} as Send indications; {} channel binds, {} permission rounds",
        stats.channel_sends, stats.indication_sends, stats.bindings, stats.permissions
    );
    let mut ok = true;
    let mut members: Vec<_> = rtts.iter().collect();
    members.sort_by_key(|(m, _)| **m);
    for (m, got) in members {
        if got.is_empty() {
            ok = false;
            println!("member {m}: no answers");
            continue;
        }
        let ms = |d: &Duration| d.as_secs_f64() * 1000.0;
        let min = got.iter().map(ms).fold(f64::INFINITY, f64::min);
        let max = got.iter().map(ms).fold(0.0, f64::max);
        let avg = got.iter().map(ms).sum::<f64>() / got.len() as f64;
        println!(
            "member {m}: {} answers, rtt min {min:.1} avg {avg:.1} max {max:.1} ms",
            got.len()
        );
    }
    Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(1) })
}

fn read(path: &Path) -> Result<Fingerprint> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let f: Fingerprint = serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    if f.version != fingerprint::VERSION {
        bail!(
            "{}: fingerprint version {} is not {}",
            path.display(),
            f.version,
            fingerprint::VERSION
        );
    }
    Ok(f)
}
