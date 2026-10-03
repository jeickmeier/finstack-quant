//! Uniform `__repr__` rendering for value-object wrappers.
//!
//! Most `Py*` value objects wrap a `Serialize` Rust type, so the fields worth
//! showing are exactly the fields the wire format already names. Rendering the
//! repr from that one source keeps every class's repr consistent — and keeps it
//! correct when a field is added, because there is no second list to update.
//!
//! Collections are summarised rather than expanded: a repr is an identification
//! aid, and a hundred-element vector in a notebook cell is noise. Use
//! `to_json()` or a DataFrame exit when the contents matter.

use serde_json::Value;

/// Maximum number of object fields rendered before the tail is elided.
const MAX_FIELDS: usize = 6;

/// Render `Name(field=value, ...)` from a value's serde representation.
///
/// Falls back to `Name(...)` when the value does not serialize to a JSON
/// object, so a repr can never fail or panic.
pub(crate) fn repr_from_serde<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    let Ok(Value::Object(map)) = serde_json::to_value(value) else {
        return format!("{type_name}(...)");
    };
    format!("{type_name}({})", render_fields(&map))
}

/// Render externally tagged enums using the same bounded field renderer.
pub(crate) fn enum_repr_from_serde<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(variant)) => format!("{type_name}.{}", variant.to_ascii_uppercase()),
        Ok(Value::Object(map)) if map.len() == 1 => {
            let Some((variant, payload)) = map.iter().next() else {
                return format!("{type_name}(...)");
            };
            let fields = match payload {
                Value::Object(fields) => render_fields(fields),
                other => render(other),
            };
            format!("{type_name}.{variant}({fields})")
        }
        _ => format!("{type_name}(...)"),
    }
}

fn render_fields(map: &serde_json::Map<String, Value>) -> String {
    let mut parts: Vec<String> = map
        .iter()
        .take(MAX_FIELDS)
        .map(|(k, v)| format!("{k}={}", render(v)))
        .collect();
    if map.len() > MAX_FIELDS {
        parts.push("...".to_string());
    }
    parts.join(", ")
}

/// Render one field value, summarising anything unbounded.
fn render(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("{s:?}"),
        Value::Array(items) => format!("[<{} items>]", items.len()),
        Value::Object(fields) => format!("{{<{} fields>}}", fields.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_named_fields() {
        let value = serde_json::json!({"a": 1, "b": "x", "c": true});
        assert_eq!(repr_from_serde("T", &value), r#"T(a=1, b="x", c=True)"#);
    }

    #[test]
    fn summarises_collections() {
        let value = serde_json::json!({"xs": [1, 2, 3], "m": {"k": 1}});
        assert_eq!(
            repr_from_serde("T", &value),
            "T(m={<1 fields>}, xs=[<3 items>])"
        );
    }

    #[test]
    fn elides_beyond_the_field_cap() {
        let value = serde_json::json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5, "f": 6, "g": 7
        });
        let rendered = repr_from_serde("T", &value);
        assert!(rendered.ends_with(", ...)"), "{rendered}");
    }

    #[test]
    fn falls_back_for_non_objects() {
        assert_eq!(repr_from_serde("T", &[1, 2, 3]), "T(...)");
    }
}
