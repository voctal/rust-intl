//! Scans `<dir>/<locale>/<namespace>.json`, flattens, parses, and
//! validates every keys and arguments.
//!
//! The results are cached for the lifetime of the proc-macro process.

use crate::icu::{self, AstNode, VarKind};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub const DEFAULT_LOCALES_DIR: &str = "locales";

#[derive(Debug)]
pub struct KeyInfo {
    pub vars: Vec<(String, VarKind)>,
    #[allow(dead_code)]
    pub source_file: PathBuf,
}

pub struct Schema {
    pub default_locale: String,
    pub locales: Vec<String>,
    pub keys: Vec<String>,
    pub key_info: HashMap<String, KeyInfo>,
    pub messages: HashMap<(String, String), Vec<AstNode>>,
    pub files: Vec<PathBuf>,
}

impl Schema {
    pub fn ast_for<'a>(&'a self, locale: &str, key: &str) -> &'a [AstNode] {
        self.messages
            .get(&(locale.to_string(), key.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn suggest(&self, attempted: &str) -> Option<&str> {
        self.keys
            .iter()
            .map(|k| (k.as_str(), edit_distance(attempted, k)))
            .filter(|(_, d)| *d <= 3)
            .min_by_key(|(_, d)| *d)
            .map(|(k, _)| k)
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[a.len()][b.len()]
}

fn flatten(namespace: &str, value: &Value, out: &mut Vec<(String, String)>) -> Result<(), String> {
    match value {
        Value::String(s) => {
            out.push((namespace.to_string(), s.clone()));
            Ok(())
        }
        Value::Object(map) => {
            for (k, v) in map {
                let next = if namespace.is_empty() {
                    k.clone()
                } else {
                    format!("{namespace}.{k}")
                };
                flatten(&next, v, out)?;
            }
            Ok(())
        }
        other => Err(format!(
            "key '{namespace}' has a {} value; only strings and nested objects are allowed",
            match other {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::Array(_) => "array",
                _ => "non-string",
            }
        )),
    }
}

fn load_json(path: &Path) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
}

struct LocaleData {
    locale: String,
    entries: HashMap<String, (String, PathBuf)>,
}

fn scan_locale_dir(root: &Path) -> Result<Vec<LocaleData>, String> {
    let mut locale_dirs: Vec<String> = std::fs::read_dir(root)
        .map_err(|e| format!("failed to read '{}': {e}", root.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| !n.starts_with('_'))
                .unwrap_or(false)
        })
        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
        .collect();
    locale_dirs.sort();

    if locale_dirs.is_empty() {
        return Err(format!(
            "'{}' contains no locale subdirectories (expected e.g. 'en/', 'fr/', ...)",
            root.display()
        ));
    }

    let mut result = Vec::new();
    let mut errors = Vec::new();

    for locale in &locale_dirs {
        let dir = root.join(locale);
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("json"))
                        .unwrap_or(false)
                })
                .collect(),
            Err(e) => {
                errors.push(format!("failed to read '{}': {e}", dir.display()));
                continue;
            }
        };
        files.sort();

        let mut entries: HashMap<String, (String, PathBuf)> = HashMap::new();
        for file in files.drain(..) {
            let namespace = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("ns")
                .to_string();
            let value = match load_json(&file) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let mut flat = Vec::new();
            if let Err(e) = flatten(&namespace, &value, &mut flat) {
                errors.push(format!("in '{}': {e}", file.display()));
                continue;
            }
            for (key, msg) in flat {
                entries.insert(key, (msg, file.clone()));
            }
        }
        result.push(LocaleData {
            locale: locale.clone(),
            entries,
        });
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(result)
}

pub struct LoadConfig {
    /// Locales directory, relative to `CARGO_MANIFEST_DIR`.
    /// Default: `"locales"`
    pub path: Option<String>,
    pub default_locale: Option<String>,
}

