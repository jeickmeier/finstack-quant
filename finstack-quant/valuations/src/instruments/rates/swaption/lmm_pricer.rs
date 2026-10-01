//! Bermudan swaption pricer using LMM/BGM Monte Carlo dynamics.
//!
//! Wraps the standalone `price_bermudan_lmm` engine in the `Pricer` trait
//! so it can be dispatched via the pricing registry under
//! `(BermudanSwaption, LmmMonteCarlo)`.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::SettlementType;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::hw1f::RateExoticMcConfig;
use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::instruments::rates::swaption::pricing::lmm_bermudan::price_bermudan_lmm;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::instruments::rates::swaption::BermudanType;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::process::lmm::LmmParams;

/// Bermudan swaption pricer using LMM/BGM Monte Carlo with LSMC exercise.
///
/// Builds [`LmmParams`] from the swaption's canonical underlying fixed-leg
/// schedule from the earliest future exercise and a single discount curve,
/// then delegates to `price_bermudan_lmm` for
/// LSMC-based Bermudan exercise valuation.
///
/// # Parameter Construction
///
/// Supports physical, co-terminal exercise on tenor start dates, matching fixed
/// and simple floating schedules, zero spread/reset lag, and payments exactly
/// at accrual ends. Projection and both discount curve identifiers must match.
/// Unsupported contracts fail before simulation. A flat 2-factor loading structure is used
/// (a linear-decay proxy for the first two principal components of the
/// forward-rate correlation matrix). The *shape* of the loadings is fixed,
/// and their overall scale comes exclusively from the positive, finite
/// `model_config.lmm_base_vol` input. Calibration is an explicit upstream
/// operation; this pricer never queries a volatility surface.
pub struct BermudanSwaptionLmmPricer {
    config: RateExoticMcConfig,
}

impl Default for BermudanSwaptionLmmPricer {
    fn default() -> Self {
        Self::with_config(RateExoticMcConfig::lmm_bermudan())
    }
}

impl BermudanSwaptionLmmPricer {
    /// Create a pricer with an explicit configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Monte Carlo settings; see `price_bermudan_lmm` for the
    ///   path-count constraints. [`RateExoticMcConfig::lmm_bermudan`] gives
    ///   the registry defaults.
    pub fn with_config(config: RateExoticMcConfig) -> Self {
        Self { config }
    }

