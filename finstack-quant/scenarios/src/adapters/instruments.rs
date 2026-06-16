//! Instrument-level shock adapters.
//!
//! Applies price and spread shocks to instrument collections via pricing overrides.
//! When instruments support `scenario_overrides_mut()`, shocks are applied functionally;
//! otherwise they are stored as metadata attributes for downstream processing.

use crate::adapters::traits::ScenarioEffect;
use crate::warning::Warning;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;

fn accumulate_optional_shock(current: Option<f64>, delta: f64) -> f64 {
    current.unwrap_or(0.0) + delta
}

/// Accumulate a shock into an instrument's metadata map.
fn accumulate_meta_shock(attrs: &mut Attributes, key: &str, delta: f64) {
    let current = attrs
        .meta
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    attrs
        .meta
        .insert(key.to_string(), format!("{}", current + delta));
}

fn instrument_label(attrs: &Attributes) -> String {
    attrs
        .meta
        .get("id")
        .or_else(|| attrs.meta.get("instrument_id"))
        .or_else(|| attrs.meta.get("name"))
        .cloned()
        .unwrap_or_else(|| "<unidentified>".to_string())
}

/// Generate a price-shock effect by instrument types.
pub(crate) fn instrument_price_by_type_effects(
    instrument_types: &[InstrumentType],
    pct: f64,
) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::InstrumentPriceShock {
        types: Some(instrument_types.to_vec()),
        attrs: None,
        pct,
    }]
}

/// Generate a price-shock effect by attribute filter.
pub(crate) fn instrument_price_by_attr_effects(
    attrs: &indexmap::IndexMap<String, String>,
    pct: f64,
) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::InstrumentPriceShock {
        types: None,
        attrs: Some(attrs.clone()),
        pct,
    }]
}

/// Generate a spread-shock effect by instrument types.
pub(crate) fn instrument_spread_by_type_effects(
    instrument_types: &[InstrumentType],
    bp: f64,
) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::InstrumentSpreadShock {
        types: Some(instrument_types.to_vec()),
        attrs: None,
        bp,
    }]
}

/// Generate a spread-shock effect by attribute filter.
pub(crate) fn instrument_spread_by_attr_effects(
    attrs: &indexmap::IndexMap<String, String>,
    bp: f64,
) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::InstrumentSpreadShock {
        types: None,
        attrs: Some(attrs.clone()),
        bp,
    }]
}

/// Kind of instrument shock: price (percent) or spread (bp).
#[derive(Clone, Copy)]
enum ShockKind {
    Price,
    Spread,
}

impl ShockKind {
    fn meta_key(self) -> &'static str {
        match self {
            ShockKind::Price => "scenario_price_shock_pct",
            ShockKind::Spread => "scenario_spread_shock_bp",
        }
    }

    fn label(self) -> &'static str {
        match self {
            ShockKind::Price => "price",
            ShockKind::Spread => "spread",
        }
    }

    fn internal_value(self, raw: f64) -> f64 {
        match self {
            ShockKind::Price => raw / 100.0,
            ShockKind::Spread => raw,
        }
    }
}

