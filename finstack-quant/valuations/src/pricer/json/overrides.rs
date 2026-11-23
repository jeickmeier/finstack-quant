//! Pricing-override JSON patching, validation, and envelope decoding.

use crate::instruments::Instrument;
use crate::instruments::envelope::{MAX_JSON_BYTES, instrument_load_error};
use crate::instruments::json_loader::InstrumentEnvelope;
use crate::instruments::pricing_overrides::{InstrumentPricingOverrides, MetricPricingOverrides};
use finstack_quant_core::Error;
use finstack_quant_core::contract::{LoadLimits, parse_json_value};
use serde_json::Value;

/// Suffix `[instrument=<id>]` to an error message when an id is known.
pub(super) fn with_id_suffix(message: String, id: Option<&str>) -> String {
    match id {
        Some(id) => format!("{message} [instrument={id}]"),
        None => message,
    }
}

/// Validate a JSON override patch as an object matching `T`'s schema without
/// materializing absent defaults into the patch (key presence determines which
/// stored fields are replaced).
pub(super) fn validate_override_patch<T: serde::de::DeserializeOwned>(
    patch: &Value,
    field_name: &str,
    label: &str,
    id: Option<&str>,
) -> finstack_quant_core::Result<()> {
    if !patch.is_object() {
        return Err(Error::Validation(with_id_suffix(
            format!("{label} must be an object"),
            id,
        )));
    }
    let _: T = serde_json::from_value(patch.clone()).map_err(|error| {
        Error::Validation(with_id_suffix(
            format!("invalid {field_name} JSON: {error}"),
            id,
        ))
    })?;
    Ok(())
}

pub(super) fn parse_metric_patch(
    overrides_json: &str,
    id: Option<&str>,
) -> finstack_quant_core::Result<Value> {
    let patch: Value = serde_json::from_str(overrides_json).map_err(|error| {
        Error::Validation(with_id_suffix(
            format!("invalid metric_pricing_overrides JSON: {error}"),
            id,
        ))
    })?;
    validate_override_patch::<MetricPricingOverrides>(
        &patch,
        "metric_pricing_overrides",
        "metric pricing overrides",
        id,
    )?;
    Ok(patch)
}

pub(super) fn apply_validated_override_patch<T>(
    current: &mut T,
    patch: &Value,
    field_name: &str,
    id: &str,
    merge: fn(&mut Value, &Value),
) -> finstack_quant_core::Result<()>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let error = |error: serde_json::Error| {
        Error::Validation(with_id_suffix(
            format!("invalid {field_name} JSON: {error}"),
            Some(id),
        ))
    };
    let mut value = serde_json::to_value(&*current).map_err(error)?;
    merge(&mut value, patch);
    *current = serde_json::from_value(value).map_err(error)?;
    Ok(())
}

pub(super) fn apply_metric_pricing_overrides(
    instrument: &mut dyn Instrument,
    overrides_json: &str,
) -> finstack_quant_core::Result<()> {
    let patch = parse_metric_patch(overrides_json, Some(instrument.id()))?;
    apply_metric_pricing_overrides_value(instrument, &patch)
}

pub(super) fn apply_metric_pricing_overrides_value(
    instrument: &mut dyn Instrument,
    patch: &Value,
) -> finstack_quant_core::Result<()> {
    let id = instrument.id().to_owned();
    let current = instrument
        .get_metric_pricing_overrides_mut()
        .ok_or_else(|| {
            Error::Validation(with_id_suffix(
                "instrument does not support metric pricing overrides".into(),
                Some(&id),
            ))
        })?;
    apply_validated_override_patch(
        current,
        patch,
        "metric_pricing_overrides",
        &id,
        merge_metric_override_values,
    )
}

pub(super) fn apply_instrument_pricing_overrides(
    instrument: &mut dyn Instrument,
    patch: &Value,
) -> finstack_quant_core::Result<()> {
    let id = instrument.id().to_owned();
    let current = instrument
        .get_instrument_pricing_overrides_mut()
        .ok_or_else(|| {
            Error::Validation(with_id_suffix(
                "instrument does not support instrument pricing overrides".into(),
                Some(&id),
            ))
        })?;
    validate_override_patch::<InstrumentPricingOverrides>(
        patch,
        "instrument_pricing_overrides",
        "instrument pricing overrides",
        Some(&id),
    )?;
    apply_validated_override_patch(
        current,
        patch,
        "instrument_pricing_overrides",
        &id,
        merge_override_values,
    )
}

pub(super) fn merge_metric_override_values(current: &mut Value, patch: &Value) {
    let (Some(current), Some(patch)) = (current.as_object_mut(), patch.as_object()) else {
        *current = patch.clone();
        return;
    };
    for (key, value) in patch {
        if key == "bump_config"
            && value.is_object()
            && let Some(existing) = current.get_mut(key)
            && existing.is_object()
        {
            merge_override_values(existing, value);
        } else {
            current.insert(key.clone(), value.clone());
        }
    }
}

pub(super) fn merge_override_values(current: &mut Value, patch: &Value) {
    if let (Some(current), Some(patch)) = (current.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            merge_override_values(current.entry(key.clone()).or_insert(Value::Null), value);
        }
    } else {
        *current = patch.clone();
    }
}

pub(super) fn raw_metric_overrides_mut(document: &mut Value) -> Option<&mut Value> {
    let pointer = if document.pointer("/instrument/type").and_then(Value::as_str)
        == Some("structured_credit_tranche")
    {
        "/instrument/spec/deal/metric_pricing_overrides"
    } else {
        "/instrument/spec/metric_pricing_overrides"
    };
    document.pointer_mut(pointer)
}

pub(super) fn decode_envelope_with_metric_overrides(
    json: &str,
    metric_pricing_overrides: Option<&str>,
) -> finstack_quant_core::Result<InstrumentEnvelope> {
    let limits = LoadLimits::default().with_max_bytes(MAX_JSON_BYTES);
    let Some(patch_str) = metric_pricing_overrides else {
        return InstrumentEnvelope::decode_bytes(json.as_bytes(), &limits)
            .map_err(instrument_load_error);
    };

    let mut document = parse_json_value(json.as_bytes(), &limits).map_err(instrument_load_error)?;
    let id = document
        .pointer("/instrument/spec/id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let patch = parse_metric_patch(patch_str, id.as_deref())?;

    // If the wire envelope carries an un-deserializable stored
    // `metric_pricing_overrides` object that the caller's patch repairs before
    // validation, patch that object in place so decoding can proceed to the
    // canonical typed override path below.
    if let Some(existing) = raw_metric_overrides_mut(&mut document)
        && existing.is_object()
        && serde_json::from_value::<MetricPricingOverrides>(existing.clone()).is_err()
    {
        merge_metric_override_values(existing, &patch);
    }

    let mut envelope =
        InstrumentEnvelope::decode_value(document, &limits).map_err(instrument_load_error)?;
    apply_metric_pricing_overrides_value(envelope.instrument.as_dyn_instrument_mut(), &patch)?;
    Ok(envelope)
}
