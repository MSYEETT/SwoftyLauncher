// The client side of the helper protocol: the mirror of core's HelperAddon. A
// helper binary hands its Addon to run_helper, which reads Request<Call> lines
// from stdin, drives the addon, and answers its StateStore, GameFiles and
// Progress through host calls back up stdout. Usage is single-threaded, so the
// shared stdin/stdout handles and request-id counter live behind one Mutex that
// a host call holds across its write-then-read exchange.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::Value;

use crate::helper::{API_VERSION, Built, Call, HostCall, Invalidate, Reply, Request, Welcome};
use crate::launcher::{Addon, BuildCx, Claim, Cx, GameFiles, GameStatus, ProfileInfo, Progress, StateStore};
use crate::{Error, Result};

pub fn run_helper(addon: Arc<dyn Addon>) {
    if let [verb, path] = std::env::args().skip(1).collect::<Vec<_>>().as_slice()
        && verb == "claims"
    {
        let claim = addon
            .importer()
            .map_or(Claim::No, |i| i.claims(std::path::Path::new(path)));
        println!("{}", claim_word(claim));
        return;
    }
    let host = Host::new();
    let mut session: Option<Session> = None;
    while let Some(Request { id, call }) = host.next_call() {
        if let Call::Profile(profile) = call {
            if let Some(session) = session.as_mut() {
                session.profile = profile;
            }
            continue;
        }
        let Some(id) = id else {
            continue;
        };
        let result = dispatch(addon.as_ref(), &host, &mut session, call);
        host.reply(id, result);
    }
}

struct Session {
    addon: String,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    profile: ProfileInfo,
}

fn dispatch(
    addon: &dyn Addon,
    host: &Host,
    session: &mut Option<Session>,
    call: Call,
) -> std::result::Result<Value, String> {
    if let Call::Hello(hello) = call {
        *session = Some(Session {
            addon: hello.addon,
            data_dir: hello.data_dir,
            cache_dir: hello.cache_dir,
            profile: hello.profile,
        });
        return json(Welcome { api: API_VERSION });
    }
    let session = session
        .as_ref()
        .ok_or_else(|| "the host spoke before saying hello".to_string())?;
    // The host answers game calls with errors when it has no game at hand, such as
    // a command run without one, so the addon sees no game then, as a built-in would.
    let game = (!host.build_key().is_empty()).then_some(host as &dyn GameFiles);
    let cx = Cx::new(
        &session.addon,
        &session.profile,
        &session.data_dir,
        &session.cache_dir,
        game,
        host as &dyn StateStore,
    );
    match call {
        Call::View { section } => addon.view(&cx, &section).map_err(text).and_then(json),
        Call::Event { section, event } => addon.event(&cx, &section, event).map_err(text).and_then(json),
        Call::Import { paths } => addon
            .importer()
            .ok_or_else(|| "this addon imports nothing".to_string())?
            .import(&cx, &paths, host as &dyn Progress)
            .map_err(text)
            .and_then(json),
        Call::Build { options, features } => {
            let batch = addon
                .build(&BuildCx {
                    cx,
                    options: &options,
                    features: &features,
                })
                .map_err(text)?;
            let items = match batch.ops.iter().any(|op| op.origin.item.is_some()) {
                true => batch.ops.iter().map(|op| op.origin.item.clone()).collect(),
                false => Vec::new(),
            };
            json(Built {
                ops: batch.ops.into_iter().map(|op| op.op).collect(),
                items,
                lines: batch.lines,
                warnings: batch.warnings,
            })
        }
        Call::Command { path, args } => addon.command(&cx, &path, &args).map_err(text).and_then(json),
        Call::Summary => json(addon.summary(&cx)),
        Call::Tick { section } => {
            if addon.tick(&cx, &section).map_err(text)? {
                host.invalidate(Some(section));
            }
            Ok(Value::Null)
        }
        Call::Hello(_) | Call::Profile(_) => unreachable!("handled above"),
    }
}

// What `<program> claims <path>` prints; core's HelperAddon reads it back.
pub fn claim_word(claim: Claim) -> &'static str {
    match claim {
        Claim::Yes => "yes",
        Claim::Maybe => "maybe",
        Claim::No => "no",
    }
}

struct Io {
    stdin: std::io::Stdin,
    stdout: std::io::Stdout,
    next: u64,
}

