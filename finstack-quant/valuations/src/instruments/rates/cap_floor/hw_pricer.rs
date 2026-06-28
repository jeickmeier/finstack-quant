//! Hull-White 1-factor closed-form pricer for interest rate caps and floors.
//!
//! Prices a cap/floor as a sum of caplet/floorlet values. Each caplet is an
//! option on the forward rate for a single accrual period; under the
//! Hull-White 1-factor model the forward rate is Gaussian, so each
//! caplet/floorlet is priced **in closed form** with a Bachelier (normal)
//! formula using the HW1F-implied normal volatility — no tree or backward
//! induction is built.
//!
//! # Algorithm
//!
//! For each caplet/floorlet period `[T_start, T_end]`:
//!
//! 1. The payoff at `T_end` is `N * tau * max(±(L(T_start, T_end) - K), 0)`,
//!    where `L` is the simply-compounded forward rate and `tau` the accrual.
//!
//! 2. The HW1F-implied normal volatility of the forward rate over the period is
//!    computed via [`hw1f_caplet_forward_rate_normal_vol`] (from the HW `B(t,T)`
//!    and `G(t,T)` variance terms), and the caplet/floorlet is valued with the
//!    Bachelier formula.
//!
//! 3. The cap/floor value is the sum of all caplet/floorlet values.
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592.
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*,
//!   Chapter 3: One-factor Short-Rate Models, Section 3.3.2 (Gaussian forward-rate
//!   dynamics underpinning the closed-form caplet normal volatility).

use crate::calibration::hull_white::hw1f_caplet_forward_rate_normal_vol;
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::types::{CapFloor, RateOptionType};
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fCapletSurfacePoint, Hw1fResolveRequest,
    Hw1fSurfaceCalibration,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Hull-White 1-factor closed-form pricer for caps and floors.
///
/// Prices each caplet/floorlet with a Bachelier (normal) formula using the
/// HW1F-implied normal volatility of the forward rate; no tree is built.
pub(crate) struct CapFloorHullWhitePricer;

impl Pricer for CapFloorHullWhitePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CapFloor, ModelKey::HullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cap_floor = instrument
            .as_any()
            .downcast_ref::<CapFloor>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CapFloor, instrument.key())
            })?;

        self.price_internal(cap_floor, market, as_of)
    }
}