    /// Build the canonical Bermudan LMM structure with an explicit loading scale.
    ///
    /// # Arguments
    ///
    /// * `swaption` - Bermudan contract whose fixed schedule and curve roles define the tenor model.
    /// * `disc` - Discount curve used for single-curve forward initialization.
    /// * `as_of` - Valuation date used for year-fraction coordinates; exercises
    ///   on or before this date and periods preceding the next exercise are omitted.
    /// * `base_vol` - Positive finite annualized decimal loading scale.
    ///
    /// # Errors
    ///
    /// Returns a pricing error for unsupported contractual terms (see the pricer
    /// supported domain), invalid schedules or forwards, or a non-positive/non-finite
    /// loading scale, or a schedule without future exercises. Model time uses
    /// ACT/365F independently of coupon accruals.
    pub fn build_lmm_params(
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        as_of: finstack_quant_core::dates::Date,
        base_vol: f64,
    ) -> std::result::Result<LmmParams, PricingError> {
        if !base_vol.is_finite() || base_vol <= 0.0 {
            return Err(PricingError::model_failure_with_context(
                format!("LMM base_vol must be positive and finite, got {base_vol}"),
                PricingErrorContext::default(),
            ));
        }
        let fixed = &swaption.underlying_fixed_leg;
        let float = &swaption.underlying_float_leg;
        let unsupported = |reason: &str| {
            PricingError::model_failure_with_context(
                format!("LMM unsupported contract: {reason}"),
                PricingErrorContext::default(),
            )
        };
        if fixed.discount_curve_id != float.discount_curve_id
            || fixed.discount_curve_id != float.forward_curve_id
        {
            return Err(unsupported(
                "projection and both discount curves must be identical",
            ));
        }
        if swaption.settlement != SettlementType::Physical
            || swaption.bermudan_type != BermudanType::CoTerminal
        {
            return Err(unsupported("requires physical co-terminal settlement"));
        }
        if swaption.exercise_schedule.notice_days != 0 {
            return Err(unsupported("requires zero notice days"));
        }
        if !float.spread_bp.is_zero()
            || float.reset_lag_days != 0
            || float.compounding != FloatingLegCompounding::Simple
        {
            return Err(unsupported(
                "requires zero floating spread, zero reset lag, and simple compounding",
            ));
        }
        if fixed.start != float.start
            || fixed.end != float.end
            || fixed.frequency != float.frequency
            || fixed.day_count != float.day_count
            || fixed.stub != float.stub
            || fixed.end_of_month != float.end_of_month
            || fixed.business_day_convention != float.business_day_convention
            || fixed.calendar_id != float.calendar_id
            || fixed.payment_lag_days != 0
            || float.payment_lag_days != 0
        {
            return Err(unsupported(
                "fixed and floating schedules must match with zero payment lag",
            ));
        }
        let periods = swaption.fixed_schedule_periods().map_err(|e| {
            PricingError::model_failure_with_context(
                format!("LMM tenor schedule construction failed: {e}"),
                PricingErrorContext::default(),
            )
        })?;
        if periods.is_empty() {
            return Err(PricingError::model_failure_with_context(
                "LMM requires at least one fixed-leg schedule period".to_string(),
                PricingErrorContext::default(),
            ));
        }
        if periods
            .iter()
            .any(|period| period.payment_date != period.accrual_end)
        {
            return Err(unsupported(
                "payment dates must equal accrual ends without business-day displacement",
            ));
        }
        if swaption
            .exercise_schedule
            .effective_dates()
            .iter()
            .any(|date| !periods.iter().any(|period| period.accrual_start == *date))
        {
            return Err(unsupported(
                "exercise dates must equal tenor accrual starts",
            ));
        }
        let next_exercise = swaption
            .exercise_schedule
            .effective_dates()
            .into_iter()
            .find(|date| *date > as_of)
            .ok_or_else(|| unsupported("requires at least one future exercise date"))?;
        let original_num_forwards = periods.len();
        let first_live = periods.partition_point(|period| period.accrual_start < next_exercise);
        let periods = &periods[first_live..];
        let mut tenor_dates = Vec::with_capacity(periods.len() + 1);
        tenor_dates.push(next_exercise);
        tenor_dates.extend(periods.iter().map(|period| period.accrual_end));

        let tenors: Vec<f64> = tenor_dates
            .iter()
            .map(|&date| year_fraction(DayCount::Act365F, as_of, date))
            .collect::<finstack_quant_core::Result<Vec<_>>>()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let num_forwards = tenors.len() - 1;
        if num_forwards == 0 {
            return Err(PricingError::model_failure_with_context(
                "LMM requires at least one forward rate period".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Contractual accruals come from the canonical fixed-leg schedule and
        // therefore retain its day count, stubs, EOM rule, and calendar policy.
        let accrual_factors = periods
            .iter()
            .map(|period| period.accrual_year_fraction)
            .collect::<Vec<_>>();

        // Initialize each LMM tenor forward from its canonical market role.
        let mut initial_forwards: Vec<f64> = Vec::with_capacity(num_forwards);
        for i in 0..num_forwards {
            let tau = accrual_factors[i];
            if !tau.is_finite() || tau <= 0.0 {
                return Err(PricingError::model_failure_with_context(
                    format!("LMM schedule has invalid accrual in period {i}: {tau}"),
                    PricingErrorContext::default(),
                ));
            }
            let fwd = {
                let df_start = disc.df_between_dates(as_of, tenor_dates[i]).map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                let df_end = disc
                    .df_between_dates(as_of, tenor_dates[i + 1])
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                if !df_start.is_finite() || !df_end.is_finite() || df_start <= 0.0 || df_end <= 0.0
                {
                    return Err(PricingError::model_failure_with_context(
                        format!(
                            "LMM forward bootstrap has invalid discount factors in period {i}: \
                             df_start={df_start}, df_end={df_end}"
                        ),
                        PricingErrorContext::default(),
                    ));
                }
                (df_start / df_end - 1.0) / tau
            };
            if !fwd.is_finite() {
                return Err(PricingError::model_failure_with_context(
                    format!("LMM forward initialization is non-finite in period {i}: {fwd}"),
                    PricingErrorContext::default(),
                ));
            }
            initial_forwards.push(fwd);
        }

        // Displacement (shifted-lognormal shift). A small positive shift is
        // needed only when forwards can approach or cross zero; for a
        // comfortably-positive curve a pure lognormal model (zero shift) is
        // consistent with the lognormal Black swaption surface the
        // calibration targets. Pick the shift from the realised forwards
        // instead of hardcoding a magic constant.
        let min_forward = initial_forwards
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let shift = if min_forward > 0.01 {
            0.0
        } else {
            // Lift the most negative/near-zero forward to a +1% effective
            // floor so the displaced-lognormal diffusion stays well posed.
            (0.01 - min_forward).max(0.0)
        };
        let displacements = vec![shift; num_forwards];

        // Flat 2-factor loading structure with linear decay (the *shape*):
        //   ĝ_i = [1 - alpha * i/N, alpha * i/N, 0]
        // This approximates the first two principal components of swaption
        // correlation matrices. The full loading is `lambda_i = base_vol * ĝ_i`.
        let alpha = 0.4; // decay parameter; scale is supplied explicitly
        let loading_shapes: Vec<[f64; 3]> = (0..num_forwards)
            .map(|i| {
                // Preserve the contractual tenor's loading shape as the
                // remaining model is rolled past earlier exercise dates.
                let frac = (first_live + i) as f64 / original_num_forwards as f64;
                [1.0 - alpha * frac, alpha * frac, 0.0]
            })
            .collect();

        let vol_row: Vec<[f64; 3]> = loading_shapes
            .iter()
            .map(|g| [base_vol * g[0], base_vol * g[1], base_vol * g[2]])
            .collect();
        let vol_values = vec![vol_row]; // single vol period (no breakpoints)
        let vol_times: Vec<f64> = vec![]; // empty => single period

        LmmParams {
            num_forwards,
            num_factors: 2,
            tenors,
            accrual_factors,
            displacements,
            vol_times,
            vol_values,
            initial_forwards,
        }
        .validate()
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }
}

impl Pricer for BermudanSwaptionLmmPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BermudanSwaption, ModelKey::LmmMonteCarlo)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption = crate::pricer::expect_inst::<BermudanSwaption>(
            instrument,
            InstrumentType::BermudanSwaption,
        )?;

        let disc = market
            .get_discount(swaption.get_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if ttm <= 0.0 {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
            ));
        }

