use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use swofty_addon::asset::{AssetBatch, Op, Origin};
use swofty_addon::helper::{
    API_VERSION, Built, Call, Hello, HostCall, Incoming, RESTART, Reply, Request, Welcome,
};
use swofty_addon::launcher::{
    Addon, BuildCx, Claim, CommandSpec, Cx, Importer, ProfileInfo, Progress, Summary,
};
use swofty_addon::manifest::{AddonManifest, Launcher};
use swofty_addon::view::{Body, Button, Effect, Event, Notice, SectionSpec, Style, Tone, View};

type Result<T> = swofty_addon::Result<T>;

pub const LOG_FILE: &str = "helper.log";

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub quick: Duration,
    pub long: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            quick: Duration::from_secs(5),
            long: Duration::from_secs(300),
        }
    }
}

pub struct HelperAddon {
    manifest: AddonManifest,
    spec: Launcher,
    dir: PathBuf,
    timeouts: Timeouts,
    run: Mutex<Run>,
}

enum Run {
    Idle,
    Live(Live),
    Dead(String),
}

struct Live {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next: u64,
    profile: ProfileInfo,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

impl HelperAddon {
    pub fn new(manifest: AddonManifest, dir: impl Into<PathBuf>) -> Option<Self> {
        let spec = manifest.launcher.clone()?;
        Some(Self {
            manifest,
            spec,
            dir: dir.into(),
            timeouts: Timeouts::default(),
            run: Mutex::new(Run::Idle),
        })
    }

    pub fn with_timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn running(&self) -> bool {
        matches!(*self.lock(), Run::Live(_))
    }

    pub fn failure(&self) -> Option<String> {
        match &*self.lock() {
            Run::Dead(e) => Some(e.clone()),
            _ => None,
        }
    }

    pub fn stop(&self) {
        *self.lock() = Run::Idle;
    }

    fn lock(&self) -> MutexGuard<'_, Run> {
        self.run.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn call<T: DeserializeOwned>(
        &self,
        cx: &Cx,
        call: Call,
        wait: Duration,
        progress: Option<&dyn Progress>,
    ) -> Result<T> {
        self.call_with(self.lock(), cx, call, wait, progress)
    }

    fn call_with<T: DeserializeOwned>(
        &self,
        mut run: MutexGuard<'_, Run>,
        cx: &Cx,
        call: Call,
        wait: Duration,
        progress: Option<&dyn Progress>,
    ) -> Result<T> {
        if let Run::Dead(e) = &*run {
            return Err(swofty_addon::Error::Other(e.clone()));
        }
        if matches!(*run, Run::Idle) {
            *run = Run::Live(self.start(cx).map_err(|e| self.fail(&mut run, e))?);
        }
        let Run::Live(live) = &mut *run else {
            unreachable!("started above");
        };
        let exchange = self.exchange(live, cx, call, wait, progress);
        match exchange {
            Ok(Ok(value)) => serde_json::from_value(value)
                .map_err(|e| self.fail(&mut run, format!("sent a bad result: {e}"))),
            Ok(Err(e)) => Err(swofty_addon::Error::Other(format!("{}: {e}", self.name()))),
            Err(e) => Err(self.fail(&mut run, e)),
        }
    }

    fn fail(&self, run: &mut Run, e: String) -> swofty_addon::Error {
        let e = format!("{} {e}", self.name());
        *run = Run::Dead(e.clone());
        swofty_addon::Error::Other(e)
    }

