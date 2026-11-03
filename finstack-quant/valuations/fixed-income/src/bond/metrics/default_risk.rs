//! Recovery01 and jump-to-default on a hazard-priced bond.
//!
//! The hazard kernels (`hazard_rate`, and `rates_credit` for a bond with
//! exercise rights) take recovery from the hazard curve named by
//! `credit_curve_id` and apply it as fractional recovery of par: a default pays
//! `R` times the outstanding principal, with no accrued interest. Both metrics
//! use that source and that claim. Neither applies under any other model, where
//! default is not priced, and asking for one there is an error rather than a zero.
//!
//! A bond's notional must be positive, so a bond is always the holder's long
//! position; a short is a position with negative quantity, which scales both
//! metrics and so flips their signs.

use super::risk_view::{active_model_consumes_credit, with_bond_risk_view};
use crate::bond::Bond;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::metrics::sensitivities::recovery01::{
    RecoveryReplayTerms, market_with_recovery, recovery01_fd,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use finstack_quant_valuations_core::pricer::ModelKey;

/// The hazard curve of a hazard-priced bond, or the error saying it is not one.
fn hazard_curve_id(
    context: &MetricContext,
    bond: &Bond,
    metric: &str,
) -> finstack_quant_core::Result<CurveId> {
    match &bond.credit_curve_id {
        Some(id) if active_model_consumes_credit(context, bond) => Ok(id.clone()),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "{metric} for bond '{}' requires hazard pricing: a credit_curve_id and the \
             'hazard_rate' or 'rates_credit' model (active model '{}')",
            bond.id.as_str(),
            super::active_model(context, bond).as_str()
        ))),
    }
}

/// Reprice `bond` on `market` through the selected model.
fn reprice_on(
    context: &MetricContext,
    bond: &Bond,
    model: ModelKey,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if context.pricing_model().is_some() {
        context.reprice_instrument_raw(bond, market, as_of)
    } else {
        bond.price_for_model_raw(model, market, as_of)
    }
}

/// `recovery01`: PV change in currency per +1% (0.01 absolute) move in the
/// hazard curve's recovery rate, from the holder's perspective.
///
/// A central finite difference of the full PV at `R ± 0.01`, one-sided at a
/// bound. As for a CDS, a curve carrying its calibration recipe is re-bootstrapped
/// to the same par spreads at the bumped recovery, which raises its hazard rates
/// as recovery rises; a hand-built curve keeps its hazard rates, and only the
/// recovery moves. The two differ greatly: at 250bp flat spreads and 40% recovery,
/// a 5% five-year bond on $1mm has 8.03 recalibrated against 1,767.89 frozen, and
/// a premium bond's recalibrated figure is negative. A quoted bond is held at its
/// calibrated OAS, as CS01 is.
pub(super) struct BondRecovery01Calculator;

impl MetricCalculator for BondRecovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let hazard_id = {
            let bond: &Bond = context.instrument_as()?;
            hazard_curve_id(context, bond, "recovery01")?
        };
        with_bond_risk_view(context, |context| {
            let bond: &Bond = context.instrument_as()?;
            let model = super::active_model(context, bond);
            let as_of = context.as_of;
            let terms = RecoveryReplayTerms::on_discount_curve(bond.discount_curve_id.clone());
            let base_recovery = context.curves.get_hazard(&hazard_id)?.recovery_rate();
            recovery01_fd(base_recovery, context.base_value.amount(), |recovery| {
                let market = market_with_recovery(
                    context,
                    &context.curves,
                    hazard_id.as_str(),
                    recovery,
                    &terms,
                    "Bond Recovery01",
                )?;
                reprice_on(context, bond, model, &market, as_of)
            })
        })
    }
}

/// `jump_to_default`: value on immediate default less current value, in currency,
/// for the whole notional, from the holder's perspective.
///
/// ```text
/// jump_to_default = R × outstanding principal(as_of) − PV(as_of)
/// ```
///
/// The value on default is what the hazard kernel pays: `R` of the curve times
/// the outstanding principal after events on `as_of`, without accrued interest.
/// `PV` is the current value, the quoted one on a quoted bond. Negative for a
/// bond worth more than its recovery.
pub(super) struct BondJumpToDefaultCalculator;

impl MetricCalculator for BondJumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let hazard_id = hazard_curve_id(context, bond, "jump_to_default")?;
        let recovery = context
            .curves
            .get_hazard(&hazard_id)?
            .recovery_rate()
            .clamp(0.0, 1.0);
        let schedule = bond.full_cashflow_schedule(&context.curves)?;
        let path = crate::bond::pricing::settlement::recovery_principal_path(&schedule)?;
        let principal = crate::bond::pricing::settlement::balance_on(
            &path,
            context.as_of,
            schedule.get_notional().initial.amount(),
        );
        Ok(recovery * principal - context.base_value.amount())
    }
}
