//! Exported bindings take host values as `JsValue` and convert them through
//! `utils::input`.
//!
//! wasm-bindgen's glue for primitive parameters checks nothing at runtime: a
//! `&str`/`String` parameter traps the instance (and leaks) on a non-string,
//! integers go through `ToInt32`/`ToBigInt64` wrapping, `bool` through
//! truthiness, `f64` through `ToNumber` (`null` becomes `0`, `"5"` becomes
//! `5`), `&[f64]`/`Vec<f64>` through typed-array glue (a string becomes
//! `NaN`s, `{}` becomes `[]`), and `Vec<String>` splits strings into
//! characters or accepts `{}` as empty. This test parses every source file
//! under `src/api` with `syn` and rejects those parameter types on
//! `#[wasm_bindgen]` exports, so a new binding cannot reintroduce the silent
//! coercions.

use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{Attribute, FnArg, ImplItem, Item, Pat, Visibility};

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

/// One exported function, constructor, method or accessor.
struct Export {
    /// Source file relative to the crate root.
    file: String,
    /// Rust function name; a `macro_rules!` metavariable keeps its `$`.
    rust_name: String,
    /// `(name, type)` per parameter in order, without `self`; types are
    /// [`compact`]ed and carry no lifetimes.
    params: Vec<(String, String)>,
}

/// Whitespace removed except one space between two word characters, and
/// lifetimes dropped, so `& 'a mut [f64]` and `&mut [f64]` compare equal.
fn compact(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    let mut pending_space = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                chars.next();
            }
            pending_space = false;
        } else if ch.is_whitespace() {
            pending_space = true;
        } else {
            let word = |c: char| c.is_alphanumeric() || c == '_';
            if pending_space && out.chars().last().is_some_and(word) && word(ch) {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }
    out
}

/// The `#[wasm_bindgen]` attribute among `attrs`, if any.
fn wasm_bindgen(attrs: &[Attribute]) -> Option<&Attribute> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("wasm_bindgen"))
}

fn is_skip(attr: &Attribute) -> bool {
    attr.to_token_stream().to_string().contains("skip")
}

fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && compact(&attr.to_token_stream().to_string()) == "#[cfg(test)]"
    })
}

fn signature_params(inputs: impl IntoIterator<Item = FnArg>) -> Vec<(String, String)> {
    inputs
        .into_iter()
        .filter_map(|input| match input {
            FnArg::Typed(typed) => {
                let name = match *typed.pat {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    other => other.to_token_stream().to_string(),
                };
                Some((name, compact(&typed.ty.to_token_stream().to_string())))
            }
            FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Exports of the parsed items, recursing into inline modules except
/// `#[cfg(test)]` ones.
fn scan_items(items: Vec<Item>, file: &str, out: &mut Vec<Export>) {
    for item in items {
        match item {
            Item::Fn(item) => {
                if matches!(item.vis, Visibility::Public(_))
                    && wasm_bindgen(&item.attrs).is_some_and(|attr| !is_skip(attr))
                {
                    out.push(Export {
                        file: file.to_string(),
                        rust_name: item.sig.ident.to_string(),
                        params: signature_params(item.sig.inputs),
                    });
                }
            }
            Item::Impl(item) => {
                if wasm_bindgen(&item.attrs).is_none() {
                    continue;
                }
                for member in item.items {
                    let ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if matches!(method.vis, Visibility::Public(_))
                        && !wasm_bindgen(&method.attrs).is_some_and(is_skip)
                    {
                        out.push(Export {
                            file: file.to_string(),
                            rust_name: method.sig.ident.to_string(),
                            params: signature_params(method.sig.inputs),
                        });
                    }
                }
            }
            Item::Mod(item) if !is_cfg_test(&item.attrs) => {
                if let Some((_, items)) = item.content {
                    scan_items(items, file, out);
                }
            }
            Item::Macro(item) => scan_macro_tokens(item.mac.tokens, false, file, out),
            _ => {}
        }
    }
}

/// Exports written inside a `macro_rules!` body or an item-level macro call,
/// which `syn` leaves as tokens: `#[wasm_bindgen] pub fn`, and `pub fn` inside
/// a `#[wasm_bindgen] impl` body. A function name may be a metavariable
/// (`$name`), and a parameter type one (`$ty`).
fn scan_macro_tokens(tokens: TokenStream, in_export_impl: bool, file: &str, out: &mut Vec<Export>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    // `Some(skip)` once a `#[wasm_bindgen(...)]` attribute precedes the next item.
    let mut pending: Option<bool> = None;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if let Some(TokenTree::Group(attr)) = tokens.get(i + 1)
                    && attr.delimiter() == Delimiter::Bracket
                {
                    let text = attr.stream().to_string();
                    if text.starts_with("wasm_bindgen") {
                        pending = Some(text.contains("skip"));
                    }
                    i += 2;
                    continue;
                }
            }
            TokenTree::Ident(ident) if ident == "impl" => {
                let exported = pending.is_some_and(|skip| !skip);
                if let Some(offset) = tokens[i..].iter().position(
                    |t| matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace),
                ) {
                    if let TokenTree::Group(body) = &tokens[i + offset] {
                        scan_macro_tokens(body.stream(), exported, file, out);
                    }
                    i += offset + 1;
                    pending = None;
                    continue;
                }
            }
            TokenTree::Ident(ident) if ident == "fn" => {
                let public = i > 0 && matches!(&tokens[i - 1], TokenTree::Ident(p) if p == "pub");
                let exported = match pending {
                    Some(skip) => !skip,
                    None => in_export_impl,
                };
                let (name, after_name) = match (tokens.get(i + 1), tokens.get(i + 2)) {
                    (Some(TokenTree::Punct(dollar)), Some(TokenTree::Ident(meta)))
                        if dollar.as_char() == '$' =>
                    {
                        (format!("${meta}"), i + 3)
                    }
                    (Some(TokenTree::Ident(name)), _) => (name.to_string(), i + 2),
                    _ => (String::new(), i + 1),
                };
                let params = tokens[after_name..].iter().find_map(|t| match t {
                    TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => {
                        Some(g.stream())
                    }
                    _ => None,
                });
                if public && exported && !name.is_empty() {
                    out.push(Export {
                        file: file.to_string(),
                        rust_name: name,
                        params: params.map(token_params).unwrap_or_default(),
                    });
                }
                pending = None;
            }
            TokenTree::Group(group) => {
                scan_macro_tokens(group.stream(), false, file, out);
            }
            _ => {}
        }
        i += 1;
    }
}

