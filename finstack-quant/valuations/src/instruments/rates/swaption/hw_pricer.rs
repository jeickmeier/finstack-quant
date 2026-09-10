//! Hull-White 1-factor tree pricer for European swaptions.
//!
//! Prices a European swaption by building a calibrated Hull-White trinomial
//! tree and performing backward induction with a single exercise date at
//! expiry. This is the short-rate analogue of the Black-76 pricer and is
//! particularly useful when consistent pricing with Bermudan swaptions
//! (which also use the HW tree) is required.
//!
//! # Algorithm
//!
//! 1. Calibrate a Hull-White tree to the discount curve over the swap
//!    maturity horizon.
//! 2. At the terminal step, compute continuation values of zero.
//! 3. During backward induction, at the tree step corresponding to the
//!    swaption expiry, compute the exercise value:
//!    - Payer: max(0, (S - K) * A * N)
//!    - Receiver: max(0, (K - S) * A * N)
//!   where S is the forward swap rate, A the annuity, N the notional.
//!   4. The root node value is the present value.
//!
//! # References
//!
//! - Hull, J. & White, A. (1994). "Numerical Procedures for Implementing
//!   Term Structure Models I: Single-Factor Models", *Journal of Derivatives*. `docs/REFERENCES.md#hull-white-1994-numerical-procedures`
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and
//!   Practice*, Chapter 4. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::hw1f::{resolve_hw1f_params, Hw1fParamFamily};
use crate::instruments::rates::swaption::types::Swaption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::trees::{HullWhiteTree, HullWhiteTreeConfig};

/// Number of tree steps used for the HW tree pricing.
const DEFAULT_TREE_STEPS: usize = 100;

/// Hull-White 1-factor tree pricer for European swaptions.
///
/// Prices European swaptions via backward induction on a calibrated
/// Hull-White trinomial tree. The tree is calibrated to the initial
/// discount curve and exercise is evaluated at the single expiry date.
pub(crate) struct SwaptionHullWhitePricer {
    tree_steps: usize,
}