/// Apply a shock to every instrument matching `matcher`.
fn apply_shock<M>(
    instruments: &mut [Box<dyn Instrument>],
    matcher: M,
    kind: ShockKind,
    raw_value: f64,
) -> (usize, Vec<Warning>)
where
    M: Fn(&Box<dyn Instrument>) -> bool,
{
    let delta = kind.internal_value(raw_value);
    let mut count = 0;
    let mut warnings = Vec::new();

    for instrument in instruments.iter_mut() {
        if !matcher(instrument) {
            continue;
        }

        match kind {
            ShockKind::Price => {
                if let Some(overrides) = instrument.scenario_overrides_mut() {
                    overrides.scenario_price_shock_pct = Some(accumulate_optional_shock(
                        overrides.scenario_price_shock_pct,
                        delta,
                    ));
                } else {
                    let label = instrument_label(instrument.attributes());
                    let type_name = format!("{:?}", instrument.key());
                    accumulate_meta_shock(instrument.attributes_mut(), kind.meta_key(), delta);
                    warnings.push(Warning::InstrumentShockFallback {
                        shock_kind: kind.label().to_string(),
                        inst_type: type_name,
                        label,
                    });
                }
            }
            ShockKind::Spread => {
                // First-class path: pricers that consume the shock exactly
                // (e.g. vanilla bonds via flat Z-spread repricing) accumulate
                // it in the scenario overrides. Everything else falls back to
                // metadata tagging with an explicit warning so the shock never
                // silently no-ops.
                let routed = instrument.scenario_spread_shock_supported()
                    && instrument
                        .scenario_overrides_mut()
                        .map(|overrides| {
                            overrides.scenario_spread_shock_bp = Some(accumulate_optional_shock(
                                overrides.scenario_spread_shock_bp,
                                delta,
                            ));
                        })
                        .is_some();
                if !routed {
                    let label = instrument_label(instrument.attributes());
                    let type_name = format!("{:?}", instrument.key());
                    accumulate_meta_shock(instrument.attributes_mut(), kind.meta_key(), delta);
                    warnings.push(Warning::InstrumentShockFallback {
                        shock_kind: kind.label().to_string(),
                        inst_type: type_name,
                        label,
                    });
                }
            }
        }
        count += 1;
    }

    (count, warnings)
}

/// Apply a percentage price shock to instruments matching the provided types.
pub fn apply_instrument_type_price_shock(
    instruments: &mut [Box<dyn Instrument>],
    instrument_types: &[InstrumentType],
    pct: f64,
) -> (usize, Vec<Warning>) {
    apply_shock(
        instruments,
        |inst| instrument_types.contains(&inst.key()),
        ShockKind::Price,
        pct,
    )
}

/// Apply a spread shock to instruments matching the provided types.
pub fn apply_instrument_type_spread_shock(
    instruments: &mut [Box<dyn Instrument>],
    instrument_types: &[InstrumentType],
    bp: f64,
) -> (usize, Vec<Warning>) {
    apply_shock(
        instruments,
        |inst| instrument_types.contains(&inst.key()),
        ShockKind::Spread,
        bp,
    )
}

/// Apply a percentage price shock to instruments matching the provided attributes.
pub fn apply_instrument_attr_price_shock(
    instruments: &mut [Box<dyn Instrument>],
    attrs: &indexmap::IndexMap<String, String>,
    pct: f64,
) -> (usize, Vec<Warning>) {
    let filters = normalise_filters(attrs);
    let (count, mut warnings) = apply_shock(
        instruments,
        |inst| matches_attr_filter(inst.attributes(), &filters),
        ShockKind::Price,
        pct,
    );
    if count == 0 {
        warnings.push(Warning::InstrumentShockNoMatch {
            filter_desc: format!("{attrs:?}"),
        });
    }
    (count, warnings)
}

/// Apply a spread shock to instruments matching the provided attributes.
pub fn apply_instrument_attr_spread_shock(
    instruments: &mut [Box<dyn Instrument>],
    attrs: &indexmap::IndexMap<String, String>,
    bp: f64,
) -> (usize, Vec<Warning>) {
    let filters = normalise_filters(attrs);
    let (count, mut warnings) = apply_shock(
        instruments,
        |inst| matches_attr_filter(inst.attributes(), &filters),
        ShockKind::Spread,
        bp,
    );
    if count == 0 {
        warnings.push(Warning::InstrumentShockNoMatch {
            filter_desc: format!("{attrs:?}"),
        });
    }
    (count, warnings)
}

fn matches_attr_filter(attrs: &Attributes, filters: &[(String, String)]) -> bool {
    if filters.is_empty() {
        return true;
    }

    filters.iter().all(|(key, value)| {
        attrs
            .meta
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case(key) && v.eq_ignore_ascii_case(value))
    })
}

fn normalise_filters(attrs: &indexmap::IndexMap<String, String>) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.to_lowercase()))
        .collect()
}
