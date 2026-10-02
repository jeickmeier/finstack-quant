//! LLM projection of generated schemas ([`project_llm`], [`LlmProfile`]).
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::externalize::*;
use crate::{Error, Result};

/// Rustdoc sections that describe the Rust API rather than the wire contract.
///
/// Kept out of the projected form: a model filling in a payload gains nothing
/// from a Rust usage example, and `# Examples` alone accounts for 346 of the
/// 1,578 headings in the corpus. Domain sections — `# References`,
/// `# Standards Reference`, `# Market Conventions` and the like — are the
/// grounding that makes the schema readable, and are deliberately kept.
pub(super) const RUSTDOC_SECTIONS_TO_DROP: &[&str] = &[
    "Arguments",
    "Errors",
    "Example",
    "Examples",
    "Panics",
    "Safety",
    "See Also",
    "Thread Safety",
    "Type Parameters",
];

/// Keywords a projected schema drops because provider subsets ignore or reject them.
pub(super) const NON_PORTABLE_KEYWORDS: &[&str] = &[
    "$comment",
    "default",
    "deprecated",
    "format",
    "readOnly",
    "uniqueItems",
    "writeOnly",
];

/// Which projection passes to apply.
///
/// Every pass defaults on; turn one off to isolate its effect when measuring.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LlmProfile {
    /// Pull externally referenced documents into `$defs` so nothing is fetched.
    pub inline_references: bool,
    /// Rewrite all-string-`const` unions as a flat `enum`.
    pub flatten_const_unions: bool,
    /// Strip Rust API prose from `description` text.
    pub trim_descriptions: bool,
    /// Close objects, require every declared property, and drop non-portable keywords.
    pub strict_shape: bool,
    /// Largest referenced document to inline, in compact bytes.
    ///
    /// Inlining is what makes a schema self-contained, but a reference to a
    /// large document turns a small contract into a large one — a portfolio
    /// bundle that names the instrument union inherits all seventy branches.
    /// Anything over this budget becomes a handle instead: an opaque string
    /// carrying [`RESOLVES_FROM_KEYWORD`] so the caller knows which contract to
    /// fetch and validate separately.
    pub max_inline_bytes: usize,
    /// Longest description to keep, in characters. Zero disables the cap.
    ///
    /// Rustdoc prose is written for a reader deciding *whether* to use a type;
    /// a payload author already decided and needs to know only what the field
    /// means. The leading paragraph carries that, and the elaboration after it
    /// is the single largest remaining cost once sections and code blocks are
    /// gone — measured at 53% to 65% of every projected instrument's bytes.
    pub max_description_chars: usize,
}

/// Extension keyword naming the contract a handle stands in for.
pub const RESOLVES_FROM_KEYWORD: &str = "x-finstack-resolves-from";

/// Default ceiling for inlining a referenced document, in compact bytes.
///
/// 16 KiB is not a tuning knob; it is the gap in the corpus between the two
/// kinds of shared document. The largest primitive a payload author needs in
/// front of them is `money` at 11.4 KB (`currency` is 10.6 KB, `day_count`
/// 4.9 KB). The smallest bag they almost never fill in is
/// `metric_pricing_overrides` at 21.4 KB, followed by
/// `instrument_pricing_overrides` at 44.5 KB — and those two drag in the whole
/// pricing-model configuration universe behind them.
///
/// Measured across all 109 artifacts, moving this ceiling from 64 KiB to
/// 16 KiB takes the median projected instrument from 80.4 KB to 32.4 KB and
/// the count fitting in 8k tokens from 32 to 90.
pub const DEFAULT_MAX_INLINE_BYTES: usize = 16 * 1024;

/// Default ceiling for one description, in characters.
///
/// 240 characters is about two sentences — enough for the summary line plus a
/// unit or convention note, which is what a payload author acts on. Anything
/// longer is elaboration a reader can fetch from the canonical artifact.
pub(crate) const DEFAULT_MAX_DESCRIPTION_CHARS: usize = 240;

impl Default for LlmProfile {
    fn default() -> Self {
        Self {
            inline_references: true,
            flatten_const_unions: true,
            trim_descriptions: true,
            strict_shape: true,
            max_inline_bytes: DEFAULT_MAX_INLINE_BYTES,
            max_description_chars: DEFAULT_MAX_DESCRIPTION_CHARS,
        }
    }
}

