//! Hull-White 1-factor closed-form pricer for interest rate caps and floors.
//!
//! Prices a cap/floor as a sum of caplet/floorlet values. Standard term-index
//! caplets use the exact HW1F zero-coupon-bond option equivalence. Options on
//! compounded overnight coupons use the explicitly documented moment-matched
//! normal approximation below.
//!
//! # Algorithm
//!
//! For each caplet/floorlet period `[T_start, T_end]`:
//!
//! 1. Term-index caplets are transformed into options on `P(T,S)` and priced
//!    exactly under HW1F.
//! 2. Compounded-RFR coupons are projected from every contractual overnight
//!    factor. Their normal volatility is moment-matched from each factor's
//!    product derivative, affine bond loading, and pairwise OU state covariance.
//! 3. The cap/floor value is the sum of all caplet/floorlet values.
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592.
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*,
//!   Chapter 3: One-factor Short-Rate Models, Section 3.3.2 (Gaussian forward-rate
//!   dynamics underpinning the closed-form caplet normal volatility).

use crate::calibration::hull_white::hw1f_term_caplet_price_from_dfs;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::pricing::projection::{
    resolve_optioned_caplet_inputs, OptionedCouponProjection,
};
use crate::instruments::rates::cap_floor::types::{CapFloor, RateOptionType};
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fCapletSurfacePoint, Hw1fResolveRequest,
    Hw1fSurfaceCalibration,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Hull-White 1-factor closed-form pricer for caps and floors.
///
/// Prices term caplets with exact HW1F bond options and compounded-RFR caplets
/// with a date-specific first-order normal moment match; no tree is built.
pub(crate) struct CapFloorHullWhitePricer;

/// One date-specific HW1F loading retained by the compounded-coupon moment match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hw1fObservationLoading {
    /// Observation time from valuation in ACT/365F years.
    pub fixing_time: f64,
    /// Projected overnight interval rate.
    pub projected_rate: f64,
    /// Day-count fraction used to quote that interval rate.
    pub rate_accrual_year_fraction: f64,
    /// Stable Hull-White bond loading `B(tᵢ,Tᵢ)`.
    pub bond_state_loading: f64,
    /// First derivative `∂Lᵢ/∂x(tᵢ)`.
    pub forward_state_loading: f64,
    /// Coupon loading `(∂C/∂Lᵢ)(∂Lᵢ/∂x(tᵢ))`.
    pub coupon_state_loading: f64,
}

/// First-order normal moment match for one compounded-RFR coupon.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompoundedRfrMomentMatch {
    /// Annualized normal volatility consumed by the Bachelier formula.
    pub normal_vol: f64,
    /// First-order coupon-rate variance at the final fixing date.
    pub variance: f64,
    /// Time from valuation to final fixing in ACT/365F years.
    pub option_time: f64,
    /// Date-specific stochastic loadings used in the covariance sum.
    pub observation_loadings: Vec<Hw1fObservationLoading>,
}

fn hw1f_b(kappa: f64, tenor: f64) -> f64 {
    if kappa.abs() < 1.0e-8 {
        tenor
    } else {
        -(-kappa * tenor).exp_m1() / kappa
    }
}

fn hw1f_ou_covariance(kappa: f64, sigma: f64, left_time: f64, right_time: f64) -> f64 {
    let min_time = left_time.min(right_time).max(0.0);
    if sigma <= 0.0 || min_time <= 0.0 {
        return 0.0;
    }
    if kappa.abs() < 1.0e-8 {
        sigma * sigma * min_time
    } else {
        sigma
            * sigma
            * (-kappa * (left_time - right_time).abs()).exp()
            * (-(-2.0 * kappa * min_time).exp_m1())
            / (2.0 * kappa)
    }
}

