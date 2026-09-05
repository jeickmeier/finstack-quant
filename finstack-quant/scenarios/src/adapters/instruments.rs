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

fn accumulate_optional_shock(current: Option<f64>, delta: f64, kind: ShockKind) -> f64 {
    let current = current.unwrap_or(0.0);
    match kind {
        ShockKind::Price if current == 0.0 => delta,
        ShockKind::Price => (1.0 + current) * (1.0 + delta) - 1.0,
        ShockKind::Spread => current + delta,
    }
}

/// Accumulate a shock into an instrument's metadata map.
fn accumulate_meta_shock(attrs: &mut Attributes, kind: ShockKind, delta: f64) {
    let current = attrs
        .meta
        .get(kind.meta_key())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    attrs.meta.insert(
        kind.meta_key().to_string(),
        accumulate_optional_shock(Some(current), delta, kind).to_string(),
    );
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
    M: Fn(&dyn Instrument) -> bool,
{
    let delta = kind.internal_value(raw_value);
    let mut changed_indices = Vec::new();
    let mut warnings = Vec::new();

    for (index, instrument) in instruments.iter_mut().enumerate() {
        let changed = apply_shock_to_instrument(instrument, &matcher, kind, delta, &mut warnings);
        if changed > 0 {
            changed_indices.push(index);
        }
    }

    InstrumentShockOutcome {
        count: changed_indices.len(),
        changed_indices,
        warnings,
    }
}

/// Apply at the first matching node on each instrument branch.
///
/// A matching composite receives the shock once and stops descent. Otherwise
/// its self-contained legs are rebuilt recursively with unchanged quantities,
/// so scenario application cannot trigger a rebalance.
fn apply_shock_to_instrument<M>(
    instrument: &mut Box<dyn Instrument>,
    matcher: &M,
    kind: ShockKind,
    delta: f64,
    warnings: &mut Vec<Warning>,
) -> usize
where
    M: Fn(&dyn Instrument) -> bool,
{
    if matcher(instrument.as_ref()) {
        apply_shock_to_matching_instrument(instrument.as_mut(), kind, delta, warnings);
        return 1;
    }

    let Some(composite) = instrument
        .as_any_mut()
        .downcast_mut::<finstack_quant_valuations::instruments::CompositeInstrument>(
    ) else {
        return 0;
    };

    let mut changed = 0usize;
    for leg in &mut composite.spec.legs {
        let Ok(mut child) = leg.instrument.as_ref().clone().into_boxed() else {
            continue;
        };
        let child_changed = apply_shock_to_instrument(&mut child, matcher, kind, delta, warnings);
        if child_changed == 0 {
            continue;
        }
        if let Some(updated) = child.to_instrument_json() {
            *leg.instrument = updated;
            changed += child_changed;
        }
    }
    changed
}

fn apply_shock_to_matching_instrument(
    instrument: &mut dyn Instrument,
    kind: ShockKind,
    delta: f64,
    warnings: &mut Vec<Warning>,
) {
    match kind {
        ShockKind::Price => {
            if let Some(overrides) = instrument.get_scenario_pricing_overrides_mut() {
                overrides.scenario_price_shock_pct = Some(accumulate_optional_shock(
                    overrides.scenario_price_shock_pct,
                    delta,
                    kind,
                ));
            } else {
                record_fallback(instrument, kind, delta, warnings);
            }
        }
        ShockKind::Spread => {
            let routed = instrument.scenario_spread_shock_supported()
                && instrument
                    .get_scenario_pricing_overrides_mut()
                    .map(|overrides| {
                        overrides.scenario_spread_shock_bp = Some(accumulate_optional_shock(
                            overrides.scenario_spread_shock_bp,
                            delta,
                            kind,
                        ));
                    })
                    .is_some();
            if !routed {
                record_fallback(instrument, kind, delta, warnings);
            }
        }
    }
}

fn record_fallback(
    instrument: &mut dyn Instrument,
    kind: ShockKind,
    delta: f64,
    warnings: &mut Vec<Warning>,
) {
    let label = instrument_label(instrument.attributes());
    let instrument_type = instrument.key();
    accumulate_meta_shock(instrument.attributes_mut(), kind, delta);
    warnings.push(Warning::InstrumentShockFallback {
        shock_kind: kind.label().to_string(),
        inst_type: instrument_type,
        label,
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations::instruments::CompositeInstrument;

    #[test]
    fn sequential_price_losses_compound_and_spreads_remain_additive() {
        let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
            finstack_quant_valuations::instruments::Bond::example().expect("bond"),
        )];
        for _ in 0..2 {
            apply_instrument_type_price_shock(&mut instruments, &[InstrumentType::Bond], -60.0);
            apply_instrument_type_spread_shock(&mut instruments, &[InstrumentType::Bond], 25.0);
        }
        let overrides = instruments[0]
            .get_scenario_pricing_overrides()
            .expect("overrides");
        let shock = overrides.scenario_price_shock_pct.expect("price shock");
        assert!((100.0 * (1.0 + shock) - 16.0).abs() < 1e-12);
        assert_eq!(overrides.scenario_spread_shock_bp, Some(50.0));
        let mut attrs = Attributes::new();
        accumulate_meta_shock(&mut attrs, ShockKind::Price, -0.6);
        accumulate_meta_shock(&mut attrs, ShockKind::Price, -0.6);
        let stored: f64 = attrs.meta["scenario_price_shock_pct"]
            .parse()
            .expect("stored shock");
        assert!((100.0 * (1.0 + stored) - 16.0).abs() < 1e-12);
    }

    #[test]
    fn type_shock_descends_into_composite_without_rebalancing() {
        let composite = CompositeInstrument::example().expect("composite example should build");
        let quantities: Vec<f64> = composite
            .state
            .resolved_legs
            .iter()
            .map(|leg| leg.quantity)
            .collect();
        let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(composite)];

        let outcome =
            apply_instrument_type_price_shock(&mut instruments, &[InstrumentType::Equity], 10.0);
        assert_eq!(outcome.count, 1);
        let shocked = instruments[0]
            .as_any()
            .downcast_ref::<CompositeInstrument>()
            .expect("root should remain a composite");
        assert_eq!(
            quantities,
            shocked
                .state
                .resolved_legs
                .iter()
                .map(|leg| leg.quantity)
                .collect::<Vec<_>>()
        );
        for leg in &shocked.spec.legs {
            let child = leg
                .instrument
                .as_ref()
                .clone()
                .into_boxed()
                .expect("embedded child should materialize");
            assert_eq!(
                child
                    .get_scenario_pricing_overrides()
                    .and_then(|overrides| overrides.scenario_price_shock_pct),
                Some(0.1)
            );
        }
    }

    #[test]
    fn matching_composite_root_stops_branch_descent() {
        let composite = CompositeInstrument::example().expect("composite example should build");
        let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(composite)];

        let outcome =
            apply_instrument_type_price_shock(&mut instruments, &[InstrumentType::Composite], 10.0);
        assert_eq!(outcome.count, 1);
        let shocked = instruments[0]
            .as_any()
            .downcast_ref::<CompositeInstrument>()
            .expect("root should remain a composite");
        assert_eq!(
            shocked
                .get_scenario_pricing_overrides()
                .and_then(|overrides| overrides.scenario_price_shock_pct),
            Some(0.1)
        );
        for leg in &shocked.spec.legs {
            let child = leg
                .instrument
                .as_ref()
                .clone()
                .into_boxed()
                .expect("embedded child should materialize");
            assert_eq!(
                child
                    .get_scenario_pricing_overrides()
                    .and_then(|overrides| overrides.scenario_price_shock_pct),
                None
            );
        }
    }
}
