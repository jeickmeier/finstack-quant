use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED: &[(&str, usize)] = &[
    ("src/instruments/composite/types/instrument.rs", 1),
    ("src/instruments/composite/types/reporting.rs", 1),
    ("src/instruments/exotics/basket/types.rs", 1),
    // `Tranche::payment_priority` and `TrancheStructure::currency` are
    // derived when the structure is assembled, never read from the wire.
    (
        "src/instruments/fixed_income/structured_credit/types/tranches.rs",
        2,
    ),
];

fn rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("instrument source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

#[test]
fn serde_skip_is_limited_to_documented_derived_artifacts() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_sources(&manifest.join("src/instruments"), &mut files);

    let mut unexpected = Vec::new();
    let mut allowed_hits = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("Rust source is readable");
        let skip_count = source.matches("#[serde(skip)]").count();
        if skip_count > 0 {
            let relative = file
                .strip_prefix(manifest)
                .expect("source is under manifest")
                .to_string_lossy()
                .replace('\\', "/");
            if ALLOWED.iter().any(|(path, _)| *path == relative) {
                allowed_hits.push((relative, skip_count));
            } else {
                unexpected.push((relative, skip_count));
            }
        }
    }

    assert!(
        unexpected.is_empty(),
        "undocumented #[serde(skip)] fields: {unexpected:?}"
    );
    allowed_hits.sort();
    assert_eq!(
        allowed_hits,
        ALLOWED
            .iter()
            .map(|(path, count)| ((*path).to_owned(), *count))
            .collect::<Vec<_>>(),
        "the allowlist must identify only documented derived-artifact skips"
    );
}