impl Io {
    fn read_line(&mut self) -> Option<String> {
        let mut buf = String::new();
        match self.stdin.lock().read_line(&mut buf) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(buf),
        }
    }

    fn write(&mut self, message: &impl Serialize) {
        if let Ok(mut line) = serde_json::to_vec(message) {
            line.push(b'\n');
            let _ = self.stdout.write_all(&line);
            let _ = self.stdout.flush();
        }
    }
}

struct Host {
    io: Mutex<Io>,
}

impl Host {
    fn new() -> Self {
        Self {
            io: Mutex::new(Io {
                stdin: std::io::stdin(),
                stdout: std::io::stdout(),
                next: 0,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Io> {
        self.io.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn next_call(&self) -> Option<Request<Call>> {
        let mut io = self.lock();
        loop {
            let line = io.read_line()?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Request<Call>>(line) {
                Ok(request) => return Some(request),
                Err(e) => eprintln!("helper: ignoring message ({e}): {line}"),
            }
        }
    }

    fn reply(&self, id: u64, result: std::result::Result<Value, String>) {
        let reply = match result {
            Ok(result) => Reply::Ok { id, result },
            Err(error) => Reply::Err { id, error },
        };
        self.lock().write(&reply);
    }

    fn invalidate(&self, section: Option<String>) {
        self.lock().write(&Request::<HostCall> {
            id: None,
            call: HostCall::Invalidate(Invalidate { section }),
        });
    }

    fn call(&self, call: HostCall) -> std::result::Result<Value, String> {
        let mut io = self.lock();
        let id = io.next;
        io.next += 1;
        io.write(&Request { id: Some(id), call });
        loop {
            let Some(line) = io.read_line() else {
                std::process::exit(0);
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let reply: Reply =
                serde_json::from_str(line).map_err(|e| format!("the host sent a bad reply: {e}"))?;
            return match reply {
                Reply::Ok { id: got, result } if got == id => Ok(result),
                Reply::Err { id: got, error } if got == id => Err(error),
                _ => Err("the host answered a request it was not sent".into()),
            };
        }
    }
}

impl StateStore for Host {
    fn get(&self, _addon: &str) -> Result<Option<Value>> {
        match self.call(HostCall::GetState).map_err(Error::Other)? {
            Value::Null => Ok(None),
            value => Ok(Some(value)),
        }
    }

    fn set(&self, _addon: &str, value: Value) -> Result<()> {
        self.call(HostCall::SetState(value)).map_err(Error::Other)?;
        Ok(())
    }
}

impl GameFiles for Host {
    fn build_key(&self) -> String {
        match self.call(HostCall::GameBuildKey) {
            Ok(Value::String(key)) => key,
            _ => String::new(),
        }
    }

    fn exe_version(&self) -> Option<String> {
        match self.call(HostCall::GameExeVersion) {
            Ok(Value::String(version)) => Some(version),
            _ => None,
        }
    }

    fn is_stock(&self, rel: &str) -> bool {
        matches!(
            self.call(HostCall::GameIsStock { rel: rel.into() }),
            Ok(Value::Bool(true))
        )
    }

    fn original(&self, rel: &str) -> Result<PathBuf> {
        host_path(self.call(HostCall::GameOriginal { rel: rel.into() }))
    }

    fn original_lang(&self, code: &str) -> Result<PathBuf> {
        host_path(self.call(HostCall::GameOriginalLang { code: code.into() }))
    }

    fn dir(&self) -> Option<PathBuf> {
        host_path(self.call(HostCall::GameDir)).ok()
    }

    fn status(&self) -> GameStatus {
        self.call(HostCall::GameStatus)
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    fn steam_save_dir(&self) -> Option<PathBuf> {
        host_path(self.call(HostCall::SteamSaveDir)).ok()
    }
}

impl Progress for Host {
    fn report(&self, done: f32, text: &str) {
        self.lock().write(&Request::<HostCall> {
            id: None,
            call: HostCall::Progress {
                done,
                text: text.into(),
            },
        });
    }
}

fn host_path(reply: std::result::Result<Value, String>) -> Result<PathBuf> {
    match reply {
        Ok(Value::String(path)) => Ok(PathBuf::from(path)),
        Ok(Value::Null) => Err(Error::Other("the game is not available".into())),
        Ok(_) => Err(Error::Other("the host sent a bad path".into())),
        Err(e) => Err(Error::Other(e)),
    }
}

fn json<T: Serialize>(value: T) -> std::result::Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn text(error: Error) -> String {
    error.to_string()
}