impl CapFloorHullWhitePricer {
    /// Core pricing routine.
    fn price_internal(
        &self,
        cap_floor: &CapFloor,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let ctx = DayCountContext::default();

        // Get discount and projection curves. Bloomberg's HW1F cap/floor setup is
        // still a projected SOFR payoff discounted on the OIS curve.
        let disc = market
            .get_discount(cap_floor.discount_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        let fwd = market
            .get_forward(cap_floor.forward_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Time to maturity (cap end)
        let maturity_time = cap_floor
            .day_count
            .year_fraction(as_of, cap_floor.maturity, ctx)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if maturity_time <= 0.0 {
            return Ok(ValuationResult::stamped(
                cap_floor.id.as_str(),
                as_of,
                Money::new(0.0, cap_floor.notional.currency()),
            ));
        }

        // Build schedule periods
        let periods = cap_floor.pricing_periods().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if periods.is_empty() {
            return Ok(ValuationResult::stamped(
                cap_floor.id.as_str(),
                as_of,
                Money::new(0.0, cap_floor.notional.currency()),
            ));
        }

        let strike = cap_floor.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let notional = cap_floor.notional.amount();

        let is_cap = matches!(
            cap_floor.rate_option_type,
            RateOptionType::Cap | RateOptionType::Caplet
        );

        // Resolve HW1F parameters following the documented precedence:
        // explicit `pricing_overrides` κ/σ → calibrated MarketContext scalars
        // → warned `HullWhiteParams::default()`.
        let hw_params = resolve_capfloor_hw1f_params(cap_floor, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Price each caplet/floorlet in closed form (Bachelier with the
        // HW1F-implied normal vol); no tree is built.
        let mut total_pv = 0.0;

        for period in &periods {
            let fixing_date = cap_floor.option_fixing_date(period);
            let t_fix = cap_floor
                .day_count
                .year_fraction(as_of, fixing_date, ctx)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;

            // Skip expired caplets
            if t_fix <= 0.0 {
                continue;
            }

            let tau = year_fraction(
                cap_floor.day_count,
                period.accrual_start,
                period.accrual_end,
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

            if tau <= 0.0 {
                continue;
            }

            let forward =
                rate_period_on_dates(fwd.as_ref(), period.accrual_start, period.accrual_end)
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, period.payment_date)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let hw_vol =
                hw1f_caplet_forward_rate_normal_vol(hw_params.kappa, hw_params.sigma, t_fix, tau);
            let caplet_pv =
                crate::instruments::rates::cap_floor::pricing::normal::price_caplet_floorlet(
                    CapletFloorletInputs {
                        is_cap,
                        notional,
                        strike,
                        forward,
                        discount_factor: df,
                        volatility: hw_vol,
                        time_to_fixing: t_fix,
                        accrual_year_fraction: tau,
                        currency: cap_floor.notional.currency(),
                    },
                )
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?
                .amount();

            total_pv += caplet_pv;
        }

        Ok(ValuationResult::stamped(
            cap_floor.id.as_str(),
            as_of,
            Money::new(total_pv, cap_floor.notional.currency()),
        ))
    }
}

/// Build the HW1F override JSON blob from a cap/floor's typed pricing overrides.
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
fn hw1f_overrides_json(cap_floor: &CapFloor) -> Option<serde_json::Value> {
    let kappa = cap_floor
        .pricing_overrides
        .model_config
        .hw1f_mean_reversion?;
    let sigma = cap_floor.pricing_overrides.model_config.hw1f_sigma?;
    Some(serde_json::json!({ "hw1f_kappa": kappa, "hw1f_sigma": sigma }))
}

/// Resolve the effective Hull-White 1F (κ, σ) the tree pricer uses for `cap_floor`.
///
/// Applies the documented precedence (explicit `pricing_overrides` κ/σ →
/// calibrated `MarketContext` scalars → defaults). Sharing this with the vega
/// calculator keeps the model-consistent vega bump aligned with the σ the
/// pricer actually consumes, rather than an unrelated `implied_volatility`.
pub(crate) fn resolve_capfloor_hw1f_params(
    cap_floor: &CapFloor,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<crate::calibration::hull_white::HullWhiteParams> {
    let context_label = format!("CapFloor {}", cap_floor.id);
    let overrides = hw1f_overrides_json(cap_floor);
    let surface_points = capfloor_surface_points(cap_floor, market, as_of)?;
    let req = Hw1fResolveRequest {
        curve_id: cap_floor.discount_curve_id.as_str(),
        flavor: Hw1fCalibrationFlavor::CapFloor,
        overrides: overrides.as_ref(),
        surface: Some(Hw1fSurfaceCalibration::CapFloor {
            surface_id: cap_floor.vol_surface_id.as_str(),
            points: &surface_points,
        }),
        fallback: None,
        context: context_label.as_str(),
    };
    // Provenance (`hw1f_param_source`) is stamped by the resolver's
    // structured logs under the instrument context label.
    resolve_hw1f_params(&req, market).map(|(params, _source)| params)
}

fn capfloor_surface_points(
    cap_floor: &CapFloor,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<Vec<Hw1fCapletSurfacePoint>> {
    let disc = market.get_discount(cap_floor.discount_curve_id.as_str())?;
    let periods = cap_floor.pricing_periods()?;
    let strike = cap_floor.strike_f64()?;
    let ctx = DayCountContext::default();
    let mut points = Vec::with_capacity(periods.len());
    for period in &periods {
        let fixing_date = cap_floor.option_fixing_date(period);
        let t_fix = cap_floor.day_count.year_fraction(as_of, fixing_date, ctx)?;
        if t_fix <= 0.0 {
            continue;
        }
        let tau = year_fraction(
            cap_floor.day_count,
            period.accrual_start,
            period.accrual_end,
        )?;
        if tau <= 0.0 {
            continue;
        }
        let df = relative_df_discount_curve(disc.as_ref(), as_of, period.payment_date)?;
        points.push(Hw1fCapletSurfacePoint {
            t_fix,
            accrual: tau,
            strike,
            weight: (cap_floor.notional.amount() * tau * df).abs(),
        });
    }
    Ok(points)
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
    use test_utils::{date, flat_discount_with_tenor, flat_forward_with_tenor};

    /// Pricing a cap via the HW pricer (which falls back to uncalibrated
    /// `HullWhiteParams::default()` absent overrides) must still produce a
    /// finite PV. This locks in that adding the uncalibrated-params diagnostic
    /// did not change numerics.
    #[test]
    fn hw_cap_floor_produces_finite_pv() {
        let as_of = date(2023, 12, 1);
        let cap = CapFloor::example().expect("CapFloor example should build");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor(
                cap.discount_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .insert(flat_forward_with_tenor(
                cap.forward_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ));

        let pricer = CapFloorHullWhitePricer;
        let result = pricer
            .price_internal(&cap, &market, as_of)
            .expect("HW cap pricing should succeed");

        let pv = result.value.amount();
        assert!(pv.is_finite(), "HW cap PV must be finite, got {pv}");
        assert!(pv >= 0.0, "cap PV must be non-negative, got {pv}");
    }

    /// Builds a cap with flat discount/forward curves.
    fn example_cap_market() -> (finstack_quant_core::dates::Date, CapFloor, MarketContext) {
        let as_of = date(2023, 12, 1);
        let cap = CapFloor::example().expect("CapFloor example should build");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor(
                cap.discount_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .insert(flat_forward_with_tenor(
                cap.forward_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ));
        (as_of, cap, market)
    }

    /// When the `MarketContext` carries calibrated `{curve}_CAPFLOOR_HW1F_*`
    /// scalars, the pricer must consume them: the PV differs from the
    /// default-params PV.
    #[test]
    fn hw_cap_floor_uses_calibrated_market_scalars() {
        use crate::calibration::hull_white::capfloor_hw1f_scalar_keys;
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let (as_of, cap, default_market) = example_cap_market();
        let default_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &default_market, as_of)
            .expect("default-params pricing should succeed")
            .value
            .amount();

        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys(cap.discount_curve_id.as_str());
        let calibrated_market = default_market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.030));

        let calibrated_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &calibrated_market, as_of)
            .expect("calibrated pricing should succeed")
            .value
            .amount();

        assert!(calibrated_pv.is_finite());
        assert!(
            (calibrated_pv - default_pv).abs() > 1e-9,
            "calibrated PV ({calibrated_pv}) must differ from default PV ({default_pv})"
        );
    }

    /// Explicit `pricing_overrides` κ/σ win over calibrated market scalars.
    #[test]
    fn hw_cap_floor_overrides_win_over_market_scalars() {
        use crate::calibration::hull_white::capfloor_hw1f_scalar_keys;
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let (as_of, mut cap, market) = example_cap_market();
        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys(cap.discount_curve_id.as_str());
        let market = market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.030));

        let market_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("market-scalar pricing should succeed")
            .value
            .amount();

        // HW1F-specific overrides via the dedicated short-rate-vol field (NOT
        // implied_volatility which is an option vol). PV must differ from the
        // market-scalar PV.
        cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
        cap.pricing_overrides.model_config.hw1f_sigma = Some(0.01);
        let override_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
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
        let (as_of, mut cap, market) = example_cap_market();

        let default_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("default pricing should succeed")
            .value
            .amount();

        // Override with a significantly different short-rate σ (~3× default).
        cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.05);
        cap.pricing_overrides.model_config.hw1f_sigma = Some(0.030);
        let overridden_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
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
    /// pricer must fall through to the calibrated-scalar / default branch and
    /// the PV must be unchanged.
    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let (as_of, cap_no_iv, market) = example_cap_market();
        let (_, mut cap_with_iv, _) = example_cap_market();

        let pv_no_iv = CapFloorHullWhitePricer
            .price_internal(&cap_no_iv, &market, as_of)
            .expect("no-iv pricing should succeed")
            .value
            .amount();

        // Typical lognormal cap vol (0.20) with NO hw1f_sigma set.
        // If the bug were present, this would feed 0.20 into the HW tree as σ.
        cap_with_iv
            .pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
        let pv_with_iv = CapFloorHullWhitePricer
            .price_internal(&cap_with_iv, &market, as_of)
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
