//! Quote-reproducing risk view for bond bump-based sensitivities.
//!
//! When a bond carries a price-driving quote override, `Bond::base_value`
//! short-circuits to the constant quoted price, so any metric that bumps a curve
//! and reprices sees no change (DV01/CS01 collapse to zero). This module builds a
//! single calibrated *risk view* — a clone whose price is pinned by a calibrated
//! spread/shift (not the raw quote) plus the market it reprices on — so the bump
//! moves the PV. The view reproduces the quote by construction and the expensive
//! hazard solve is cached in `context.computed` (one solve per metric pass).
//!
//! Routing:
//! - **Callable** → OAS-pinned clone (the existing tree calibration), base market.
//!   The clone keeps `credit_curve_id`, so for a risky callable it reprices on the
//!   two-factor `RatesCreditTree`: CS01 bumps the hazard curve and DV01 bumps the
//!   discount curve, both holding the calibrated OAS constant (call + credit aware).
//! - **Credit** (hazard curve, non-callable) → quote-cleared clone + hazard-shifted
//!   market (a flat λ-shift solved to reproduce the quote). Mirrors the library's
//!   bond convention: IR DV01 bumps discount holding hazard; CS01 bumps hazard via
//!   a parallel λ-shift.
//! - **Plain rate** → periodic `quoted_z_spread` clone, base market (the convention
//!   used by `ZSpread`/`price_from_z_spread`; a continuous curve shift is deliberately
//!   NOT used — wrong compounding basis).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::effective::option_risk_bond_and_base_price;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::types::CurveId;
use std::sync::Arc;
use time::Date;

/// Quote-reproducing risk view: a calibrated `(instrument, market)` pair.
type RiskView = (Arc<dyn Instrument>, Arc<MarketContext>);

/// Cache key (non-standard `MetricId`) for the calibrated flat hazard shift, so the
/// Brent solve runs once per metric pass and is shared across `Cs01`/`BucketedCs01`/
/// `BucketedDv01` on a credit bond.
fn hazard_shift_cache_key() -> MetricId {
    MetricId::custom("bond_quote_hazard_shift")
}

/// Build the quote-reproducing risk view `(instrument, market)` a bump-based
/// sensitivity should reprice, or `None` when the bond has no price driver (the
/// caller then behaves exactly as without a quote). Generalizes the former
/// `price_driven_risk_bond` to also cover the credit hazard-shift case.
pub(crate) fn bond_risk_view(
    context: &mut MetricContext,
) -> finstack_quant_core::Result<Option<RiskView>> {
    // Phase 1: read everything off the (immutable) bond, ending the borrow.
    let (bond_clone, has_options, credit_id) = {
        let bond: &Bond = context.instrument_as()?;
        if !bond
            .instrument_pricing_overrides
            .market_quotes
            .has_price_driver()
        {
            return Ok(None);
        }
        let has_options = bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let credit_id = bond
            .market_dependencies()?
            .curves
            .credit_curves
            .first()
            .cloned();
        (bond.clone(), has_options, credit_id)
    };

    // Callable → existing OAS clone (needs the quote intact to solve OAS), base market.
    if has_options {
        let (risk_bond, _) =
            option_risk_bond_and_base_price(&bond_clone, context.curves.as_ref(), context.as_of)?;
        return Ok(Some((
            Arc::new(risk_bond) as Arc<dyn Instrument>,
            Arc::clone(&context.curves),
        )));
    }

    // Credit → quote-cleared clone + hazard-shifted market (λ-shift reproduces the quote).
    if let Some(ref hazard_id) = credit_id {
        let target_dirty = context.base_value.amount();
        let as_of = context.as_of;
        let base_market = Arc::clone(&context.curves);
        let hazard = base_market.get_hazard(hazard_id.as_str())?;

        let mut cleared = bond_clone;
        clear_price_driving_overrides(&mut cleared);

        // Calibrate once: cache the solved shift across calculators.
        let cache_key = hazard_shift_cache_key();
        let shift = match context.computed.get(&cache_key).copied() {
            Some(s) => s,
            None => {
                let s = solve_hazard_shift(
                    &cleared,
                    hazard.as_ref(),
                    hazard_id,
                    base_market.as_ref(),
                    as_of,
                    target_dirty,
                );
                context.computed.insert(cache_key, s);
                s
            }
        };

        let shifted_market =
            market_with_hazard_shift(base_market.as_ref(), hazard.as_ref(), hazard_id, shift)?;
        return Ok(Some((
            Arc::new(cleared) as Arc<dyn Instrument>,
            Arc::new(shifted_market),
        )));
    }

    // Plain rate → periodic quoted_z_spread clone (convention-correct), base market.
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
    Ok(Some((
        Arc::new(cleared) as Arc<dyn Instrument>,
        Arc::clone(&context.curves),
    )))
}

/// Run `f` with `context.instrument`/`context.curves` temporarily replaced by the
/// calibrated risk view (restored on all paths). When the bond has no price driver,
/// runs `f` against the context unchanged.
pub(crate) fn with_bond_risk_view<R>(
    context: &mut MetricContext,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    match bond_risk_view(context)? {
        None => f(context),
        Some((instrument, curves)) => {
            let orig_instrument = Arc::clone(&context.instrument);
            let orig_curves = Arc::clone(&context.curves);
            context.instrument = instrument;
            context.curves = curves;
            let result = f(context);
            context.instrument = orig_instrument;
            context.curves = orig_curves;
            result
        }
    }
}

/// Build a market with the bond's hazard curve shifted by `s`, **preserving the
/// curve id** so the inserted curve replaces the original for downstream pricing.
/// (`HazardCurve::with_parallel_bump` renames the curve, which would otherwise
/// leave the unshifted curve in place under its original id.)
fn market_with_hazard_shift(
    base_market: &MarketContext,
    hazard: &HazardCurve,
    hazard_id: &CurveId,
    s: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let shifted = hazard
        .with_parallel_bump(s)?
        .to_builder_with_id(hazard_id.clone())
        .build()?;
    Ok(base_market.clone().insert(shifted))
}

/// Solve a flat additive hazard shift `s` so the quote-cleared `bond` priced on the
/// `s`-shifted market reproduces `target_dirty` (the quoted dirty price). Solved
/// against the exact pricing path the sensitivities reprice, so the view reproduces
/// the quote by construction. Degrades to `0.0` if the solve fails (matches how the
/// z-spread/OAS solves degrade) — see plan open item on the bracket for distressed quotes.
fn solve_hazard_shift(
    bond: &Bond,
    hazard: &HazardCurve,
    hazard_id: &CurveId,
    base_market: &MarketContext,
    as_of: Date,
    target_dirty: f64,
) -> f64 {
    let objective = |s: f64| -> f64 {
        // s too negative makes some λ < 0 → invalid; treat as "PV far above target"
        // (less hazard ⇒ higher PV) so the solver is pushed toward larger s.
        let Ok(market) = market_with_hazard_shift(base_market, hazard, hazard_id, s) else {
            return 1.0e9;
        };
        match bond.value_raw(&market, as_of) {
            Ok(pv) => pv - target_dirty,
            Err(_) => 1.0e9,
        }
    };
    BrentSolver::new()
        .initial_bracket_size(Some(0.005))
        .solve(objective, 0.0)
        .unwrap_or(0.0)
}