        let exercise_times = swaption
            .exercise_schedule
            .exercise_times(as_of, DayCount::Act365F)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
            ));
        }

        let base_vol = swaption
            .instrument_pricing_overrides
            .model_config
            .lmm_base_vol
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                PricingError::model_failure_with_context(
                    format!(
                        "Bermudan swaption '{}' requires positive finite \
                         instrument_pricing_overrides.model_config.lmm_base_vol; calibrate it upstream",
                        swaption.id
                    ),
                    PricingErrorContext::default(),
                )
            })?;
        let lmm_params = Self::build_lmm_params(swaption, disc.as_ref(), as_of, base_vol)?;

        // Strike and payer/receiver flag
        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let is_payer =
            swaption.option_type == crate::instruments::common_impl::parameters::OptionType::Call;
        let notional = swaption.notional.amount();
        let currency = swaption.notional.currency();

        // Terminal discount factor P(0, T_N) for the last tenor
        let df_terminal = disc
            .df_between_dates(as_of, swaption.get_underlying_maturity())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Price via LSMC with LMM dynamics
        let estimate = price_bermudan_lmm(
            &lmm_params,
            &exercise_times,
            strike,
            is_payer,
            notional,
            df_terminal,
            currency,
            &self.config,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, estimate.mean);
        if estimate.stderr > 0.0 {
            result.measures.insert(
                crate::metrics::MetricId::custom("mc_stderr"),
                estimate.stderr,
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::swaption::BermudanSchedule;
    use crate::instruments::{InstrumentPricingOverrides, OptionType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, Tenor};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use rust_decimal::Decimal;
    use time::macros::date;

    fn supported_contract() -> (Date, BermudanSwaption, MarketContext) {
        let as_of = date!(2025 - 01 - 02);
        let start = date!(2026 - 01 - 02);
        let end = date!(2031 - 01 - 02);
        let mut option = BermudanSwaption::new(
            "LMM-DOMAIN",
            OptionType::Call,
            Money::from((10_000_000_i64, Currency::USD)),
            0.02,
            start,
            end,
            BermudanSchedule::new(vec![start]),
            "D",
            "D",
            "UNUSED",
        )
        .expect("valid contract");
        option.underlying_fixed_leg.frequency = Tenor::annual();
        option.underlying_fixed_leg.day_count = DayCount::Act360;
        option.underlying_fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
        option.underlying_float_leg.frequency = Tenor::annual();
        option.underlying_float_leg.day_count = DayCount::Act360;
        option.underlying_float_leg.business_day_convention = BusinessDayConvention::Unadjusted;
        option
            .instrument_pricing_overrides
            .model_config
            .lmm_base_vol = Some(1e-10);
        let discount = DiscountCurve::builder("D")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.97), (3.0, 0.88), (7.0, 0.70)])
            .build()
            .expect("discount curve");
        (as_of, option, MarketContext::new().insert(discount))
    }

    #[test]
    fn lmm_supported_domain_reproduces_contract_and_terminal_bond() {
        let (as_of, option, market) = supported_contract();
        let discount = market.get_discount("D").expect("discount");
        let params =
            BermudanSwaptionLmmPricer::build_lmm_params(&option, discount.as_ref(), as_of, 1e-10)
                .expect("supported domain");
        // Accrual convention and model clock are independent.
        assert_eq!(params.tenors[0], 1.0);
        assert!((params.accrual_factors[0] - 365.0 / 360.0).abs() < 1e-14);
        let forward_bond: f64 = params
            .initial_forwards
            .iter()
            .zip(&params.accrual_factors)
            .map(|(forward, tau)| 1.0 / (1.0 + tau * forward))
            .product();
        let p_start = discount
            .df_between_dates(as_of, option.get_underlying_start_date())
            .expect("start DF");
        let p_end = discount
            .df_between_dates(as_of, option.get_underlying_maturity())
            .expect("end DF");
        assert!((forward_bond - p_end / p_start).abs() < 1e-14);
        let pricer = BermudanSwaptionLmmPricer::with_config(RateExoticMcConfig {
            num_paths: 4,
            antithetic: true,
            oos_lsmc: false,
            ..Default::default()
        });
        for option_type in [OptionType::Call, OptionType::Put] {
            let mut contract = option.clone();
            contract.option_type = option_type;
            // Keep each side in the money in its own comparison.
            if option_type == OptionType::Put {
                contract.underlying_fixed_leg.rate = Decimal::new(8, 2);
            }
            let actual = pricer
                .price_dyn(&contract, &market, as_of)
                .expect("LMM price")
                .value
                .amount();
            let mut european = contract.to_european().expect("European contract");
            european.instrument_pricing_overrides =
                InstrumentPricingOverrides::default().with_implied_volatility(0.0);
            let expected = european
                .value(&market, as_of)
                .expect("deterministic price")
                .amount();
            assert!(
                (actual - expected).abs() < 0.01,
                "{option_type:?}: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn lmm_seasoned_contract_prices_only_remaining_exercises() {
        let (original_as_of, mut option, market) = supported_contract();
        let first = option.get_underlying_start_date();
        let next = date!(2027 - 01 - 02);
        let last = date!(2028 - 01 - 02);
        option.exercise_schedule = BermudanSchedule::new(vec![first, next, last]);
        let discount = market.get_discount("D").expect("discount");
        let original = BermudanSwaptionLmmPricer::build_lmm_params(
            &option,
            discount.as_ref(),
            original_as_of,
            1e-10,
        )
        .expect("original structure");
        let periods = option
            .fixed_schedule_periods()
            .expect("contractual periods");
        let pricer = BermudanSwaptionLmmPricer::with_config(RateExoticMcConfig {
            num_paths: 4,
            min_steps_between_events: 1,
            antithetic: true,
            oos_lsmc: false,
            ..Default::default()
        });

        for as_of in [first, date!(2026 - 07 - 02), next] {
            let params = BermudanSwaptionLmmPricer::build_lmm_params(
                &option,
                discount.as_ref(),
                as_of,
                1e-10,
            )
            .expect("remaining structure");
            let future_exercises: Vec<_> = option
                .exercise_schedule
                .effective_dates()
                .into_iter()
                .filter(|date| *date > as_of)
                .collect();
            let first_live =
                periods.partition_point(|period| period.accrual_start < future_exercises[0]);
            assert_eq!(
                params.tenors[0],
                year_fraction(DayCount::Act365F, as_of, future_exercises[0])
                    .expect("next exercise time")
            );
            assert_eq!(params.num_forwards, periods.len() - first_live);
            assert_eq!(params.vol_values[0], original.vol_values[0][first_live..]);
            assert!(params.tenors.iter().all(|time| *time > 0.0));

            // In the zero-volatility limit, the optimal exercise is the
            // largest discounted remaining swap payoff on the live schedule.
            let terminal_df = discount
                .df_between_dates(as_of, option.get_underlying_maturity())
                .expect("terminal discount");
            let expected = future_exercises
                .into_iter()
                .map(|exercise| {
                    let annuity: f64 = periods
                        .iter()
                        .filter(|period| period.accrual_start >= exercise)
                        .map(|period| {
                            period.accrual_year_fraction
                                * discount
                                    .df_between_dates(as_of, period.payment_date)
                                    .expect("payment discount")
                        })
                        .sum();
                    let start_df = discount
                        .df_between_dates(as_of, exercise)
                        .expect("exercise discount");
                    (start_df - terminal_df - 0.02 * annuity).max(0.0) * option.notional.amount()
                })
                .fold(0.0_f64, f64::max);
            let actual = pricer
                .price_dyn(&option, &market, as_of)
                .expect("seasoned LMM price")
                .value
                .amount();
            assert!(
                (actual - expected).abs() < 0.01,
                "as_of={as_of}: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn lmm_without_remaining_exercises_is_zero_without_loading_override() {
        let (_, mut option, market) = supported_contract();
        let as_of = option.last_exercise().expect("exercise date");
        option
            .instrument_pricing_overrides
            .model_config
            .lmm_base_vol = None;
        let result = BermudanSwaptionLmmPricer::default()
            .price_dyn(&option, &market, as_of)
            .expect("expired exercise schedule");
        assert_eq!(result.value.amount(), 0.0);
    }

    #[test]
    fn lmm_rejects_unrepresented_contract_terms_before_simulation() {
        let (as_of, option, market) = supported_contract();
        let discount = market.get_discount("D").expect("discount");
        type ContractMutation = fn(&mut BermudanSwaption);
        let cases: &[(&str, ContractMutation)] = &[
            ("projection", |s| {
                s.underlying_float_leg.forward_curve_id = "F".into()
            }),
            ("spread", |s| {
                s.underlying_float_leg.spread_bp = Decimal::from(100)
            }),
            ("reset", |s| s.underlying_float_leg.reset_lag_days = 2),
            ("compounding", |s| {
                s.underlying_float_leg.compounding =
                    FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
            }),
            ("schedules", |s| {
                s.underlying_float_leg.frequency = Tenor::quarterly()
            }),
            ("schedules", |s| s.underlying_fixed_leg.payment_lag_days = 2),
            ("schedules", |s| s.underlying_float_leg.payment_lag_days = 2),
            ("schedules", |s| {
                s.underlying_float_leg.day_count = DayCount::Act365F
            }),
            ("physical", |s| s.settlement = SettlementType::Cash),
            ("co-terminal", |s| {
                s.bermudan_type = BermudanType::NonCoTerminal
            }),
            ("exercise", |s| {
                s.exercise_schedule.exercise_dates[0] += time::Duration::days(1)
            }),
            ("notice", |s| s.exercise_schedule.notice_days = 2),
            ("payment dates", |s| {
                s.underlying_fixed_leg.business_day_convention = BusinessDayConvention::Following;
                s.underlying_float_leg.business_day_convention = BusinessDayConvention::Following;
            }),
        ];
        for (message, mutate) in cases {
            let mut invalid = option.clone();
            mutate(&mut invalid);
            let error = BermudanSwaptionLmmPricer::build_lmm_params(
                &invalid,
                discount.as_ref(),
                as_of,
                1e-10,
            )
            .expect_err("unrepresented contract must fail")
            .to_string();
            assert!(error.contains(message), "expected {message:?} in {error:?}");
        }
    }
}
