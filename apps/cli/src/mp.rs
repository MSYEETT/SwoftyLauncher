use std::collections::HashMap;
use std::io::{BufRead, IsTerminal};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use swofty_core::catalog;
use swofty_core::multiplayer::fingerprint::{self, Fingerprint};
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
        MpCommand::TurnProbe {
            ice_json,
            self_member,
            peer,
            count,
            wait,
        } => turn_probe(&ice_json, self_member, &peer, count, Duration::from_secs(wait)),
    }
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

fn parse_peer(spec: &str) -> Result<(u8, SocketAddr)> {
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

    let mut peers: HashMap<u8, SocketAddr> = specs.iter().map(|s| parse_peer(s)).collect::<Result<_>>()?;
    if peers.is_empty() {
        if std::io::stdin().is_terminal() {
            eprintln!("enter peers as N=IP:PORT, one per line, then an empty line:");
        }
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                break;
            }
            let (n, addr) = parse_peer(&line)?;
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
