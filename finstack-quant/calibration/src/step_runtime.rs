//! Shared runtime types and solver contracts for market calibration.
//!
use crate::api::schema::{
    CalibrationStep, CapFloorHullWhiteStepParams, HullWhiteVolatilityMode, StepParams,
};
use crate::config::CalibrationConfig;
use crate::hull_white::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn,
    calibrate_hull_white_to_cap_floors_with_fn, calibrate_hull_white_to_swaptions_with_fn,
    capfloor_hw1f_scalar_keys, capfloor_hw1f_sigma_schedule_key, hw1f_scalar_keys,
    CapFloorCalibrationConfig, CapFloorQuote, CapFloorSchedule, CapletSchedule,
    PiecewiseSigmaCalibrationConfig, SwapFrequency, SwaptionFloatingPeriod, SwaptionQuote,
    SwaptionSchedule,
};
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::vol::VolQuote;
use crate::targets::base_correlation::BaseCorrelationTarget;
use crate::targets::discount::DiscountCurveTarget;
use crate::targets::forward::ForwardCurveTarget;
use crate::targets::hazard::HazardCurveTarget;
use crate::targets::inflation::InflationCurveTarget;
use crate::targets::parametric::ParametricCurveTarget;
use crate::targets::student_t::StudentTTarget;
use crate::targets::svi::SviSurfaceTarget;
use crate::targets::swaption::SwaptionVolTarget;
use crate::targets::vol::VolSurfaceTarget;
use crate::targets::xccy_basis::XccyBasisTarget;
use crate::validation::surfaces::validate_surface;
use crate::validation::CurveValidator;
use crate::validation::ValidationMode;
use crate::CalibrationReport;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::explain::TraceEntry;
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{VolCube, VolQuoteType, VolSurface};
use finstack_quant_core::market_data::term_structures::{CreditIndexData, DiscountCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_models::rates::clock::model_time_on_curve;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use std::sync::Arc;

/// Normalized output payload for a step.
pub(crate) enum StepOutput {
    Curve(CurveStorage),
    Curves(Vec<CurveStorage>),
    Surface(Arc<VolSurface>),
    VolCube(Arc<VolCube>),
    Scalars(Vec<(String, MarketScalar)>),
    ScalarsAndSeries {
        scalars: Vec<(String, MarketScalar)>,
        series: ScalarTimeSeries,
    },
}

/// Aggregated outcome of a single calibration step.
pub(crate) struct StepOutcome {
    pub output: StepOutput,
    pub credit_index_update: Option<(String, CreditIndexData)>,
    pub report: CalibrationReport,
}

fn attach_validation_result(
    report: CalibrationReport,
    validation: Result<()>,
    global_config: &CalibrationConfig,
) -> CalibrationReport {
    match validation {
        Ok(()) => report.with_validation_result(true, None),
        Err(err) => match global_config.validation_mode {
            ValidationMode::Error => report.with_validation_result(false, Some(err.to_string())),
            ValidationMode::Warn => {
                let mut report = report;
                report.update_metadata("validation_warning", err.to_string());
                report
            }
        },
    }
}

/// Hull-White quote structs carry a normal/Black flag but no displacement.
/// Reject shifted coordinates before a quote can be reinterpreted as Black.
fn hw_quote_is_normal(quote_type: VolQuoteType) -> Result<bool> {
    match quote_type {
        VolQuoteType::Normal => Ok(true),
        VolQuoteType::BlackLognormal => Ok(false),
        VolQuoteType::ShiftedBlackLognormal => Err(finstack_quant_core::Error::Validation(
            "Hull-White calibration does not accept shifted Black quotes: its quote contract has no displacement; supply normal or unshifted Black quotes".to_string(),
        )),
    }
}

fn prepare_hw_swaption_input(
    vol_quote: &VolQuote,
    disc_curve: &DiscountCurve,
    expected_currency: Currency,
    base_date: finstack_quant_core::dates::Date,
) -> Result<(SwaptionQuote, SwaptionSchedule)> {
    let VolQuote::SwaptionVol {
        expiry,
        maturity,
        strike,
        vol,
        quote_type,
        ..
    } = vol_quote
    else {
        return Err(finstack_quant_core::Error::Validation(
            "Hull-White calibration expected a swaption volatility quote".to_string(),
        ));
    };
    vol_quote.validate()?;
    let is_normal_vol = hw_quote_is_normal(*quote_type)?;
    let conventions = SwaptionVolTarget::resolve_quote_leg_conventions(vol_quote)?;
    if conventions.currency != expected_currency {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Hull-White step currency {expected_currency} conflicts with swaption convention currency {}",
            conventions.currency
        )));
    }

    let (swap_start, swap_end) =
        SwaptionVolTarget::resolve_underlying_dates(vol_quote, &conventions)?;
    let periods = SwaptionVolTarget::build_fixed_leg_periods(swap_start, swap_end, &conventions)?;
    let floating_periods =
        SwaptionVolTarget::build_float_leg_periods(swap_start, swap_end, &conventions)?;
    use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
    let floating_is_compounded = match conventions.float_compounding {
        FloatingLegCompounding::Simple => false,
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
        | FloatingLegCompounding::CompoundedWithObservationShift { shift_days: 0 }
        | FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 0 } => true,
        _ => {
            return Err(finstack_quant_core::Error::Validation(
                "Hull-White swaption calibration requires simple coupons or unshifted compounded overnight coupons".into(),
            ));
        }
    };
    if periods.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption quote {expiry} to {maturity} produced an empty fixed-leg schedule"
        )));
    }

    let model_curve =
        finstack_quant_models::rates::clock::ModelDiscountCurve::new(disc_curve, base_date)?;
    let time_from_base = |date| -> Result<f64> {
        Ok(finstack_quant_models::rates::clock::model_time(
            base_date, date,
        ))
    };
    let expiry_time = time_from_base(*expiry)?;
    let swap_start_time = time_from_base(swap_start)?;
    let maturity_time = time_from_base(swap_end)?;
    let tenor = finstack_quant_models::rates::clock::model_time(swap_start, swap_end);
    if expiry_time <= 0.0 || tenor <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption quote must expire after the calibration date and have positive tenor; expiry={expiry}, maturity={maturity}"
        )));
    }

    let payment_times = periods
        .iter()
        .map(|period| time_from_base(period.payment_date))
        .collect::<Result<Vec<_>>>()?;
    let accruals: Vec<f64> = periods
        .iter()
        .map(|period| period.accrual_year_fraction)
        .collect();
    let floating_periods = floating_periods
        .iter()
        .map(|period| {
            Ok(SwaptionFloatingPeriod {
                fixing_time: time_from_base(if floating_is_compounded {
                    period.accrual_start
                } else {
                    period.reset_date.unwrap_or(period.accrual_start)
                })?,
                start_time: time_from_base(period.accrual_start)?,
                end_time: time_from_base(period.accrual_end)?,
                payment_time: time_from_base(period.payment_date)?,
                accrual: period.accrual_year_fraction,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let annuity: f64 = payment_times
        .iter()
        .zip(&accruals)
        .map(|(time, accrual)| model_curve.get_df(*time).unwrap_or(f64::NAN) * accrual)
        .sum();
    let floating_pv = floating_periods
        .iter()
        .map(|period| {
            Ok(
                (model_curve.get_df(period.start_time)? / model_curve.get_df(period.end_time)?
                    - 1.0)
                    * model_curve.get_df(period.payment_time)?,
            )
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum::<f64>();
    let forward = floating_pv / annuity;
    // SwaptionQuote is an ATM-only contract. Never reinterpret an explicit
    // off-ATM strike as the forward rate. Tolerance is 0.0001 basis points.
    if !forward.is_finite() || (*strike - forward).abs() > 1e-8 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Hull-White calibration requires ATM swaption quotes: strike {strike:.12} differs from contractual forward {forward:.12} (tolerance 1e-8 in decimal rate units)"
        )));
    }

    Ok((
        SwaptionQuote {
            expiry: expiry_time,
            tenor,
            volatility: *vol,
            is_normal_vol,
        },
        SwaptionSchedule {
            swap_start_time,
            payment_times,
            accruals,
            maturity_time,
            floating_periods,
            floating_is_compounded,
        },
    ))
}

fn prepare_hw_cap_schedule(
    params: &CapFloorHullWhiteStepParams,
    maturity: finstack_quant_core::dates::Date,
) -> Result<CapFloorSchedule> {
    use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
    use finstack_quant_core::dates::{adjust, DateExt, StubKind};
    use finstack_quant_models::rates::clock::model_time;
    use finstack_quant_valuations::market::conventions::{ConventionRegistry, RateIndexKind};
    let convention = ConventionRegistry::try_global()?.require_rate_index(&params.index_id)?;
    if convention.currency != params.currency {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor HW currency {} conflicts with index '{}' currency {}",
            params.currency, params.index_id, convention.currency
        )));
    }
    if convention.kind != RateIndexKind::Term {
        return Err(finstack_quant_core::Error::Validation(
            "cap/floor HW calibration requires a term index; compounded overnight quote calibration is not supported".into(),
        ));
    }
    let calendar = finstack_quant_cashflows::builder::calendar::resolve_calendar_strict(
        &convention.market_calendar_id,
    )?;
    let start = adjust(
        params
            .base_date
            .add_business_days(convention.market_settlement_days, calendar)?,
        convention.market_business_day_convention,
        calendar,
    )?;
    let periods = build_periods(BuildPeriodsParams {
        start,
        end: maturity,
        frequency: convention.default_payment_frequency,
        stub: StubKind::ShortBack,
        business_day_convention: convention.market_business_day_convention,
        calendar_id: &convention.market_calendar_id,
        end_of_month: false,
        day_count: convention.day_count,
        payment_lag_days: convention.default_payment_lag_days,
        reset_lag_days: Some(convention.default_reset_lag_days),
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    })?;
    Ok(CapFloorSchedule {
        periods: periods
            .iter()
            // Standard term cap quotes exclude the first spot-start coupon.
            .skip(1)
            .map(|period| CapletSchedule {
                fixing_time: model_time(
                    params.base_date,
                    period.reset_date.unwrap_or(period.accrual_start),
                ),
                start_time: model_time(params.base_date, period.accrual_start),
                end_time: model_time(params.base_date, period.accrual_end),
                payment_time: model_time(params.base_date, period.payment_date),
                accrual: period.accrual_year_fraction,
            })
            .collect(),
    })
}