/// Project a published schema into a form a language model can consume.
///
/// The published artifacts are tuned for validation and rustdoc parity. That
/// makes them correct and unusable as tool schemas: they reference an
/// unresolvable host, spend roughly half their bytes on Rust prose, and express
/// every unit enum as a union of `const` branches. This rewrites all three,
/// plus the shape differences that provider subsets reject.
///
/// # This is not a validator
///
/// The result is simultaneously **stricter** than the runtime contract (every
/// property is required, with optionality moved into the type) and **looser**
/// (tuples become fixed-length arrays, non-portable assertions are dropped).
/// Validate against the artifact from
/// [`SchemaArtifact::generate`](super::SchemaArtifact::generate) — never against this.
///
/// # Arguments
///
/// * `schema` - A generated artifact, as written to disk.
/// * `resolve` - Maps an absolute `$id` to that document. Callers that own the
///   whole corpus can look up their registries; returning `None` leaves the
///   reference in place rather than failing.
/// * `profile` - Which passes to run.
///
/// # Errors
///
/// Returns [`Error::Internal`] if the document is not a JSON object.
pub fn project_llm(
    schema: &Value,
    resolve: &dyn Fn(&str) -> Option<Value>,
    profile: &LlmProfile,
) -> Result<Value> {
    if !schema.is_object() {
        return Err(Error::Internal(
            "projected schema must be a JSON object".to_string(),
        ));
    }
    let mut projected = schema.clone();

    if profile.inline_references {
        inline_external_references(&mut projected, resolve, profile.max_inline_bytes);
    }
    if profile.max_inline_bytes > 0 {
        substitute_oversized_local_definitions(&mut projected, profile.max_inline_bytes);
        prune_unreachable_defs(&mut projected);
    }
    if profile.flatten_const_unions {
        flatten_const_unions(&mut projected);
    }
    if profile.trim_descriptions {
        trim_descriptions(&mut projected, profile.max_description_chars);
    }
    if profile.strict_shape {
        apply_strict_shape(&mut projected);
    }

    Ok(projected)
}

/// Turn an absolute `$id` into a `$defs` key.
pub(super) fn definition_name_for(id: &str) -> String {
    let stem = id
        .rsplit('/')
        .next()
        .unwrap_or(id)
        .trim_end_matches(".schema.json");
    let mut name = String::new();
    let mut capitalize = true;
    for character in stem.chars() {
        if character == '_' || character == '-' || character == '.' {
            capitalize = true;
        } else if capitalize {
            name.extend(character.to_uppercase());
            capitalize = false;
        } else {
            name.push(character);
        }
    }
    if name.is_empty() {
        "External".to_string()
    } else {
        name
    }
}

