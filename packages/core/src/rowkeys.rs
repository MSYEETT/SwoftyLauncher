use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::sync::OnceLock;

use flate2::read::GzDecoder;

use crate::ops::{ANY_SHAPE, FieldNames};

const BUILTIN: &[u8] = include_bytes!("activation/row_keys.txt.gz");

#[derive(Debug, Default)]
pub struct RowKeys {
    shapes: HashMap<String, Vec<String>>,
}

impl RowKeys {
    pub fn builtin() -> &'static RowKeys {
        static KEYS: OnceLock<RowKeys> = OnceLock::new();
        KEYS.get_or_init(|| {
            let mut text = String::new();
            GzDecoder::new(BUILTIN)
                .read_to_string(&mut text)
                .expect("row_keys.txt.gz is valid");
            RowKeys::parse(&text)
        })
    }

    pub fn parse(text: &str) -> Self {
        let mut shapes: HashMap<String, Vec<String>> = HashMap::new();
        let mut any = BTreeSet::new();
        let mut current = None;
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line.strip_prefix('@') {
                Some(shape) => current = Some(shapes.entry(shape.to_owned()).or_default()),
                None => {
                    if let Some(entries) = current.as_mut() {
                        entries.push(line.to_owned());
                    }
                    any.insert(line.to_owned());
                }
            }
        }
        shapes.insert(ANY_SHAPE.to_owned(), any.into_iter().collect());
        Self { shapes }
    }

    pub fn shapes(&self) -> usize {
        self.shapes.len() - 1
    }

    pub fn get(&self, shape: &str) -> Option<&[String]> {
        self.shapes.get(shape).map(Vec::as_slice)
    }
}

pub struct Names<'a> {
    pub fields: &'a BTreeMap<String, Vec<String>>,
    pub keys: &'a RowKeys,
}

impl FieldNames for Names<'_> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.fields.fields(table, row)
    }

    fn row_keys(&self, shape: &str) -> Option<&[String]> {
        self.keys.get(shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_catalogue_names_the_keys_runtime_compositions_build() {
        let keys = RowKeys::builtin();
        assert!(keys.shapes() > 500, "{} shapes", keys.shapes());
        let has = |shape: &str, key: &str| keys.get(shape).is_some_and(|e| e.iter().any(|k| k == key));
        let mode = "entrants.entrant_setup_modes.entrant_setup_mode_N";
        assert!(has(mode, "player.driver.preloaded"));
        assert!(has(mode, "ai.vehicle.networkReplicated"));
        let package = "entrants.vehicles.vehicle_packages.vehicle_package_N";
        assert!(has(
            package,
            "ai_vehicle_package_config.driver_parameter_maps.wet_weather_ability.characteristic"
        ));
        let championship = "championships.Championship_N";
        assert!(has(championship, "+ComInfluence"));
        assert!(has(championship, "_ComCareer_instancecount_"));
        assert!(has("championships.Championship_N.ComCareer_N", "save_data.group"));
    }

    #[test]
    fn parses_shapes_sub_tables_and_the_any_shape() {
        let keys = RowKeys::parse("# c\n@t.r_N\n_metadata_\n+sub\n@t.r_N.sub_N\nv\n");
        assert_eq!(keys.get("t.r_N").unwrap(), ["_metadata_", "+sub"]);
        assert_eq!(keys.get("t.r_N.sub_N").unwrap(), ["v"]);
        assert_eq!(keys.get(ANY_SHAPE).unwrap(), ["+sub", "_metadata_", "v"]);
        assert_eq!(keys.shapes(), 2);
    }
}