    fn start(&self, cx: &Cx) -> std::result::Result<Live, String> {
        let rel = self
            .spec
            .program()
            .ok_or_else(|| format!("has no helper for {}", std::env::consts::OS))?;
        let program = self.dir.join(rel);
        executable(&program).map_err(|e| format!("could not start {}: {e}", program.display()))?;
        let log = std::fs::File::create(cx.cache_dir.join(LOG_FILE)).map(Stdio::from);
        let mut command = Command::new(&program);
        command
            .current_dir(&self.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log.unwrap_or_else(|_| Stdio::null()));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", program.display()))?;
        let (stdin, stdout) = match (child.stdin.take(), child.stdout.take()) {
            (Some(i), Some(o)) => (i, o),
            _ => return Err("has no pipes".into()),
        };
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut live = Live {
            child,
            stdin,
            lines,
            next: 0,
            profile: cx.profile.clone(),
        };
        let hello = Call::Hello(Hello {
            api: API_VERSION,
            addon: cx.addon.to_owned(),
            data_dir: cx.data_dir.to_path_buf(),
            cache_dir: cx.cache_dir.to_path_buf(),
            profile: cx.profile.clone(),
        });
        let welcome = self
            .exchange(&mut live, cx, hello, self.timeouts.quick, None)?
            .map_err(|e| format!("refused hello: {e}"))?;
        let welcome: Welcome =
            serde_json::from_value(welcome).map_err(|e| format!("sent a bad hello: {e}"))?;
        if welcome.api != API_VERSION {
            return Err(format!(
                "speaks launcher API {}; this launcher speaks {API_VERSION}",
                welcome.api
            ));
        }
        Ok(live)
    }

    fn exchange(
        &self,
        live: &mut Live,
        cx: &Cx,
        call: Call,
        wait: Duration,
        progress: Option<&dyn Progress>,
    ) -> std::result::Result<std::result::Result<Value, String>, String> {
        if live.profile != *cx.profile && !matches!(call, Call::Hello(_)) {
            live.profile = cx.profile.clone();
            send(
                live,
                &Request {
                    id: None,
                    call: Call::Profile(cx.profile.clone()),
                },
            )?;
        }
        let id = live.next;
        live.next += 1;
        send(live, &Request { id: Some(id), call })?;
        loop {
            let line = match live.lines.recv_timeout(wait) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("did not answer within {} s", wait.as_secs_f32()));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let status = live
                        .child
                        .wait()
                        .map_or_else(|e| e.to_string(), |s| s.to_string());
                    return Err(format!("stopped ({status})"));
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let message: Incoming =
                serde_json::from_str(&line).map_err(|e| format!("sent a bad message: {e}"))?;
            match message {
                Incoming::Reply(Reply::Ok { id: got, result }) if got == id => return Ok(Ok(result)),
                Incoming::Reply(Reply::Err { id: got, error }) if got == id => return Ok(Err(error)),
                Incoming::Reply(_) => return Err("answered a request it was not sent".into()),
                Incoming::Call(Request {
                    id: None,
                    call: HostCall::Progress { done, text },
                }) => {
                    if let Some(p) = progress {
                        p.report(done, &text);
                    }
                }
                Incoming::Call(Request { id: Some(host), call }) => {
                    let reply = match host_call(cx, call) {
                        Ok(result) => Reply::Ok { id: host, result },
                        Err(error) => Reply::Err { id: host, error },
                    };
                    send(live, &reply)?;
                }
                Incoming::Call(Request { id: None, .. }) => {
                    return Err("sent a host call without an id".into());
                }
            }
        }
    }

    fn dead_view(&self, error: String) -> View {
        let mut restart = Button::new(RESTART, "Restart");
        restart.style = Style::Primary;
        View {
            subtitle: String::new(),
            actions: Vec::new(),
            notices: vec![Notice {
                text: error,
                tone: Tone::Bad,
                action: Some(restart),
            }],
            body: Body::Empty {
                title: format!("{} is not running", self.name()),
                text: "Restart it to load this page.".into(),
                action: None,
            },
            footer: None,
        }
    }
}

fn host_call(cx: &Cx, call: HostCall) -> std::result::Result<Value, String> {
    match call {
        HostCall::GetState => cx.state::<Option<Value>>().map(|v| v.unwrap_or(Value::Null)),
        HostCall::SetState(value) => cx.set_state(&value).map(|()| Value::Null),
        HostCall::Progress { .. } => Err(swofty_addon::Error::Other("progress takes no id".into())),
    }
    .map_err(|e| e.to_string())
}

fn send(live: &mut Live, message: &impl Serialize) -> std::result::Result<(), String> {
    let mut line = serde_json::to_vec(message).map_err(|e| e.to_string())?;
    line.push(b'\n');
    live.stdin
        .write_all(&line)
        .and_then(|()| live.stdin.flush())
        .map_err(|e| format!("stopped reading ({e})"))
}