/// Collect every absolute `$ref` target in a document.
pub(super) fn collect_external_refs(node: &Value, found: &mut BTreeSet<String>) {
    match node {
        Value::Object(object) => {
            if let Some(Value::String(target)) = object.get("$ref") {
                if !target.starts_with('#') {
                    found.insert(target.clone());
                }
            }
            for nested in object.values() {
                collect_external_refs(nested, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_external_refs(item, found);
            }
        }
        _ => {}
    }
}

/// Rewrite absolute `$ref`s to local ones, given the id-to-name mapping.
pub(super) fn rewrite_refs(node: &mut Value, names: &BTreeMap<String, String>) {
    match node {
        Value::Object(object) => {
            if let Some(Value::String(target)) = object.get("$ref") {
                if let Some(name) = names.get(target) {
                    let local = format!("#/$defs/{name}");
                    object.insert("$ref".to_string(), Value::String(local));
                }
            }
            for nested in object.values_mut() {
                rewrite_refs(nested, names);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_refs(item, names);
            }
        }
        _ => {}
    }
}

/// Pull every resolvable external reference into local `$defs`.
///
/// Definitions are added once and shared, and each newly inlined document is
/// scanned in turn, so a chain of references resolves in one pass. Unresolvable
/// targets are left untouched — a missing shared definition should surface as a
/// dangling reference, not as a silently different contract.
pub(super) fn inline_external_references(
    schema: &mut Value,
    resolve: &dyn Fn(&str) -> Option<Value>,
    max_inline_bytes: usize,
) {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut bodies: BTreeMap<String, Value> = BTreeMap::new();
    let mut pending = BTreeSet::new();
    collect_external_refs(schema, &mut pending);

    while let Some(id) = pending.iter().next().cloned() {
        pending.remove(&id);
        if names.contains_key(&id) {
            continue;
        }
        let Some(mut document) = resolve(&id) else {
            continue;
        };

        // Too large to carry: stand it in as a handle rather than let one
        // reference dominate the projected document.
        let measured = serde_json::to_vec(&document).map(|bytes| bytes.len());
        if measured.is_ok_and(|size| size > max_inline_bytes) {
            let summary = document
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut handle = Map::new();
            // Keep the target's own top-level type. Forcing a string here would
            // be wrong wherever the reference stands for an object, so the
            // handle stays shape-correct and merely permissive about contents.
            if let Some(kind) = document.get("type").cloned() {
                handle.insert("type".to_string(), kind);
            }
            if handle.get("type").is_some_and(|kind| kind == "object") {
                handle.insert("additionalProperties".to_string(), Value::Bool(true));
            }
            handle.insert(RESOLVES_FROM_KEYWORD.to_string(), Value::String(id.clone()));
            handle.insert(
                "description".to_string(),
                Value::String(match summary {
                    Some(text) => format!(
                        "{text}\n\nToo large to inline here: build this against `{id}` and \
                         validate it separately."
                    ),
                    None => format!(
                        "Value matching `{id}`; build and validate it against that contract."
                    ),
                }),
            );
            let mut candidate = definition_name_for(&id);
            while bodies.contains_key(&candidate) {
                candidate.push('_');
            }
            names.insert(id, candidate.clone());
            bodies.insert(candidate, Value::Object(handle));
            continue;
        }

        // The inlined body becomes a definition, so document-level identity and
        // its own examples are dropped; nested `$defs` are hoisted alongside.
        let mut nested_defs = Map::new();
        if let Some(object) = document.as_object_mut() {
            for key in ["$id", "$schema", "examples"] {
                object.remove(key);
            }
            if let Some(Value::Object(defs)) = object.remove("$defs") {
                nested_defs = defs;
            }
        }

        let mut candidate = definition_name_for(&id);
        while bodies.contains_key(&candidate) {
            candidate.push('_');
        }
        collect_external_refs(&document, &mut pending);
        for value in nested_defs.values() {
            collect_external_refs(value, &mut pending);
        }
        names.insert(id, candidate.clone());
        bodies.insert(candidate, document);
        for (key, value) in nested_defs {
            bodies.entry(key).or_insert(value);
        }
    }

    if bodies.is_empty() {
        return;
    }
    rewrite_refs(schema, &names);
    for body in bodies.values_mut() {
        rewrite_refs(body, &names);
    }
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let defs = object
        .entry("$defs".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(defs) = defs.as_object_mut() {
        for (name, body) in bodies {
            defs.entry(name).or_insert(body);
        }
    }
}

/// Replace locally-defined definitions that exceed the inline budget with handles.
///
/// [`inline_external_references`] can only substitute a handle for a document
/// reached by `$ref`. A container instrument — a basket, a levered equity
/// wrapper — embeds the *whole* instrument union as local `$defs` instead, so
/// the budget never applies and one contract carries all seventy branches:
/// measured at 302 definitions and 393 KB for `basket`. Standing the oversized
/// definition down to a handle makes its dependents unreachable, which
/// [`prune_unreachable_definitions`] then removes.
pub(super) fn substitute_oversized_local_definitions(schema: &mut Value, max_inline_bytes: usize) {
    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        return;
    };
    let oversized: Vec<String> = defs
        .iter()
        .filter(|(_, body)| {
            serde_json::to_vec(body).is_ok_and(|bytes| bytes.len() > max_inline_bytes)
        })
        .map(|(name, _)| name.clone())
        .collect();
    if oversized.is_empty() {
        return;
    }

    let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
        return;
    };
    for name in oversized {
        let Some(body) = defs.get(&name) else {
            continue;
        };
        let summary = body
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut handle = Map::new();
        // Keep the definition's own top-level type where it declares one, so
        // the handle stays shape-correct and is merely permissive about
        // contents.
        if let Some(kind) = body.get("type").cloned() {
            handle.insert("type".to_string(), kind);
        }
        if handle.get("type").is_some_and(|kind| kind == "object") {
            handle.insert("additionalProperties".to_string(), Value::Bool(true));
        }
        handle.insert(
            "description".to_string(),
            Value::String(match summary {
                Some(text) => format!(
                    "{text}\n\nToo large to inline here: build this sub-document on its own \
                     and validate it against the canonical contract."
                ),
                None => format!(
                    "A `{name}` value. Too large to inline here: build this sub-document on \
                     its own and validate it against the canonical contract."
                ),
            }),
        );
        defs.insert(name, Value::Object(handle));
    }
}
/// Rewrite `oneOf` unions of string `const`s as a flat `enum`.
///
/// schemars emits a unit enum as one branch per variant so each keeps its doc
/// comment. That is the single largest avoidable cost in the corpus, and a flat
/// `enum` is what constrained decoding handles natively. Variant documentation
/// is folded into the parent description when it is short enough to be worth
/// the tokens.
pub(super) fn flatten_const_unions(node: &mut Value) {
    match node {
        Value::Object(object) => {
            for nested in object.values_mut() {
                flatten_const_unions(nested);
            }

            let Some(branches) = object.get("oneOf").and_then(Value::as_array) else {
                return;
            };
            if branches.len() < 2 {
                return;
            }
            let mut values = Vec::with_capacity(branches.len());
            let mut notes = Vec::new();
            for branch in branches {
                let Some(branch) = branch.as_object() else {
                    return;
                };
                let Some(Value::String(constant)) = branch.get("const") else {
                    return;
                };
                if branch
                    .keys()
                    .any(|key| !matches!(key.as_str(), "const" | "type" | "description" | "title"))
                {
                    return;
                }
                if let Some(Value::String(note)) = branch.get("description") {
                    if let Some(first) = note.lines().find(|line| !line.trim().is_empty()) {
                        let mut gloss = format!("`{constant}`: {}", first.trim());
                        // The variant's standards citation is the part that
                        // tells a payload author whether this is the spelling
                        // their contract actually calls for, so it rides along
                        // with the summary rather than being dropped with the
                        // rest of the body.
                        let citations = collect_citation_lines(note);
                        if !citations.is_empty() {
                            gloss.push_str(" (");
                            gloss.push_str(&citations.join("; "));
                            gloss.push(')');
                        }
                        notes.push(gloss);
                    }
                }
                values.push(Value::String(constant.clone()));
            }

            // A long variant gloss costs more than it teaches; ISO currency
            // codes are the case that made this threshold necessary. Glosses
            // carrying a standards citation are exempt: a wrong day count is a
            // priced error, and the citation is what prevents it.
            const MAX_FOLDED_NOTE_BYTES: usize = 600;
            let carries_citation = notes.iter().any(|note| mentions_a_standard(note));
            let folded = notes.join(" · ");
            object.remove("oneOf");
            object.insert("type".to_string(), Value::String("string".to_string()));
            object.insert("enum".to_string(), Value::Array(values));
            if !folded.is_empty() && (carries_citation || folded.len() <= MAX_FOLDED_NOTE_BYTES) {
                let existing = object
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let combined = if existing.is_empty() {
                    folded
                } else {
                    format!("{existing}\n\n{folded}")
                };
                object.insert("description".to_string(), Value::String(combined));
            }
        }
        Value::Array(items) => {
            for item in items {
                flatten_const_unions(item);
            }
        }
        _ => {}
    }
}

