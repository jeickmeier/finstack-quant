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
//!    where S is the forward swap rate, A the annuity, N the notional.
//! 4. The root node value is the present value.
//!
//! # References
//!
//! - Hull, J. & White, A. (1994). "Numerical Procedures for Implementing
//!   Term Structure Models I: Single-Factor Models", *Journal of Derivatives*.
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and
//!   Practice*, Chapter 4.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fResolveRequest, Hw1fSurfaceCalibration,
};
use crate::instruments::rates::swaption::types::Swaption;
use crate::models::trees::{HullWhiteTree, HullWhiteTreeConfig};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{BusinessDayConvention, DayCountContext, StubKind};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

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
        let swaption = instrument
            .as_any()
            .downcast_ref::<Swaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Swaption, instrument.key())
            })?;

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
        // Single-curve requirement (same as Bermudan pricer)
        if swaption.underlying_forward_curve_id() != swaption.underlying_discount_curve_id() {
            return Err(PricingError::model_failure_with_context(
                "Hull-White tree pricing is currently single-curve only. \
                 Set forward_curve_id equal to discount_curve_id or use a multi-curve-capable engine."
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Get discount curve
        let disc = market
            .get_discount(swaption.underlying_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Time to expiry
        let time_to_expiry = year_fraction(swaption.underlying_day_count(), as_of, swaption.expiry)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if time_to_expiry <= 0.0 {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Time horizon is swap end (need the tree to cover the full swap)
        let ctx = DayCountContext::default();
        let swap_end_time = swaption
            .day_count
            .year_fraction(as_of, swaption.swap_end, ctx)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if swap_end_time <= 0.0 {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Resolve HW1F parameters following the documented precedence:
        // explicit `pricing_overrides` κ/σ → calibrated MarketContext scalars
        // → warned `HullWhiteParams::default()`.
        let context_label = format!("Swaption {}", swaption.id);
        let overrides = hw1f_overrides_json(swaption);
        let req = Hw1fResolveRequest {
            curve_id: swaption.underlying_discount_curve_id().as_str(),
            flavor: Hw1fCalibrationFlavor::Swaption,
            overrides: overrides.as_ref(),
            surface: Some(Hw1fSurfaceCalibration::Swaption {
                surface_id: swaption.vol_surface_id.as_str(),
                max_expiry: Some(swap_end_time),
                frequency: crate::calibration::hull_white::SwapFrequency::SemiAnnual,
            }),
            fallback: None,
            context: context_label.as_str(),
        };
        // Provenance (`hw1f_param_source`) is stamped by the resolver's
        // structured logs under the instrument context label.
        let (hw_params, _hw1f_param_source) = resolve_hw1f_params(&req, market).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build and calibrate HW tree with the expiry threaded as a
        // mandatory grid date so the exercise decision lands exactly on a
        // grid point.
        let config = HullWhiteTreeConfig::new(hw_params.kappa, hw_params.sigma, self.tree_steps);
        let tree = HullWhiteTree::calibrate_with_times(
            config,
            disc.as_ref(),
            swap_end_time,
            &[time_to_expiry],
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build swap schedule for the underlying
        let calendar_id = swaption
            .calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);

        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: swaption.swap_start,
                end: swaption.swap_end,
                frequency: swaption.underlying_fixed_frequency(),
                stub: StubKind::None,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id,
                end_of_month: false,
                day_count: swaption.underlying_day_count(),
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if periods.is_empty() {
            return Err(PricingError::model_failure_with_context(
                "Swap schedule has fewer than 2 dates".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Compute payment times and accrual fractions
        let mut payment_times = Vec::with_capacity(periods.len());
        let mut accrual_fractions = Vec::with_capacity(periods.len());
        for period in periods {
            let t = swaption
                .day_count
                .year_fraction(as_of, period.payment_date, ctx)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let accrual = year_fraction(
                swaption.underlying_day_count(),
                period.accrual_start,
                period.accrual_end,
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            payment_times.push(t);
            accrual_fractions.push(accrual);
        }

        let swap_start_time = swaption
            .day_count
            .year_fraction(as_of, swaption.swap_start, ctx)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
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
                    let t = tree.time_at_step(step);

                    // Find remaining payments after this time
                    let start_idx = payment_times.partition_point(|&pt| pt <= t);
                    if start_idx >= payment_times.len() {
                        return continuation;
                    }

                    let remaining_payment_times = &payment_times[start_idx..];
                    let remaining_accruals = &accrual_fractions[start_idx..];

                    let swap_start = swap_start_time.max(t);
                    let swap_rate = tree.forward_swap_rate(
                        step,
                        node_idx,
                        swap_start,
                        swap_end_time,
                        remaining_payment_times,
                        remaining_accruals,
                        disc.as_ref(),
                    );

                    let annuity = tree.annuity(
                        step,
                        node_idx,
                        remaining_payment_times,
                        remaining_accruals,
                        disc.as_ref(),
                    );

                    let intrinsic = match swaption.option_type {
                        OptionType::Call => (swap_rate - strike).max(0.0),
                        OptionType::Put => (strike - swap_rate).max(0.0),
                    };

                    let exercise_value = intrinsic * annuity * notional;

                    // European: take max of continuation and exercise at the single
                    // exercise date (for a well-calibrated tree these should be close,
                    // but max handles numerical edge cases).
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
            Money::new(pv, swaption.notional.currency()),
        ))
    }
}

/// Build the HW1F override JSON blob from a swaption's typed pricing overrides.
///
/// Reads `model_config.hw1f_mean_reversion` → `hw1f_kappa` and
/// `model_config.hw1f_sigma` → `hw1f_sigma` (the Hull-White short-rate absolute
/// volatility). Returns `Some` only when **both** are present, so that a partial
/// override falls through to the calibrated-market-scalar / default branches in
/// [`resolve_hw1f_params`].
///
/// # Unit contract
///
/// `hw1f_sigma` is a **short-rate** absolute volatility (annual decimal, ~0.005–0.015).
/// It must NOT be confused with an option implied volatility (e.g. 0.20 lognormal),
/// which lives in `market_quotes.implied_volatility`. Feeding an option vol directly
/// into the HW tree would produce a ~13–40× mis-priced result.
fn hw1f_overrides_json(swaption: &Swaption) -> Option<serde_json::Value> {
    let kappa = swaption
        .pricing_overrides
        .model_config
        .hw1f_mean_reversion?;
    let sigma = swaption.pricing_overrides.model_config.hw1f_sigma?;
    Some(serde_json::json!({ "hw1f_kappa": kappa, "hw1f_sigma": sigma }))
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use test_utils::{date, flat_discount_with_tenor};

    /// Pricing a European swaption via the HW pricer (which uses uncalibrated
    /// `HullWhiteParams::default()`) must still produce a finite PV. This locks
    /// in that adding the uncalibrated-params diagnostic did not change numerics.
    #[test]
    fn hw_swaption_produces_finite_pv() {
        let as_of = date(2025, 1, 1);
        let mut swaption = Swaption::example();
        // example() uses an OIS discount curve; HW tree pricing is single-curve.
        swaption.forward_curve_id = swaption.discount_curve_id.clone();
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            swaption.discount_curve_id.as_str(),
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
        let as_of = date(2025, 1, 1);
        let mut swaption = Swaption::example();
        swaption.forward_curve_id = swaption.discount_curve_id.clone();
        let market = MarketContext::new().insert(flat_discount_with_tenor(
            swaption.discount_curve_id.as_str(),
            as_of,
            0.03,
            10.0,
        ));
        (as_of, swaption, market)
    }

    /// When the `MarketContext` carries calibrated `{curve}_HW1F_*` scalars, the
    /// pricer must consume them: the PV differs from the default-params PV.
    #[test]
    fn hw_swaption_uses_calibrated_market_scalars() {
        use crate::calibration::hull_white::hw1f_scalar_keys;
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let (as_of, swaption, default_market) = example_single_curve();
        let default_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &default_market, as_of)
            .expect("default-params pricing should succeed")
            .value
            .amount();

        let (kappa_key, sigma_key) = hw1f_scalar_keys(swaption.discount_curve_id.as_str());
        // Calibrated σ deliberately far from the default 0.01.
        let calibrated_market = default_market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.025));

        let calibrated_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &calibrated_market, as_of)
            .expect("calibrated pricing should succeed")
            .value
            .amount();

        assert!(calibrated_pv.is_finite());
        assert!(
            (calibrated_pv - default_pv).abs() > 1e-9,
            "calibrated PV ({calibrated_pv}) must differ from default PV ({default_pv})"
        );
    }

    #[test]
    fn hw_swaption_surface_shock_moves_pv() {
        use finstack_quant_core::market_data::bumps::{
            BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
        };
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::types::CurveId;

        let (as_of, swaption, market) = example_single_curve();
        // The HW1F swaption calibration reads an expiry × TENOR matrix of
        // NORMAL vols; the surface must be tagged accordingly.
        let surface = VolSurface::builder(swaption.vol_surface_id.clone())
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[1.0, 2.0, 5.0])
            .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Tenor)
            .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::Normal)
            .row(&[0.020, 0.022, 0.024])
            .row(&[0.022, 0.024, 0.026])
            .row(&[0.024, 0.026, 0.028])
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
        assert!(
            (shocked_pv - base_pv).abs() > 1e-6,
            "HW swaption PV must move under a vol surface shock: base={base_pv}, shocked={shocked_pv}"
        );
    }

    /// Explicit `pricing_overrides` κ/σ win over calibrated market scalars.
    #[test]
    fn hw_swaption_overrides_win_over_market_scalars() {
        use crate::calibration::hull_white::hw1f_scalar_keys;
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let (as_of, mut swaption, market) = example_single_curve();
        let (kappa_key, sigma_key) = hw1f_scalar_keys(swaption.discount_curve_id.as_str());
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
        swaption.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
        swaption.pricing_overrides.model_config.hw1f_sigma = Some(0.01);
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

        // Default params PV.
        let default_pv = SwaptionHullWhitePricer::default()
            .price_internal(&swaption, &market, as_of)
            .expect("default pricing should succeed")
            .value
            .amount();

        // Override with a significantly different short-rate σ. The HW1F default
        // σ is ~0.01; using 0.03 (3×) should produce a clearly different PV.
        swaption.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.05);
        swaption.pricing_overrides.model_config.hw1f_sigma = Some(0.030);
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
            "hw1f_sigma override must change PV vs default: override={overridden_pv}, default={default_pv}"
        );
    }

    /// Regression (W26): `market_quotes.implied_volatility` must NOT be silently
    /// treated as the HW1F short-rate σ. When only `implied_volatility` is set
    /// (without the dedicated `hw1f_sigma`/`hw1f_mean_reversion` fields), the
    /// pricer must fall through to the calibrated-scalar / default branch — NOT
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
            .pricing_overrides
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
