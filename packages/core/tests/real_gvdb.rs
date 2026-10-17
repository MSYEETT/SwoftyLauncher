use std::collections::BTreeMap;
use std::path::PathBuf;

use swofty_core::manifest::AddonManifest;
use swofty_core::ops::{DbOps, apply_db_ops};
use swofty_core::owned;
use swofty_core::profile::Profile;
use swofty_core::settings::render_ops;
use swofty_formats::gvdb::Gvdb;

#[test]
fn template_db_ops_resolve_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let mut db = Gvdb::parse(&data).unwrap();
    let template = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/addon");
    let m = AddonManifest::load(&template).unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(template.join("db/example.json")).unwrap()).unwrap();
    render_ops(&mut doc, &m, &Profile::new("test")).unwrap();
    let ops: DbOps = serde_json::from_value(doc).unwrap();
    apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    assert_eq!(
        db.get_text("rules.stop_go_penalty.penalty_duration").as_deref(),
        Some("5")
    );
    assert_eq!(db.records.len(), Gvdb::parse(&data).unwrap().records.len());
}

#[test]
fn added_se_livery_unlock_is_owned_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let pristine = Gvdb::parse(&data).unwrap();
    let ops: DbOps = serde_json::from_str(
        r#"{"schema_version":1,
        "fields":{
          "customisation.custom_se_liveries.se_liveries.se_livery":["_metadata_","id","compatible_vehicle","wep_override","team_customisation","name_loc_tag","desc_loc_tag","ui_priority"],
          "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery":["_metadata_","customisation_item","is_new_on_boot","rarity","statId","type_loctag"]
        },
        "ops":[
          {"op":"add-row","table":"customisation.custom_se_liveries.se_liveries",
           "from":"customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_miami_2026]",
           "set":{"_metadata_":"toro_rosso_rm_alphatauri_2026","id":"22","wep_override":"_barcelona_26",
                  "name_loc_tag":"lng_team_tor_hashtag","desc_loc_tag":"lng_team_tor_hashtag","ui_priority":"22"}},
          {"op":"add-row","table":"unlockable_content.unlockable_items.unlockable_item_list_2.folder_23",
           "from":"unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery[_metadata_=toro_rosso_miami_26]",
           "set":{"_metadata_":"toro_rosso_rm_alphatauri_2026","customisation_item":"toro_rosso_rm_alphatauri_2026","statId":"5280"}}
        ]}"#,
    )
    .unwrap();
    let mut db = pristine.clone();
    let before = owned::counts(&db);
    apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    let added = owned::added(&db, "rmdesigns.alphatauri-orlen-livery", &before);
    let found = owned::owned_items(&db, &pristine, &added);
    assert_eq!(
        found.items,
        vec![owned::OwnedItem {
            key: "0xf01d03c7".into(),
            addon: "rmdesigns.alphatauri-orlen-livery".into(),
            what: "se_livery 22".into(),
        }]
    );
    let stock = std::collections::HashSet::from([owned::item_key("eSELivery", "toro_rosso_miami_26")]);
    assert_eq!(owned::stock_collisions(&pristine, &stock), stock);
}
