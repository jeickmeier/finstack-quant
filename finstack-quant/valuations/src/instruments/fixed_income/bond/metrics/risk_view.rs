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
//! - **Embedded options** (call, put, or return floor) → OAS-pinned clone, base
//!   market. Every reprice preserves the caller-selected model; under
//!   `rates_credit`, CS01 bumps the hazard curve and DV01 bumps the discount
//!   curve while holding the calibrated OAS constant.
//! - **Credit** (hazard curve, non-callable) → OAS-pinned clone on the original
//!   market for rate and par-spread risk. This retains the calibrated hazard
//!   recipe and holds the bond-specific quote adjustment constant.
//! - **Direct intensity metrics** → quote-cleared clone + hazard-shifted market
//!   (a flat λ-shift solved to reproduce the quote). These metrics deliberately
//!   measure hazard-rate risk and do not require a par-CDS calibration recipe.
//! - **Plain rate** → periodic `quoted_z_spread` clone, base market (the convention
//!   used by `ZSpread`/`price_from_z_spread`; a continuous curve shift is deliberately
//!   NOT used — wrong compounding basis).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::effective::option_risk_bond_and_base_price;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::metrics::{MetricContext, MetricId};
use crate::pricer::ModelKey;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

/// Quote-reproducing risk view: a calibrated `(instrument, market)` pair.
type RiskView = (Arc<dyn Instrument>, Arc<MarketContext>);

#[derive(Clone, Copy)]
enum CreditQuoteRiskView {
    DirectHazardShift,
    PreserveCalibratedOas,
}

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

/// Cache key (non-standard `MetricId`) for the calibrated flat hazard shift, so the
/// Brent solve runs once per metric pass and is shared across the direct
/// `Cs01Hazard` and `BucketedCs01Hazard` metrics on a credit bond.
fn hazard_shift_cache_key() -> MetricId {
    MetricId::custom("bond_quote_hazard_shift")
}

fn bond_risk_view_with_credit_quote_basis(
    context: &mut MetricContext,
    credit_quote_basis: CreditQuoteRiskView,
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
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let credit_id = if active_model_consumes_credit(context, bond) {
            bond.market_dependencies()?
                .curves
                .credit_curves
                .first()
                .cloned()
        } else {
            None
        };
        (bond.clone(), has_options, credit_id)
    };

    // Embedded option → OAS clone (needs the quote intact to solve OAS), base market.
    if has_options {
        let (risk_bond, _) = option_risk_bond_and_base_price(&bond_clone, context)?;
        return Ok(Some((
            Arc::new(risk_bond) as Arc<dyn Instrument>,
            Arc::clone(&context.curves),
        )));
    }

    // Rate and quote-space spread risk must start from a hazard curve that
    // still carries its exact calibration recipe. Pin the bond quote with OAS
    // and leave the original market intact, so each market bump reprices under
    // the same calibrated credit model.
    if credit_id.is_some()
        && matches!(
            credit_quote_basis,
            CreditQuoteRiskView::PreserveCalibratedOas
        )
    {
        let (risk_bond, _) = option_risk_bond_and_base_price(&bond_clone, context)?;
        return Ok(Some((
            Arc::new(risk_bond) as Arc<dyn Instrument>,
            Arc::clone(&context.curves),
        )));
    }

    // Explicit direct-hazard risk retains the quote-calibrated intensity view.
    if let Some(ref hazard_id) = credit_id {
        let target_dirty = context.base_value.amount();
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
                    context,
                    target_dirty,
                )?;
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

/// Run rate or quote-space spread risk against the quote-reproducing view.
///
/// Quoted credit bonds retain the source hazard calibration recipe and pin the
/// observed bond price with OAS. The original context is restored on all paths.
pub(crate) fn with_bond_risk_view<R>(
    context: &mut MetricContext,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    with_selected_risk_view(context, CreditQuoteRiskView::PreserveCalibratedOas, f)
}

/// Run explicitly requested direct intensity risk around a flat hazard shift
/// calibrated to the observed bond quote.
pub(crate) fn with_bond_direct_hazard_risk_view<R>(
    context: &mut MetricContext,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    with_selected_risk_view(context, CreditQuoteRiskView::DirectHazardShift, f)
}

fn with_selected_risk_view<R>(
    context: &mut MetricContext,
    credit_quote_basis: CreditQuoteRiskView,
    f: impl FnOnce(&mut MetricContext) -> finstack_quant_core::Result<R>,
) -> finstack_quant_core::Result<R> {
    match bond_risk_view_with_credit_quote_basis(context, credit_quote_basis)? {
        None => f(context),
        Some((instrument, curves)) => {
            let orig_instrument = Arc::clone(&context.instrument);
            let orig_curves = Arc::clone(&context.curves);
            context.set_instrument(instrument);
            context.set_market(curves);
            let result = f(context);
            context.set_instrument(orig_instrument);
            context.set_market(orig_curves);
            result
        }
    }
}