/// Date-specific first-order HW1F moment match for a compounded-RFR coupon.
///
/// For each stochastic overnight observation `Lᵢ`, the shared coupon projection
/// supplies the exact product-rule derivative `∂C/∂Lᵢ`. The affine HW1F bond
/// ratio gives
///
/// `∂Lᵢ/∂x(tᵢ) = (1 + qᵢLᵢ) B(tᵢ,Tᵢ) / qᵢ`,
///
/// so the coupon state loading is `aᵢ = (∂C/∂Lᵢ)(∂Lᵢ/∂x(tᵢ))`. The first-order
/// variance is the full date-specific covariance sum
///
/// `Var[C] = ΣᵢΣⱼ aᵢaⱼ Cov[x(tᵢ),x(tⱼ)]`,
///
/// with OU covariance measured from `as_of`. The returned Bachelier volatility
/// is `sqrt(Var[C] / T_option)`. Historical observations have zero
/// `∂C/∂Lᵢ` and therefore zero stochastic loading.
///
/// This linearizes the compounded product and affine forward mapping at today's
/// curve. It omits higher-order product terms and convexity/measure corrections,
/// so it is an approximation rather than the exact term-caplet bond option.
/// `B` and OU covariance use continuous limits as `kappa -> 0`.
pub(crate) fn hw1f_compounded_rfr_moment_match(
    as_of: Date,
    kappa: f64,
    sigma: f64,
    projection: &OptionedCouponProjection,
) -> finstack_quant_core::Result<CompoundedRfrMomentMatch> {
    let context = DayCountContext::default();
    let option_time = DayCount::Act365F.year_fraction(as_of, projection.fixing_date, context)?;
    let mut observation_loadings = Vec::with_capacity(projection.observation_exposures.len());
    for exposure in &projection.observation_exposures {
        let fixing_time = if exposure.observation_start <= as_of {
            0.0
        } else {
            DayCount::Act365F.year_fraction(as_of, exposure.observation_start, context)?
        };
        let interval_time = DayCount::Act365F.year_fraction(
            exposure.observation_start,
            exposure.observation_end,
            context,
        )?;
        let bond_state_loading = hw1f_b(kappa, interval_time);
        let forward_state_loading = (1.0
            + exposure.projected_rate * exposure.rate_accrual_year_fraction)
            * bond_state_loading
            / exposure.rate_accrual_year_fraction;
        observation_loadings.push(Hw1fObservationLoading {
            fixing_time,
            projected_rate: exposure.projected_rate,
            rate_accrual_year_fraction: exposure.rate_accrual_year_fraction,
            bond_state_loading,
            forward_state_loading,
            coupon_state_loading: exposure.coupon_forward_derivative * forward_state_loading,
        });
    }
    let variance = observation_loadings
        .iter()
        .map(|left| {
            observation_loadings
                .iter()
                .map(|right| {
                    left.coupon_state_loading
                        * right.coupon_state_loading
                        * hw1f_ou_covariance(kappa, sigma, left.fixing_time, right.fixing_time)
                })
                .sum::<f64>()
        })
        .sum::<f64>()
        .max(0.0);
    let normal_vol = if option_time > 0.0 {
        (variance / option_time).sqrt()
    } else {
        0.0
    };
    Ok(CompoundedRfrMomentMatch {
        normal_vol,
        variance,
        option_time: option_time.max(0.0),
        observation_loadings,
    })
}

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
        cap_floor.validate_for_pricing().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Get discount and projection curves. Bloomberg's HW1F cap/floor setup is
        // still a projected SOFR payoff discounted on the OIS curve.
        let fwd = market
            .get_forward(cap_floor.forward_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

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
        let term_strike = strike
            - cap_floor.spread_f64().map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
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
            if period.payment_date <= as_of {
                continue;
            }
            let resolved_inputs = resolve_optioned_caplet_inputs(cap_floor, period, market, as_of)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let projection = &resolved_inputs.coupon;
            if projection.payment_date <= as_of {
                continue;
            }
            let fixing_date = projection.fixing_date;
            let tau = projection.accrual_year_fraction;
            if tau <= 0.0 {
                continue;
            }
            let forward = projection.forward;
            let df = resolved_inputs.discount_factor;
            if fixing_date <= as_of {
                let intrinsic_rate = if is_cap {
                    (forward - strike).max(0.0)
                } else {
                    (strike - forward).max(0.0)
                };
                total_pv += notional * tau * df * intrinsic_rate;
                continue;
            }
            let t_fix = resolved_inputs.time_to_fixing;

            let caplet_pv = if projection.is_compounded_overnight {
                let moment_match = hw1f_compounded_rfr_moment_match(
                    as_of,
                    hw_params.kappa,
                    hw_params.sigma,
                    projection,
                )
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                crate::instruments::rates::cap_floor::pricing::normal::price_caplet_floorlet(
                    CapletFloorletInputs {
                        is_cap,
                        notional,
                        strike,
                        forward,
                        discount_factor: df,
                        volatility: moment_match.normal_vol,
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
                .amount()
            } else {
                let projection_df_as_of = fwd.df_on_date_curve(as_of).map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                let pf_start = fwd.df_on_date_curve(period.accrual_start).map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })? / projection_df_as_of;
                let pf_pay = fwd.df_on_date_curve(period.accrual_end).map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })? / projection_df_as_of;
                let t_pay = finstack_quant_core::dates::DayCount::Act365F
                    .year_fraction(as_of, period.accrual_end, ctx)
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                let t_start = finstack_quant_core::dates::DayCount::Act365F
                    .year_fraction(as_of, period.accrual_start, ctx)
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                notional
                    * hw1f_term_caplet_price_from_dfs(
                        hw_params.kappa,
                        hw_params.sigma,
                        pf_start,
                        pf_pay,
                        df,
                        t_fix,
                        t_start,
                        t_pay,
                        tau,
                        term_strike,
                        is_cap,
                    )
            };

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
/// volatility). A κ-only override is retained so [`resolve_hw1f_params`] can
/// hold mean reversion fixed while calibrating σ from the normal-vol surface.
/// A σ-only override is retained as well and rejected by the shared resolver.
///
/// # Unit contract
///
/// `hw1f_sigma` is a **short-rate** absolute volatility (annual decimal, ~0.005–0.015).
/// It must NOT be confused with an option implied volatility (e.g. 0.20 lognormal),
/// which lives in `market_quotes.implied_volatility`. Feeding an option vol directly
/// into the HW tree would produce a ~13–40× mis-priced result.
fn hw1f_overrides_json(cap_floor: &CapFloor) -> Option<serde_json::Value> {
    let mut overrides = serde_json::Map::new();
    if let Some(kappa) = cap_floor.pricing_overrides.model_config.hw1f_mean_reversion {
        overrides.insert("hw1f_kappa".to_owned(), serde_json::json!(kappa));
    }
    if let Some(sigma) = cap_floor.pricing_overrides.model_config.hw1f_sigma {
        overrides.insert("hw1f_sigma".to_owned(), serde_json::json!(sigma));
    }
    (!overrides.is_empty()).then_some(serde_json::Value::Object(overrides))
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
    let periods = cap_floor.pricing_periods()?;
    let strike = if cap_floor.overnight_coupon.is_some() {
        cap_floor.strike_f64()?
    } else {
        cap_floor.strike_f64()? - cap_floor.spread_f64()?
    };
    let mut points = Vec::with_capacity(periods.len());
    for period in &periods {
        if period.payment_date <= as_of {
            continue;
        }
        let resolved_inputs = resolve_optioned_caplet_inputs(cap_floor, period, market, as_of)?;
        let projection = &resolved_inputs.coupon;
        if projection.payment_date <= as_of || projection.fixing_date <= as_of {
            continue;
        }
        let t_fix = resolved_inputs.time_to_fixing;
        if t_fix <= 0.0 {
            continue;
        }
        let tau = projection.accrual_year_fraction;
        if tau <= 0.0 {
            continue;
        }
        let df = resolved_inputs.discount_factor;
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
    use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_coupon;
    use crate::instruments::rates::cap_floor::{
        OvernightCouponConvention, OvernightSpreadCompounding,
    };
    use crate::instruments::rates::irs::FloatingLegCompounding;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{CalendarRegistry, DateExt, DayCount, DayCountContext};
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::money::Money;
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

    #[test]
    fn term_caplet_hw_pricer_uses_exact_zcb_option_formula() {
        let as_of = date(2024, 1, 2);
        let mut caplet = CapFloor::new_caplet(
            "TERM-HW-EXACT",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2025, 1, 2),
            date(2025, 4, 2),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-3M",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        caplet.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.05);
        caplet.pricing_overrides.model_config.hw1f_sigma = Some(0.012);
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
            .insert(flat_forward_with_tenor("USD-SOFR-3M", as_of, 0.04, 5.0));

        let actual = CapFloorHullWhitePricer
            .price_internal(&caplet, &market, as_of)
            .expect("HW price")
            .value
            .amount();
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let disc = market.get_discount("USD-OIS").expect("discount");
        let fwd = market.get_forward("USD-SOFR-3M").expect("forward");
        let projection_base_df = fwd.df_on_date_curve(as_of).expect("base projection df");
        let pf_start = fwd
            .df_on_date_curve(period.accrual_start)
            .expect("start projection df")
            / projection_base_df;
        let pf_end = fwd
            .df_on_date_curve(period.accrual_end)
            .expect("end projection df")
            / projection_base_df;
        let pd_pay = disc
            .df_between_dates(as_of, period.payment_date)
            .expect("payment df");
        let fixing_date = period.reset_date.unwrap_or(period.accrual_start);
        let t_fix = DayCount::Act365F
            .year_fraction(as_of, fixing_date, DayCountContext::default())
            .expect("fix time");
        let t_start = DayCount::Act365F
            .year_fraction(as_of, period.accrual_start, DayCountContext::default())
            .expect("start time");
        let t_end = DayCount::Act365F
            .year_fraction(as_of, period.accrual_end, DayCountContext::default())
            .expect("end time");
        let expected = caplet.notional.amount()
            * hw1f_term_caplet_price_from_dfs(
                0.05,
                0.012,
                pf_start,
                pf_end,
                pd_pay,
                t_fix,
                t_start,
                t_end,
                period.accrual_year_fraction,
                0.04,
                true,
            );

        assert!(
            (actual - expected).abs() < 1.0e-8,
            "term caplet should remain on exact HW bond-option formula: {actual} vs {expected}"
        );
    }

    #[test]
    fn term_caplet_hw_spread_is_equivalent_to_strike_reduction() {
        let as_of = date(2024, 1, 2);
        let mut with_spread = CapFloor::new_caplet(
            "TERM-HW-SPREAD",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2025, 1, 2),
            date(2025, 4, 2),
            DayCount::Act360,
            "USD-OIS",
            "TEST-TERM-3M",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        with_spread.spread = rust_decimal::Decimal::try_from(0.01).expect("spread");
        with_spread
            .pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        with_spread.pricing_overrides.model_config.hw1f_sigma = Some(0.012);
        let mut reduced_strike = with_spread.clone();
        reduced_strike.spread = rust_decimal::Decimal::ZERO;
        reduced_strike.strike = rust_decimal::Decimal::try_from(0.03).expect("strike");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
            .insert(flat_forward_with_tenor("TEST-TERM-3M", as_of, 0.04, 5.0));

        let spread_pv = CapFloorHullWhitePricer
            .price_internal(&with_spread, &market, as_of)
            .expect("spread price")
            .value
            .amount();
        let reduced_strike_pv = CapFloorHullWhitePricer
            .price_internal(&reduced_strike, &market, as_of)
            .expect("reduced strike price")
            .value
            .amount();

        assert!(
            (spread_pv - reduced_strike_pv).abs() < 1.0e-8,
            "term spread should shift the exact HW strike: {spread_pv} vs {reduced_strike_pv}"
        );
    }

    #[test]
    fn compounded_moment_match_uses_date_specific_loadings_and_ou_covariance() {
        let as_of = date(2024, 12, 2);
        let mut caplet = CapFloor::new_caplet(
            "SOFR-HW-SENSITIVITY",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2025, 1, 2),
            date(2025, 4, 2),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        caplet.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let discount = flat_discount_with_tenor("USD-OIS", as_of, 0.05, 5.0);
        let forward = flat_forward_with_tenor("USD-SOFR-OIS", as_of, 0.045, 5.0);
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let market = MarketContext::new().insert(discount).insert(forward);
        let projection =
            resolve_optioned_coupon(&caplet, &period, &market, as_of).expect("projection");
        let kappa = 0.05;
        let sigma = 0.012;
        let matched = hw1f_compounded_rfr_moment_match(as_of, kappa, sigma, &projection)
            .expect("moment match");

        assert!(
            matched.observation_loadings.len() > 2,
            "the compounded coupon should retain every stochastic observation"
        );
        assert!(
            matched
                .observation_loadings
                .windows(2)
                .any(
                    |pair| (pair[0].coupon_state_loading - pair[1].coupon_state_loading).abs()
                        > 1.0e-12
                ),
            "date-specific coupon state loadings must not collapse to one scalar"
        );

        let h = 1.0e-4;
        for loading in matched.observation_loadings.iter().take(3) {
            let shifted_forward = |state: f64| {
                ((1.0 + loading.projected_rate * loading.rate_accrual_year_fraction)
                    * (loading.bond_state_loading * state).exp()
                    - 1.0)
                    / loading.rate_accrual_year_fraction
            };
            let finite_difference = (shifted_forward(h) - shifted_forward(-h)) / (2.0 * h);
            assert!(
                (loading.forward_state_loading - finite_difference).abs() < 1.0e-9,
                "analytic interval state loading {} should match finite difference {}",
                loading.forward_state_loading,
                finite_difference
            );
        }

        let covariance = |left: f64, right: f64| {
            let min_time = left.min(right);
            sigma
                * sigma
                * (-kappa * (left - right).abs()).exp()
                * (-(-2.0 * kappa * min_time).exp_m1())
                / (2.0 * kappa)
        };
        let expected_variance: f64 = matched
            .observation_loadings
            .iter()
            .map(|left| {
                matched
                    .observation_loadings
                    .iter()
                    .map(|right| {
                        left.coupon_state_loading
                            * right.coupon_state_loading
                            * covariance(left.fixing_time, right.fixing_time)
                    })
                    .sum::<f64>()
            })
            .sum();
        assert!((matched.variance - expected_variance).abs() < 1.0e-16);
        assert!(
            (matched.normal_vol * matched.normal_vol * matched.option_time - matched.variance)
                .abs()
                < 1.0e-16
        );

        let zero_kappa =
            hw1f_compounded_rfr_moment_match(as_of, 0.0, sigma, &projection).expect("zero kappa");
        let tiny_kappa = hw1f_compounded_rfr_moment_match(as_of, 1.0e-12, sigma, &projection)
            .expect("tiny kappa");
        assert!(zero_kappa.normal_vol.is_finite());
        assert!((zero_kappa.normal_vol - tiny_kappa.normal_vol).abs() < 1.0e-14);
    }

    #[test]
    fn compounded_full_coupon_loading_and_ou_variance_have_independent_cross_checks() {
        let as_of = date(2024, 12, 2);
        let mut caplet = CapFloor::new_caplet(
            "SOFR-HW-FULL-COUPON",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2025, 1, 2),
            date(2025, 2, 3),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        caplet.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_delay_days: 0,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.05, 5.0))
            .insert(flat_forward_with_tenor("USD-SOFR-OIS", as_of, 0.045, 5.0));
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let projection =
            resolve_optioned_coupon(&caplet, &period, &market, as_of).expect("projection");
        let kappa = 0.05;
        let sigma = 0.012;
        let matched = hw1f_compounded_rfr_moment_match(as_of, kappa, sigma, &projection)
            .expect("moment match");
        let directions: Vec<f64> = (0..matched.observation_loadings.len())
            .map(|index| 0.5 + index as f64 / matched.observation_loadings.len() as f64)
            .collect();
        let coupon_at_state = |scale: f64| {
            let factor = projection
                .observation_exposures
                .iter()
                .zip(&matched.observation_loadings)
                .zip(&directions)
                .map(|((exposure, loading), direction)| {
                    let shifted_rate = ((1.0
                        + exposure.projected_rate * exposure.rate_accrual_year_fraction)
                        * (loading.bond_state_loading * scale * direction).exp()
                        - 1.0)
                        / exposure.rate_accrual_year_fraction;
                    1.0 + shifted_rate * exposure.factor_accrual_year_fraction
                })
                .product::<f64>();
            (factor - 1.0) / projection.accrual_year_fraction
        };
        let h = 1.0e-5;
        let full_coupon_finite_difference = (coupon_at_state(h) - coupon_at_state(-h)) / (2.0 * h);
        let analytic_directional_loading: f64 = matched
            .observation_loadings
            .iter()
            .zip(&directions)
            .map(|(loading, direction)| loading.coupon_state_loading * direction)
            .sum();
        assert!(
            (full_coupon_finite_difference - analytic_directional_loading).abs() < 1.0e-8,
            "full compounded-coupon finite difference {full_coupon_finite_difference} should \
             match analytic directional loading {analytic_directional_loading}"
        );

        // Independent OU check: write x(t)=σ∫₀ᵗexp(-κ(t-s))dW(s), then integrate
        // the squared aggregate kernel piecewise between ordered observation times.
        let mut previous_time = 0.0;
        let mut kernel_variance = 0.0;
        for (index, loading) in matched.observation_loadings.iter().enumerate() {
            let right_time = loading.fixing_time;
            let kernel_coefficient: f64 = matched.observation_loadings[index..]
                .iter()
                .map(|item| item.coupon_state_loading * (-kappa * item.fixing_time).exp())
                .sum();
            kernel_variance += sigma
                * sigma
                * kernel_coefficient
                * kernel_coefficient
                * ((2.0 * kappa * right_time).exp() - (2.0 * kappa * previous_time).exp())
                / (2.0 * kappa);
            previous_time = right_time;
        }
        assert!(
            (matched.variance - kernel_variance).abs() < 1.0e-15,
            "OU covariance sum {} should equal independent kernel integral {}",
            matched.variance,
            kernel_variance
        );
    }

    #[test]
    fn compounded_hw_pricer_uses_cutoff_coupon_and_payment_delay() {
        let as_of = date(2024, 12, 2);
        let mut delayed = CapFloor::new_caplet(
            "SOFR-HW-COMPOUNDED",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2025, 1, 2),
            date(2025, 4, 2),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        delayed.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        delayed.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.05);
        delayed.pricing_overrides.model_config.hw1f_sigma = Some(0.012);
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.05, 5.0))
            .insert(flat_forward_with_tenor("USD-SOFR-OIS", as_of, 0.045, 5.0));
        let delayed_pv = CapFloorHullWhitePricer
            .price_internal(&delayed, &market, as_of)
            .expect("delayed HW price")
            .value
            .amount();
        let mut immediate = delayed;
        immediate
            .overnight_coupon
            .as_mut()
            .expect("overnight terms")
            .payment_delay_days = 0;
        let immediate_pv = CapFloorHullWhitePricer
            .price_internal(&immediate, &market, as_of)
            .expect("immediate HW price")
            .value
            .amount();

        assert!(delayed_pv.is_finite() && delayed_pv > 0.0);
        assert!(
            delayed_pv < immediate_pv,
            "positive-rate discounting should make the delayed contractual payment worth less: \
             delayed={delayed_pv}, immediate={immediate_pv}"
        );
    }

    #[test]
    fn fixed_unpaid_hw_caplet_matches_standard_intrinsic_value() {
        let fixing_date = date(2024, 1, 2);
        let as_of = date(2024, 2, 15);
        let payment_date = date(2024, 4, 2);
        let caplet = CapFloor::new_caplet(
            "FIXED-UNPAID-HW",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            fixing_date,
            payment_date,
            DayCount::Act360,
            "USD-OIS",
            "TEST-TERM-3M",
            "UNUSED-VOL",
        )
        .expect("caplet");
        let fixings = ScalarTimeSeries::new("FIXING:TEST-TERM-3M", vec![(fixing_date, 0.07)], None)
            .expect("fixings");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
            .insert(flat_forward_with_tenor("TEST-TERM-3M", as_of, 0.12, 5.0))
            .insert_series(fixings);

        let standard = caplet
            .value(&market, as_of)
            .expect("standard fixed-unpaid price")
            .amount();
        let hw = CapFloorHullWhitePricer
            .price_internal(&caplet, &market, as_of)
            .expect("HW fixed-unpaid price")
            .value
            .amount();

        assert!(standard > 0.0);
        assert!(
            (hw - standard).abs() < 1.0e-8,
            "HW must discount the known intrinsic payoff through payment: {hw} vs {standard}"
        );
    }

    #[test]
    fn fixed_compounded_caplet_survives_until_delayed_payment() {
        let accrual_start = date(2025, 1, 2);
        let accrual_end = date(2025, 4, 2);
        let as_of = date(2025, 4, 3);
        let mut caplet = CapFloor::new_caplet(
            "FIXED-RFR-DELAYED",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            accrual_start,
            accrual_end,
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "UNUSED-VOL",
        )
        .expect("caplet");
        caplet.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let calendar = CalendarRegistry::global()
            .resolve_str("usny")
            .expect("USNY calendar");
        let mut fixing_values = Vec::new();
        let mut observation_date = accrual_start;
        while observation_date < accrual_end {
            fixing_values.push((observation_date, 0.07));
            observation_date = observation_date
                .add_business_days(1, calendar)
                .expect("next observation")
                .min(accrual_end);
        }
        let fixings =
            ScalarTimeSeries::new("FIXING:USD-SOFR-OIS", fixing_values, None).expect("fixings");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
            .insert(flat_forward_with_tenor("USD-SOFR-OIS", as_of, 0.12, 5.0))
            .insert_series(fixings);

        let standard = caplet
            .value(&market, as_of)
            .expect("standard delayed price")
            .amount();
        let hw = CapFloorHullWhitePricer
            .price_internal(&caplet, &market, as_of)
            .expect("HW delayed price")
            .value
            .amount();

        assert!(standard > 0.0);
        assert!(
            (hw - standard).abs() < 1.0e-8,
            "fixed compounded coupon must remain payable through 2025-04-04: {hw} vs {standard}"
        );
    }
}
