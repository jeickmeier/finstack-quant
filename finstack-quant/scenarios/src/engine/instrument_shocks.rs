//! Instrument and structured-credit correlation shock application.

use crate::adapters::instruments::{
    apply_instrument_shock, InstrumentFilter, InstrumentShockOutcome, ShockKind,
};
use crate::error::Result;
use crate::spec::OperationSpec;
use crate::warning::Warning;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_valuations::instruments::Instrument;

/// Apply an instrument-scoped operation to the inventory.
///
/// Returns `None` for every operation that does not mutate instruments.
pub(super) fn apply_instrument_operation(
    op: &OperationSpec,
    instruments: &mut Option<&mut Vec<Box<dyn Instrument>>>,
) -> Result<Option<InstrumentShockOutcome>> {
    let outcome = match op {
        OperationSpec::InstrumentPricePctByType {
            instrument_types,
            pct,
        } => apply_instrument_shock(
            inventory(instruments)?,
            InstrumentFilter::Types(instrument_types),
            ShockKind::Price,
            *pct,
        ),
        OperationSpec::InstrumentPricePctByAttr { attrs, pct } => apply_instrument_shock(
            inventory(instruments)?,
            InstrumentFilter::Attrs(attrs),
            ShockKind::Price,
            *pct,
        ),
        OperationSpec::InstrumentSpreadBpByType {
            instrument_types,
            bp,
        } => apply_instrument_shock(
            inventory(instruments)?,
            InstrumentFilter::Types(instrument_types),
            ShockKind::Spread,
            *bp,
        ),
        OperationSpec::InstrumentSpreadBpByAttr { attrs, bp } => apply_instrument_shock(
            inventory(instruments)?,
            InstrumentFilter::Attrs(attrs),
            ShockKind::Spread,
            *bp,
        ),
        OperationSpec::AssetCorrelationPts { delta_pts } => {
            apply_correlation_shock(CorrelationKind::Asset, *delta_pts, inventory(instruments)?)
        }
        OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => apply_correlation_shock(
            CorrelationKind::PrepayDefault,
            *delta_pts,
            inventory(instruments)?,
        ),
        _ => return Ok(None),
    };
    Ok(Some(outcome))
}

/// The instrument inventory an instrument-scoped operation mutates.
///
/// [`super::ScenarioEngine::apply`] rejects instrument-scoped operations
/// without an inventory before any operation runs, so a missing inventory here
/// is an engine invariant violation.
fn inventory<'a>(
    instruments: &'a mut Option<&mut Vec<Box<dyn Instrument>>>,
) -> Result<&'a mut Vec<Box<dyn Instrument>>> {
    instruments.as_deref_mut().ok_or_else(|| {
        crate::error::Error::internal(
            "instrument-scoped operation reached the engine without an instrument inventory",
        )
    })
}

/// Which structured-credit correlation parameter a shock targets.
#[derive(Debug, Clone, Copy)]
enum CorrelationKind {
    /// Asset correlation (clamped to `[0, 0.99]`).
    Asset,
    /// Prepay-default correlation (clamped to `[-0.99, 0.99]`).
    PrepayDefault,
}

fn apply_correlation_shock(
    kind: CorrelationKind,
    delta_pts: f64,
    instruments: &mut [Box<dyn Instrument>],
) -> InstrumentShockOutcome {
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
            CorrelationKind::Asset => corr.bump_asset(delta_pts),
            CorrelationKind::PrepayDefault => corr.bump_prepay_default(delta_pts),
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

    InstrumentShockOutcome {
        count: changed_indices.len(),
        changed_indices,
        warnings,
    }
}
