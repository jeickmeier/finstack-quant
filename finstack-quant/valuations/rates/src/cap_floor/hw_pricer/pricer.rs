//! Hull-White 1F cap/floor pricer implementation.

use crate::cap_floor::pricing::CapletFloorletInputs;
use crate::cap_floor::pricing::projection::{
    OptionedCouponProjection, resolve_optioned_caplet_inputs,
};
use crate::cap_floor::types::CapFloor;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::rates::hull_white::{
    HullWhiteCalibrationParams, HullWhiteParams, TermCapletPeriod, capfloor_hw1f_scalar_keys,
    capfloor_hw1f_sigma_schedule_key, hw_b, hw1f_term_caplet_price_from_dfs_with_model,
};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing::hw1f_params::{
    Hw1fParamFamily, resolve_hw1f_params,
};
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use finstack_quant_valuations_core::results::ValuationResult;

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

fn hw1f_ou_covariance(
    params: &HullWhiteParams,
    left_time: f64,
    right_time: f64,
) -> finstack_quant_core::Result<f64> {
    params.state_covariance(left_time.max(0.0), right_time.max(0.0))
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
/// The OU covariance is the exact piecewise-volatility integral of `params`.
pub(crate) fn hw1f_compounded_rfr_moment_match(
    as_of: Date,
    params: &HullWhiteParams,
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
        let bond_state_loading = hw_b(params.kappa, 0.0, interval_time);
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
                    Ok::<f64, finstack_quant_core::Error>(
                        left.coupon_state_loading
                            * right.coupon_state_loading
                            * hw1f_ou_covariance(params, left.fixing_time, right.fixing_time)?,
                    )
                })
                .sum::<finstack_quant_core::Result<f64>>()
        })
        .sum::<finstack_quant_core::Result<f64>>()?
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
        let cap_floor = finstack_quant_valuations_core::pricer::expect_inst::<CapFloor>(
            instrument,
            InstrumentType::CapFloor,
        )?;

        self.price_internal(cap_floor, market, as_of)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let cap_floor = finstack_quant_valuations_core::pricer::expect_inst::<CapFloor>(
            instrument,
            InstrumentType::CapFloor,
        )?;

        self.price_internal_raw(cap_floor, market, as_of)
    }
}

impl CapFloorHullWhitePricer {
    /// Core pricing routine.
    pub(super) fn price_internal(
        &self,
        cap_floor: &CapFloor,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let pv_raw = self.price_internal_raw(cap_floor, market, as_of)?;
        let pv =
            Money::from_model_value(pv_raw, cap_floor.notional.currency()).map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(cap_floor))
            })?;
        Ok(ValuationResult::stamped(cap_floor.id.as_str(), as_of, pv))
    }

    fn price_internal_raw(
        &self,
        cap_floor: &CapFloor,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let ctx = DayCountContext::default();
        cap_floor.validate_for_pricing()?;

        // Standard term caplets require an explicit forward curve. Compounded
        // overnight coupons resolve through the shared projection path, which
        // can derive forwards from a single OIS discount curve.
        let fwd = if cap_floor.overnight_coupon.is_none() {
            Some(
                market
                    .get_forward(cap_floor.forward_curve_id.as_str())
                    .map_err(|e| {
                        PricingError::missing_market_data_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?,
            )
        } else {
            None
        };

        let periods = cap_floor.pricing_periods()?;

        let net_result = |option_value: f64| {
            cap_floor
                .net_of_premium_raw(option_value, market, as_of)
                .map_err(|error| {
                    PricingError::from_core(error, PricingErrorContext::from_instrument(cap_floor))
                })
        };

        if periods.is_empty() {
            return net_result(0.0);
        }

        let strike = cap_floor.strike_f64()?;
        let term_strike = strike - cap_floor.spread_rate()?;
        let notional = cap_floor.notional.amount();

        let is_cap = cap_floor.rate_option_type.is_cap();

        // Resolve a complete HW1F input from either explicit overrides or
        // pre-fitted MarketContext parameters. Missing/partial inputs fail.
        let hw_model = resolve_capfloor_hw1f_model_params(cap_floor, market, as_of)?;

        // Price each caplet/floorlet in closed form (Bachelier with the
        // HW1F-implied normal vol); no tree is built.
        let mut total_pv = 0.0;

        for period in &periods {
            if period.payment_date <= as_of {
                continue;
            }
            let resolved_inputs = resolve_optioned_caplet_inputs(cap_floor, period, market, as_of)?;
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
                let moment_match = hw1f_compounded_rfr_moment_match(as_of, &hw_model, projection)?;
                crate::cap_floor::pricing::normal::price_caplet_floorlet_raw(
                    CapletFloorletInputs {
                        is_cap,
                        notional,
                        strike,
                        forward,
                        discount_factor: df,
                        sigma: moment_match.normal_vol,
                        time_to_fixing: t_fix,
                        accrual_year_fraction: tau,
                        currency: cap_floor.notional.currency(),
                    },
                )?
            } else {
                let fwd = fwd.as_ref().ok_or_else(|| {
                    PricingError::missing_market_data_with_context(
                        format!(
                            "Forward curve '{}' is required for term-index cap/floor '{}'",
                            cap_floor.forward_curve_id, cap_floor.id
                        ),
                        PricingErrorContext::default(),
                    )
                })?;
                let projection_df_as_of = fwd.df_on_date(as_of)?;
                let pf_start = fwd.df_on_date(period.accrual_start)? / projection_df_as_of;
                let pf_end = fwd.df_on_date(period.accrual_end)? / projection_df_as_of;
                let t_end = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
                    as_of,
                    period.accrual_end,
                    ctx,
                )?;
                let t_pay = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
                    as_of,
                    period.payment_date,
                    ctx,
                )?;
                let t_start = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
                    as_of,
                    period.accrual_start,
                    ctx,
                )?;
                notional
                    * hw1f_term_caplet_price_from_dfs_with_model(
                        &hw_model,
                        &TermCapletPeriod {
                            pf_start,
                            pf_end,
                            pd_pay: df,
                            t_fix,
                            t_start,
                            t_end,
                            t_pay,
                            accrual: tau,
                        },
                        term_strike,
                        is_cap,
                    )?
            };

            total_pv += caplet_pv;
        }

        net_result(total_pv)
    }
}

