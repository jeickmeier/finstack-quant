//! Shared JSON loading helpers for strict persisted contracts.

use serde::de::DeserializeOwned;

use super::{ContractError, Diagnostic, LoadLimits, LoadPhase, Severity, ValidationReport};

/// Check encoded-size and JSON nesting-depth limits without building a tree.
///
/// The depth pass is lexical and allocation-free. It counts object and array
/// delimiters outside strings, honors JSON string escapes, and deliberately
/// does not validate JSON syntax. Callers must still parse the document once
/// into their target type after this check succeeds.
///
/// # Arguments
///
/// * `bytes` - Complete JSON document whose encoded size and lexical nesting
///   depth are checked.
/// * `limits` - Resource policy providing the maximum byte length and nesting
///   depth.
///
/// # Errors
///
/// Returns [`ContractError::LimitExceeded`] when the byte or JSON-depth bound
/// is exceeded. Malformed JSON within those bounds is accepted here and must
/// be rejected by the caller's parser.
pub fn check_json_limits(bytes: &[u8], limits: &LoadLimits) -> Result<(), ContractError> {
    if bytes.len() > limits.max_bytes {
        return Err(ContractError::LimitExceeded {
            what: "bytes",
            found: bytes.len(),
            limit: limits.max_bytes,
        });
    }

    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                maximum = maximum.max(depth);
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    if maximum > limits.max_depth {
        return Err(ContractError::LimitExceeded {
            what: "JSON depth",
            found: maximum,
            limit: limits.max_depth,
        });
    }
    Ok(())
}

/// Parse bounded UTF-8 JSON bytes into a generic value.
///
/// The encoded byte length and resulting container depth are checked against
/// `limits`. JSON syntax failures are returned as structured parse diagnostics
/// with their source line and column.
///
/// # Arguments
///
/// * `bytes` - Complete UTF-8 JSON document to parse.
/// * `limits` - Resource policy bounding encoded bytes, nesting depth, and
///   retained diagnostics.
///
/// # Errors
///
/// Parse UTF-8 bytes as a [`serde_json::Value`] under explicit resource bounds,
/// rejecting duplicate object keys at any nesting level.
///
/// # Arguments
///
/// * `bytes` - Raw UTF-8 JSON payload.
/// * `limits` - Resource limits enforced before JSON parsing begins.
///
/// # Errors
///
/// Returns [`ContractError::LimitExceeded`] when the byte or depth bound is
/// exceeded, or [`ContractError::Report`] when JSON parsing fails or an object
/// contains duplicate keys.
pub fn parse_json_value(
    bytes: &[u8],
    limits: &LoadLimits,
) -> Result<serde_json::Value, ContractError> {
    check_json_limits(bytes, limits)?;
    let mut duplicate_pointer = None;
    let mut pointer = String::new();
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let result = serde::de::DeserializeSeed::deserialize(
        UniqueKeyValueSeed {
            pointer: &mut pointer,
            duplicate_pointer: &mut duplicate_pointer,
        },
        &mut de,
    )
    .and_then(|value| {
        de.end()?;
        Ok(value)
    });
    result.map_err(|error| {
        let mut report = ValidationReport::default();
        report.push_bounded(
            limits,
            Diagnostic::from_parse_error(&error, duplicate_pointer),
        );
        ContractError::Report(Box::new(report))
    })
}

struct UniqueKeyValueSeed<'a> {
    pointer: &'a mut String,
    duplicate_pointer: &'a mut Option<String>,
}

impl<'de> serde::de::DeserializeSeed<'de> for UniqueKeyValueSeed<'_> {
    type Value = serde_json::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for UniqueKeyValueSeed<'_> {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("any valid JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Number(v.into()))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .ok_or_else(|| serde::de::Error::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
        Ok(serde_json::Value::String(v))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::de::DeserializeSeed::deserialize(self, deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut items = Vec::new();
        let mut index = 0usize;
        loop {
            let saved_len = self.pointer.len();
            self.pointer.push('/');
            self.pointer.push_str(&index.to_string());
            let next = seq.next_element_seed(UniqueKeyValueSeed {
                pointer: self.pointer,
                duplicate_pointer: self.duplicate_pointer,
            });
            self.pointer.truncate(saved_len);
            match next? {
                Some(value) => {
                    items.push(value);
                    index += 1;
                }
                None => break,
            }
        }
        Ok(serde_json::Value::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut entries = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let saved_len = self.pointer.len();
            append_json_pointer_segment(self.pointer, &key);
            if entries.contains_key(&key) {
                *self.duplicate_pointer = Some(self.pointer.clone());
                self.pointer.truncate(saved_len);
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            let value = map.next_value_seed(UniqueKeyValueSeed {
                pointer: self.pointer,
                duplicate_pointer: self.duplicate_pointer,
            });
            self.pointer.truncate(saved_len);
            entries.insert(key, value?);
        }
        Ok(serde_json::Value::Object(entries))
    }
}

fn append_json_pointer_segment(pointer: &mut String, segment: &str) {
    pointer.push('/');
    for ch in segment.chars() {
        match ch {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            _ => pointer.push(ch),
        }
    }
}

/// Deserialize a parsed JSON value with a structured shape diagnostic.
///
/// # Arguments
///
/// * `value` - Parsed JSON document to deserialize into `T`.
/// * `limits` - Resource policy bounding retained diagnostics.
///
/// # Errors
///
/// Returns [`ContractError::Report`] with code
/// `"contract/structure-invalid"` when the document does not match `T`.
pub fn deserialize_json_value<T>(
    value: serde_json::Value,
    limits: &LoadLimits,
) -> Result<T, ContractError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|error| {
        let mut report = ValidationReport::default();
        report.push_bounded(
            limits,
            Diagnostic::new(
                "contract/structure-invalid",
                LoadPhase::Structure,
                Severity::Error,
                error.to_string(),
            ),
        );
        ContractError::Report(Box::new(report))
    })
}