/// `(name, type)` of each parameter of a token-level parameter list, without `self`.
fn token_params(list: TokenStream) -> Vec<(String, String)> {
    let mut params = Vec::new();
    let mut current: Vec<TokenTree> = Vec::new();
    let mut depth = 0i32;
    let mut previous_dash = false;
    for token in list
        .into_iter()
        .chain(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
            ',',
            proc_macro2::Spacing::Alone,
        ))))
    {
        if let TokenTree::Punct(p) = &token {
            match p.as_char() {
                '<' => depth += 1,
                '>' if !previous_dash => depth -= 1,
                ',' if depth == 0 => {
                    let text: TokenStream = current.drain(..).collect();
                    let text = text.to_string();
                    if let Some((name, ty)) = text.split_once(':') {
                        let name = name.trim().trim_start_matches("mut ").trim();
                        params.push((name.to_string(), compact(ty)));
                    }
                    previous_dash = false;
                    continue;
                }
                _ => {}
            }
            previous_dash = p.as_char() == '-';
        } else {
            previous_dash = false;
        }
        current.push(token);
    }
    params
}

/// Every export under `src/api`, in path and source order.
fn scan_exports() -> Vec<Export> {
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src/api"), &mut files);
    files.sort();
    let mut exports = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("readable source");
        let parsed = syn::parse_file(&text)
            .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
        let file = path
            .strip_prefix(root)
            .expect("inside crate")
            .display()
            .to_string();
        scan_items(parsed.items, &file, &mut exports);
    }
    exports
}

#[test]
fn exported_bindings_take_host_values_as_js_value() {
    let exports = scan_exports();
    let forbidden: Vec<String> = FORBIDDEN.iter().map(|ty| compact(ty)).collect();
    let mut offenders = Vec::new();
    for export in &exports {
        for (param, ty) in &export.params {
            if forbidden.contains(ty)
                && !ALLOWED.contains(&(
                    export.file.as_str(),
                    export.rust_name.as_str(),
                    param.as_str(),
                ))
            {
                offenders.push(format!(
                    "{}: {}({param}: {ty})",
                    export.file, export.rust_name
                ));
            }
        }
    }
    assert!(
        exports.len() > 500,
        "scanner found only {} exports",
        exports.len()
    );
    assert!(
        offenders.is_empty(),
        "take these as JsValue / Option<JsValue> and convert with utils::input:\n{}",
        offenders.join("\n")
    );
}
