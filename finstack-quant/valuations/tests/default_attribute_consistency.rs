//! Default attribute consistency tests for valuations.

use std::fs;
use std::path::{Path, PathBuf};

fn collect_types_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_types_rs_files(&path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some("types.rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_attribute(lines: &[&str], start_idx: usize) -> (String, usize) {
    let mut attr = lines[start_idx].trim().to_string();
    let mut i = start_idx;

    // Handle multiline attributes like:
    // #[serde(
    //   default,
    //   skip_serializing_if = "Option::is_none"
    // )]
    while !attr.contains(']') && i + 1 < lines.len() {
        i += 1;
        attr.push(' ');
        attr.push_str(lines[i].trim());
    }

    (attr, i)
}

#[test]
fn builder_default_requires_serde_default_in_instrument_types() {
    let instruments_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/instruments");
    let mut files = Vec::new();
    collect_types_rs_files(&instruments_dir, &mut files)
        .expect("should collect instrument types.rs files");
    files.sort();

    let mut violations = Vec::new();

    for file in files {
        let content = fs::read_to_string(&file).expect("should read instrument types.rs");
        let lines: Vec<&str> = content.lines().collect();

        let mut pending_attrs: Vec<String> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();

            if trimmed.starts_with("#[") {
                let (attr, end_idx) = parse_attribute(&lines, i);
                pending_attrs.push(attr);
                i = end_idx + 1;
                continue;
            }

            if trimmed.starts_with("pub ") {
                let has_builder_default =
                    pending_attrs.iter().any(|a| a.contains("builder(default"));
                let has_serde_default = pending_attrs
                    .iter()
                    .any(|a| a.contains("serde(") && a.contains("default"));
                let generated_focused_override_field = [
                    "pub instrument_pricing_overrides:",
                    "pub metric_pricing_overrides:",
                    "pub scenario_pricing_overrides:",
                ]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix));

                if has_builder_default && !has_serde_default && !generated_focused_override_field {
                    let rel = file
                        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                        .unwrap_or(&file)
                        .display()
                        .to_string();
                    violations.push(format!("{}:{}: {}", rel, i + 1, trimmed));
                }

                pending_attrs.clear();
            } else if !trimmed.is_empty()
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("///")
            {
                // Reset when we move on to non-field, non-attribute code.
                pending_attrs.clear();
            }

            i += 1;
        }
    }

    assert!(
        violations.is_empty(),
        "Fields with #[builder(default)] must also have #[serde(default)] unless their wire serde is generated.\n{}",
        violations.join("\n")
    );
}

#[test]
fn instrument_types_do_not_store_the_legacy_full_override_bag() {
    let instruments_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/instruments");
    let mut files = Vec::new();
    collect_types_rs_files(&instruments_dir, &mut files)
        .expect("should collect instrument types.rs files");
    files.sort();

    let mut violations = Vec::new();

    for file in files {
        let content = fs::read_to_string(&file).expect("should read instrument types.rs");
        if content.contains("pub pricing_overrides:") {
            let rel = file
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(&file)
                .display()
                .to_string();
            violations.push(rel);
        }
    }

    assert!(
        violations.is_empty(),
        "Instrument types must store only focused pricing override categories.\n{}",
        violations.join("\n")
    );
}