fn build_schema(cfg: LoadConfig) -> Result<Schema, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_string())?;
    let dir_name = cfg.path.unwrap_or_else(|| DEFAULT_LOCALES_DIR.to_string());
    let root = PathBuf::from(manifest_dir).join(&dir_name);

    if !root.is_dir() {
        return Err(format!(
            "locales directory not found at '{}'. Create it with files like '{}/en/common.json', \
             or point load!() at a different directory: rust_intl::load!(path = \"./other-dir\")",
            root.display(),
            dir_name
        ));
    }

    let mut locale_data = scan_locale_dir(&root)?;
    locale_data.sort_by(|a, b| a.locale.cmp(&b.locale));

    let locales: Vec<String> = locale_data.iter().map(|d| d.locale.clone()).collect();
    if locales.is_empty() {
        return Err("no locales found".to_string());
    }

    let default_locale = cfg
        .default_locale
        .clone()
        .filter(|d| locales.contains(d))
        .unwrap_or_else(|| {
            if locales.iter().any(|l| l == "en") {
                "en".to_string()
            } else {
                locales[0].clone()
            }
        });

    let default_data = locale_data
        .iter()
        .find(|d| d.locale == default_locale)
        .expect("default locale must be in the scanned locales");

    let mut keys: Vec<String> = default_data.entries.keys().cloned().collect();
    keys.sort();

    let mut key_info: HashMap<String, KeyInfo> = HashMap::new();
    let mut messages: HashMap<(String, String), Vec<AstNode>> = HashMap::new();
    let mut all_files: Vec<PathBuf> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for key in &keys {
        let (msg, file) = &default_data.entries[key];
        all_files.push(file.clone());
        let ast = match icu::parse(msg) {
            Ok(a) => a,
            Err(e) => {
                errors.push(format!(
                    "failed to parse message for key '{key}' in '{}' ({}): {e}",
                    file.display(),
                    default_locale
                ));
                continue;
            }
        };
        let vars = match icu::collect_vars(&ast) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!(
                    "key '{key}' in '{}' ({}): {e}",
                    file.display(),
                    default_locale
                ));
                continue;
            }
        };
        messages.insert((default_locale.clone(), key.clone()), ast);
        key_info.insert(
            key.clone(),
            KeyInfo {
                vars,
                source_file: file.clone(),
            },
        );
    }

    for data in &locale_data {
        if data.locale == default_locale {
            continue;
        }
        for key in &keys {
            let Some(info) = key_info.get(key) else {
                continue;
            }; // parse error in default locale, already reported
            match data.entries.get(key) {
                None => {
                    eprintln!(
                        "warning: rust_intl: locale '{}' is missing key '{key}' (falling back to '{}')",
                        data.locale, default_locale
                    );
                    messages.insert(
                        (data.locale.clone(), key.clone()),
                        messages[&(default_locale.clone(), key.clone())].clone(),
                    );
                }
                Some((msg, file)) => {
                    all_files.push(file.clone());
                    let ast = match icu::parse(msg) {
                        Ok(a) => a,
                        Err(e) => {
                            errors.push(format!(
                                "failed to parse message for key '{key}' in '{}' ({}): {e}",
                                file.display(),
                                data.locale
                            ));
                            continue;
                        }
                    };
                    let vars = match icu::collect_vars(&ast) {
                        Ok(v) => v,
                        Err(e) => {
                            errors.push(format!(
                                "key '{key}' in '{}' ({}): {e}",
                                file.display(),
                                data.locale
                            ));
                            continue;
                        }
                    };
                    if vars != info.vars {
                        errors.push(format!(
                            "key '{key}' has different arguments in locale '{}' ({:?}) vs default locale '{}' ({:?}), \
                             every locale must use identical argument names and kinds",
                            data.locale, vars, default_locale, info.vars
                        ));
                        continue;
                    }
                    messages.insert((data.locale.clone(), key.clone()), ast);
                }
            }
        }
        for key in data.entries.keys() {
            if !key_info.contains_key(key) {
                eprintln!(
                    "warning: rust_intl: locale '{}' has key '{key}' that the default locale '{}' does not, it won't be reachable from t!()",
                    data.locale, default_locale
                );
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    all_files.sort();
    all_files.dedup();

    Ok(Schema {
        default_locale,
        locales,
        keys,
        key_info,
        messages,
        files: all_files,
    })
}

static SCHEMA: OnceLock<Result<Arc<Schema>, String>> = OnceLock::new();

/// Called by `load!()` with the config parsed from its arguments.
pub fn get_schema_with(cfg: LoadConfig) -> Result<Arc<Schema>, String> {
    SCHEMA
        .get_or_init(|| build_schema(cfg).map(Arc::new))
        .clone()
}

/// Called by `t!()` and `t_ns!()`, schema must already be
/// initialized (or will be with defaults).
pub fn get_schema() -> Result<Arc<Schema>, String> {
    get_schema_with(LoadConfig {
        path: None,
        default_locale: None,
    })
}