/// Pull the standards-citation lines out of a rustdoc body, stripped of markup.
///
/// Citations are written as Markdown list items under a reference heading, for
/// example `- **ISDA**: 2006 ISDA Definitions, Section 4.16(d)`. The bullet and
/// emphasis carry no meaning once the text is folded into a one-line gloss.
pub(super) fn collect_citation_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('-') && mentions_a_standard(line))
        .map(|line| {
            line.trim_start_matches('-')
                .trim()
                .replace("**", "")
                .replace("Also known as:", "aka")
        })
        .collect()
}

/// Reduce one rustdoc description to the prose a payload author needs.
pub(super) fn project_description(text: &str) -> String {
    let mut output: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut dropping_section = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("# ") {
            dropping_section = RUSTDOC_SECTIONS_TO_DROP.contains(&heading.trim());
            if dropping_section {
                continue;
            }
        }
        if dropping_section {
            continue;
        }
        output.push(line.to_string());
    }

    let joined = output.join("\n");
    // `[`Type`]` and `[`mod::fn`]` are rustdoc link syntax, not emphasis.
    let mut cleaned = String::with_capacity(joined.len());
    let mut rest = joined.as_str();
    while let Some(start) = rest.find("[`") {
        cleaned.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("`]") {
            Some(end) => {
                cleaned.push('`');
                cleaned.push_str(&after[..end]);
                cleaned.push('`');
                rest = &after[end + 2..];
            }
            None => {
                cleaned.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    cleaned.push_str(rest);

    cleaned
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Apply [`project_description`] to every `description` in the document.
pub(super) fn trim_descriptions(node: &mut Value, max_chars: usize) {
    match node {
        Value::Object(object) => {
            if let Some(Value::String(text)) = object.get("description") {
                let projected = shorten_description(&project_description(text), max_chars);
                if projected.is_empty() {
                    object.remove("description");
                } else {
                    object.insert("description".to_string(), Value::String(projected));
                }
            }
            for (key, nested) in object.iter_mut() {
                if key != "examples" {
                    trim_descriptions(nested, max_chars);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                trim_descriptions(item, max_chars);
            }
        }
        _ => {}
    }
}

/// Reduce one description to its leading paragraph, within a character budget.
///
/// Falls back through three steps: keep the text if it already fits, else keep
/// the leading paragraph, else keep whole sentences from that paragraph while
/// they fit. A description that cannot be cut at a sentence boundary is left
/// whole rather than truncated mid-clause — a half-sentence about a market
/// convention is worse than a long one.
pub(super) fn shorten_description(text: &str, max_chars: usize) -> String {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return text.to_string();
    }

    let (lead, sections) = split_leading_prose(text);
    let mut kept = shorten_prose(lead, max_chars);
    for section in sections {
        // A citation earns its place by being compact. The discriminating
        // case — which ISDA section each accepted spelling implements — is
        // folded into the lead prose by `flatten_const_unions` and preserved
        // there; a long reference block hanging off a single field is
        // background the caller can read in the canonical artifact.
        if mentions_a_standard(section) && section.chars().count() <= max_chars {
            kept.push_str("\n\n");
            kept.push_str(section.trim_end());
        }
    }
    kept
}

/// Split a description into its lead prose and its `# Heading` sections.
pub(super) fn split_leading_prose(text: &str) -> (&str, Vec<&str>) {
    let mut boundaries: Vec<usize> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("# ") {
            boundaries.push(offset);
        }
        offset += line.len();
    }
    let Some(&first) = boundaries.first() else {
        return (text, Vec::new());
    };
    let mut sections = Vec::with_capacity(boundaries.len());
    for (index, &start) in boundaries.iter().enumerate() {
        let end = boundaries.get(index + 1).copied().unwrap_or(text.len());
        sections.push(&text[start..end]);
    }
    (&text[..first], sections)
}

/// Whether a section cites a standards body or market convention.
///
/// These citations are what make a wire contract auditable — which ISDA
/// section a day count implements, which ISO code a value corresponds to — so
/// they are never treated as elaboration and never dropped for length.
pub(super) fn mentions_a_standard(section: &str) -> bool {
    const BODIES: &[&str] = &[
        "ISDA", "ISO", "ICMA", "ISMA", "IFRS", "FpML", "SIFMA", "Basel", "AFB", "FINRA", "IMM",
        "GAAP", "EMIR", "CFTC", "BCBS", "IOSCO",
    ];
    BODIES.iter().any(|body| section.contains(body))
}

/// Reduce free prose to its opening paragraph, plus any paragraph that cites a
/// standard, within a character budget.
///
/// The citation carve-out matters here as much as at section level: folded
/// enum glosses arrive as a paragraph, not under a heading, and they are where
/// a flattened unit enum keeps its ISDA and ISO references.
pub(super) fn shorten_prose(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut paragraphs = text.split("\n\n");
    let leading = paragraphs.next().unwrap_or(text).trim();
    let cited: Vec<&str> = paragraphs
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty() && mentions_a_standard(paragraph))
        .collect();

    let head = shorten_paragraph(leading, max_chars);
    if cited.is_empty() {
        return head;
    }
    let mut kept = head;
    for paragraph in cited {
        kept.push_str("\n\n");
        kept.push_str(paragraph);
    }
    kept
}

/// Reduce one paragraph to whole sentences within a character budget.
pub(super) fn shorten_paragraph(leading: &str, max_chars: usize) -> String {
    if leading.chars().count() <= max_chars {
        return leading.to_string();
    }

    let mut kept = String::new();
    let mut rest = leading;
    while let Some(end) = sentence_end(rest) {
        let (sentence, remainder) = rest.split_at(end);
        if kept.chars().count() + sentence.chars().count() > max_chars {
            break;
        }
        kept.push_str(sentence);
        rest = remainder;
    }

    let kept = kept.trim();
    if kept.is_empty() {
        leading.to_string()
    } else {
        kept.to_string()
    }
}

/// Byte offset just past the first sentence terminator followed by a space.
///
/// Abbreviations inside these descriptions are written without a following
/// space (`e.g.` appears mid-clause), so requiring whitespace after the period
/// avoids cutting one in half.
pub(super) fn sentence_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    for (offset, character) in text.char_indices() {
        if !matches!(character, '.' | '!' | '?') {
            continue;
        }
        let next = offset + character.len_utf8();
        match bytes.get(next) {
            None => return Some(text.len()),
            Some(following) if following.is_ascii_whitespace() => return Some(next + 1),
            _ => {}
        }
    }
    None
}

