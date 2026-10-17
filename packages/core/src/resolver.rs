use std::collections::{BTreeMap, BTreeSet};

use crate::builtin::CORE_ID;
use crate::manifest::AddonManifest;
use crate::profile::Profile;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub order: Vec<String>,
    pub warnings: Vec<String>,
}

fn glob_match(pattern: &str, id: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => id.starts_with(prefix),
        None => pattern == id,
    }
}

pub fn resolve(profile: &Profile, installed: &BTreeMap<String, AddonManifest>) -> Result<Plan> {
    let mut warnings = Vec::new();
    if profile.is_vanilla() {
        return Ok(Plan {
            order: Vec::new(),
            warnings,
        });
    }
    let enabled: Vec<&str> = profile.enabled().collect();
    let mut addons = BTreeMap::new();
    for id in &enabled {
        let m = installed
            .get(*id)
            .ok_or_else(|| Error::Resolve(format!("{id} is enabled but not installed")))?;
        addons.insert(*id, m);
    }
    if !addons.contains_key(CORE_ID) {
        return Err(Error::Resolve(format!("{CORE_ID} is missing")));
    }
    for m in addons.values() {
        for r in &m.requires {
            match addons.get(r.id.as_str()) {
                None => {
                    return Err(Error::Resolve(format!(
                        "{} requires {}, which is not enabled",
                        m.id, r.id
                    )));
                }
                Some(dep) if !r.version.matches(&dep.version) => {
                    return Err(Error::Resolve(format!(
                        "{} requires {} {}, found {}",
                        m.id, r.id, r.version, dep.version
                    )));
                }
                Some(_) => {}
            }
        }
        for c in m.conflicts.iter().filter(|c| c.touches.is_none()) {
            if let Some(other) = addons.keys().find(|o| **o != m.id && glob_match(&c.id, o)) {
                return Err(Error::Resolve(format!("{} conflicts with {other}", m.id)));
            }
        }
        if m.conflicts.iter().any(|c| c.touches.is_some()) {
            warnings.push(format!("{}: touch conflicts are checked at build time", m.id));
        }
    }
    let position: BTreeMap<&str, usize> = enabled.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut after: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for m in addons.values() {
        let deps = after.entry(m.id.as_str()).or_default();
        for r in &m.requires {
            deps.insert(r.id.as_str());
        }
        for pat in &m.load_after {
            deps.extend(addons.keys().filter(|o| **o != m.id && glob_match(pat, o)));
        }
        if m.id != CORE_ID {
            deps.insert(CORE_ID);
        }
    }
    let mut order = Vec::new();
    let mut done = BTreeSet::new();
    while done.len() < addons.len() {
        let next = addons
            .keys()
            .filter(|id| !done.contains(*id) && after[*id].iter().all(|d| done.contains(d)))
            .min_by_key(|id| position[*id]);
        let Some(&id) = next else {
            let stuck: Vec<&str> = addons.keys().filter(|id| !done.contains(*id)).copied().collect();
            return Err(Error::Resolve(format!(
                "load order cycle among {}",
                stuck.join(", ")
            )));
        };
        done.insert(id);
        order.push(id.to_owned());
    }
    Ok(Plan { order, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, extra: &str) -> (String, AddonManifest) {
        let json = format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t",
            "compat":{{"game":"f1_25"}}{extra}}}"#
        );
        (id.to_owned(), AddonManifest::parse(&json).unwrap())
    }

    fn profile(ids: &[&str]) -> Profile {
        let mut p = Profile::new("t");
        for id in ids {
            p.set_enabled(id, true).unwrap();
        }
        p
    }

    #[test]
    fn core_first_then_load_after() {
        let installed = BTreeMap::from([
            m(CORE_ID, ""),
            m("a.grid", r#","load_after":["a.track.*"]"#),
            m("a.track.okayama", ""),
        ]);
        let plan = resolve(&profile(&["a.grid", "a.track.okayama"]), &installed).unwrap();
        assert_eq!(plan.order, [CORE_ID, "a.track.okayama", "a.grid"]);
    }

    #[test]
    fn missing_requirement_fails() {
        let installed = BTreeMap::from([
            m(CORE_ID, ""),
            m("a.x", r#","requires":[{"id":"a.y"}]"#),
            m("a.y", ""),
        ]);
        assert!(resolve(&profile(&["a.x"]), &installed).is_err());
        assert!(resolve(&profile(&["a.x", "a.y"]), &installed).is_ok());
    }

    #[test]
    fn cycles_fail() {
        let installed = BTreeMap::from([
            m(CORE_ID, ""),
            m("a.x", r#","load_after":["a.y"]"#),
            m("a.y", r#","load_after":["a.x"]"#),
        ]);
        assert!(resolve(&profile(&["a.x", "a.y"]), &installed).is_err());
    }
}