/// Apply a normalized step output into the mutable market context.
pub(crate) fn apply_output(
    context: &mut MarketContext,
    output: StepOutput,
    credit_index_update: Option<(String, CreditIndexData)>,
) -> Result<()> {
    match output {
        StepOutput::Curve(curve) => {
            *context = std::mem::take(context).insert(curve);
        }
        StepOutput::Curves(curves) => {
            let mut updated = std::mem::take(context);
            for curve in curves {
                updated = updated.insert(curve);
            }
            *context = updated;
        }
        StepOutput::Surface(surface) => {
            *context = std::mem::take(context).insert_surface(surface);
        }
        StepOutput::VolCube(cube) => {
            *context = std::mem::take(context).insert_vol_cube(cube);
        }
        StepOutput::Scalars(values) => {
            let mut updated = std::mem::take(context);
            for (key, value) in values {
                updated = updated.insert_price(&key, value);
            }
            *context = updated;
        }
        StepOutput::ScalarsAndSeries { scalars, series } => {
            let mut updated = std::mem::take(context);
            for (key, value) in scalars {
                updated = updated.insert_price(&key, value);
            }
            *context = updated.insert_series(series);
        }
    }

    if let Some((id, data)) = credit_index_update {
        context.insert_credit_index_mut(id, data)?;
    }
    Ok(())
}

