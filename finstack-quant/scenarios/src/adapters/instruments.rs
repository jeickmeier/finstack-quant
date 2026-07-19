//! Instrument-level shock adapters.
//!
//! Applies price and spread shocks to instrument collections via pricing overrides.
//! When instruments support `get_scenario_pricing_overrides_mut()`, shocks are applied functionally;
//! otherwise they are stored as metadata attributes for downstream processing.

use crate::adapters::traits::ScenarioEffect;
use crate::warning::Warning;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;

/// Result of applying one instrument shock.
pub(crate) struct InstrumentShockOutcome {
    /// Number of instruments mutated by the shock.
    pub(crate) count: usize,
    /// Zero-based portfolio indices of the mutated instruments.
    pub(crate) changed_indices: Vec<usize>,
    /// Non-fatal warnings raised while routing the shock.
    pub(crate) warnings: Vec<Warning>,
}

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
) -> InstrumentShockOutcome
where
    M: Fn(&Box<dyn Instrument>) -> bool,
{
    let delta = kind.internal_value(raw_value);
    let mut changed_indices = Vec::new();
    let mut warnings = Vec::new();

    for (index, instrument) in instruments.iter_mut().enumerate() {
        if !matcher(instrument) {
            continue;
        }

        match kind {
            ShockKind::Price => {
                if let Some(overrides) = instrument.get_scenario_pricing_overrides_mut() {
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
                        .get_scenario_pricing_overrides_mut()
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
        changed_indices.push(index);
    }

    InstrumentShockOutcome {
        count: changed_indices.len(),
        changed_indices,
        warnings,
    }
}

/// Apply a percentage price shock to instruments matching the provided types.
pub(crate) fn apply_instrument_type_price_shock(
    instruments: &mut [Box<dyn Instrument>],
    instrument_types: &[InstrumentType],
    pct: f64,
) -> InstrumentShockOutcome {
    apply_shock(
        instruments,
        |inst| instrument_types.contains(&inst.key()),
        ShockKind::Price,
        pct,
    )
}

/// Apply a spread shock to instruments matching the provided types.
pub(crate) fn apply_instrument_type_spread_shock(
    instruments: &mut [Box<dyn Instrument>],
    instrument_types: &[InstrumentType],
    bp: f64,
) -> InstrumentShockOutcome {
    apply_shock(
        instruments,
        |inst| instrument_types.contains(&inst.key()),
        ShockKind::Spread,
        bp,
    )
}

/// Apply a percentage price shock to instruments matching the provided attributes.
pub(crate) fn apply_instrument_attr_price_shock(
    instruments: &mut [Box<dyn Instrument>],
    attrs: &indexmap::IndexMap<String, String>,
    pct: f64,
) -> InstrumentShockOutcome {
    let filters = normalise_filters(attrs);
    let mut outcome = apply_shock(
        instruments,
        |inst| matches_attr_filter(inst.attributes(), &filters),
        ShockKind::Price,
        pct,
    );
    if outcome.count == 0 {
        outcome.warnings.push(Warning::InstrumentShockNoMatch {
            filter_desc: format!("{attrs:?}"),
        });
    }
    outcome
}

/// Apply a spread shock to instruments matching the provided attributes.
pub(crate) fn apply_instrument_attr_spread_shock(
    instruments: &mut [Box<dyn Instrument>],
    attrs: &indexmap::IndexMap<String, String>,
    bp: f64,
) -> InstrumentShockOutcome {
    let filters = normalise_filters(attrs);
    let mut outcome = apply_shock(
        instruments,
        |inst| matches_attr_filter(inst.attributes(), &filters),
        ShockKind::Spread,
        bp,
    );
    if outcome.count == 0 {
        outcome.warnings.push(Warning::InstrumentShockNoMatch {
            filter_desc: format!("{attrs:?}"),
        });
    }
    outcome
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
