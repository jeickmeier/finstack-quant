//! Compile the `.pyi` stub docstrings into the extension's runtime `__doc__`.
//!
//! The stubs under `finstack_quant/` are the single source of every Python
//! docstring. This module extracts each docstring (module, class, function,
//! method, property) and writes it to
//! `$OUT_DIR/pydoc/<module>/<qualified name>.txt`, where `<module>` is the
//! public dotted path (`finstack_quant.valuations.market`) and the qualified
//! name is `Class`, `Class.member` or `function`; the module docstring is
//! `__module__`. The crate's `pydoc!` macro (`src/bindings/pydoc.rs`) expands
//! to `include_str!` of that file, so a key with no stub docstring is a compile
//! error rather than an empty `__doc__`.
//!
//! The scanner is a small Python tokenizer: it tracks strings, comments,
//! bracket depth and indentation, which is all a ruff-formatted stub needs.
//! Rules:
//! - a docstring is a statement made only of string literals that directly
//!   follows a `def`/`class` header ending in `:` (or starts the file);
//! - raw strings (`r"""`) are taken verbatim; a backslash in a non-raw
//!   docstring is rejected, so no escape decoding can drift from Python's;
//! - the text is normalised exactly as `inspect.cleandoc` does;
//! - a qualified name documented twice in one stub with different text is
//!   ambiguous: no file is written for it (using it fails to compile) and it is
//!   listed in `pydoc/AMBIGUOUS.txt`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// One token of a logical line.
#[derive(Debug)]
enum Token {
    /// Identifier, keyword or number.
    Name(String),
    /// String literal; `None` when it is not a plain docstring candidate.
    Str(Option<String>),
    /// Any other single character (operators, punctuation).
    Op(char),
}

/// A logical line: its indentation column and its tokens.
struct Line {
    indent: usize,
    tokens: Vec<Token>,
}

/// Docstrings of one stub keyed by qualified name (`__module__` for the module).
type Docs = BTreeMap<String, String>;

/// Generate `$OUT_DIR/pydoc` from every stub under `package_root`.
///
/// # Arguments
///
/// * `package_root` - The `finstack_quant` package directory holding the stubs.
/// * `out_dir` - Cargo's `OUT_DIR`; files land in `out_dir/pydoc`.
///
/// # Panics
///
/// Panics (failing the build) when a stub cannot be read or tokenized, a
/// non-raw docstring contains a backslash, or two keys of one module differ only
/// in case (they would collide on case-insensitive file systems).
pub fn generate(package_root: &Path, out_dir: &Path) {
    let mut stubs = Vec::new();
    collect_stubs(package_root, &mut stubs);
    stubs.sort();

    let mut files: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut ambiguous = Vec::new();
    let out_root = out_dir.join("pydoc");
    for stub in &stubs {
        println!("cargo:rerun-if-changed={}", stub.display());
        let module = module_name(package_root, stub);
        let source = fs::read_to_string(stub)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", stub.display()));
        let (docs, duplicates) =
            extract(&source).unwrap_or_else(|err| panic!("cannot parse {}: {err}", stub.display()));
        let mut lowered = BTreeSet::new();
        for key in docs.keys() {
            assert!(
                lowered.insert(key.to_lowercase()),
                "{}: docstring key `{key}` differs from another only in case",
                stub.display()
            );
        }
        for key in duplicates {
            ambiguous.push(format!("{module}:{key}"));
        }
        for (key, text) in docs {
            files.insert(out_root.join(&module).join(format!("{key}.txt")), text);
        }
    }
    files.insert(out_root.join("AMBIGUOUS.txt"), ambiguous.join("\n"));
    sync_dir(&out_root, &files);
}

/// Recursively collect `.pyi` files, skipping caches.
fn collect_stubs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("cannot list {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "__pycache__") {
                collect_stubs(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "pyi") {
            out.push(path);
        }
    }
}

/// Public dotted module path of a stub (`valuations/market/__init__.pyi` ->
/// `finstack_quant.valuations.market`).
fn module_name(package_root: &Path, stub: &Path) -> String {
    let relative = stub
        .strip_prefix(package_root)
        .expect("stub under package root");
    let mut parts = vec!["finstack_quant".to_string()];
    for component in relative.with_extension("").components() {
        let part = component.as_os_str().to_string_lossy().into_owned();
        if part != "__init__" {
            parts.push(part);
        }
    }
    parts.join(".")
}

