//! Scanner for the `#[wasm_bindgen]` exports under `src/api`, shared by the host
//! tests that read the binding sources as text.

use std::fs;
use std::path::{Path, PathBuf};

/// One exported function, constructor, method or accessor.
pub(crate) struct Export {
    /// Source file relative to the crate root.
    pub(crate) file: String,
    /// Rust function name.
    pub(crate) rust_name: String,
    /// Published JS name: `js_name`, else the camelCase Rust name;
    /// `constructor` for a `#[wasm_bindgen(constructor)]`.
    pub(crate) js_name: String,
    /// JS class of the enclosing `impl`; `None` for a free function.
    pub(crate) class: Option<String>,
    /// Whether this is a getter or setter, which has no positional call form.
    pub(crate) accessor: bool,
    /// `(name, type)` per parameter in order, without `self`.
    pub(crate) params: Vec<(String, String)>,
}

/// Every export under `src/api`, in path and source order.
pub(crate) fn scan_exports() -> Vec<Export> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src/api"), &mut files);
    files.sort();
    let mut exports = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("readable source");
        let blanked = blank(&text);
        let blanked_text = String::from_utf8(blanked.clone()).expect("utf8");
        let file = path
            .strip_prefix(root)
            .expect("inside crate")
            .display()
            .to_string();
        for raw in exported_fns(&blanked) {
            let js_name = if attr_has(&raw.attr, "constructor") {
                "constructor".to_string()
            } else {
                attr_value(&raw.attr, "js_name")
                    .map_or_else(|| camel_case(&raw.rust_name), str::to_string)
            };
            exports.push(Export {
                file: file.clone(),
                js_name,
                class: raw.class,
                accessor: attr_has(&raw.attr, "getter") || attr_has(&raw.attr, "setter"),
                params: params(&blanked_text, raw.open, &blanked),
                rust_name: raw.rust_name,
            });
        }
    }
    exports
}

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
pub(crate) fn blank(src: &str) -> Vec<u8> {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let end = src[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |offset| i + 2 + offset + 2);
                while i < end {
                    if bytes[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
            }
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

pub(crate) fn matching(src: &[u8], open_at: usize, open: u8, close: u8) -> usize {
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

/// The `value` of `key = value` inside an attribute.
fn attr_value<'a>(attr: &'a str, key: &str) -> Option<&'a str> {
    let (_, rest) = attr.split_once(key)?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    // Empty for a `macro_rules!` metavariable (`js_name = $name`).
    Some(&rest[..end]).filter(|value| !value.is_empty())
}

fn attr_has(attr: &str, word: &str) -> bool {
    attr.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|token| token == word)
}

/// The camelCase form of a Rust `snake_case` name.
pub(crate) fn camel_case(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in name.trim_start_matches('_').chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// One exported function before its parameter list is split: the offset of
/// its opening `(`, its Rust name, the `#[wasm_bindgen]` attribute text before
/// it, and the JS class of the enclosing `impl`.
struct RawExport {
    open: usize,
    rust_name: String,
    attr: String,
    class: Option<String>,
}

/// Every exported function in one blanked source file, in source order.
fn exported_fns(src: &[u8]) -> Vec<RawExport> {
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
            let class = attr_value(attr, "js_class")
                .map(str::to_string)
                .or_else(|| {
                    rest["impl".len()..]
                        .trim_start()
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .map(|name| name.trim_start_matches("Js").to_string())
                        .filter(|name| !name.is_empty())
                });
            let body = &text[open..close];
            let mut at = 0;
            while let Some(pos) = body[at..].find("pub fn ") {
                let fn_at = open + at + pos;
                at += pos + 1;
                let preceding = &text[..fn_at];
                let attrs_start = preceding.rfind(['{', '}', ';']).unwrap_or(0);
                let method_attr = &text[attrs_start..fn_at];
                if method_attr.contains("wasm_bindgen(skip") {
                    continue;
                }
                let name_start = fn_at + "pub fn ".len();
                let paren = name_start + text[name_start..].find('(').expect("params");
                let name = text[name_start..paren]
                    .split('<')
                    .next()
                    .unwrap_or("")
                    .trim();
                found.insert(
                    paren,
                    RawExport {
                        open: paren,
                        rust_name: name.to_string(),
                        attr: method_attr.to_string(),
                        class: class.clone(),
                    },
                );
            }
        } else if let Some(sig) = rest.strip_prefix("pub fn ") {
            let paren = item + "pub fn ".len() + sig.find('(').expect("params");
            let name = sig.split(['(', '<']).next().unwrap_or("").trim();
            // A method's own attribute lands here too; its `impl` came first.
            found.entry(paren).or_insert_with(|| RawExport {
                open: paren,
                rust_name: name.to_string(),
                attr: attr.to_string(),
                class: None,
            });
        }
    }
    found.into_values().collect()
}

/// `(name, type)` of each parameter in the list opened at `open`, without `self`.
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
