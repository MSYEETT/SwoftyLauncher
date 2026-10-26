use std::collections::HashMap;
use std::io::BufRead;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use swofty_core::catalog;
use swofty_core::multiplayer::bridge::{self, Bridge, BridgeConfig, Echoed, Stats};
use swofty_core::multiplayer::direct::DirectUdp;
use swofty_core::multiplayer::fingerprint::{self, Fingerprint};
use swofty_core::multiplayer::game::{self as mp_game, Category, Command as GameCommand};
use swofty_core::multiplayer::transport::Transport;
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
        about = "Drive the running game's LAN session: status, host from the main menu, join a host, or leave"
    )]
    Game {
        #[arg(long, help = crate::GAME_HELP)]
        game: Option<PathBuf>,
        #[command(subcommand)]
        cmd: GameCmd,
    },
}

#[derive(Subcommand)]
pub enum GameCmd {
    #[command(
        about = "Print the game's screen (main_menu, lan_lobby, other), whether it is in a LAN session, and its role"
    )]
    Status,
    #[command(about = "Host a LAN game with a car category; the game must be on its main menu")]
    Host {
        #[arg(long, default_value = "f1-2026", help = "Car category: f1-2026 or f1-2025")]
        category: Category,
    },
    #[command(about = "Join the LAN game hosted at an IPv4 address; the game must be on its main menu")]
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
        MpCommand::Game { game, cmd } => run_game(store, game, cmd),
    }
}

fn run_game(store: &Store, game: Option<PathBuf>, cmd: GameCmd) -> Result<ExitCode> {
    let dir = crate::build::game(store, game)?.dir;
    let cmd = match cmd {
        GameCmd::Status => GameCommand::Status,
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