/// Write `files` under `root`, touching only changed files and deleting stale
/// ones, so unchanged docstrings keep their mtime and a removed docstring stops
/// resolving.
fn sync_dir(root: &Path, files: &BTreeMap<PathBuf, String>) {
    let mut existing = Vec::new();
    if root.exists() {
        collect_files(root, &mut existing);
    }
    for path in existing {
        if !files.contains_key(&path) {
            fs::remove_file(&path)
                .unwrap_or_else(|err| panic!("cannot remove {}: {err}", path.display()));
        }
    }
    for (path, text) in files {
        if fs::read_to_string(path).is_ok_and(|current| &current == text) {
            continue;
        }
        let parent = path.parent().expect("file has a parent directory");
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("cannot create {}: {err}", parent.display()));
        fs::write(path, text)
            .unwrap_or_else(|err| panic!("cannot write {}: {err}", path.display()));
    }
}

/// Recursively collect every file under `dir`.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("cannot list {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Extract the docstrings of one stub.
///
/// Returns the unambiguous docstrings and the qualified names documented more
/// than once with different text (which are dropped from the map).
fn extract(source: &str) -> Result<(Docs, Vec<String>), String> {
    let lines = tokenize(source)?;
    let mut docs = Docs::new();
    let mut duplicates = BTreeSet::new();
    let mut record = |key: String, text: String| match docs.get(&key) {
        Some(existing) if *existing != text => {
            duplicates.insert(key);
        }
        Some(_) => {}
        None => {
            docs.insert(key, text);
        }
    };

    if let Some(first) = lines.first() {
        if let Some(text) = docstring(first)? {
            record("__module__".to_string(), text);
        }
    }

    // Enclosing scopes: (header indent, class qualified name, or `None` inside a function).
    let mut scopes: Vec<(usize, Option<String>)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        while scopes
            .last()
            .is_some_and(|(indent, _)| line.indent <= *indent)
        {
            scopes.pop();
        }
        let Some((is_class, name)) = definition(line) else {
            continue;
        };
        let opens_block = matches!(line.tokens.last(), Some(Token::Op(':')));
        let in_function = scopes.iter().any(|(_, class)| class.is_none());
        let qualified = match scopes.last() {
            Some((_, Some(class))) => format!("{class}.{name}"),
            _ => name,
        };
        if opens_block && !in_function {
            if let Some(next) = lines.get(index + 1) {
                if next.indent > line.indent {
                    if let Some(text) = docstring(next)? {
                        record(qualified.clone(), text);
                    }
                }
            }
        }
        if opens_block {
            scopes.push((line.indent, is_class.then_some(qualified)));
        }
    }
    for key in &duplicates {
        docs.remove(key);
    }
    Ok((docs, duplicates.into_iter().collect()))
}

/// `Some((is_class, name))` when the line is a `def`/`async def`/`class` header.
fn definition(line: &Line) -> Option<(bool, String)> {
    let mut tokens = line.tokens.iter();
    let mut keyword = match tokens.next()? {
        Token::Name(word) => word.as_str(),
        _ => return None,
    };
    if keyword == "async" {
        keyword = match tokens.next()? {
            Token::Name(word) => word.as_str(),
            _ => return None,
        };
    }
    let is_class = match keyword {
        "def" => false,
        "class" => true,
        _ => return None,
    };
    match tokens.next()? {
        Token::Name(name) => Some((is_class, name.clone())),
        _ => None,
    }
}

/// The cleaned docstring if `line` is a statement made only of string literals.
fn docstring(line: &Line) -> Result<Option<String>, String> {
    if line.tokens.is_empty()
        || !line
            .tokens
            .iter()
            .all(|token| matches!(token, Token::Str(_)))
    {
        return Ok(None);
    }
    let mut text = String::new();
    for token in &line.tokens {
        match token {
            Token::Str(Some(part)) => text.push_str(part),
            _ => return Err("docstring contains a backslash escape or a prefixed string".into()),
        }
    }
    Ok(Some(cleandoc(&text)))
}

