use finstack_quant_core::wire::NonFiniteFields;

/// Assert that `T::NON_FINITE_FIELDS` names exactly the keys that
/// serialize as sentinel strings. Callers pass a value whose `f64` fields
/// are all `NaN`: fields routed through `core::wire::non_finite_f64` come
/// out as strings, plain `f64` fields as `null`.
pub(crate) fn assert_non_finite_fields<T: serde::Serialize + NonFiniteFields>(value: &T) {
    let json = serde_json::to_value(value).expect("serialize");
    let object = json.as_object().expect("struct serializes as an object");
    let mut actual: Vec<&str> = object
        .iter()
        .filter(|(_, v)| v.is_string())
        .map(|(k, _)| k.as_str())
        .collect();
    actual.sort_unstable();
    let mut expected = T::NON_FINITE_FIELDS.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
