use super::MetricId;
use std::fmt::Write as _;

impl MetricId {
    /// Build a flattened composite metric key from a base ID and components.
    ///
    /// The wire format is `base::component[::component...]` and is also the
    /// human-facing form: ordinary identifiers such as `USD-OIS`, `usd_ois`
    /// or `10y` are written literally, so a key reads exactly as documented
    /// (`bucketed_dv01::USD-OIS::10y`, `pv01::USD-OIS`). Only what would
    /// break the framing is escaped as `_xHH`: the `:` delimiter byte, an
    /// underscore immediately followed by `x` (so a literal `_x` can never be
    /// mistaken for an escape marker), and the reserved `_empty` spelling. The
    /// empty component is encoded as `_empty`. [`Self::decode_components`] is
    /// the exact inverse. Persisted keys must use this canonical representation.
    ///
    /// # Arguments
    ///
    /// * `base` - Base metric the components qualify (for example
    ///   `MetricId::BucketedDv01` or `MetricId::Pv01`).
    /// * `components` - Ordered coordinate labels (curve ids, bucket labels,
    ///   row/column labels) appended after the base, in order.
    pub fn composite(base: &MetricId, components: &[&str]) -> Self {
        let mut key = String::with_capacity(base.as_str().len() + components.len() * 8);
        key.push_str(base.as_str());

        for component in components {
            key.push_str("::");
            encode_component(&mut key, component);
        }
        Self(std::borrow::Cow::Owned(key))
    }

    /// Decode this metric's components when it is a composite of `base`.
    ///
    /// Returns `None` for the scalar base metric or a different base.
    /// Canonical escape markers decode to their literal coordinates. Wire
    /// parsing rejects noncanonical spellings before a key enters a result.
    ///
    /// # Arguments
    ///
    /// * `base` - Base metric whose composite keys are being decoded (for
    ///   example `MetricId::BucketedDv01`); the key must start with
    ///   `"{base}::"` or `None` is returned.
    pub fn decode_components(&self, base: &MetricId) -> Option<Vec<String>> {
        let suffix = self
            .as_str()
            .strip_prefix(base.as_str())?
            .strip_prefix("::")?;

        suffix.split("::").map(decode_component).collect()
    }

    pub(super) fn validate_wire(id: &str) -> finstack_quant_core::Result<()> {
        let invalid = || {
            finstack_quant_core::Error::Validation(format!(
                "noncanonical composite metric key {id:?}; construct coordinates with MetricId::composite"
            ))
        };
        let components: Vec<String> = id
            .split("::")
            .map(decode_component)
            .collect::<Option<_>>()
            .ok_or_else(invalid)?;
        let mut encoded = String::with_capacity(id.len());
        for (index, component) in components.iter().enumerate() {
            if index > 0 {
                encoded.push_str("::");
            }
            encode_component(&mut encoded, component);
        }
        if encoded != id {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(super) fn encode_component(key: &mut String, component: &str) {
    if component.is_empty() {
        key.push_str("_empty");
        return;
    }
    if component == "_empty" {
        key.push_str("_x5fempty");
        return;
    }
    for (index, ch) in component.char_indices() {
        let escape =
            ch == ':' || (ch == '_' && component[index + ch.len_utf8()..].starts_with('x'));
        if escape {
            // Both `:` and `_` are single-byte ASCII.
            let _ = write!(key, "_x{:02x}", ch as u32);
        } else {
            key.push(ch);
        }
    }
}

fn decode_component(component: &str) -> Option<String> {
    if component == "_empty" {
        return Some(String::new());
    }

    let bytes = component.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + 3 < bytes.len() && bytes[index] == b'_' && bytes[index + 1] == b'x' {
            let (Some(high), Some(low)) =
                (decode_hex(bytes[index + 2]), decode_hex(bytes[index + 3]))
            else {
                return None;
            };
            decoded.push((high << 4) | low);
            index += 4;
        } else if bytes[index] == b'_' && index + 1 < bytes.len() && bytes[index + 1] == b'x' {
            return None;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
