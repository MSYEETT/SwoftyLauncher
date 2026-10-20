use std::path::{Path, PathBuf};

use crate::build::PLAN_FILE;

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect()
}

fn planned_sources(plan: &Path) -> Vec<PathBuf> {
    let Ok(v) = crate::read_json::<serde_json::Value>(plan) else {
        return Vec::new();
    };
    ["replace", "add", "loader"]
        .iter()
        .filter_map(|k| v.get(k)?.as_object())
        .flat_map(|m| m.values())
        .filter_map(|s| s.as_str().map(PathBuf::from))
        .collect()
}

pub fn latest_plan(profile_dir: &Path) -> Option<PathBuf> {
    children(profile_dir)
        .into_iter()
        .filter_map(|b| {
            let plan = b.join(PLAN_FILE);
            Some((plan.metadata().ok()?.modified().ok()?, plan))
        })
        .max()
        .map(|(_, plan)| plan)
}

pub fn prune(cache: &Path, profile: &str, used: &[PathBuf]) -> usize {
    let overlays = cache.join("overlay");
    let mut used = used.to_vec();
    for dir in children(&overlays) {
        if dir.file_name().is_some_and(|n| n != profile)
            && let Some(plan) = latest_plan(&dir)
        {
            used.extend(planned_sources(&plan));
        }
    }
    let maps = cache.join("maps");
    let mut stamps = children(&overlays.join(profile));
    stamps.extend(children(&cache.join("menus")));
    stamps.extend(children(&cache.join("ers")));
    for code in children(&maps) {
        stamps.extend(children(&code));
    }
    let mut removed = 0;
    for stamp in stamps {
        if !used.iter().any(|u| u.starts_with(&stamp)) && std::fs::remove_dir_all(&stamp).is_ok() {
            removed += 1;
        }
    }
    for code in children(&maps) {
        std::fs::remove_dir(code).ok();
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(".done"), "").unwrap();
        dir.to_path_buf()
    }

    #[test]
    fn keeps_the_current_build_and_other_profiles_latest_plans() {
        let cache = std::env::temp_dir().join(format!("swofty-cache-prune-{}", std::process::id()));
        std::fs::remove_dir_all(&cache).ok();
        let track = |code: &str, s: &str| stamp(&cache.join("maps").join(code).join(s));
        let (okayama_22, okayama_24) = (track("okayama", "a22"), track("okayama", "a24"));
        let (bahrain_old, bahrain_new) = (track("bahrain24", "b1"), track("bahrain24", "b2"));
        let gone = track("gone", "g1");
        let (menus_old, menus_new) = (stamp(&cache.join("menus/m1")), stamp(&cache.join("menus/m2")));
        let (ers_old, ers_new) = (stamp(&cache.join("ers/e1")), stamp(&cache.join("ers/e2")));
        let (mine_old, mine_new) = (
            stamp(&cache.join("overlay/default/h1")),
            stamp(&cache.join("overlay/default/h2")),
        );
        let other_old = stamp(&cache.join("overlay/other/o1"));
        crate::write_json(
            &other_old.join(PLAN_FILE),
            &serde_json::json!({ "replace": { "x.erp": bahrain_old.join("x.erp") } }),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let other_new = stamp(&cache.join("overlay/other/o2"));
        crate::write_json(
            &other_new.join(PLAN_FILE),
            &serde_json::json!({ "add": { "t.erp": okayama_22.join("t.erp") } }),
        )
        .unwrap();
        let used = [
            okayama_24.join("2025_asset_groups/t.erp"),
            bahrain_new.clone(),
            menus_new.clone(),
            ers_new.clone(),
            mine_new.clone(),
        ];
        assert_eq!(prune(&cache, "default", &used), 5);
        for kept in [
            &okayama_22,
            &okayama_24,
            &bahrain_new,
            &menus_new,
            &ers_new,
            &mine_new,
            &other_old,
            &other_new,
        ] {
            assert!(kept.is_dir(), "{} was removed", kept.display());
        }
        for removed in [&bahrain_old, &menus_old, &ers_old, &mine_old, &gone] {
            assert!(!removed.exists(), "{} was kept", removed.display());
        }
        assert!(!cache.join("maps/gone").exists());
        assert_eq!(prune(&cache, "default", &used), 0);
        std::fs::remove_dir_all(&cache).ok();
    }
}
