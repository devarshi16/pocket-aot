//! Read-only progress dashboard data. Capability, execution coverage, and game
//! compatibility deliberately remain separate measurements.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Supported,
    Partial,
    Missing,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Supported => "Supported",
            Self::Partial => "Partial",
            Self::Missing => "Missing",
        }
    }
    fn parse(value: &Value) -> Result<Self, String> {
        match value.as_str() {
            Some("supported") => Ok(Self::Supported),
            Some("partial") => Ok(Self::Partial),
            Some("missing") => Ok(Self::Missing),
            _ => Err(format!("Invalid capability status: {value}")),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub supported: usize,
    pub partial: usize,
    pub missing: usize,
}
impl Counts {
    pub fn total(self) -> usize {
        self.supported + self.partial + self.missing
    }
    pub fn functional(self) -> usize {
        self.supported + self.partial
    }
    pub fn functional_percent(self) -> f64 {
        percent(self.functional(), self.total())
    }
    fn add(&mut self, status: Status) {
        match status {
            Status::Supported => self.supported += 1,
            Status::Partial => self.partial += 1,
            Status::Missing => self.missing += 1,
        }
    }
}
pub fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        100.0 * numerator as f64 / denominator as f64
    }
}
#[derive(Clone, Debug)]
pub struct ApiRow {
    pub name: String,
    pub category: String,
    pub status: Status,
    pub reason: String,
    pub evidence: Vec<String>,
    pub aliases: Vec<String>,
    /// Curated pointers, not proof that the API was executed or verified.
    pub selected_regression_tests: Vec<String>,
    pub observed_in_regression: bool,
    pub host_calls: u64,
    pub reference_validated: bool,
}
#[derive(Clone, Debug)]
pub struct FeatureRow {
    pub id: String,
    pub name: String,
    pub status: Status,
    pub reason: String,
    pub evidence: Vec<String>,
    pub tests: Vec<String>,
    pub reference_validated: bool,
}
#[derive(Clone, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub counts: Counts,
    pub members: Vec<FeatureRow>,
}
#[derive(Clone, Debug)]
pub struct GameRow {
    pub rank: u32,
    pub title: String,
    pub source_url: String,
    pub stage: String,
    pub error: String,
    pub owner_confirmation: String,
    pub built: bool,
    pub hardware_confirmed: bool,
    pub host_frames: u64,
}
#[derive(Clone, Debug)]
pub struct Freshness {
    pub fresh: bool,
    pub changed: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct RuntimeCoverage {
    pub present: bool,
    pub fresh: bool,
    pub changed: Vec<String>,
    pub scenarios: Vec<String>,
    pub api_names_observed: usize,
}
#[derive(Clone, Debug)]
pub struct Dashboard {
    pub baseline: String,
    pub baseline_url: String,
    pub assessed_date: String,
    pub apis: Vec<ApiRow>,
    pub api_counts: Counts,
    pub categories: Vec<Group>,
    pub components: Vec<Group>,
    pub tooling: Vec<FeatureRow>,
    pub core_counts: Counts,
    pub weighted_score: f64,
    pub games: Vec<GameRow>,
    pub sample_built: usize,
    pub sample_confirmed: usize,
    pub freshness: Freshness,
    pub runtime_coverage: RuntimeCoverage,
}
fn json(root: &Path, path: &str) -> Result<Value, String> {
    let data = fs::read(root.join(path)).map_err(|e| format!("Cannot read {path}: {e}"))?;
    serde_json::from_slice(&data).map_err(|e| format!("Invalid {path}: {e}"))
}
fn string(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("Missing array '{key}' in progress data"))
}
fn schema(value: &Value, file: &str) -> Result<(), String> {
    if value["schema_version"].as_u64() != Some(1) {
        return Err(format!("Unsupported schema version in {file}"));
    }
    Ok(())
}
fn hash_file(root: &Path, relative: &str) -> Option<String> {
    fs::read(root.join(relative))
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
}
fn compare_hashes(root: &Path, hashes: &Value) -> Vec<String> {
    let Some(hashes) = hashes.as_object() else {
        return vec!["Source hashes are absent".to_owned()];
    };
    if hashes.is_empty() {
        return vec!["Source hashes are empty".to_owned()];
    }
    hashes
        .iter()
        .filter_map(|(path, expected)| {
            let actual = hash_file(root, path);
            if actual.as_deref() != expected.as_str() || expected.as_str().is_none() {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect()
}
fn feature(value: &Value) -> Result<FeatureRow, String> {
    let id = string(value, "id");
    if id.is_empty() {
        return Err("Capability row has no id".to_owned());
    }
    let reason = ["limits", "acceptance", "reason"]
        .iter()
        .find_map(|k| value[*k].as_str())
        .unwrap_or_default()
        .to_owned();
    Ok(FeatureRow {
        id,
        name: string(value, "name"),
        status: Status::parse(&value["status"])?,
        reason,
        evidence: strings(value, "evidence"),
        tests: strings(value, "tests"),
        reference_validated: value["reference_validated"].as_bool().unwrap_or(false),
    })
}
fn title(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn insert_feature(map: &mut BTreeMap<String, FeatureRow>, row: FeatureRow) -> Result<(), String> {
    let id = row.id.clone();
    if map.insert(id.clone(), row).is_some() {
        Err(format!("Duplicate capability id {id}"))
    } else {
        Ok(())
    }
}
fn runtime_coverage(root: &Path) -> (RuntimeCoverage, BTreeMap<String, u64>) {
    const PATH: &str = "compatibility/coverage/runtime-api-coverage.json";
    let mut coverage = RuntimeCoverage {
        present: root.join(PATH).exists(),
        fresh: false,
        changed: Vec::new(),
        scenarios: Vec::new(),
        api_names_observed: 0,
    };
    let mut hits = BTreeMap::new();
    if !coverage.present {
        return (coverage, hits);
    }
    let value = match json(root, PATH).and_then(|v| {
        schema(&v, PATH)?;
        Ok(v)
    }) {
        Ok(v) => v,
        Err(e) => {
            coverage.changed.push(e);
            return (coverage, hits);
        }
    };
    coverage.changed = compare_hashes(root, &value["source_sha256"]);
    for required in ["converter/runtime/p8.c", "converter/runtime/p8.h"] {
        if value["source_sha256"].get(required).is_none() {
            coverage
                .changed
                .push(format!("Missing profile source hash: {required}"));
        }
    }
    if let Some(values) = value["api_hits"].as_object() {
        for (name, n) in values {
            if let Some(n) = n.as_u64() {
                hits.insert(name.clone(), n);
            } else {
                coverage
                    .changed
                    .push(format!("Invalid API hit count: {name}"));
            }
        }
    } else {
        coverage
            .changed
            .push("API hit counts are absent".to_owned());
    }
    if let Some(values) = value["scenarios"].as_array() {
        coverage.scenarios = values
            .iter()
            .filter_map(|v| {
                v.as_str().map(str::to_owned).or_else(|| {
                    ["name", "title", "id"]
                        .iter()
                        .find_map(|key| v[*key].as_str().map(str::to_owned))
                })
            })
            .collect();
    }
    if coverage.scenarios.is_empty() {
        coverage
            .changed
            .push("No regression scenarios recorded".to_owned());
    }
    coverage.fresh = coverage.changed.is_empty();
    (coverage, hits)
}
impl Dashboard {
    pub fn load(root: &Path) -> Result<Self, String> {
        let audit = json(root, "compatibility/coverage/api-audit.json")?;
        let language = json(root, "compatibility/coverage/language-audit.json")?;
        let manifest = json(root, "compatibility/coverage/capability-manifest.json")?;
        let summary = json(root, "compatibility/coverage/progress-summary.json")?;
        schema(&audit, "api-audit.json")?;
        schema(&language, "language-audit.json")?;
        schema(&manifest, "capability-manifest.json")?;
        let (mut runtime_coverage, hits) = runtime_coverage(root);
        let mut apis = Vec::new();
        let mut api_counts = Counts::default();
        let mut features = BTreeMap::new();
        let mut category_members: BTreeMap<String, Vec<FeatureRow>> = BTreeMap::new();
        for api in array(&audit, "apis")? {
            let name = string(api, "name");
            if name.is_empty() {
                return Err("API row has no canonical name".to_owned());
            }
            let aliases = strings(api, "aliases");
            let host_calls = std::iter::once(&name)
                .chain(aliases.iter())
                .filter_map(|n| hits.get(n))
                .fold(0u64, |a, b| a.saturating_add(*b));
            let row = ApiRow {
                name,
                category: string(api, "category"),
                status: Status::parse(&api["capability_status"])?,
                reason: string(api, "reason"),
                evidence: strings(api, "evidence"),
                aliases,
                selected_regression_tests: strings(api, "selected_regression_tests"),
                observed_in_regression: runtime_coverage.fresh
                    && host_calls > 0
                    && api["capability_status"].as_str() != Some("missing"),
                host_calls,
                reference_validated: api["reference_validated"].as_bool().unwrap_or(false)
                    || api["reference_equivalence_verified"]
                        .as_bool()
                        .unwrap_or(false),
            };
            let f = FeatureRow {
                id: format!("api.{}", row.name),
                name: format!("{}()", row.name),
                status: row.status,
                reason: row.reason.clone(),
                evidence: row.evidence.clone(),
                tests: row.selected_regression_tests.clone(),
                reference_validated: row.reference_validated,
            };
            insert_feature(&mut features, f.clone())?;
            category_members
                .entry(row.category.clone())
                .or_default()
                .push(f);
            api_counts.add(row.status);
            apis.push(row);
        }
        runtime_coverage.api_names_observed =
            apis.iter().filter(|api| api.observed_in_regression).count();
        let categories = category_members
            .into_iter()
            .map(|(id, members)| {
                let mut counts = Counts::default();
                for m in &members {
                    counts.add(m.status);
                }
                Group {
                    name: title(&id),
                    id,
                    counts,
                    members,
                }
            })
            .collect();
        for value in array(&language, "features")?
            .iter()
            .chain(array(&manifest, "custom_features")?.iter())
        {
            insert_feature(&mut features, feature(value)?)?;
        }
        let mut components = Vec::new();
        let mut counted = BTreeSet::new();
        let mut core_counts = Counts::default();
        let mut weighted_score = 0.0;
        let mut weight_total = 0.0;
        for category in array(&manifest, "categories")? {
            let mut members = Vec::new();
            let mut counts = Counts::default();
            for id in strings(category, "members") {
                if !counted.insert(id.clone()) {
                    return Err(format!("Core capability {id} is counted twice"));
                }
                let row = features
                    .get(&id)
                    .ok_or_else(|| format!("Unknown capability {id}"))?
                    .clone();
                counts.add(row.status);
                core_counts.add(row.status);
                members.push(row);
            }
            let weight = category["weight"]
                .as_f64()
                .ok_or("Capability category has no numeric weight")?;
            if weight < 0.0 || !weight.is_finite() {
                return Err("Capability category has invalid weight".to_owned());
            }
            weight_total += weight;
            if counts.total() > 0 {
                weighted_score += weight * (counts.supported as f64 + counts.partial as f64 * 0.5)
                    / counts.total() as f64;
            }
            components.push(Group {
                id: string(category, "id"),
                name: string(category, "name"),
                counts,
                members,
            });
        }
        if weight_total > 0.0 {
            weighted_score *= 100.0 / weight_total;
        }
        let mut tooling = Vec::new();
        for id in strings(&manifest, "tooling_members") {
            if counted.contains(&id) {
                return Err(format!(
                    "Supporting tooling {id} is also counted as a core capability"
                ));
            }
            tooling.push(
                features
                    .get(&id)
                    .ok_or_else(|| format!("Unknown tooling capability {id}"))?
                    .clone(),
            );
        }
        let source = json(root, "compatibility/top200/results.json")?;
        let values = source
            .as_array()
            .ok_or("Game sample results must be an array")?;
        let mut games = Vec::new();
        for game in values {
            games.push(GameRow {
                rank: game["rank"].as_u64().unwrap_or(0) as u32,
                title: string(game, "title"),
                source_url: string(game, "page"),
                stage: string(game, "conversion"),
                error: string(game, "error"),
                owner_confirmation: string(game, "owner_confirmation"),
                built: game["conversion"].as_str() == Some("built"),
                hardware_confirmed: game["hardware_confirmed"].as_bool().unwrap_or(false),
                host_frames: game["host_frames"].as_u64().unwrap_or(0),
            });
        }
        games.sort_by_key(|game| game.rank);
        let sample_built = games.iter().filter(|game| game.built).count();
        let sample_confirmed = games.iter().filter(|game| game.hardware_confirmed).count();
        let mut changed = compare_hashes(root, &manifest["source_hashes"]);
        // An out-of-date summary must not silently override the actual inventory.
        if summary["core_gate_count"]
            .as_u64()
            .is_some_and(|n| n as usize != core_counts.total())
        {
            changed
                .push("progress-summary.json: core gate count differs from inventory".to_owned());
        }
        if summary["core_capability_score_percent"]
            .as_f64()
            .is_some_and(|n| (n - weighted_score).abs() > 0.01)
        {
            changed.push("progress-summary.json: weighted index differs from inventory".to_owned());
        }
        Ok(Self {
            baseline: string(&manifest, "baseline"),
            baseline_url: string(&manifest, "baseline_url"),
            assessed_date: string(&manifest, "assessed_date"),
            apis,
            api_counts,
            categories,
            components,
            tooling,
            core_counts,
            weighted_score,
            games,
            sample_built,
            sample_confirmed,
            freshness: Freshness {
                fresh: changed.is_empty(),
                changed,
            },
            runtime_coverage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "pocket-progress-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("compatibility/coverage")).unwrap();
            fs::create_dir_all(root.join("compatibility/top200")).unwrap();
            fs::create_dir_all(root.join("converter/runtime")).unwrap();
            fs::write(root.join("converter/runtime/p8.c"), "runtime").unwrap();
            fs::write(root.join("converter/runtime/p8.h"), "header").unwrap();
            let this = Self(root);
            this.write("api-audit.json", json!({"schema_version":1,"apis":[{"name":"time","aliases":["t"],"category":"system","capability_status":"supported","reason":"Fixed frame clock","selected_regression_tests":["curated-pointer"]},{"name":"print","category":"graphics","capability_status":"partial"},{"name":"music","category":"audio","capability_status":"missing"}]}));
            this.write("language-audit.json", json!({"schema_version":1,"features":[{"id":"language.loops","name":"Loops","status":"supported"}]}));
            this.write("capability-manifest.json", json!({"schema_version":1,"baseline":"PICO-8","categories":[{"id":"core","name":"Core","weight":100,"members":["api.time","api.print","api.music","language.loops"]}],"tooling_members":["tooling.gui"],"custom_features":[{"id":"tooling.gui","name":"GUI","status":"supported"}],"source_hashes":{"converter/runtime/p8.c":hash_file(&this.0,"converter/runtime/p8.c").unwrap()}}));
            this.write(
                "progress-summary.json",
                json!({"core_gate_count":4,"core_capability_score_percent":62.5}),
            );
            fs::write(this.0.join("compatibility/top200/results.json"), serde_json::to_vec(&json!([{"rank":19,"title":"Dusk Child","conversion":"built","hardware_confirmed":true},{"rank":2,"title":"Blocked","conversion":"unsupported","error":"missing API"}])).unwrap()).unwrap();
            this
        }
        fn write(&self, file: &str, data: Value) {
            fs::write(
                self.0.join("compatibility/coverage").join(file),
                serde_json::to_vec(&data).unwrap(),
            )
            .unwrap();
        }
        fn profile(&self) {
            self.write("runtime-api-coverage.json", json!({"schema_version":1,"source_sha256":{"converter/runtime/p8.c":hash_file(&self.0,"converter/runtime/p8.c").unwrap(),"converter/runtime/p8.h":hash_file(&self.0,"converter/runtime/p8.h").unwrap()},"api_hits":{"t":3,"print":7,"music":9},"scenarios":[{"name":"semantics"}]}));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn inventory_separates_partial_missing_tooling_and_game_evidence() {
        let f = Fixture::new();
        let d = Dashboard::load(&f.0).unwrap();
        assert_eq!(
            d.api_counts,
            Counts {
                supported: 1,
                partial: 1,
                missing: 1
            }
        );
        assert_eq!(d.core_counts.total(), 4);
        assert_eq!(d.tooling.len(), 1);
        assert_eq!(d.weighted_score, 62.5);
        assert_eq!(d.sample_built, 1);
        assert_eq!(d.sample_confirmed, 1);
        assert_eq!(d.games[0].rank, 2);
        assert!(d.freshness.fresh);
        assert!(!d.runtime_coverage.present);
        assert!(
            !d.apis[0].observed_in_regression,
            "A test-file pointer is not execution evidence"
        );
    }
    #[test]
    fn observed_calls_use_fresh_profile_and_merge_aliases() {
        let f = Fixture::new();
        f.profile();
        let d = Dashboard::load(&f.0).unwrap();
        assert!(d.runtime_coverage.fresh);
        assert_eq!(d.runtime_coverage.api_names_observed, 2);
        assert_eq!(d.apis[0].host_calls, 3);
        assert!(d.apis[0].observed_in_regression);
        assert!(!d.apis[2].observed_in_regression);
        assert!(!d.apis[0].reference_validated);
    }
    #[test]
    fn source_change_stales_audit_and_profile_without_mutating_evidence() {
        let f = Fixture::new();
        f.profile();
        fs::write(f.0.join("converter/runtime/p8.c"), "changed").unwrap();
        let d = Dashboard::load(&f.0).unwrap();
        assert!(!d.freshness.fresh);
        assert!(!d.runtime_coverage.fresh);
        assert_eq!(d.apis[0].host_calls, 3);
        assert!(!d.apis[0].observed_in_regression);
        assert_eq!(d.sample_confirmed, 1);
    }
    #[test]
    fn invalid_optional_profile_keeps_inventory_readable() {
        let f = Fixture::new();
        f.write("runtime-api-coverage.json", json!({"schema_version":99}));
        let d = Dashboard::load(&f.0).unwrap();
        assert!(d.runtime_coverage.present);
        assert!(!d.runtime_coverage.fresh);
        assert!(!d.runtime_coverage.changed.is_empty());
        assert_eq!(d.api_counts.total(), 3);
    }
    #[test]
    fn duplicate_core_members_are_rejected() {
        let f = Fixture::new();
        let path = f.0.join("compatibility/coverage/capability-manifest.json");
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["categories"][0]["members"]
            .as_array_mut()
            .unwrap()
            .push(json!("api.time"));
        f.write("capability-manifest.json", v);
        assert!(Dashboard::load(&f.0).unwrap_err().contains("counted twice"));
    }
}
