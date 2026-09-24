//! Quote-reproducing risk view for bond bump-based sensitivities.
//!
//! When a bond carries a price-driving quote override, `Bond::base_value`
//! short-circuits to the constant quoted price, so any metric that bumps a curve
//! and reprices sees no change (DV01/CS01 collapse to zero). This module builds a
//! single calibrated *risk view* — a clone whose price is pinned by a calibrated
//! spread (not the raw quote), repriced on the unchanged market — so the bump
//! moves the PV. The view reproduces the quote by construction and the expensive
//! hazard solve is cached in `context.computed` (one solve per metric pass).
//!
//! Routing:
//! - **Embedded options** (call, put, or return floor) → OAS-pinned clone, base
//!   market. Every reprice preserves the caller-selected model; under
//!   `rates_credit`, CS01 bumps the hazard curve and DV01 bumps the discount
//!   curve while holding the calibrated OAS constant.
//! - **Credit** (hazard curve, non-callable) → OAS-pinned clone on the original
//!   market for rate and par-spread risk. This retains the calibrated hazard
//!   recipe and holds the bond-specific quote adjustment constant.
//! - **Plain rate** → periodic `quoted_z_spread` clone, base market (the convention
//!   used by `ZSpread`/`price_from_z_spread`; a continuous curve shift is deliberately
//!   NOT used — wrong compounding basis).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::effective::option_risk_bond_and_base_price;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::metrics::{MetricContext, MetricId};
use crate::pricer::ModelKey;
use std::sync::Arc;

pub(super) fn active_model_consumes_credit(context: &MetricContext, bond: &Bond) -> bool {
    let model = context
        .pricing_model()
        .unwrap_or_else(|| bond.default_pricing_model());
    matches!(model, ModelKey::HazardRate | ModelKey::RatesCredit)
}

pub(super) fn plain_rate_quote_requires_z_spread(context: &MetricContext, bond: &Bond) -> bool {
    let has_options =
        bond.return_floor.is_some() || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
    bond.instrument_pricing_overrides
        .market_quotes
        .has_price_driver()
        && !has_options
        && !active_model_consumes_credit(context, bond)
}

/// Quote-reproducing clone of the context bond, or `None` when the bond has
/// no price-driving quote. The view always reprices on the original market.
fn bond_risk_view(context: &mut MetricContext) -> finstack_quant_core::Result<Option<Bond>> {
    // Phase 1: read everything off the (immutable) bond, ending the borrow.
    let (bond_clone, pin_with_oas) = {
        let bond: &Bond = context.instrument_as()?;
        if !bond
            .instrument_pricing_overrides
            .market_quotes
            .has_price_driver()
        {
            return Ok(None);
        }
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let has_credit_curve = active_model_consumes_credit(context, bond)
            && !bond.market_dependencies()?.curves.credit_curves.is_empty();
        (bond.clone(), has_options || has_credit_curve)
    };

    // Embedded options need the quote intact to solve OAS. Rate and
    // quote-space spread risk on a credit model must also start from a hazard
    // curve that still carries its exact calibration recipe, so the quote is
    // pinned with OAS and each market bump reprices under the same calibrated
    // model.
    if pin_with_oas {
        let (risk_bond, _) = option_risk_bond_and_base_price(&bond_clone, context)?;
        return Ok(Some(risk_bond));
    }

    // Plain rate → periodic quoted_z_spread clone (convention-correct).
    let z = context
        .computed
        .get(&MetricId::ZSpread)
        .copied()
        .ok_or_else(|| crate::metrics::metric_not_found(MetricId::ZSpread))?;
    let mut cleared = bond_clone;
    clear_price_driving_overrides(&mut cleared);
    cleared
        .instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread = Some(z);
    Ok(Some(cleared))
}

/// Run rate or quote-space spread risk against the quote-reproducing view.
///
/// Quoted credit bonds retain the source hazard calibration recipe and pin the
/// observed bond price with OAS. The original context is restored on all paths.
pub(crate) fn with_bond_risk_view<R>(
    context: &mut MetricContext,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    match bond_risk_view(context)? {
        None => f(context),
        Some(risk_bond) => {
            let orig_instrument = Arc::clone(&context.instrument);
            context.set_instrument(Arc::new(risk_bond) as Arc<dyn Instrument>);
            let result = f(context);
            context.set_instrument(orig_instrument);
            result
        }
    }
}