/// Python's `inspect.cleandoc`.
fn cleandoc(doc: &str) -> String {
    let mut lines: Vec<String> = doc.split('\n').map(expand_tabs).collect();
    let margin = lines
        .iter()
        .skip(1)
        .filter_map(|line| {
            let content = line.trim_start().len();
            (content > 0).then(|| line.len() - content)
        })
        .min();
    if let Some(first) = lines.first_mut() {
        *first = first.trim_start().to_string();
    }
    if let Some(margin) = margin {
        for line in lines.iter_mut().skip(1) {
            *line = line.get(margin..).unwrap_or("").to_string();
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    lines.join("\n")
}

/// Python's `str.expandtabs()` with the default tab size of 8.
fn expand_tabs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut column = 0;
    for c in line.chars() {
        if c == '\t' {
            let spaces = 8 - column % 8;
            out.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            out.push(c);
            column += 1;
        }
    }
    out
}

/// Split Python source into logical lines of tokens.
fn tokenize(source: &str) -> Result<Vec<Line>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut lines = Vec::new();
    let mut tokens = Vec::new();
    let mut indent = 0;
    let mut at_line_start = true;
    let mut depth = 0usize;
    let mut i = 0;
    let mut line_number = 1;
    while i < chars.len() {
        let c = chars[i];
        if at_line_start {
            let mut column = 0;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                column = if chars[i] == '\t' {
                    (column / 8 + 1) * 8
                } else {
                    column + 1
                };
                i += 1;
            }
            indent = column;
            at_line_start = false;
            continue;
        }
        match c {
            '\n' => {
                line_number += 1;
                i += 1;
                if depth == 0 {
                    if !tokens.is_empty() {
                        lines.push(Line {
                            indent,
                            tokens: std::mem::take(&mut tokens),
                        });
                    }
                    at_line_start = true;
                }
            }
            ' ' | '\t' | '\r' | '\x0c' => i += 1,
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '\\' if chars.get(i + 1) == Some(&'\n') => {
                line_number += 1;
                i += 2;
            }
            '(' | '[' | '{' => {
                depth += 1;
                tokens.push(Token::Op(c));
                i += 1;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                tokens.push(Token::Op(c));
                i += 1;
            }
            '"' | '\'' => {
                let (token, next, newlines) = string(&chars, i, "", line_number)?;
                tokens.push(token);
                line_number += newlines;
                i = next;
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let is_prefix = word.len() <= 2
                    && word.chars().all(|ch| "rRbBuUfF".contains(ch))
                    && matches!(chars.get(i), Some('"' | '\''));
                if is_prefix {
                    let (token, next, newlines) = string(&chars, i, &word, line_number)?;
                    tokens.push(token);
                    line_number += newlines;
                    i = next;
                } else {
                    tokens.push(Token::Name(word));
                }
            }
            other => {
                tokens.push(Token::Op(other));
                i += 1;
            }
        }
    }
    if !tokens.is_empty() {
        lines.push(Line { indent, tokens });
    }
    Ok(lines)
}

/// Scan one string literal starting at its opening quote `chars[start]`.
///
/// Returns the token, the index after the closing quote, and the number of
/// newlines consumed. Only unprefixed and `r`-prefixed strings yield text; a
/// backslash in an unprefixed string yields `Str(None)`.
fn string(
    chars: &[char],
    start: usize,
    prefix: &str,
    line_number: usize,
) -> Result<(Token, usize, usize), String> {
    let quote = chars[start];
    let triple = chars.get(start + 1) == Some(&quote) && chars.get(start + 2) == Some(&quote);
    let raw = prefix.eq_ignore_ascii_case("r");
    let plain = prefix.is_empty() || raw;
    let mut i = start + if triple { 3 } else { 1 };
    let mut text = String::new();
    let mut escaped = false;
    let mut newlines = 0;
    loop {
        let Some(&c) = chars.get(i) else {
            return Err(format!(
                "unterminated string starting on line {line_number}"
            ));
        };
        if c == '\\' {
            escaped = true;
            text.push(c);
            if let Some(&next) = chars.get(i + 1) {
                if next == '\n' {
                    newlines += 1;
                }
                text.push(next);
            }
            i += 2;
            continue;
        }
        if c == quote {
            if !triple {
                i += 1;
                break;
            }
            if chars.get(i + 1) == Some(&quote) && chars.get(i + 2) == Some(&quote) {
                i += 3;
                break;
            }
        } else if c == '\n' {
            if !triple {
                return Err(format!(
                    "newline in single-quoted string on line {line_number}"
                ));
            }
            newlines += 1;
        }
        text.push(c);
        i += 1;
    }
    let value = (plain && (raw || !escaped)).then_some(text);
    Ok((Token::Str(value), i, newlines))
}
