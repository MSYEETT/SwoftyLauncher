//! The `career` environment: the save sandbox, a fixture's saves copied into
//! it, then the test agent continues the fixture's career to the place the
//! test names and the CLI waits for the agent to say it is there.

use super::suite::Env;

/// Where a career environment starts: `at` in the test file.
pub const AT: [&str; 3] = ["team_hq", "weekend_hub", "session"];
pub const DEFAULT_AT: &str = "team_hq";

/// Why a career test is skipped until the agent can continue a career: the
/// sandbox and fixtures are in place, the title screen, the career tile and
/// the resume card are the agent's (docs/design/std-2/test-suite.md).
pub const NEEDS: &str = "needs the test agent to continue the fixture's career";

/// The agent logs this once the career is at the place asked for:
/// `[test] career ready at team_hq (slot 3)`.
pub const READY_PREFIX: &str = "[test] career ready at ";

pub fn at(env: &Env) -> &str {
    env.str("at").unwrap_or(DEFAULT_AT)
}

/// The agent's commands for the environment (docs/std/testing.md: `career
/// load <slot>` in `swofty test shell`): continue the career in `slot`, or
/// the newest one when the test names none, and go on to `at`.
pub fn agent_commands(env: &Env) -> Vec<String> {
    let slot = env.uint("slot").map_or("newest".to_owned(), |s| s.to_string());
    vec![format!("career load {slot} at {}", at(env))]
}

/// The loader.log text that says the environment is ready.
pub fn ready_line(env: &Env) -> String {
    format!("{READY_PREFIX}{}", at(env))
}
