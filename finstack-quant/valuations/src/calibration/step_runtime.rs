use crate::calibration::api::schema::{
    CalibrationStep, HullWhiteVolatilityMode, StepParams, StepPrimaryOutput,
};
use crate::calibration::config::CalibrationConfig;
use crate::calibration::hull_white::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors, calibrate_hull_white_to_cap_floors,
    calibrate_hull_white_to_swaptions_with_schedules, capfloor_hw1f_scalar_keys,
    capfloor_hw1f_sigma_schedule_key, hw1f_scalar_keys, CapFloorCalibrationConfig, CapFloorQuote,
    HullWhiteParams, PiecewiseSigmaCalibrationConfig, SwapFrequency, SwaptionQuote,
};
use crate::calibration::targets::base_correlation::BaseCorrelationTarget;
use crate::calibration::targets::discount::DiscountCurveTarget;
use crate::calibration::targets::forward::ForwardCurveTarget;
use crate::calibration::targets::hazard::HazardCurveTarget;
use crate::calibration::targets::inflation::InflationCurveTarget;
use crate::calibration::targets::parametric::ParametricCurveTarget;
use crate::calibration::targets::student_t::StudentTTarget;
use crate::calibration::targets::svi::SviSurfaceTarget;
use crate::calibration::targets::swaption::SwaptionVolTarget;
use crate::calibration::targets::vol::VolSurfaceTarget;
use crate::calibration::targets::xccy_basis::XccyBasisTarget;
use crate::calibration::validation::surfaces::validate_surface;
use crate::calibration::validation::ValidationMode;
use crate::calibration::{CalibrationReport, CurveValidator};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::vol::VolQuote;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::explain::TraceEntry;
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{VolCube, VolSurface};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Normalized output key for a step.
pub(crate) enum OutputKey {
    Curve(CurveId),
    Surface(CurveId),
    Scalar(String),
}