/// Close objects, require every property, and drop non-portable keywords.
///
/// Optionality moves from "absent from `required`" into the type as a nullable
/// union, which is what strict structured-output modes expect. Tuples become
/// fixed-length arrays because `prefixItems` is outside every provider subset;
/// the positional meaning survives in the description.
pub(super) fn apply_strict_shape(node: &mut Value) {
    match node {
        Value::Object(object) => {
            for keyword in NON_PORTABLE_KEYWORDS {
                if let Some(default) = object.remove(*keyword) {
                    if *keyword == "default" {
                        let note = format!("Defaults to `{default}` when omitted upstream.");
                        let existing = object
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let combined = if existing.is_empty() {
                            note
                        } else {
                            format!("{existing}\n\n{note}")
                        };
                        object.insert("description".to_string(), Value::String(combined));
                    }
                }
            }

            // `type: [T, "null"]` and `anyOf: [T, null]` mean the same thing;
            // one spelling is easier for a generator to follow.
            if let Some(Value::Array(types)) = object.get("type") {
                if types.len() == 2 && types.iter().any(|entry| entry == "null") {
                    if let Some(concrete) = types.iter().find(|entry| *entry != "null").cloned() {
                        object.insert("type".to_string(), concrete);
                    }
                }
            }

            if let Some(Value::Array(prefix_items)) = object.remove("prefixItems") {
                let count = prefix_items.len();
                // Draft 2020-12 has no array form of `items`, so a tuple becomes
                // a length-pinned array whose element schema admits every
                // position. Positional typing is lost; the description keeps it
                // readable.
                let element = match prefix_items.first() {
                    Some(first) if prefix_items.iter().all(|item| item == first) => first.clone(),
                    Some(_) => {
                        let mut union = Map::new();
                        union.insert("anyOf".to_string(), Value::Array(prefix_items));
                        Value::Object(union)
                    }
                    None => Value::Object(Map::new()),
                };
                object.insert("items".to_string(), element);
                object.insert(
                    "minItems".to_string(),
                    Value::Number(serde_json::Number::from(count)),
                );
                object.insert(
                    "maxItems".to_string(),
                    Value::Number(serde_json::Number::from(count)),
                );
                let note = format!("Fixed-length array of {count} positional values.");
                let existing = object
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let combined = if existing.is_empty() {
                    note
                } else {
                    format!("{existing}\n\n{note}")
                };
                object.insert("description".to_string(), Value::String(combined));
            }

            if let Some(Value::Object(properties)) = object.get("properties") {
                let names: Vec<Value> = properties.keys().cloned().map(Value::String).collect();
                object.insert("additionalProperties".to_string(), Value::Bool(false));
                object.insert("required".to_string(), Value::Array(names));
            }

            for nested in object.values_mut() {
                apply_strict_shape(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                apply_strict_shape(item);
            }
        }
        _ => {}
    }
}