#[cfg(unix)]
fn executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    if perms.mode() & 0o111 == 0 {
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn executable(path: &Path) -> std::io::Result<()> {
    std::fs::metadata(path).map(|_| ())
}

impl Addon for HelperAddon {
    fn manifest(&self) -> &AddonManifest {
        &self.manifest
    }

    fn sections(&self) -> Vec<SectionSpec> {
        self.spec.sections.clone()
    }

    fn view(&self, cx: &Cx, section: &str) -> Result<View> {
        if let Some(e) = self.failure() {
            return Ok(self.dead_view(e));
        }
        let call = Call::View {
            section: section.into(),
        };
        self.call(cx, call, self.timeouts.quick, None)
            .or_else(|e| self.failure().map(|e| self.dead_view(e)).ok_or(e))
    }

    fn event(&self, cx: &Cx, section: &str, event: Event) -> Result<Effect> {
        if matches!(&event, Event::Click { id } if id == RESTART) {
            self.stop();
            return Ok(Effect::Reload);
        }
        let call = Call::Event {
            section: section.into(),
            event,
        };
        self.call(cx, call, self.timeouts.long, None)
    }

    fn importer(&self) -> Option<&dyn Importer> {
        (!self.spec.imports.is_empty()).then_some(self as &dyn Importer)
    }

    fn build(&self, cx: &BuildCx) -> Result<AssetBatch> {
        if !self.spec.build {
            return Ok(AssetBatch::default());
        }
        let call = Call::Build {
            options: cx.options.clone(),
            features: cx.features.clone(),
        };
        let built: Built = self.call(&cx.cx, call, self.timeouts.long, None)?;
        let origin = Origin {
            addon: self.manifest.id.clone(),
            item: None,
        };
        Ok(AssetBatch {
            ops: built
                .ops
                .into_iter()
                .map(|op| Op {
                    op,
                    origin: origin.clone(),
                })
                .collect(),
            lines: built.lines,
            warnings: built.warnings,
        })
    }

    fn commands(&self) -> Vec<CommandSpec> {
        self.spec.commands.clone()
    }

    fn command(&self, cx: &Cx, path: &[String], args: &[String]) -> Result<String> {
        let call = Call::Command {
            path: path.to_vec(),
            args: args.to_vec(),
        };
        self.call(cx, call, self.timeouts.long, None)
    }

    fn summary(&self, cx: &Cx) -> Option<Summary> {
        let run = match self.run.try_lock() {
            Ok(run) => run,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        if !matches!(*run, Run::Live(_)) {
            return None;
        }
        self.call_with(run, cx, Call::Summary, self.timeouts.quick, None)
            .ok()
            .flatten()
    }
}

impl Importer for HelperAddon {
    fn claims(&self, path: &Path) -> Claim {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        if path.is_file() && self.spec.imports.iter().any(|i| i.eq_ignore_ascii_case(ext)) {
            Claim::Yes
        } else {
            Claim::No
        }
    }

    fn import(&self, cx: &Cx, paths: &[PathBuf], progress: &dyn Progress) -> Result<Vec<String>> {
        let call = Call::Import {
            paths: paths.to_vec(),
        };
        self.call(cx, call, self.timeouts.long, Some(progress))
    }
}

#[derive(Default)]
pub struct Helpers(Mutex<Vec<Arc<HelperAddon>>>);

impl std::fmt::Debug for Helpers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Helpers")
    }
}

impl Helpers {
    pub fn get(&self, manifest: &AddonManifest, dir: &Path) -> Option<Arc<HelperAddon>> {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = all.iter().find(|h| h.manifest == *manifest && h.dir == dir) {
            return Some(h.clone());
        }
        let helper = Arc::new(HelperAddon::new(manifest.clone(), dir)?);
        all.retain(|h| h.manifest.id != manifest.id);
        all.push(helper.clone());
        Some(helper)
    }