/// Execute calibration logic for the provided [`StepParams`].
pub(crate) fn execute_params(
    params: &StepParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<StepOutcome> {
    for quote in quotes {
        quote.validate()?;
    }
    match params {
        StepParams::Discount(p) => {
            let (ctx, report) = DiscountCurveTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_discount(&p.curve_id)?;
            let output = StepOutput::Curve(Arc::clone(&curve).into());
            let report = attach_validation_result(
                report,
                curve.validate(&global_config.validation),
                global_config,
            );
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
        StepParams::Forward(p) => {
            let (ctx, report) = ForwardCurveTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_forward(&p.curve_id)?;
            let output = StepOutput::Curve(Arc::clone(&curve).into());
            let report = attach_validation_result(
                report,
                curve.validate(&global_config.validation),
                global_config,
            );
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
        StepParams::Hazard(p) => {
            let (ctx, report) = HazardCurveTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_hazard(&p.curve_id)?;
            let output = StepOutput::Curve(Arc::clone(&curve).into());
            let mut validation_cfg = global_config.validation.clone();
            if quotes.iter().any(|quote| match quote {
                MarketQuote::Cds(crate::quotes::cds::CdsQuote::CdsParSpread {
                    spread_bp, ..
                })
                | MarketQuote::Cds(crate::quotes::cds::CdsQuote::CdsUpfront {
                    coupon_bp: spread_bp,
                    ..
                }) => *spread_bp >= 1_000.0,
                _ => false,
            }) {
                validation_cfg.max_hazard_rate = validation_cfg.max_hazard_rate.max(2.0);
            }
            let report =
                attach_validation_result(report, curve.validate(&validation_cfg), global_config);
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
        StepParams::Inflation(p) => {
            let (ctx, report) = InflationCurveTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_inflation_curve(&p.curve_id)?;
            let output = StepOutput::Curve(Arc::clone(&curve).into());
            let report = attach_validation_result(
                report,
                curve.validate(&global_config.validation),
                global_config,
            );
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
        StepParams::BaseCorrelation(p) => {
            let (ctx, report) = BaseCorrelationTarget::solve(p, quotes, context, global_config)?;
            let curve_id = CurveId::from(format!("{}_CORR", p.index_id));
            let curve = ctx.get_base_correlation(curve_id.as_str())?;
            let output = StepOutput::Curve(Arc::clone(&curve).into());
            let report = attach_validation_result(
                report,
                curve.validate(&global_config.validation),
                global_config,
            );
            let credit_index_update = ctx
                .get_credit_index(&p.index_id)
                .ok()
                .map(|idx| (p.index_id.clone(), idx.as_ref().clone()));
            Ok(StepOutcome {
                output,
                credit_index_update,
                report,
            })
        }
        StepParams::VolSurface(p) => {
            let (surface, report) = VolSurfaceTarget::solve(p, quotes, context, global_config)?;
            let mut new_report = report;
            new_report
                .explanation
                .get_or_insert_with(|| {
                    finstack_quant_core::explain::ExplanationTrace::new("vol_surface")
                })
                .push(
                    TraceEntry::ComputationStep {
                        name: "surface_built".to_string(),
                        description: "Vol surface constructed".to_string(),
                        metadata: None,
                    },
                    global_config.explain.max_entries,
                );
            let new_report = attach_validation_result(
                new_report,
                validate_surface(&surface, &global_config.validation),
                global_config,
            );
            Ok(StepOutcome {
                output: StepOutput::Surface(surface.into()),
                credit_index_update: None,
                report: new_report,
            })
        }
        StepParams::SwaptionVol(p) => {
            let (cube, report) = SwaptionVolTarget::solve(p, quotes, context, global_config)?;
            Ok(StepOutcome {
                output: StepOutput::VolCube(cube.into()),
                credit_index_update: None,
                report,
            })
        }
        StepParams::StudentT(p) => {
            let (_, calibrated_df, report) =
                StudentTTarget::solve(p, quotes, context, global_config)?;
            let scalar_key = format!("{}_STUDENT_T_DF", p.tranche_instrument_id);
            Ok(StepOutcome {
                output: StepOutput::Scalars(vec![(
                    scalar_key,
                    MarketScalar::Unitless(calibrated_df),
                )]),
                credit_index_update: None,
                report,
            })
        }
        StepParams::HullWhite(p) => {
            let disc_curve = context.get_discount(&p.curve_id)?;
            let model_curve = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                disc_curve.as_ref(),
                p.base_date,
            )?;
            let df = |t| model_curve.get_df(t).unwrap_or(f64::NAN);

            let mut hw_quotes = Vec::new();
            let mut hw_schedules = Vec::new();
            for quote in quotes {
                let MarketQuote::Vol(vol_quote @ VolQuote::SwaptionVol { .. }) = quote else {
                    continue;
                };
                let (prepared_quote, schedule) = prepare_hw_swaption_input(
                    vol_quote,
                    disc_curve.as_ref(),
                    p.currency,
                    p.base_date,
                )?;
                hw_quotes.push(prepared_quote);
                hw_schedules.push(schedule);
            }

            let initial_guess = match (p.initial_kappa, p.initial_sigma) {
                (Some(kappa), Some(sigma)) => Some(HullWhiteCalibrationParams::new(kappa, sigma)?),
                (None, None) => None,
                _ => {
                    return Err(finstack_quant_core::Error::Validation(
                        "Hull-White calibration requires both `initial_kappa` and `initial_sigma` when overriding defaults"
                            .to_string(),
                    ))
                }
            };
            let (hw_params, report) = calibrate_hull_white_to_swaptions_with_fn(
                &df,
                &hw_quotes,
                SwapFrequency::Annual,
                Some(&hw_schedules),
                initial_guess,
                p.fit_tolerance,
            )?;

            let (kappa_key, sigma_key) = hw1f_scalar_keys(p.curve_id.as_str());
            Ok(StepOutcome {
                output: StepOutput::Scalars(vec![
                    (kappa_key, MarketScalar::Unitless(hw_params.kappa)),
                    (sigma_key, MarketScalar::Unitless(hw_params.sigma)),
                ]),
                credit_index_update: None,
                report,
            })
        }
        StepParams::CapFloorHullWhite(p) => {
            let disc_curve = context.get_discount(&p.discount_curve_id)?;
            let discount_time = |t| {
                model_time_on_curve(
                    p.base_date,
                    t,
                    disc_curve.base_date(),
                    disc_curve.day_count(),
                )
            };
            let discount_base_time = discount_time(0.0)?;
            // Quote/model time is ACT/365F from p.base_date. Both supplied
            // functions must return factors relative to that same origin.
            let discount_df = |t: f64| {
                discount_time(t)
                    .and_then(|end| disc_curve.df_between_times(discount_base_time, end))
                    .unwrap_or(f64::NAN)
            };
            let forward_curve = if p.forward_curve_id == p.discount_curve_id {
                None
            } else {
                Some(context.get_forward(&p.forward_curve_id)?)
            };
            let forward_base_df = match &forward_curve {
                Some(curve) => curve.df(model_time_on_curve(
                    p.base_date,
                    0.0,
                    curve.base_date(),
                    curve.day_count(),
                )?)?,
                None => 1.0,
            };
            let forward_df = |t: f64| -> f64 {
                let Some(curve) = forward_curve.as_ref() else {
                    return discount_df(t);
                };
                model_time_on_curve(p.base_date, t, curve.base_date(), curve.day_count())
                    .and_then(|time| curve.df(time))
                    .map(|df| df / forward_base_df)
                    .unwrap_or(f64::NAN)
            };
            let day_count = DayCount::Act365F;

            let mut cap_floor_quotes = Vec::new();
            let mut cap_floor_schedules = Vec::new();
            for quote in quotes {
                let MarketQuote::Vol(VolQuote::CapFloorVol {
                    expiry,
                    strike,
                    vol,
                    quote_type,
                    is_cap,
                    ..
                }) = quote
                else {
                    continue;
                };
                let is_normal_vol = hw_quote_is_normal(*quote_type)?;
                let maturity =
                    day_count.year_fraction(p.base_date, *expiry, DayCountContext::default())?;
                if maturity <= 0.0 {
                    continue;
                }
                cap_floor_quotes.push(CapFloorQuote {
                    maturity,
                    strike: *strike,
                    volatility: *vol,
                    is_cap: *is_cap,
                    is_normal_vol,
                });
                cap_floor_schedules.push(prepare_hw_cap_schedule(p, *expiry)?);
            }

            let initial_guess = match (p.initial_kappa, p.initial_sigma) {
                (Some(kappa), Some(sigma)) => Some(HullWhiteCalibrationParams::new(kappa, sigma)?),
                (None, None) => None,
                _ => {
                    return Err(finstack_quant_core::Error::Validation(
                        "Cap/floor Hull-White calibration requires both `initial_kappa` and `initial_sigma` when overriding defaults"
                            .to_string(),
                    ))
                }
            };
            let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys(p.discount_curve_id.as_str());
            match p.volatility_mode {
                HullWhiteVolatilityMode::Scalar => {
                    let (hw_params, report) = calibrate_hull_white_to_cap_floors_with_fn(
                        &discount_df,
                        &forward_df,
                        &cap_floor_quotes,
                        Some(&cap_floor_schedules),
                        CapFloorCalibrationConfig {
                            fit_tolerance: p.fit_tolerance,
                            frequency: SwapFrequency::Annual,
                            fixed_kappa: p.fixed_kappa,
                            initial_guess,
                        },
                    )?;
                    Ok(StepOutcome {
                        output: StepOutput::Scalars(vec![
                            (kappa_key, MarketScalar::Unitless(hw_params.kappa)),
                            (sigma_key, MarketScalar::Unitless(hw_params.sigma)),
                        ]),
                        credit_index_update: None,
                        report,
                    })
                }
                HullWhiteVolatilityMode::Piecewise => {
                    let fixed_kappa = p.fixed_kappa.ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "piecewise cap/floor HW1F calibration requires fixed_kappa".into(),
                        )
                    })?;
                    let (model, report) =
                        bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn(
                            &discount_df,
                            &forward_df,
                            &cap_floor_quotes,
                            Some(&cap_floor_schedules),
                            PiecewiseSigmaCalibrationConfig {
                                fit_tolerance: p.fit_tolerance,
                                fixed_kappa,
                                sigma_min: 1.0e-5,
                                sigma_max: 2.0,
                                frequency: SwapFrequency::Annual,
                            },
                        )?;
                    let observations = model
                        .volatility
                        .times()
                        .iter()
                        .zip(model.volatility.values())
                        .map(|(&time, &sigma)| {
                            (
                                p.base_date + time::Duration::days((time * 365.0).round() as i64),
                                sigma,
                            )
                        })
                        .collect();
                    let series = ScalarTimeSeries::new(
                        capfloor_hw1f_sigma_schedule_key(p.discount_curve_id.as_str()),
                        observations,
                        None,
                    )?;
                    Ok(StepOutcome {
                        output: StepOutput::ScalarsAndSeries {
                            scalars: vec![(kappa_key, MarketScalar::Unitless(model.kappa))],
                            series,
                        },
                        credit_index_update: None,
                        report,
                    })
                }
            }
        }
        StepParams::SviSurface(p) => {
            let (surface, report) = SviSurfaceTarget::solve(p, quotes, context, global_config)?;
            Ok(StepOutcome {
                output: StepOutput::Surface(surface.into()),
                credit_index_update: None,
                report,
            })
        }
        StepParams::XccyBasis(p) => {
            let (ctx, report) = XccyBasisTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_discount(&p.curve_id)?;
            let report = attach_validation_result(
                report,
                curve.validate(&global_config.validation),
                global_config,
            );
            let output = match &p.basis_spread_curve_id {
                Some(spread_id) if ctx.get_basis_spread(spread_id).is_ok() => {
                    let spread = ctx.get_basis_spread(spread_id)?;
                    StepOutput::Curves(vec![curve.into(), (*spread).clone().into()])
                }
                _ => StepOutput::Curve(curve.into()),
            };
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
        StepParams::Parametric(p) => {
            let (ctx, report) = ParametricCurveTarget::solve(p, quotes, context, global_config)?;
            let curve = ctx.get_parametric(&p.curve_id)?;
            let output = StepOutput::Curve(curve.into());
            Ok(StepOutcome {
                output,
                credit_index_update: None,
                report,
            })
        }
    }
}

/// Execute a calibration step and normalize its output/result.
pub(crate) fn execute(
    step: &CalibrationStep,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<StepOutcome> {
    let _span = tracing::info_span!("calibration_step", step_id = %step.id).entered();
    let outcome = execute_params(&step.params, quotes, context, global_config)?;
    tracing::info!(
        success = %outcome.report.success,
        max_residual = %outcome.report.max_residual,
        iterations = %outcome.report.iterations,
        "calibration step completed"
    );
    Ok(outcome)
}

/// Execute [`StepParams`] directly and apply the output to a cloned context.
pub(crate) fn execute_params_and_apply(
    params: &StepParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<(MarketContext, CalibrationReport)> {
    let outcome = execute_params(params, quotes, context, global_config)?;
    let StepOutcome {
        output,
        credit_index_update,
        report,
    } = outcome;

    let mut new_context = context.clone();
    apply_output(&mut new_context, output, credit_index_update)?;
    Ok((new_context, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::{
        CapFloorHullWhiteStepParams, HullWhiteStepParams, StudentTParams, SviSurfaceParams,
    };
    use crate::hull_white::{
        contractual_swaption_price, scheduled_cap_floor_implied_normal_vol,
        scheduled_cap_floor_price, HullWhiteParams,
    };
    use crate::quotes::cds_tranche::CdsTrancheQuote;
    use crate::quotes::ids::QuoteId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::term_structures::{
        BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
    };
    use finstack_quant_core::types::{CurveId, UnderlyingId};
    use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
        CdsTranche, CdsTranchePricer, CdsTranchePricerConfig,
    };
    use finstack_quant_valuations::instruments::OptionType;
    use finstack_quant_valuations::market::conventions::ids::{
        CdsConventionKey, CdsDocClause, SwaptionConventionId,
    };
    use std::sync::Arc;
    use time::Month;

    fn build_flat_discount_curve(
        rate: f64,
        base_date: finstack_quant_core::dates::Date,
        curve_id: &str,
    ) -> DiscountCurve {
        DiscountCurve::builder(curve_id)
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .expect("flat discount curve should build")
    }

    fn build_student_t_market(
        base_date: finstack_quant_core::dates::Date,
        correlation: f64,
    ) -> MarketContext {
        let discount = build_flat_discount_curve(0.03, base_date, "USD-OIS");
        let hazard = HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.40)
            .knots([(1.0, 0.0010), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve");
        let base_corr = BaseCorrelationCurve::builder("CDX_CORR")
            .knots([(3.0, correlation), (7.0, correlation)])
            .build()
            .expect("base correlation curve");
        let credit_index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(base_corr.clone()))
            .build()
            .expect("credit index");

        MarketContext::new()
            .insert(discount)
            .insert(hazard)
            .insert(base_corr)
            .insert_credit_index("CDX.NA.IG", credit_index)
            .expect("credit index dependencies")
    }

    /// Synthetic 5Y [3,7] CDX mezzanine upfront at ν = 6, ρ = 0.3 under the
    /// default Student-t pricer (product Gauss over the exact conditional
    /// binomial). Quoted cash settles on 2025-03-25: the no-upfront trade PV
    /// -0.215368524909 per unit tranche notional is divided by its settlement
    /// discount factor exp(-0.03 * 5/365). The step recovers ν near 6 without
    /// manufacturing the quote from a live price.
    const STUDENT_T_FIXTURE_UPFRONT_PCT: f64 = -0.215_457_050_711;

    fn build_student_t_quote(upfront_pct: f64) -> CdsTrancheQuote {
        let maturity = Date::from_calendar_date(2030, Month::March, 20).expect("valid maturity");
        CdsTrancheQuote {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX.NA.IG".to_string(),
            series: 42,
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct,
            coupon_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }
    }

    fn set_atm_strike(quote: &mut VolQuote, discount: &DiscountCurve) {
        let conventions =
            SwaptionVolTarget::resolve_quote_leg_conventions(quote).expect("conventions");
        let (start, end) =
            SwaptionVolTarget::resolve_underlying_dates(quote, &conventions).expect("dates");
        let periods =
            SwaptionVolTarget::build_fixed_leg_periods(start, end, &conventions).expect("schedule");
        let floating_periods = SwaptionVolTarget::build_float_leg_periods(start, end, &conventions)
            .expect("floating schedule");
        let annuity: f64 = periods
            .iter()
            .map(|period| {
                period.accrual_year_fraction
                    * discount
                        .df_on_date_curve(period.payment_date)
                        .expect("payment DF")
            })
            .sum();
        let floating_pv: f64 = floating_periods
            .iter()
            .map(|period| {
                (discount
                    .df_on_date_curve(period.accrual_start)
                    .expect("start DF")
                    / discount
                        .df_on_date_curve(period.accrual_end)
                        .expect("end DF")
                    - 1.0)
                    * discount
                        .df_on_date_curve(period.payment_date)
                        .expect("payment DF")
            })
            .sum();
        if let VolQuote::SwaptionVol { strike, .. } = quote {
            *strike = floating_pv / annuity;
        }
    }

    fn cap_quote_vol(params: &StepParams, maturity: Date, strike: f64, sigma: f64) -> f64 {
        let StepParams::CapFloorHullWhite(params) = params else {
            unreachable!("cap quote helper requires cap calibration parameters");
        };
        let schedule =
            prepare_hw_cap_schedule(params, maturity).expect("contractual term schedule");
        let model = HullWhiteParams::constant(params.fixed_kappa.expect("fixed kappa"), sigma)
            .expect("fixture model");
        let df = |time: f64| (-0.03 * time).exp();
        let price = scheduled_cap_floor_price(&model, &df, &df, &schedule, strike, true)
            .expect("contractual cap price");
        scheduled_cap_floor_implied_normal_vol(price, &df, &df, &schedule, strike, true)
            .expect("contractual normal cap quote")
    }

    #[test]
    fn student_t_step_calibrates_and_returns_scalar_output() {
        let base_date = Date::from_calendar_date(2025, Month::March, 20).expect("valid date");
        let params = StepParams::StudentT(StudentTParams {
            tranche_instrument_id: "TRANCHE-1".to_string(),
            base_correlation_curve_id: "CDX_CORR".to_string(),
            discount_curve_id: Some("USD-OIS".into()),
            initial_df: 6.0,
            df_bounds: (2.5, 12.0),
            correlation: 0.3,
        });
        let quotes = vec![MarketQuote::CdsTranche(build_student_t_quote(
            STUDENT_T_FIXTURE_UPFRONT_PCT,
        ))];
        let context = build_student_t_market(base_date, 0.25);

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("Student-t step should calibrate");

        let StepOutput::Scalars(values) = outcome.output else {
            unreachable!("Student-t calibration should return a scalar output");
        };
        assert_eq!(values.len(), 1);
        let (key, value) = values.into_iter().next().expect("one scalar");
        assert_eq!(key, "TRANCHE-1_STUDENT_T_DF");
        let MarketScalar::Unitless(calibrated_df) = value else {
            unreachable!("Student-t degrees of freedom should be unitless");
        };
        assert!(
            (calibrated_df - 6.0).abs() < 0.5,
            "expected calibrated df near 6.0, got {calibrated_df}"
        );
    }

    #[test]
    fn student_t_fixture_upfront_matches_default_pricer() {
        let base_date = Date::from_calendar_date(2025, Month::March, 20).expect("valid date");
        let market = build_student_t_market(base_date, 0.3);
        let quote = build_student_t_quote(STUDENT_T_FIXTURE_UPFRONT_PCT);
        let mut curve_ids = finstack_quant_core::HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
        let build_context = crate::build::BuildCtx::new(base_date, 1.0 / (0.07 - 0.03), curve_ids);
        let instrument = crate::build::cds_tranche::build_cds_tranche_instrument(
            &quote,
            &build_context,
            &crate::build::cds_tranche::CdsTrancheBuildOverrides::default(),
        )
        .expect("shared tranche builder");
        let tranche = instrument
            .as_any()
            .downcast_ref::<CdsTranche>()
            .expect("CdsTranche");
        let (settlement, _) = tranche.upfront.expect("settlement-dated upfront");
        assert_eq!(
            settlement,
            Date::from_calendar_date(2025, Month::March, 25).expect("cash settlement date")
        );
        let settlement_df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(base_date, settlement)
            .expect("cash settlement discount factor");
        let mut without_upfront = tranche.clone();
        without_upfront.upfront = None;
        let pricer = CdsTranchePricer::with_config(
            CdsTranchePricerConfig::default()
                .with_student_t_copula(6.0)
                .expect("valid fixture df"),
        )
        .expect("valid pricer");
        let live = pricer
            .price_tranche(&without_upfront, &market, base_date)
            .expect("premium/protection leg PV")
            .amount()
            / (tranche.notional.amount() * settlement_df);
        assert!(
            (live - STUDENT_T_FIXTURE_UPFRONT_PCT).abs() < 1e-10,
            "update STUDENT_T_FIXTURE_UPFRONT_PCT to {live:.12}"
        );
        let full_trade_residual = pricer
            .price_tranche(tranche, &market, base_date)
            .expect("complete settled quote")
            .amount()
            / tranche.notional.amount();
        assert!(full_trade_residual.abs() < 1e-10);
    }

    #[test]
    fn hull_white_quote_inputs_reject_shifted_black_without_a_displacement() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("base date");
        let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("expiry");
        let maturity = Date::from_calendar_date(2031, Month::January, 1).expect("maturity");
        let discount = build_flat_discount_curve(0.03, base_date, "USD-OIS");
        let swaption = VolQuote::SwaptionVol {
            id: QuoteId::new("SHIFTED-SWAPTION"),
            expiry,
            maturity,
            strike: 0.03,
            vol: 0.2,
            quote_type: VolQuoteType::ShiftedBlackLognormal,
            convention: SwaptionConventionId::new("USD"),
        };
        let error = prepare_hw_swaption_input(&swaption, &discount, Currency::USD, base_date)
            .expect_err("shifted swaption must be rejected before premium conversion");
        assert!(error.to_string().contains("no displacement"));

        let params = StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
            fit_tolerance: 1e-6,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            index_id: "USD-SOFR-3M".into(),
            volatility_mode: HullWhiteVolatilityMode::Scalar,
        });
        let quotes = vec![MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("SHIFTED-CAP"),
            expiry: maturity,
            strike: 0.03,
            vol: 0.2,
            quote_type: VolQuoteType::ShiftedBlackLognormal,
            is_cap: true,
        })];
        let context = MarketContext::new().insert(discount);
        let error = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .err()
            .expect("shifted cap must be rejected before premium conversion");
        assert!(error.to_string().contains("no displacement"));
    }

    #[test]
    fn hull_white_step_builds_convention_driven_timing_roles() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("base date");
        let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("expiry");
        let swap_start = Date::from_calendar_date(2026, Month::January, 5).expect("swap start");
        let maturity = Date::from_calendar_date(2027, Month::January, 5).expect("maturity");
        let payment = Date::from_calendar_date(2027, Month::January, 7).expect("payment");
        let discount = build_flat_discount_curve(0.03, base_date, "USD-OIS");
        let mut quote = VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-T2-LAG2"),
            expiry,
            maturity,
            strike: 0.03,
            vol: 0.01,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        };

        for strike in [0.01, 0.10] {
            if let VolQuote::SwaptionVol { strike: value, .. } = &mut quote {
                *value = strike;
            }
            let error =
                prepare_hw_swaption_input(&quote, &discount, Currency::USD, discount.base_date())
                    .expect_err("off-ATM quote");
            assert!(error.to_string().contains("requires ATM"));
        }
        set_atm_strike(&mut quote, &discount);
        let (prepared_quote, schedule) =
            prepare_hw_swaption_input(&quote, &discount, Currency::USD, discount.base_date())
                .expect("convention-driven HW input");
        let expected_expiry = DayCount::Act365F
            .year_fraction(base_date, expiry, DayCountContext::default())
            .expect("expiry time");
        let expected_start = DayCount::Act365F
            .year_fraction(base_date, swap_start, DayCountContext::default())
            .expect("start time");
        let expected_maturity = DayCount::Act365F
            .year_fraction(base_date, maturity, DayCountContext::default())
            .expect("maturity time");
        let expected_payment = DayCount::Act365F
            .year_fraction(base_date, payment, DayCountContext::default())
            .expect("payment time");

        assert!((prepared_quote.expiry - expected_expiry).abs() < 1.0e-15);
        assert!((schedule.swap_start_time - expected_start).abs() < 1.0e-15);
        assert!((schedule.maturity_time - expected_maturity).abs() < 1.0e-15);
        assert_eq!(schedule.payment_times.len(), 1);
        assert!((schedule.payment_times[0] - expected_payment).abs() < 1.0e-15);
        assert!(schedule.swap_start_time > prepared_quote.expiry);
        assert!(schedule.payment_times[0] > schedule.maturity_time);
        assert!((prepared_quote.tenor - (expected_maturity - expected_start)).abs() < 1.0e-15);
        for day_count in [DayCount::Act365F, DayCount::Act360] {
            for days_before in [0, 365] {
                let scale = if day_count == DayCount::Act360 {
                    365.0 / 360.0
                } else {
                    1.0
                };
                let equivalent = DiscountCurve::builder("USD-OIS")
                    .base_date(base_date - time::Duration::days(days_before))
                    .day_count(day_count)
                    .knots([(0.0, 1.0), (10.0 * scale, (-0.3_f64).exp())])
                    .build()
                    .expect("equivalent curve");
                let (other_quote, other_schedule) =
                    prepare_hw_swaption_input(&quote, &equivalent, Currency::USD, base_date)
                        .expect("same dated quote on another curve clock");
                assert_eq!(other_quote.expiry, prepared_quote.expiry);
                assert_eq!(other_quote.tenor, prepared_quote.tenor);
                assert_eq!(other_schedule.payment_times, schedule.payment_times);
                assert_eq!(other_schedule.accruals, schedule.accruals);
                assert_eq!(other_schedule.swap_start_time, schedule.swap_start_time);
                assert_eq!(other_schedule.maturity_time, schedule.maturity_time);
            }
        }
    }

    #[test]
    fn hull_white_step_persists_both_kappa_and_sigma_scalars() {
        // Generate internally-consistent HW1F quotes from (κ*, σ*) = (0.05, 0.01)
        // on a 3-swaption grid so the calibrator's κ-bounds
        // check sees a realistic target, not a degenerate fit. The test's
        // purpose is about output-key persistence (it checks that both
        // `*_KAPPA` and `*_SIGMA` scalars are emitted), so the specific
        // calibrated values don't matter as long as calibration succeeds.
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = StepParams::HullWhite(HullWhiteStepParams {
            fit_tolerance: 1e-6,
            curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            initial_kappa: Some(0.04),
            initial_sigma: Some(0.008),
        });

        // Convert model premiums on the actual convention-driven schedules
        // to the flat normal quotes consumed by the engine.
        let df_fn = |t: f64| (-0.03 * t).exp();

        let mut quotes = vec![
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2026, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2031, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: 0.01,
                quote_type: VolQuoteType::Normal,
                convention: SwaptionConventionId::new("USD"),
            }),
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-2Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2027, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2032, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: 0.01,
                quote_type: VolQuoteType::Normal,
                convention: SwaptionConventionId::new("USD"),
            }),
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-5Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2035, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: 0.01,
                quote_type: VolQuoteType::Normal,
                convention: SwaptionConventionId::new("USD"),
            }),
        ];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        for quote in &mut quotes {
            if let MarketQuote::Vol(vol) = quote {
                let discount = context.get_discount("USD-OIS").expect("curve");
                set_atm_strike(vol, discount.as_ref());
                let (prepared, schedule) =
                    prepare_hw_swaption_input(vol, discount.as_ref(), Currency::USD, base_date)
                        .expect("contractual swaption input");
                let VolQuote::SwaptionVol {
                    strike,
                    vol: quoted_vol,
                    ..
                } = vol
                else {
                    unreachable!("swaption fixture");
                };
                let premium = contractual_swaption_price(
                    HullWhiteCalibrationParams::new(0.05, 0.01).expect("fixture parameters"),
                    &df_fn,
                    prepared.expiry,
                    *strike,
                    &schedule,
                )
                .expect("contractual swaption premium");
                let annuity: f64 = schedule
                    .payment_times
                    .iter()
                    .zip(&schedule.accruals)
                    .map(|(payment, accrual)| df_fn(*payment) * accrual)
                    .sum();
                *quoted_vol =
                    premium / (annuity * (prepared.expiry / (2.0 * std::f64::consts::PI)).sqrt());
            }
        }

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("Hull-White step should calibrate");

        let StepOutput::Scalars(values) = outcome.output else {
            unreachable!("Hull-White calibration should return multiple scalar outputs");
        };
        assert!(
            values
                .iter()
                .any(|(key, _)| key.starts_with("USD-OIS_") && key.ends_with("KAPPA")),
            "expected calibrated kappa scalar output"
        );
        assert!(
            values
                .iter()
                .any(|(key, _)| key.starts_with("USD-OIS_") && key.ends_with("SIGMA")),
            "expected calibrated sigma scalar output"
        );
    }

    #[test]
    fn cap_floor_hull_white_step_persists_both_kappa_and_sigma_scalars() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
            fit_tolerance: 1e-6,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            index_id: "USD-SOFR-3M".into(),
            volatility_mode: HullWhiteVolatilityMode::Scalar,
        });

        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("expiry");
        let vol = cap_quote_vol(&params, maturity, 0.0365, 0.0095);
        let quotes = vec![MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("USD-CAP-VOL-20300101-0.0365"),
            expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
            strike: 0.0365,
            vol,
            quote_type: VolQuoteType::Normal,
            is_cap: true,
        })];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("cap/floor Hull-White step should calibrate");
        assert_eq!(
            outcome.report.metadata.get("frequency").map(String::as_str),
            Some("contractual")
        );

        let StepOutput::Scalars(values) = outcome.output else {
            unreachable!("cap/floor Hull-White calibration should return scalar outputs");
        };
        assert!(
            values
                .iter()
                .any(|(key, _)| key == "USD-OIS_CAPFLOOR_HW1F_KAPPA"),
            "expected calibrated cap/floor kappa scalar output"
        );
        assert!(
            values
                .iter()
                .any(|(key, _)| key == "USD-OIS_CAPFLOOR_HW1F_SIGMA"),
            "expected calibrated cap/floor sigma scalar output"
        );
        let (_, MarketScalar::Unitless(sigma)) = values
            .iter()
            .find(|(key, _)| key == "USD-OIS_CAPFLOOR_HW1F_SIGMA")
            .expect("sigma scalar")
        else {
            unreachable!("sigma must be unitless");
        };
        assert!((sigma - 0.0095).abs() < 1e-9);
    }

    #[test]
    fn cap_floor_hull_white_preserves_modified_following_month_end_payment() {
        // T+2 starts this term cap on 31 January. Its final 31 January 2026
        // accrual end is a Saturday, so modified following pays on 30 January.
        let base_date = Date::from_calendar_date(2025, Month::January, 29).expect("base date");
        let maturity = Date::from_calendar_date(2026, Month::January, 31).expect("maturity");
        let cap_params = CapFloorHullWhiteStepParams {
            fit_tolerance: 1e-6,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            index_id: "USD-SOFR-3M".into(),
            volatility_mode: HullWhiteVolatilityMode::Scalar,
        };
        let schedule = prepare_hw_cap_schedule(&cap_params, maturity).expect("calendar schedule");
        let final_period = schedule.periods.last().expect("live final caplet");
        let expected_payment = finstack_quant_models::rates::clock::model_time(
            base_date,
            Date::from_calendar_date(2026, Month::January, 30).expect("payment"),
        );
        assert_eq!(final_period.payment_time, expected_payment);
        assert!(final_period.payment_time < final_period.end_time);
        let params = StepParams::CapFloorHullWhite(cap_params);
        let quotes = [MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("USD-CAP-EARLY-PAYMENT"),
            expiry: maturity,
            strike: 0.03,
            vol: cap_quote_vol(&params, maturity, 0.03, 0.0095),
            quote_type: VolQuoteType::Normal,
            is_cap: true,
        })];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));
        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("valid term cap with an earlier adjusted payment");
        assert!(outcome.report.success);
    }

    #[test]
    fn cap_floor_hull_white_rejects_compounded_overnight_index() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("base date");
        let params = CapFloorHullWhiteStepParams {
            fit_tolerance: 1e-6,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            index_id: "USD-SOFR-OIS".into(),
            volatility_mode: HullWhiteVolatilityMode::Scalar,
        };
        let error = prepare_hw_cap_schedule(
            &params,
            Date::from_calendar_date(2030, Month::January, 1).expect("maturity"),
        )
        .expect_err("overnight coupons cannot be reinterpreted as term coupons");
        assert!(error.to_string().contains("requires a term index"));
    }

    #[test]
    fn piecewise_cap_floor_hull_white_step_persists_sigma_schedule() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
            fit_tolerance: 1e-6,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            index_id: "USD-SOFR-3M".into(),
            volatility_mode: HullWhiteVolatilityMode::Piecewise,
        });
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("expiry");
        let vol = cap_quote_vol(&params, maturity, 0.0365, 0.0095);
        let quotes = vec![MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("USD-CAP-VOL-20300101-0.0365"),
            expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
            strike: 0.0365,
            vol,
            quote_type: VolQuoteType::Normal,
            is_cap: true,
        })];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("piecewise cap/floor calibration");
        let StepOutput::ScalarsAndSeries { scalars, series } = outcome.output else {
            unreachable!("piecewise cap/floor calibration should persist scalars and a schedule");
        };
        assert_eq!(scalars.len(), 1);
        assert_eq!(scalars[0].0, "USD-OIS_CAPFLOOR_HW1F_KAPPA");
        assert_eq!(series.id().as_str(), "USD-OIS_CAPFLOOR_HW1F_SIGMA_SCHEDULE");
        assert_eq!(series.len(), 1);
    }

    #[test]
    fn svi_surface_step_builds_surface_from_option_vol_quotes() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let expiry_1 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let expiry_2 = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let time_day_count = DayCount::Act365F;
        let t1 = time_day_count
            .year_fraction(base_date, expiry_1, DayCountContext::default())
            .expect("valid year fraction");
        let t2 = time_day_count
            .year_fraction(base_date, expiry_2, DayCountContext::default())
            .expect("valid year fraction");

        let params = StepParams::SviSurface(SviSurfaceParams {
            vol_surface_id: "SPX-SVI".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: Some("USD-OIS".into()),
            target_expiries: vec![t1, t2],
            target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(100.0),
            dividend_yield_override: Some(0.0),
        });

        let quotes = vec![
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-80"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 80.0,
                vol: 0.30,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-90"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 90.0,
                vol: 0.24,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-100"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 100.0,
                vol: 0.20,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-110"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 110.0,
                vol: 0.22,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-120"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 120.0,
                vol: 0.27,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-80"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 80.0,
                vol: 0.32,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-90"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 90.0,
                vol: 0.27,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-100"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 100.0,
                vol: 0.23,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-110"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 110.0,
                vol: 0.24,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-120"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 120.0,
                vol: 0.28,
                option_type: OptionType::Call,
            }),
        ];

        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("SVI step should build a surface");

        let StepOutput::Surface(surface) = outcome.output else {
            unreachable!("SVI calibration should return a surface output");
        };
        assert_eq!(surface.id(), &CurveId::from("SPX-SVI"));
        assert_eq!(surface.grid_shape(), (2, 5));
        let atm_vol = finstack_quant_models::volatility::get_surface_vol(&surface, t1, 100.0)
            .expect("ATM point should exist");
        assert!(atm_vol.is_finite(), "ATM SVI vol should be finite");
        assert!(
            atm_vol > 0.0 && atm_vol < 1.0,
            "ATM SVI vol should be in a realistic range, got {atm_vol}"
        );
    }
}
