use serde_json::Value;

/// Recursively merge `overlay` into `base` (object-wise). Arrays and
/// scalars are replaced.
///
/// # Semantics
///
/// * Two objects are merged key-by-key; nested objects recurse.
/// * Arrays, numbers, strings, booleans, and `null` in `overlay` *replace*
///   the value at the same path in `base`. There is no "deep array merge."
/// * `null` in `overlay` does **not** delete the corresponding key — it
///   replaces the value with `Value::Null`. Downstream parsers (which
///   treat `null` as "missing / fall back to default") therefore observe
///   the same shape they would have without the overlay key. If you need
///   to *force* a value to be absent, omit the key from your overlay
///   entirely.
pub fn merge_json(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (k, v) in overlay_map {
                match base_map.get_mut(k) {
                    Some(base_child) => merge_json(base_child, v),
                    None => {
                        base_map.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (base_slot, overlay_value) => {
            *base_slot = overlay_value.clone();
        }
    }
}