    pub fn forget(&self, id: &str) {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for h in all.iter().filter(|h| h.manifest.id == id) {
            h.stop();
        }
        all.retain(|h| h.manifest.id != id);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use swofty_addon::asset::{AssetOp, Features};
    use swofty_addon::launcher::StateStore;

    use super::*;
    use crate::import::{Route, route};
    use crate::store::Store;

    const MANIFEST: &str = r#"{"schema_version":1,"id":"t.helper","name":"Helper","version":"1.0.0","author":"t",
        "compat":{"game":"f1_25"},
        "launcher":{"program":{"linux":"bin/helper","macos":"bin/helper"},
            "sections":[{"id":"items","title":"Items","icon":"package"}],"imports":["exmod"],"build":true}}"#;

    #[derive(Default)]
    struct Memory(Mutex<BTreeMap<String, Value>>);

    impl StateStore for Memory {
        fn get(&self, addon: &str) -> Result<Option<Value>> {
            Ok(self.0.lock().unwrap().get(addon).cloned())
        }

        fn set(&self, addon: &str, value: Value) -> Result<()> {
            self.0.lock().unwrap().insert(addon.into(), value);
            Ok(())
        }
    }

    #[derive(Default)]
    struct Seen(Mutex<Vec<(f32, String)>>);

    impl Progress for Seen {
        fn report(&self, done: f32, text: &str) {
            self.0.lock().unwrap().push((done, text.into()));
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-helper-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    fn addon(dir: &Path, script: &str) {
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("swofty.json"), MANIFEST).unwrap();
        std::fs::write(dir.join("bin/helper"), format!("#!/bin/sh\n{script}")).unwrap();
    }

    fn helper(dir: &Path) -> HelperAddon {
        let manifest = AddonManifest::load(dir).unwrap();
        HelperAddon::new(manifest, dir).unwrap().with_timeouts(Timeouts {
            quick: Duration::from_millis(500),
            long: Duration::from_millis(500),
        })
    }

    fn profile(name: &str) -> ProfileInfo {
        ProfileInfo {
            name: name.into(),
            vanilla: false,
            enabled: true,
        }
    }

    const ROUND_TRIP: &str = r#"
answer() { read -r l; echo "$l" >> received; echo "$1"; }
answer '{"id":0,"result":{"api":1}}'
answer '{"id":9,"method":"get_state"}'
answer '{"id":10,"method":"set_state","params":["b"]}'
answer '{"id":1,"result":{"subtitle":"s","body":{"blocks":[]}}}'
answer '{"method":"progress","params":{"done":0.5,"text":"half"}}'
echo '{"id":2,"result":["imported a.exmod"]}'
read -r l; echo "$l" >> received
answer '{"id":3,"result":{"ops":[{"op":"file.put","source":"a.dds","target":"b.dds"}],"lines":["one"]}}'
answer '{"id":4,"error":"nope"}'
read -r l
"#;

