//! Exported bindings take host values as `JsValue` and convert them through
//! `utils::input`.
//!
//! wasm-bindgen's glue for primitive parameters checks nothing at runtime: a
//! `&str`/`String` parameter traps the instance (and leaks) on a non-string,
//! integers go through `ToInt32`/`ToBigInt64` wrapping, `bool` through
//! truthiness, `f64` through `ToNumber` (`null` becomes `0`, `"5"` becomes
//! `5`), `&[f64]`/`Vec<f64>` through typed-array glue (a string becomes
//! `NaN`s, `{}` becomes `[]`), and `Vec<String>` splits strings into
//! characters or accepts `{}` as empty. This test scans every `#[wasm_bindgen]` export under
//! `src/api` and rejects those parameter types, so a new binding cannot
//! reintroduce the silent coercions.

use std::fs;
use std::path::{Path, PathBuf};

/// Parameter types whose wasm-bindgen glue coerces or traps.
const FORBIDDEN: &[&str] = &[
    "&str",
    "String",
    "Option<String>",
    "Option<&str>",
    "bool",
    "Option<bool>",
    "usize",
    "u8",
    "u16",
    "u32",
    "u64",
    "i8",
    "i16",
    "i32",
    "i64",
    "isize",
    "Option<usize>",
    "Option<u8>",
    "Option<u16>",
    "Option<u32>",
    "Option<u64>",
    "Option<i8>",
    "Option<i16>",
    "Option<i32>",
    "Option<i64>",
    "Vec<String>",
    "Option<Vec<String>>",
    "f32",
    "f64",
    "Option<f32>",
    "Option<f64>",
    "&[f32]",
    "&[f64]",
    "&mut [f64]",
    "Vec<f32>",
    "Vec<f64>",
    "Box<[f64]>",
    "Option<&[f64]>",
    "Option<Vec<f64>>",
    "Option<Box<[f64]>>",
    "Vec<Vec<f64>>",
];

/// Reasoned exceptions as `(file, function, parameter)`; keep empty.
const ALLOWED: &[(&str, &str, &str)] = &[];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable src/api") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Replace comments and string/char literal contents with spaces so braces
/// and parentheses inside them do not affect matching; offsets are kept.
fn blank(src: &str) -> Vec<u8> {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'r' if matches!(bytes.get(i + 1), Some(b'"' | b'#'))
                && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')) =>
            {
                let mut hashes = 0;
                let mut j = i + 1;
                while bytes.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if bytes.get(j) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                j += 1;
                let closing: Vec<u8> = std::iter::once(b'"')
                    .chain(std::iter::repeat_n(b'#', hashes))
                    .collect();
                while j < bytes.len() && !bytes[j..].starts_with(&closing) {
                    if bytes[j] != b'\n' {
                        out[j] = b' ';
                    }
                    j += 1;
                }
                i = j + closing.len();
            }
            b'"' => {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'"' {
                    if bytes[j] == b'\\' {
                        out[j] = b' ';
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] != b'\n' {
                        out[j] = b' ';
                    }
                    j += 1;
                }
                i = j + 1;
            }
            b'\'' if bytes.get(i + 2) == Some(&b'\'') => {
                out[i + 1] = b' ';
                i += 3;
            }
            _ => i += 1,
        }
    }
    out
}

fn matching(src: &[u8], open_at: usize, open: u8, close: u8) -> usize {
    let mut depth = 0usize;
    for (offset, &byte) in src[open_at..].iter().enumerate() {
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return open_at + offset;
            }
        }
    }
    panic!("unbalanced delimiters");
}

/// Spans of `#[cfg(test)] mod … { … }` blocks.
fn test_spans(src: &[u8]) -> Vec<(usize, usize)> {
    let text = std::str::from_utf8(src).expect("utf8");
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("#[cfg(test)]") {
        let at = from + found;
        let rest = text[at + "#[cfg(test)]".len()..].trim_start();
        let rest = rest.strip_prefix("pub(super) ").unwrap_or(rest);
        let rest = rest.strip_prefix("pub(crate) ").unwrap_or(rest);
        if rest.starts_with("mod ") {
            let brace = at + text[at..].find('{').expect("module body");
            spans.push((at, matching(src, brace, b'{', b'}')));
        }
        from = at + 1;
    }
    spans
}

