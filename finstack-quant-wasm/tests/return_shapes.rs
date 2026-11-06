//! Source checks of how WASM entry points build their results.
//!
//! The declared return shapes (Json/Text suffixes, structured results,
//! `Float64Array` vectors) are checked on the published declarations by
//! `tests/facade/return_shapes.test.mjs`, the mirror of
//! `finstack-quant-py/tests/parity/test_return_shapes.py`. These two checks
//! read the Rust and facade sources and need no build.

use std::fs;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// All Rust sources under `src/`, concatenated with their paths, so a check
/// can assert on the binding layer as a whole.
fn api_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &PathBuf, out: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let body = fs::read_to_string(&path).expect("read source");
                out.push((path, body));
            }
        }
    }
    let mut out = Vec::new();
    walk(&manifest_dir().join("src"), &mut out);
    out
}

/// Raw `serde_wasm_bindgen::to_value` emits ES2015 `Map`s for Rust maps, and
/// `JSON.stringify` silently drops those. `crate::utils::to_js_value` uses the
/// `json_compatible` serializer instead. The helper itself is the only place
/// the raw call is legal.
///
/// This is also enforced by `scripts/check_wasm_boundary.py`; keeping it here
/// too means a plain `cargo test` catches it.
#[test]
fn no_binding_bypasses_the_json_compatible_serializer() {
    let offenders: Vec<String> = api_sources()
        .into_iter()
        .filter(|(path, _)| !path.ends_with("utils/mod.rs"))
        .filter(|(_, body)| body.contains("serde_wasm_bindgen::to_value"))
        .map(|(path, _)| path.display().to_string())
        .collect();

    assert!(
        offenders.is_empty(),
        "these files call serde_wasm_bindgen::to_value directly instead of \
         crate::utils::to_js_value; the raw serializer emits ES Maps that \
         JSON.stringify drops: {offenders:?}"
    );
}

/// `text` with every `/* ... */` comment (including JSDoc) removed.
fn without_block_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start + 2..]
            .split_once("*/")
            .map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out
}

/// The JS facade is a pure namespace re-export. A `JSON.parse` in it means the
/// wasm export and the declared type disagree, and it applies inconsistently:
/// `calibrate` used to be parsed while its sibling `dryRun` was not.
#[test]
fn facade_does_no_json_parsing() {
    let exports_dir = manifest_dir().join("exports");
    let mut offenders = Vec::new();
    fn walk(dir: &PathBuf, offenders: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("read exports dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, offenders);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "js") {
                let body = fs::read_to_string(&path).expect("read facade file");
                // JSDoc examples may parse a `*Json` result; only code counts.
                if without_block_comments(&body).contains("JSON.parse") {
                    offenders.push(path.display().to_string());
                }
            }
        }
    }
    walk(&exports_dir, &mut offenders);

    assert!(
        offenders.is_empty(),
        "the facade must be a pure namespace re-export, but these files call \
         JSON.parse: {offenders:?}. Convert the underlying wasm export to \
         return an object instead."
    );
}