/// Resolve the effective Hull-White 1F (κ, σ) the cap/floor pricer uses.
///
/// Applies the documented precedence (complete explicit overrides, then a
/// complete pre-fitted `MarketContext` pair). Sharing this with the vega
/// calculator keeps the model-consistent vega bump aligned with the sigma the
/// pricer consumes, rather than an unrelated `implied_volatility`.
pub(crate) fn resolve_capfloor_hw1f_params(
    cap_floor: &CapFloor,
    market: &MarketContext,
) -> finstack_quant_core::Result<HullWhiteCalibrationParams> {
    resolve_hw1f_params(
        Hw1fParamFamily::CapFloor,
        cap_floor.discount_curve_id.as_str(),
        &cap_floor.instrument_pricing_overrides.model_config,
        None,
        &format!("CapFloor {}", cap_floor.id),
        market,
    )
}

/// Resolve the full cap/floor HW1F model, including an explicit scheduled
/// short-rate volatility override when supplied.
pub(crate) fn resolve_capfloor_hw1f_model_params(
    cap_floor: &CapFloor,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<HullWhiteParams> {
    if let Some(schedule) = &cap_floor
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma_schedule
    {
        if cap_floor
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma
            .is_some()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' supplies both hw1f_sigma and hw1f_sigma_schedule",
                cap_floor.id
            )));
        }
        let kappa = cap_floor
            .instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CapFloor '{}' requires hw1f_mean_reversion with hw1f_sigma_schedule",
                    cap_floor.id
                ))
            })?;
        return HullWhiteParams::new(kappa, schedule.clone());
    }
    let explicit_scalar_sigma = cap_floor
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma;
    if explicit_scalar_sigma.is_none() {
        let schedule_key = capfloor_hw1f_sigma_schedule_key(cap_floor.discount_curve_id.as_str());
        if let Ok(series) = market.get_series(&schedule_key) {
            let (kappa_key, _) = capfloor_hw1f_scalar_keys(cap_floor.discount_curve_id.as_str());
            let stored_kappa = market.get_price(&kappa_key).ok().and_then(|scalar| {
                scalar
                    .get_unitless()
                    .or_else(|| scalar.get_price().map(|money| money.amount()))
                    .filter(|value| value.is_finite() && *value > 0.0)
            });
            let kappa = cap_floor
                .instrument_pricing_overrides
                .model_config
                .hw1f_mean_reversion
                .filter(|value| value.is_finite() && *value > 0.0)
                .or(stored_kappa)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CapFloor '{}' has persisted HW1F sigma schedule '{}' without kappa",
                        cap_floor.id, schedule_key
                    ))
                })?;
            let discount = market.get_discount(cap_floor.discount_curve_id.as_str())?;
            let anchor = as_of.max(discount.base_date());
            let observations = series.observations();
            let mut times = Vec::with_capacity(observations.len());
            let mut values = Vec::with_capacity(observations.len());
            if let Some((_, active_sigma)) = observations
                .iter()
                .rfind(|(date, _)| *date <= anchor)
                .or_else(|| observations.first())
            {
                times.push(0.0);
                values.push(*active_sigma);
            }
            for (date, sigma) in observations.iter().filter(|(date, _)| *date > anchor) {
                times.push(DayCount::Act365F.year_fraction(
                    anchor,
                    *date,
                    DayCountContext::default(),
                )?);
                values.push(*sigma);
            }
            return HullWhiteParams::new(
                kappa,
                finstack_quant_core::math::piecewise::PiecewiseConstantCurve::new(times, values)?,
            );
        }
    }
    HullWhiteParams::try_from(resolve_capfloor_hw1f_params(cap_floor, market)?)
}