impl Default for SwaptionHullWhitePricer {
    fn default() -> Self {
        Self {
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl Pricer for SwaptionHullWhitePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Swaption, ModelKey::HullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption =
            crate::pricer::expect_inst::<Swaption>(instrument, InstrumentType::Swaption)?;

        self.price_internal(swaption, market, as_of)
    }
}

impl SwaptionHullWhitePricer {
    /// Core pricing routine.
    fn price_internal(
        &self,
        swaption: &Swaption,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        if let Some(pv) = swaption.terminal_value(market, as_of).map_err(|error| {
            PricingError::from_core(
                error,
                PricingErrorContext::from_instrument(swaption).model(ModelKey::HullWhite1F),
            )
        })? {
            return Ok(ValuationResult::stamped(swaption.id(), as_of, pv));
        }

        // Single-curve requirement (same as Bermudan pricer)
        if swaption.get_forward_curve_id() != swaption.get_discount_curve_id() {
            return Err(PricingError::model_failure_with_context(
                "Hull-White tree pricing is currently single-curve only. \
                 Set forward_curve_id equal to discount_curve_id or use a multi-curve-capable engine."
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        let disc = market
            .get_discount(swaption.get_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let time_to_expiry =
            finstack_quant_models::rates::clock::model_time(as_of, swaption.expiry);
        let discount =
            finstack_quant_models::rates::clock::ModelDiscountCurve::new(disc.as_ref(), as_of)
                .map_err(|e| {
                    PricingError::from_core(e, PricingErrorContext::from_instrument(swaption))
                })?;
        let cashflows = super::pricing::hw_cashflows::HwSwaptionCashflows::new(
            &swaption.underlying_fixed_leg,
            &swaption.underlying_float_leg,
            swaption.expiry,
            as_of,
        )
        .map_err(|e| PricingError::from_core(e, PricingErrorContext::from_instrument(swaption)))?;
        let swap_end_time = cashflows.horizon().max(time_to_expiry);

        // Resolve only complete explicit or pre-calibrated HW1F parameters.
        let hw_params = resolve_hw1f_params(
            Hw1fParamFamily::Swaption,
            swaption.get_discount_curve_id().as_str(),
            &swaption.instrument_pricing_overrides.model_config,
            None,
            &format!("Swaption {}", swaption.id),
            market,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build and calibrate HW tree with the expiry threaded as a
        // mandatory grid date so the exercise decision lands exactly on a
        // grid point.
        let config = HullWhiteTreeConfig::new(
            hw_params.kappa,
            hw_params.sigma,
            swaption
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.tree_steps),
        );
        let tree = HullWhiteTree::calibrate_with_times(
            config,
            &discount,
            swap_end_time,
            &[time_to_expiry],
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let notional = swaption.notional.amount();

        // Map expiry to its exact tree step (guaranteed by the mandatory
        // date threaded into calibration above).
        let exercise_step = tree.step_at_time(time_to_expiry).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let n = tree.num_steps();

        // Terminal values: zero (no exercise at terminal step for European)
        let terminal: Vec<f64> = vec![0.0; tree.num_nodes(n)];

        // Backward induction with exercise at expiry step only
        let pv = tree
            .backward_induction(&terminal, |step, node_idx, continuation| {
                if step == exercise_step {
                    let exercise_value = cashflows.value(
                        super::pricing::hw_cashflows::HwExerciseNode {
                            tree: &tree,
                            step,
                            index: node_idx,
                            discount: &discount,
                        },
                        super::pricing::hw_cashflows::HwExerciseTerms {
                            strike,
                            notional,
                            option_type: swaption.option_type,
                            settlement: swaption.settlement,
                            cash_method: swaption.cash_settlement_method,
                        },
                    );
                    continuation.max(exercise_value)
                } else {
                    continuation
                }
            })
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        Ok(ValuationResult::stamped(
            swaption.id.as_str(),
            as_of,
            Money::new(pv, swaption.notional.currency()).map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(swaption)
                        .model(ModelKey::HullWhite1F),
                )
            })?,
        ))
    }
}

/// Build the HW1F override JSON blob from a swaption's typed pricing overrides.
///
/// Reads `model_config.hw1f_mean_reversion` → `hw1f_kappa` and
/// `model_config.hw1f_sigma` → `hw1f_sigma` (the Hull-White short-rate absolute
/// volatility). Partial overrides are preserved so [`resolve_hw1f_params`] can
/// reject them explicitly instead of combining sources.
///
#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod date_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/date.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod discount_forward_curve_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/discount_forward_curves.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::parameters::OptionType;
    use date_support::date;
    use discount_forward_curve_support::flat_discount_with_tenor;

    #[test]
    fn b7_hw_price_is_invariant_to_equivalent_curve_date_clocks() {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        let (as_of, mut swaption, _) = example_single_curve();
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(0.01);
        let mut prices = Vec::new();
        for base in [as_of, date(2024, 7, 1)] {
            for (day_count, days) in [(DayCount::Act365F, 365.0), (DayCount::Act360, 360.0)] {
                let shift = (as_of - base).whole_days() as f64 / 365.0;
                let log_df = |t: f64| -0.02 * t - 0.003 * t * t;
                let mut knots = vec![(0.0, 1.0)];
                for t in [0.0, 1.0, 3.0, 10.0] {
                    if t + shift > 0.0 {
                        knots.push((
                            (t + shift) * 365.0 / days,
                            (log_df(t) - log_df(-shift)).exp(),
                        ));
                    }
                }
                let curve = DiscountCurve::builder(swaption.get_discount_curve_id().clone())
                    .base_date(base)
                    .day_count(day_count)
                    .knots(knots)
                    .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                    .build()
                    .expect("curve");
                let market = MarketContext::new().insert(curve);
                prices.push(
                    SwaptionHullWhitePricer::default()
                        .price_internal(&swaption, &market, as_of)
                        .expect("price")
                        .value
                        .amount(),
                );
            }
        }
        for price in &prices {
            assert!(
                (price - prices[0]).abs() < 1e-6,
                "equivalent curve PVs {prices:?}"
            );
        }
    }

    #[test]
    fn m2_hw_partial_coupon_and_spread_match_deterministic_cashflows() {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
        use rust_decimal::Decimal;
        let as_of = date(2025, 1, 1);
        let mut swaption = Swaption::example();
        swaption.expiry = date(2026, 4, 1);
        swaption.option_type = OptionType::Call;
        let fixed = &mut swaption.underlying_fixed_leg;
        fixed.start = date(2026, 1, 1);
        fixed.end = date(2028, 1, 1);
        fixed.frequency = Tenor::annual();
        fixed.day_count = DayCount::Act365F;
        fixed.business_day_convention = BusinessDayConvention::Unadjusted;
        fixed.payment_lag_days = 0;
        fixed.rate = Decimal::new(2, 2);
        let float = &mut swaption.underlying_float_leg;
        float.start = fixed.start;
        float.end = fixed.end;
        float.frequency = fixed.frequency;
        float.day_count = fixed.day_count;
        float.business_day_convention = fixed.business_day_convention;
        float.payment_lag_days = 0;
        float.reset_lag_days = 0;
        float.forward_curve_id = fixed.discount_curve_id.clone();
        float.spread_bp = Decimal::from(100);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(1e-7);
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            swaption.get_discount_curve_id().as_str(),
            as_of,
            0.04,
            10.0,
        ));
        let mut expected = 0.0;
        let mut start = swaption.expiry;
        for end in [date(2027, 1, 1), date(2028, 1, 1)] {
            let tau = (end - start).whole_days() as f64 / 365.0;
            let t = (end - as_of).whole_days() as f64 / 365.0;
            expected += ((0.04 * tau).exp() - 1.0 + (0.01 - 0.02) * tau) * (-0.04 * t).exp();
            start = end;
        }
        expected *= swaption.notional.amount();
        let actual = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("price")
            .value
            .amount();
        assert!(
            (actual - expected).abs() < 0.01,
            "actual={actual}, expected={expected}"
        );
    }