/// Normalized output payload for a step.
pub(crate) enum StepOutput {
    Curve(CurveStorage),
    Curves(Vec<CurveStorage>),
    Surface(Arc<VolSurface>),
    VolCube(Arc<VolCube>),
    Scalar {
        key: String,
        value: MarketScalar,
    },
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

/// Compute the output key for batching without executing the step.
pub(crate) fn output_key(step: &CalibrationStep) -> OutputKey {
    match step.params.io().primary_output {
        StepPrimaryOutput::Curve(id) => OutputKey::Curve(id),
        StepPrimaryOutput::Surface(id) => OutputKey::Surface(id),
        StepPrimaryOutput::Scalar(key) => OutputKey::Scalar(key),
    }
}

/// Apply a normalized step output into the mutable market context.
pub(crate) fn apply_output(
    context: &mut MarketContext,
    output: StepOutput,
    credit_index_update: Option<(String, CreditIndexData)>,
) {
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
        StepOutput::Scalar { key, value } => {
            *context = std::mem::take(context).insert_price(&key, value);
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
        *context = std::mem::take(context).insert_credit_index(id, data);
    }
}

/// Execute calibration logic for the provided [`StepParams`].
pub(crate) fn execute_params(
    params: &StepParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<StepOutcome> {
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
                MarketQuote::Cds(crate::market::quotes::cds::CdsQuote::CdsParSpread {
                    spread_bp,
                    ..
                })
                | MarketQuote::Cds(crate::market::quotes::cds::CdsQuote::CdsUpfront {
                    running_spread_bp: spread_bp,
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
            // Preserve context insertion behavior
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
                output: StepOutput::Scalar {
                    key: scalar_key,
                    value: MarketScalar::Unitless(calibrated_df),
                },
                credit_index_update: None,
                report,
            })
        }
        StepParams::HullWhite(p) => {
            let disc_curve = context.get_discount(&p.curve_id)?;
            let df = |t: f64| disc_curve.df(t);
            let dc = DayCount::Act365F;

            // Extract swaption quotes from MarketQuote::Vol(VolQuote::SwaptionVol { .. }).
            //
            // We build per-quote *real* accrual year fractions on the
            // currency-appropriate day-count (Act/360 USD, 30/360 EUR, etc.)
            // and pass them to `calibrate_hull_white_to_swaptions_with_schedules`
            // so the calibrated (κ, σ) match vendor models that use real
            // schedules (Bloomberg VCUB, QuantLib `Gaussian1dSwaptionEngine`).
            let frequency = match p.currency {
                finstack_quant_core::currency::Currency::EUR
                | finstack_quant_core::currency::Currency::GBP => SwapFrequency::Annual,
                _ => SwapFrequency::SemiAnnual,
            };
            let payment_dc = match p.currency {
                finstack_quant_core::currency::Currency::EUR
                | finstack_quant_core::currency::Currency::GBP => DayCount::Thirty360,
                _ => DayCount::Act360,
            };
            let ppy = frequency.periods_per_year();

            let mut hw_quotes = Vec::new();
            let mut hw_schedules: Vec<Vec<f64>> = Vec::new();
            for quote in quotes {
                let MarketQuote::Vol(VolQuote::SwaptionVol {
                    expiry,
                    maturity,
                    vol,
                    quote_type,
                    ..
                }) = quote
                else {
                    continue;
                };

                let t_exp = dc.year_fraction(p.base_date, *expiry, DayCountContext::default())?;
                let t_ten = dc.year_fraction(*expiry, *maturity, DayCountContext::default())?;
                if t_exp <= 0.0 || t_ten <= 0.0 {
                    continue;
                }

                // Build per-period accruals on the *real* payment day-count.
                // The schedule is uniform-period in calendar terms (no BDC
                // applied here — leg-level conventions can be threaded later
                // when the swaption schema carries them).
                let n_periods = (t_ten * ppy as f64).round().max(1.0) as usize;
                let period_days =
                    finstack_quant_core::dates::DayCount::calendar_days(*expiry, *maturity).max(0)
                        / n_periods.max(1) as i64;
                let mut accruals = Vec::with_capacity(n_periods);
                let mut acc_start = *expiry;
                for i in 0..n_periods {
                    let acc_end = if i + 1 == n_periods {
                        *maturity
                    } else {
                        acc_start + time::Duration::days(period_days)
                    };
                    let tau =
                        payment_dc.year_fraction(acc_start, acc_end, DayCountContext::default())?;
                    accruals.push(tau);
                    acc_start = acc_end;
                }

                let is_normal = quote_type.eq_ignore_ascii_case("normal");
                hw_quotes.push(SwaptionQuote {
                    expiry: t_exp,
                    tenor: t_ten,
                    volatility: *vol,
                    is_normal_vol: is_normal,
                });
                hw_schedules.push(accruals);
            }

            let initial_guess = match (p.initial_kappa, p.initial_sigma) {
                (Some(kappa), Some(sigma)) => Some(HullWhiteParams::new(kappa, sigma)?),
                (None, None) => None,
                _ => {
                    return Err(finstack_quant_core::Error::Validation(
                        "Hull-White calibration requires both `initial_kappa` and `initial_sigma` when overriding defaults"
                            .to_string(),
                    ))
                }
            };
            let (hw_params, report) = calibrate_hull_white_to_swaptions_with_schedules(
                &df,
                &hw_quotes,
                frequency,
                &hw_schedules,
                initial_guess,
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
            let discount_df = |t: f64| disc_curve.df(t);
            let forward_curve = if p.forward_curve_id == p.discount_curve_id {
                None
            } else {
                Some(context.get_forward(&p.forward_curve_id)?)
            };
            let forward_df = |t: f64| -> f64 {
                forward_curve
                    .as_ref()
                    .map_or_else(|| disc_curve.df(t), |curve| curve.df(t).unwrap_or(f64::NAN))
            };
            let dc = DayCount::Act365F;

            let mut cap_floor_quotes = Vec::new();
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

                let maturity =
                    dc.year_fraction(p.base_date, *expiry, DayCountContext::default())?;
                if maturity <= 0.0 {
                    continue;
                }
                cap_floor_quotes.push(CapFloorQuote {
                    maturity,
                    strike: *strike,
                    volatility: *vol,
                    is_cap: *is_cap,
                    is_normal_vol: quote_type.eq_ignore_ascii_case("normal"),
                });
            }

            let initial_guess = match (p.initial_kappa, p.initial_sigma) {
                (Some(kappa), Some(sigma)) => Some(HullWhiteParams::new(kappa, sigma)?),
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
                    let (hw_params, report) = calibrate_hull_white_to_cap_floors(
                        &discount_df,
                        &forward_df,
                        &cap_floor_quotes,
                        CapFloorCalibrationConfig {
                            frequency: p.payment_frequency,
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
                    let (model, report) = bootstrap_hull_white_sigma_schedule_to_cap_floors(
                        &discount_df,
                        &forward_df,
                        &cap_floor_quotes,
                        PiecewiseSigmaCalibrationConfig {
                            fixed_kappa,
                            sigma_min: 1.0e-5,
                            sigma_max: 2.0,
                            frequency: p.payment_frequency,
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
    apply_output(&mut new_context, output, credit_index_update);
    Ok((new_context, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::schema::{
        CapFloorHullWhiteStepParams, HullWhiteStepParams, StudentTParams, SviSurfaceParams,
    };
    use crate::instruments::credit_derivatives::cds_tranche::{
        CDSTranche, CDSTranchePricer, CDSTranchePricerConfig, TrancheSide,
    };
    use crate::instruments::Attributes;
    use crate::instruments::OptionType;
    use crate::market::conventions::ids::{
        CapFloorConventionId, CdsConventionKey, CdsDocClause, OptionConventionId,
        SwaptionConventionId,
    };
    use crate::market::quotes::cds_tranche::CDSTrancheQuote;
    use crate::market::quotes::ids::QuoteId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::term_structures::{
        BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
    };
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, UnderlyingId};
    use std::sync::Arc;
    use time::Month;

    fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
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

    fn build_student_t_market(base_date: Date, correlation: f64) -> MarketContext {
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
    }

    fn build_student_t_quote(base_date: Date, df: f64, correlation: f64) -> CDSTrancheQuote {
        let market = build_student_t_market(base_date, correlation);
        let maturity = Date::from_calendar_date(2030, Month::March, 20).expect("valid maturity");
        let tranche = CDSTranche::builder()
            .id("TRANCHE-1".into())
            .index_name("CDX.NA.IG".to_string())
            .series(1)
            .attach_pct(3.0)
            .detach_pct(7.0)
            .notional(Money::new(0.04, Currency::USD))
            .maturity(maturity)
            .running_coupon_bp(500.0)
            .frequency("3M".parse().expect("tenor"))
            .day_count(DayCount::Act360)
            .bdc(finstack_quant_core::dates::BusinessDayConvention::Following)
            .calendar_id_opt(None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .credit_index_id(CurveId::from("CDX.NA.IG"))
            .side(TrancheSide::SellProtection)
            .effective_date_opt(None)
            .accumulated_loss(0.0)
            .standard_imm_dates(true)
            .attributes(Attributes::new())
            .build()
            .expect("tranche");

        let pricer = CDSTranchePricer::with_params(
            CDSTranchePricerConfig::default()
                .with_student_t_copula(df)
                .expect("valid calibration fixture Student-t df"),
        );
        let upfront_pct = pricer
            .calculate_upfront(&tranche, &market, base_date)
            .expect("upfront")
            / tranche.notional.amount();

        CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct,
            running_spread_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::Cr14,
            },
        }
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
        let quotes = vec![MarketQuote::CDSTranche(build_student_t_quote(
            base_date, 6.0, 0.3,
        ))];
        let context = build_student_t_market(base_date, 0.25);

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("Student-t step should calibrate");

        match outcome.output {
            StepOutput::Scalar { key, value } => {
                assert_eq!(key, "TRANCHE-1_STUDENT_T_DF");
                let MarketScalar::Unitless(calibrated_df) = value else {
                    panic!("expected unitless calibrated df");
                };
                assert!(
                    (calibrated_df - 6.0).abs() < 0.5,
                    "expected calibrated df near 6.0, got {calibrated_df}"
                );
            }
            _ => panic!("expected scalar output"),
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
            curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            initial_kappa: Some(0.04),
            initial_sigma: Some(0.008),
        });

        // Build quotes by back-solving Bachelier vols from HW1F prices at
        // κ* = 0.05, σ* = 0.01 on a flat 3% curve.
        let df_fn = |t: f64| (-0.03 * t).exp();
        let ppy = SwapFrequency::SemiAnnual.periods_per_year();
        let synthesise = |expiry_y: f64, tenor_y: f64| -> f64 {
            let (annuity, fwd) = crate::calibration::hull_white::compute_swap_annuity_and_rate(
                &df_fn, expiry_y, tenor_y, ppy,
            );
            let price = crate::calibration::hull_white::hw1f_swaption_price(
                0.05, 0.01, &df_fn, expiry_y, tenor_y, fwd, ppy,
            );
            (price / (annuity * (expiry_y / (2.0 * std::f64::consts::PI)).sqrt())).max(1e-6)
        };

        let quotes = vec![
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2026, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2031, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: synthesise(1.0, 5.0),
                quote_type: "normal".to_string(),
                convention: SwaptionConventionId::new("USD"),
            }),
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-2Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2027, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2032, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: synthesise(2.0, 5.0),
                quote_type: "normal".to_string(),
                convention: SwaptionConventionId::new("USD"),
            }),
            MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new("USD-SWPTN-VOL-5Yx5Y-ATM"),
                expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
                maturity: Date::from_calendar_date(2035, Month::January, 1).expect("maturity"),
                strike: 0.03,
                vol: synthesise(5.0, 5.0),
                quote_type: "normal".to_string(),
                convention: SwaptionConventionId::new("USD"),
            }),
        ];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("Hull-White step should calibrate");

        match outcome.output {
            StepOutput::Scalars(values) => {
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
            _ => panic!("expected multiple scalar outputs for Hull-White calibration"),
        }
    }

    #[test]
    fn cap_floor_hull_white_step_persists_both_kappa_and_sigma_scalars() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            payment_frequency: SwapFrequency::Quarterly,
            volatility_mode: HullWhiteVolatilityMode::Scalar,
        });

        let df_fn = |t: f64| (-0.03 * t).exp();
        let vol = crate::calibration::hull_white::hw1f_cap_floor_implied_normal_vol(
            0.0342,
            0.0095,
            &df_fn,
            &df_fn,
            crate::calibration::hull_white::CapFloorPriceSpec::new(
                5.0,
                0.0365,
                true,
                SwapFrequency::Quarterly,
            ),
        );
        let quotes = vec![MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("USD-CAP-VOL-20300101-0.0365"),
            expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
            strike: 0.0365,
            vol,
            quote_type: "normal".to_string(),
            is_cap: true,
            convention: CapFloorConventionId::new("USD-SOFR-CAP"),
        })];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("cap/floor Hull-White step should calibrate");

        match outcome.output {
            StepOutput::Scalars(values) => {
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
            }
            _ => panic!("expected multiple scalar outputs for cap/floor Hull-White calibration"),
        }
    }

    #[test]
    fn piecewise_cap_floor_hull_white_step_persists_sigma_schedule() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            base_date,
            fixed_kappa: Some(0.0342),
            initial_kappa: None,
            initial_sigma: None,
            payment_frequency: SwapFrequency::Quarterly,
            volatility_mode: HullWhiteVolatilityMode::Piecewise,
        });
        let df_fn = |t: f64| (-0.03 * t).exp();
        let vol = crate::calibration::hull_white::hw1f_cap_floor_implied_normal_vol(
            0.0342,
            0.0095,
            &df_fn,
            &df_fn,
            crate::calibration::hull_white::CapFloorPriceSpec::new(
                5.0,
                0.0365,
                true,
                SwapFrequency::Quarterly,
            ),
        );
        let quotes = vec![MarketQuote::Vol(VolQuote::CapFloorVol {
            id: QuoteId::new("USD-CAP-VOL-20300101-0.0365"),
            expiry: Date::from_calendar_date(2030, Month::January, 1).expect("expiry"),
            strike: 0.0365,
            vol,
            quote_type: "normal".to_string(),
            is_cap: true,
            convention: CapFloorConventionId::new("USD-SOFR-CAP"),
        })];
        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("piecewise cap/floor calibration");
        match outcome.output {
            StepOutput::ScalarsAndSeries { scalars, series } => {
                assert_eq!(scalars.len(), 1);
                assert_eq!(scalars[0].0, "USD-OIS_CAPFLOOR_HW1F_KAPPA");
                assert_eq!(series.id().as_str(), "USD-OIS_CAPFLOOR_HW1F_SIGMA_SCHEDULE");
                assert_eq!(series.len(), 1);
            }
            _ => panic!("piecewise cap/floor calibration must persist scalars and schedule"),
        }
    }

    #[test]
    fn svi_surface_step_builds_surface_from_option_vol_quotes() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let expiry_1 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let expiry_2 = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let time_dc = DayCount::Act365F;
        let t1 = time_dc
            .year_fraction(base_date, expiry_1, DayCountContext::default())
            .expect("valid year fraction");
        let t2 = time_dc
            .year_fraction(base_date, expiry_2, DayCountContext::default())
            .expect("valid year fraction");

        let params = StepParams::SviSurface(SviSurfaceParams {
            surface_id: "SPX-SVI".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: Some("USD-OIS".into()),
            target_expiries: vec![t1, t2],
            target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(100.0),
        });

        let quotes = vec![
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-80"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 80.0,
                vol: 0.30,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-90"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 90.0,
                vol: 0.24,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-100"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 100.0,
                vol: 0.20,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-110"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 110.0,
                vol: 0.22,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1-120"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_1,
                strike: 120.0,
                vol: 0.27,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-80"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 80.0,
                vol: 0.32,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-90"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 90.0,
                vol: 0.27,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-100"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 100.0,
                vol: 0.23,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-110"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 110.0,
                vol: 0.24,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2-120"),
                underlying: UnderlyingId::new("SPX"),
                expiry: expiry_2,
                strike: 120.0,
                vol: 0.28,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-EQ"),
            }),
        ];

        let context =
            MarketContext::new().insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"));

        let outcome = execute_params(&params, &quotes, &context, &CalibrationConfig::default())
            .expect("SVI step should build a surface");

        match outcome.output {
            StepOutput::Surface(surface) => {
                assert_eq!(surface.id(), &CurveId::from("SPX-SVI"));
                assert_eq!(surface.grid_shape(), (2, 5));
                let atm_vol = surface
                    .value_checked(t1, 100.0)
                    .expect("ATM point should exist");
                assert!(atm_vol.is_finite(), "ATM SVI vol should be finite");
                assert!(
                    atm_vol > 0.0 && atm_vol < 1.0,
                    "ATM SVI vol should be in a realistic range, got {atm_vol}"
                );
            }
            _ => panic!("expected surface output"),
        }
    }
}
