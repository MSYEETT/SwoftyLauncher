use serde_json::{Value, json};

use driver_manager::app::App;
use driver_manager::host::Host;
use driver_manager::pages;

const API: u64 = 1;

fn main() {
    let mut host = Host::new();
    let mut app = App::new(&Value::Null);
    while let Some(line) = host.read_line() {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            eprintln!("not JSON: {line}");
            continue;
        };
        let params = &message["params"];
        let method = message["method"].as_str().unwrap_or_default();
        let Some(id) = message["id"].as_u64() else {
            if method == "profile" {
                app.vanilla = params["vanilla"].as_bool().unwrap_or(false);
                app.enabled = params["enabled"].as_bool().unwrap_or(true);
            }
            continue;
        };
        let result = match method {
            "hello" => {
                app = App::new(params);
                Ok(json!({ "api": API }))
            }
            "view" => Ok(pages::view(&mut app, &mut host)),
            "event" => pages::event(&mut app, &mut host, &params["event"]),
            "tick" => {
                if app.poll() {
                    host.invalidate(Some(pages::SECTION));
                }
                Ok(Value::Null)
            }
            "build" => Ok(pages::build(&mut app, &mut host, params)),
            "summary" => Ok(pages::summary(&app)),
            other => Err(format!("unknown method {other}")),
        };
        let reply = match result {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(error) => json!({ "id": id, "error": error }),
        };
        host.send(&reply);
    }
}