    /// Pricing a European swaption via the HW pricer with explicit fitted
    /// parameters must produce a finite PV.
    #[test]
    fn hw_swaption_produces_finite_pv() {
        let as_of = date(2025, 1, 1);
        let mut swaption = Swaption::example();
        // example() uses an OIS discount curve; HW tree pricing is single-curve.
        swaption.underlying_float_leg.forward_curve_id =
            swaption.underlying_fixed_leg.discount_curve_id.clone();
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(0.01);
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            swaption.get_discount_curve_id().as_str(),
            as_of,
            0.03,
            10.0,
        ));

        let pricer = SwaptionHullWhitePricer::default();
        let result = pricer
            .price_internal(&swaption, &market, as_of)
            .expect("HW swaption pricing should succeed");

        let pv = result.value.amount();
        assert!(pv.is_finite(), "HW swaption PV must be finite, got {pv}");
        assert!(pv >= 0.0, "swaption PV must be non-negative, got {pv}");
    }

    /// Builds a single-curve swaption priced over a flat discount curve.
    fn example_single_curve() -> (finstack_quant_core::dates::Date, Swaption, MarketContext) {
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_models::rates::hull_white::hw1f_scalar_keys;

        let as_of = date(2025, 1, 1);
        let mut swaption = Swaption::example();
        swaption.underlying_float_leg.forward_curve_id =
            swaption.underlying_fixed_leg.discount_curve_id.clone();
        let (kappa_key, sigma_key) = hw1f_scalar_keys(swaption.get_discount_curve_id().as_str());
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            swaption.get_discount_curve_id().as_str(),
            as_of,
            0.03,
            10.0,
        ));
        let market = market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.05))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.01));
        (as_of, swaption, market)
    }

    /// When the `MarketContext` carries a fitted `{curve}_HW1F_*` pair, the
    /// pricer must consume it: changing that complete pair changes PV.
    #[test]
    fn hw_swaption_uses_calibrated_market_scalars() {
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_models::rates::hull_white::hw1f_scalar_keys;

        let (as_of, swaption, baseline_market) = example_single_curve();
        let baseline_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &baseline_market, as_of)
            .expect("baseline market-parameter pricing should succeed")
            .value
            .amount();

        let (kappa_key, sigma_key) = hw1f_scalar_keys(swaption.get_discount_curve_id().as_str());
        // Fitted sigma deliberately far from the baseline 0.01.
        let calibrated_market = baseline_market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.025));

        let calibrated_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &calibrated_market, as_of)
            .expect("calibrated pricing should succeed")
            .value
            .amount();

        assert!(calibrated_pv.is_finite());
        assert!(
            (calibrated_pv - baseline_pv).abs() > 1e-9,
            "updated fitted PV ({calibrated_pv}) must differ from baseline PV ({baseline_pv})"
        );
    }

    #[test]
    fn hw_swaption_surface_shock_does_not_move_pv() {
        use finstack_quant_core::market_data::bumps::{
            BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
        };
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::types::CurveId;

        let (as_of, swaption, market) = example_single_curve();
        // The surface is deliberately plausible calibration input, but pricing
        // must consume only the pre-fitted scalar pair already in the market.
        let surface = VolSurface::builder(swaption.vol_surface_id.clone())
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[1.0, 2.0, 5.0])
            .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Tenor)
            .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::Normal)
            // Declining expiry vol keeps κ inside the calibration bounds.
            .row(&[0.028, 0.026, 0.024])
            .row(&[0.024, 0.022, 0.020])
            .row(&[0.020, 0.018, 0.016])
            .build()
            .expect("swaption surface");
        let market = market.insert_surface(surface);
        let shocked_market = market
            .bump([MarketBump::Curve {
                id: CurveId::from(swaption.vol_surface_id.as_str()),
                spec: BumpSpec {
                    mode: BumpMode::Multiplicative,
                    units: BumpUnits::Factor,
                    value: 1.25,
                    bump_type: BumpType::Parallel,
                },
            }])
            .expect("surface shock");

        let base_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("surface pricing should succeed")
            .value
            .amount();
        let shocked_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &shocked_market, as_of)
            .expect("shocked pricing should succeed")
            .value
            .amount();

        assert!(base_pv.is_finite());
        assert!(shocked_pv.is_finite());
        assert_eq!(
            shocked_pv, base_pv,
            "HW swaption pricing must not sample the volatility surface"
        );
    }

    /// Explicit `pricing_overrides` κ/σ win over calibrated market scalars.
    #[test]
    fn hw_swaption_overrides_win_over_market_scalars() {
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_models::rates::hull_white::hw1f_scalar_keys;

        let (as_of, mut swaption, market) = example_single_curve();
        let (kappa_key, sigma_key) = hw1f_scalar_keys(swaption.get_discount_curve_id().as_str());
        let market = market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.025));

        // PV with market scalars only.
        let market_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("market-scalar pricing should succeed")
            .value
            .amount();

        // Add HW1F-specific overrides (dedicated short-rate-vol field, NOT
        // implied_volatility which is an option vol). PV must differ from the
        // market-scalar PV.
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.03);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(0.01);
        let override_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("override pricing should succeed")
            .value
            .amount();

        assert!(
            (override_pv - market_pv).abs() > 1e-9,
            "override PV ({override_pv}) must differ from market-scalar PV ({market_pv})"
        );
    }

    /// Regression: `model_config.hw1f_sigma` (the dedicated short-rate σ field) must
    /// reach the HW tree and change the PV. A different short-rate σ must produce a
    /// different PV — confirming the dedicated channel is wired through.
    #[test]
    fn hw1f_sigma_override_field_reaches_tree() {
        let (as_of, mut swaption, market) = example_single_curve();

        // Pre-calibrated market-parameter PV.
        let default_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("market-parameter pricing should succeed")
            .value
            .amount();

        // Override with a significantly different short-rate σ.
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        swaption
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(0.030);
        let overridden_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("hw1f_sigma override pricing should succeed")
            .value
            .amount();

        assert!(
            overridden_pv.is_finite(),
            "PV must be finite: {overridden_pv}"
        );
        assert!(
            (overridden_pv - default_pv).abs() > 1e-9,
            "hw1f_sigma override must change PV vs market parameters: override={overridden_pv}, market={default_pv}"
        );
    }

    /// Regression (W26): `market_quotes.implied_volatility` must NOT be silently
    /// treated as the HW1F short-rate σ. When only `implied_volatility` is set
    /// (without the dedicated `hw1f_sigma`/`hw1f_mean_reversion` fields), the
    /// pricer must use the complete calibrated-scalar pair — NOT
    /// use the option vol as the short-rate vol.
    ///
    /// Specifically: setting `implied_volatility = 0.20` (a typical lognormal
    /// swaption vol) while leaving `hw1f_sigma = None` must yield the same PV
    /// as leaving `implied_volatility` unset, because the HW pricer does not
    /// consume `market_quotes.implied_volatility` for its short-rate σ.
    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let (as_of, mut swaption_with_iv, market) = example_single_curve();
        let (as_of2, swaption_no_iv, market2) = example_single_curve();
        let _ = (as_of2, market2); // same values, use as_of/market throughout

        // PV without implied_volatility set.
        let pv_no_iv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption_no_iv, &market, as_of)
            .expect("no-iv pricing should succeed")
            .value
            .amount();

        // Set implied_volatility = 0.20 (a typical lognormal swaption vol) but
        // leave hw1f_sigma/hw1f_mean_reversion unset. If the bug is present,
        // 0.20 would be fed into the HW tree as σ, producing a wildly different PV.
        swaption_with_iv
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
        let pv_with_iv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption_with_iv, &market, as_of)
            .expect("iv-only pricing should succeed")
            .value
            .amount();

        assert!(
            (pv_with_iv - pv_no_iv).abs() < 1e-9,
            "implied_volatility must NOT alter the HW tree pricing: \
             pv_with_iv={pv_with_iv}, pv_no_iv={pv_no_iv} (diff={})",
            (pv_with_iv - pv_no_iv).abs()
        );
    }
}