/// Offsets of the `(` opening each exported function's parameter list, with
/// the function name.
fn exported_fns(src: &[u8]) -> Vec<(usize, String)> {
    let text = std::str::from_utf8(src).expect("utf8");
    let tests = test_spans(src);
    let mut found = std::collections::BTreeMap::new();
    let mut from = 0;
    while let Some(hit) = text[from..].find("#[wasm_bindgen") {
        let attr_at = from + hit;
        from = attr_at + 1;
        if tests.iter().any(|&(a, b)| a <= attr_at && attr_at <= b) {
            continue;
        }
        let attr_end = matching(src, attr_at + 1, b'[', b']');
        let attr = &text[attr_at..=attr_end];
        if attr.contains("skip") {
            continue;
        }
        // Skip further attributes between this one and the item.
        let mut item = attr_end + 1;
        loop {
            let rest = &text[item..];
            let trimmed = rest.trim_start();
            item += rest.len() - trimmed.len();
            if trimmed.starts_with("#[") {
                item = matching(src, item + 1, b'[', b']') + 1;
            } else {
                break;
            }
        }
        let rest = &text[item..];
        if rest.starts_with("impl") {
            let open = item + rest.find('{').expect("impl body");
            let close = matching(src, open, b'{', b'}');
            let body = &text[open..close];
            let mut at = 0;
            while let Some(pos) = body[at..].find("pub fn ") {
                let fn_at = open + at + pos;
                at += pos + 1;
                let preceding = &text[..fn_at];
                let attrs_start = preceding.rfind(['{', '}', ';']).unwrap_or(0);
                if text[attrs_start..fn_at].contains("wasm_bindgen(skip") {
                    continue;
                }
                let name_start = fn_at + "pub fn ".len();
                let paren = name_start + text[name_start..].find('(').expect("params");
                let name = text[name_start..paren]
                    .split('<')
                    .next()
                    .unwrap_or("")
                    .trim();
                found.insert(paren, name.to_string());
            }
        } else if let Some(sig) = rest.strip_prefix("pub fn ") {
            let paren = item + "pub fn ".len() + sig.find('(').expect("params");
            let name = sig.split(['(', '<']).next().unwrap_or("").trim();
            found.insert(paren, name.to_string());
        }
    }
    found.into_iter().collect()
}

fn params(text: &str, open: usize, src: &[u8]) -> Vec<(String, String)> {
    let close = matching(src, open, b'(', b')');
    let list = &text[open + 1..close];
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in list
        .char_indices()
        .chain(std::iter::once((list.len(), ',')))
    {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                let param = list[start..i].trim();
                start = i + 1;
                if let Some((name, ty)) = param.split_once(':') {
                    let ty: String = ty.split_whitespace().collect::<Vec<_>>().join(" ");
                    let ty = ty.replace("& '", "&'");
                    let ty = strip_lifetime(&ty);
                    out.push((name.trim().trim_start_matches("mut ").to_string(), ty));
                }
            }
            _ => {}
        }
    }
    out
}

fn strip_lifetime(ty: &str) -> String {
    let mut out = String::new();
    let mut chars = ty.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                chars.next();
            }
            if chars.peek() == Some(&' ') {
                chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[test]
fn exported_bindings_take_host_values_as_js_value() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
    let mut files = Vec::new();
    rust_files(&api, &mut files);
    files.sort();
    let mut offenders = Vec::new();
    let mut exported = 0;
    for path in &files {
        let text = fs::read_to_string(path).expect("readable source");
        let blanked = blank(&text);
        let blanked_text = String::from_utf8(blanked.clone()).expect("utf8");
        let rel = path
            .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("inside crate")
            .display()
            .to_string();
        for (open, name) in exported_fns(&blanked) {
            exported += 1;
            for (param, ty) in params(&blanked_text, open, &blanked) {
                if FORBIDDEN.contains(&ty.as_str())
                    && !ALLOWED.contains(&(rel.as_str(), name.as_str(), param.as_str()))
                {
                    offenders.push(format!("{rel}: {name}({param}: {ty})"));
                }
            }
        }
    }
    assert!(exported > 500, "scanner found only {exported} exports");
    assert!(
        offenders.is_empty(),
        "take these as JsValue / Option<JsValue> and convert with utils::input:\n{}",
        offenders.join("\n")
    );
}