/// Build a market with the bond's hazard curve shifted by `s` (a decimal
/// hazard-rate shift). `with_parallel_hazard_rate_bump_bp` preserves the curve
/// id, so the inserted curve replaces the original for downstream pricing.
fn market_with_hazard_shift(
    base_market: &MarketContext,
    hazard: &HazardCurve,
    hazard_id: &CurveId,
    s: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let shifted = hazard.with_parallel_hazard_rate_bump_bp(s * 10_000.0)?;
    debug_assert_eq!(shifted.id(), hazard_id);
    Ok(base_market.clone().insert(shifted))
}

/// Solve a flat additive hazard shift `s` so the quote-cleared `bond` priced on the
/// `s`-shifted market reproduces `target_dirty` (the quoted dirty price). Solved
/// against the exact pricing path the sensitivities reprice, so the view reproduces
/// the quote by construction. Calibration failures propagate to the caller.
fn solve_hazard_shift(
    bond: &Bond,
    hazard: &HazardCurve,
    hazard_id: &CurveId,
    base_market: &MarketContext,
    context: &MetricContext,
    target_dirty: f64,
) -> finstack_quant_core::Result<f64> {
    let (min_hazard, max_hazard) = hazard
        .knot_points()
        .map(|(_, lambda)| lambda)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), lambda| {
            (min.min(lambda), max.max(lambda))
        });
    let lower_bound = -min_hazard;
    let upper_bound = 10.0 - max_hazard;
    let domain_width = upper_bound - lower_bound;
    if !domain_width.is_finite() || domain_width <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "quoted bond hazard-shift calibration has no valid intensity domain".to_string(),
        ));
    }
    let initial_size = 0.005_f64.min(domain_width / 4.0);
    let initial_guess = 0.0_f64.clamp(lower_bound + initial_size, upper_bound - initial_size);
    let objective = |s: f64| -> f64 {
        let Ok(market) = market_with_hazard_shift(base_market, hazard, hazard_id, s) else {
            return f64::NAN;
        };
        match context.reprice_instrument_raw(bond, &market, context.as_of) {
            Ok(pv) => pv - target_dirty,
            Err(_) => f64::NAN,
        }
    };
    BrentSolver::new()
        .initial_bracket_size(Some(initial_size))
        .bracket_bounds(lower_bound, upper_bound)
        .solve(objective, initial_guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::pricer::{
        expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingDispatch,
        PricingError,
    };
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::macros::date;

    struct HazardSensitivePricer {
        calls: Arc<AtomicUsize>,
    }

    impl HazardSensitivePricer {
        fn price(&self, instrument: &dyn Instrument, market: &MarketContext) -> f64 {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)
                .expect("hazard-shift test pricer expects a bond");
            let hazard_id = bond
                .credit_curve_id
                .as_ref()
                .expect("test bond must name its hazard curve");
            let hazard = market
                .get_hazard(hazard_id.as_str())
                .expect("test hazard curve");
            bond.notional.amount() * (1.0 - hazard.hazard_rate(1.0))
        }
    }

    impl Pricer for HazardSensitivePricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::new(self.price(instrument, market), Currency::USD),
            ))
        }

        fn price_raw_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<f64, PricingError> {
            Ok(self.price(instrument, market))
        }
    }

    #[test]
    fn hazard_shift_trials_preserve_selected_custom_registry_model() {
        let as_of = date!(2025 - 01 - 15);
        let hazard_id = CurveId::new("ACME-HZD");
        let mut bond = Bond::fixed(
            "HAZARD-SHIFT-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.credit_curve_id = Some(hazard_id.clone());
        let market = MarketContext::new()
            .insert(DiscountCurve::flat("USD-OIS", as_of, 0.04).expect("discount curve"))
            .insert(HazardCurve::flat(hazard_id.clone(), as_of, 0.02, 0.4).expect("hazard curve"));
        let hazard = market
            .get_hazard(hazard_id.as_str())
            .expect("inserted hazard curve");

        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = PricerRegistry::new();
        registry
            .register(HazardSensitivePricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let mut context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market.clone()),
            as_of,
            Money::new(970.0, Currency::USD),
            MetricContext::default_config(),
        );
        context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::HazardRate,
            Arc::new(registry),
        ));

        let shift =
            solve_hazard_shift(&bond, hazard.as_ref(), &hazard_id, &market, &context, 970.0)
                .expect("hazard-shift calibration");
        assert!((shift - 0.01).abs() < 1.0e-10, "shift={shift}");
        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "every calibration trial must use the selected registry pricer"
        );
    }
}
