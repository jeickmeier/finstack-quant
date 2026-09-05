//! Instrument and structured-credit correlation shock application.

use crate::adapters::instruments::InstrumentShockOutcome;
use crate::warning::Warning;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::InstrumentType;

type TypeShockFn = fn(&mut [Box<dyn Instrument>], &[InstrumentType], f64) -> InstrumentShockOutcome;

type AttrShockFn = fn(
    &mut [Box<dyn Instrument>],
    &indexmap::IndexMap<String, String>,
    f64,
) -> InstrumentShockOutcome;

pub(super) fn apply_instrument_shock(
    types: Option<&[InstrumentType]>,
    attrs: Option<&indexmap::IndexMap<String, String>>,
    value: f64,
    kind: &'static str,
    instruments: &mut Option<&mut Vec<Box<dyn Instrument>>>,
    type_fn: TypeShockFn,
    attr_fn: AttrShockFn,
) -> InstrumentShockOutcome {
    let Some(portfolio) = instruments.as_mut() else {
        let mut warnings = Vec::new();
        if types.is_some() {
            warnings.push(Warning::InstrumentShockNoPortfolio {
                shock_kind: kind.to_string(),
                filter: "type".to_string(),
            });
        }
        if attrs.is_some() {
            warnings.push(Warning::InstrumentShockNoPortfolio {
                shock_kind: kind.to_string(),
                filter: "attr".to_string(),
            });
        }
        return InstrumentShockOutcome {
            count: 0,
            changed_indices: Vec::new(),
            warnings,
        };
    };

    let mut applied = 0;
    let mut changed_indices = Vec::new();
    let mut warnings = Vec::new();
    if let Some(ts) = types {
        let outcome = type_fn(portfolio, ts, value);
        applied += outcome.count;
        changed_indices.extend(outcome.changed_indices);
        warnings.extend(outcome.warnings);
    }
    if let Some(ats) = attrs {
        let outcome = attr_fn(portfolio, ats, value);
        applied += outcome.count;
        changed_indices.extend(outcome.changed_indices);
        warnings.extend(outcome.warnings);
    }
    InstrumentShockOutcome {
        count: applied,
        changed_indices,
        warnings,
    }
}

/// Which structured-credit correlation parameter a shock targets.
#[derive(Debug, Clone, Copy)]
pub(super) enum CorrelationKind {
    /// Asset correlation (clamped to `[0, 0.99]`).
    Asset,
    /// Prepay-default correlation (clamped to `[-0.99, 0.99]`).
    PrepayDefault,
}

pub(super) fn apply_correlation_effect(
    kind: CorrelationKind,
    delta_pts: f64,
    ctx: &mut super::ExecutionContext,
) -> (usize, Vec<usize>, Vec<Warning>) {
    let Some(instruments) = ctx.instruments.as_mut() else {
        return (0, Vec::new(), vec![Warning::CorrelationShockNoPortfolio]);
    };

    let mut changed_indices = Vec::new();
    let mut warnings = Vec::new();

    for (index, inst) in instruments.iter_mut().enumerate() {
        let Some(sc) = inst.as_any_mut().downcast_mut::<StructuredCredit>() else {
            continue;
        };
        let Some(ref corr) = sc.credit_model.correlation_structure else {
            continue;
        };

        let (new_corr, clamp_info) = match kind {
            CorrelationKind::Asset => corr.bump_asset_with_clamp_info(delta_pts),
            CorrelationKind::PrepayDefault => corr.bump_prepay_default_with_clamp_info(delta_pts),
        };

        if let Some(info) = clamp_info {
            warnings.push(Warning::CorrelationClamped {
                instrument_id: sc.id.to_string(),
                detail: info,
            });
        }
        sc.credit_model.correlation_structure = Some(new_corr);
        changed_indices.push(index);
    }

    if changed_indices.is_empty() {
        warnings.push(Warning::CorrelationShockNoMatch);
    }

    (changed_indices.len(), changed_indices, warnings)
}