    #[test]
    fn talks_json_lines_and_answers_host_calls() {
        let root = tmp("round-trip");
        let dir = root.join("addon");
        addon(&dir, ROUND_TRIP);
        let h = helper(&dir);
        let state = Memory::default();
        state.set("t.helper", serde_json::json!(["a"])).unwrap();
        let (data, cache) = (root.join("data"), root.join("cache"));
        std::fs::create_dir_all(&cache).unwrap();
        let first = profile("default");
        let cx = Cx::new("t.helper", &first, &data, &cache, None, &state);
        assert!(h.summary(&cx).is_none() && !h.running());
        let view = h.view(&cx, "items").unwrap();
        assert_eq!(view.subtitle, "s");
        assert!(h.running());
        assert_eq!(state.get("t.helper").unwrap(), Some(serde_json::json!(["b"])));
        let file = root.join("a.exmod");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(h.importer().unwrap().claims(&file), Claim::Yes);
        assert_eq!(h.claims(&root.join("a.txt")), Claim::No);
        let seen = Seen::default();
        let lines = h.import(&cx, std::slice::from_ref(&file), &seen).unwrap();
        assert_eq!(lines, ["imported a.exmod"]);
        assert_eq!(*seen.0.lock().unwrap(), [(0.5, "half".to_owned())]);
        let second = profile("other");
        let cx = Cx::new("t.helper", &second, &data, &cache, None, &state);
        let options = BTreeMap::new();
        let batch = h
            .build(&BuildCx {
                cx: Cx::new("t.helper", &second, &data, &cache, None, &state),
                options: &options,
                features: &Features::default(),
            })
            .unwrap();
        assert!(matches!(batch.ops[0].op, AssetOp::File(_)));
        assert_eq!(batch.ops[0].origin.addon, "t.helper");
        assert_eq!(batch.lines, ["one"]);
        let e = h.command(&cx, &["x".into()], &[]).unwrap_err();
        assert_eq!(e.to_string(), "Helper: nope");
        assert!(h.running());
        let received = std::fs::read_to_string(dir.join("received")).unwrap();
        let received: Vec<Value> = received
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let methods: Vec<&str> = received
            .iter()
            .map(|m| m["method"].as_str().unwrap_or("reply"))
            .collect();
        assert_eq!(
            methods,
            [
                "hello", "view", "reply", "reply", "import", "profile", "build", "command"
            ]
        );
        assert_eq!(received[0]["params"]["api"], API_VERSION);
        assert_eq!(received[0]["params"]["addon"], "t.helper");
        assert_eq!(received[2], serde_json::json!({"id": 9, "result": ["a"]}));
        assert_eq!(received[5]["params"]["name"], "other");
        h.stop();
        assert!(!h.running());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn contains_hangs_crashes_and_bad_helpers() {
        let root = tmp("failures");
        let hello = r#"read -r l; echo '{"id":0,"result":{"api":1}}'"#;
        let cases = [
            (
                "hang",
                format!("{hello}\nread -r l; exec sleep 5"),
                "did not answer within 0.5 s",
            ),
            (
                "crash",
                format!("{hello}\nread -r l; exit 3"),
                "stopped (exit status: 3)",
            ),
            (
                "junk",
                format!("{hello}\nread -r l; echo hello there; read -r l"),
                "sent a bad message",
            ),
            (
                "shape",
                format!("{hello}\nread -r l; echo '{{\"id\":1,\"result\":7}}'; read -r l"),
                "sent a bad result",
            ),
            (
                "old",
                r#"read -r l; echo '{"id":0,"result":{"api":0}}'; read -r l"#.into(),
                "speaks launcher API 0",
            ),
        ];
        for (name, script, _) in &cases {
            addon(&root.join(name), script);
        }
        let state = Memory::default();
        let p = profile("default");
        let cache = root.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let cx = Cx::new("t.helper", &p, &root, &cache, None, &state);
        for (name, _, error) in cases {
            let h = helper(&root.join(name));
            let view = h.view(&cx, "items").unwrap();
            let notice = &view.notices[0];
            assert!(
                notice.text.starts_with(&format!("Helper {error}")),
                "{name}: {}",
                notice.text
            );
            assert_eq!(notice.action.as_ref().unwrap().id, RESTART);
            assert!(matches!(view.body, Body::Empty { .. }));
            assert!(!h.running() && h.failure().is_some());
            let click = Event::Click { id: "x".into() };
            assert!(h.event(&cx, "items", click).is_err(), "{name}");
            let restart = Event::Click { id: RESTART.into() };
            assert_eq!(h.event(&cx, "items", restart).unwrap(), Effect::Reload);
            assert!(h.failure().is_none());
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn routes_drops_to_a_helper_without_starting_it() {
        let root = tmp("routes");
        let store = Store::new(root.join("data"));
        addon(&store.addons_dir().join("t.helper"), "exit 1");
        std::fs::write(root.join("car.exmod"), "x").unwrap();
        std::fs::write(root.join("car.txt"), "x").unwrap();
        let reg = store.registry().unwrap();
        let entry = reg.get("t.helper").unwrap();
        assert!(entry.helper && !entry.declared);
        assert_eq!(entry.addon.sections()[0].id, "items");
        assert_eq!(
            route(&reg, &root.join("car.exmod")),
            Some(Route::Importer("t.helper".into()))
        );
        assert_eq!(route(&reg, &root.join("car.txt")), None);
        let again = store.registry().unwrap();
        assert!(std::ptr::addr_eq(
            Arc::as_ptr(&entry.addon),
            Arc::as_ptr(&again.get("t.helper").unwrap().addon)
        ));
        assert!(
            !store
                .helpers()
                .get(
                    &entry.addon.manifest().clone(),
                    &store.addons_dir().join("t.helper")
                )
                .unwrap()
                .running()
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
