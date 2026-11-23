//! Swaption volatility surface calibration target implementation.

use crate::CalibrationReport;
use crate::api::schema::{SurfaceExtrapolationPolicy, SwaptionVolConvention, SwaptionVolParams};
use crate::config::CalibrationConfig;
use crate::quotes::MarketQuote;
use crate::quotes::VolQuote;
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{
    BusinessDayConvention, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::SabrParameterData;
use finstack_quant_core::market_data::surfaces::VolCube;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_models::volatility::sabr::{SabrCalibrator, SabrParameters, SabrShift};
use finstack_quant_valuations_core::market::conventions::ConventionRegistry;
use finstack_quant_valuations_rates::swaption::contractual_swap_tenor_years;
use std::collections::BTreeMap;

/// Bootstrapper for calibrating swaption volatility surfaces.
///
/// Calibrates volatility surfaces from swaption quotes using the SABR model.
/// Groups quotes by expiry and tenor, calibrates SABR parameters per group,
/// and builds a volatility surface grid.
/// Validate a decimal swaption volatility against the plan convention.
///
/// Both normal absolute volatility and Black volatility use decimal units
/// throughout the quote, calibration, and model layers.
pub(super) fn validate_quoted_vol(
    quoted: f64,
    quote_type: VolQuoteType,
    convention: SwaptionVolConvention,
) -> Result<f64> {
    if !quoted.is_finite() || quoted < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption volatility must be finite, non-negative, and expressed in decimal units; got {quoted}"
        )));
    }

    let expected = match convention {
        SwaptionVolConvention::Normal => VolQuoteType::Normal,
        SwaptionVolConvention::Lognormal => VolQuoteType::BlackLognormal,
        SwaptionVolConvention::ShiftedLognormal { .. } => VolQuoteType::ShiftedBlackLognormal,
    };
    if quote_type != expected {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption quote type {quote_type} conflicts with plan convention {convention:?}; expected {expected}"
        )));
    }

    Ok(quoted)
}

/// Calibrates a swaption volatility surface from market quotes.
///
/// Groups swaption quotes by expiry and tenor, calibrates SABR parameters
/// for each group, and constructs a volatility surface grid.
///
/// # Arguments
///
/// * `params` - Parameters defining the swaption volatility surface structure
/// * `quotes` - Market quotes containing swaption volatility quotes
/// * `context` - Market context containing discount curves and forward rates
/// * `config` - Calibration configuration settings
///
/// # Returns
///
/// A tuple containing the calibrated volatility cube and calibration report.
///
/// # Errors
///
/// Returns an error if insufficient quotes are provided or calibration fails.
pub(crate) fn solve(
    params: &SwaptionVolParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    config: &CalibrationConfig,
) -> Result<(VolCube, CalibrationReport)> {
    // Group quotes by (expiry_years, tenor_years) using stable basis-point keys.
    let mut grouped_quotes: BTreeMap<(u64, u64), Vec<&VolQuote>> = BTreeMap::new();
    // Option time matches the swaption pricers and is independent of the
    // fixed leg's accrual convention.
    let day_count = DayCount::Act365F;

    for q in quotes {
        if let MarketQuote::Vol(
            vol_quote @ VolQuote::SwaptionVol {
                expiry, convention, ..
            },
        ) = q
        {
            let leg_conventions = resolve_leg_conventions(params, vol_quote)?;
            if leg_conventions.currency != params.currency {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "swaption quote '{}' convention currency {} conflicts with step currency {}",
                    vol_quote.id(),
                    leg_conventions.currency,
                    params.currency
                )));
            }
            let (swap_start, swap_end) = resolve_underlying_dates(vol_quote, &leg_conventions)?;
            let t_exp =
                day_count.year_fraction(params.base_date, *expiry, DayCountContext::default())?;
            let t_ten = contractual_swap_tenor_years(swap_start, swap_end)?;
            let key = (to_basis_points(t_exp), to_basis_points(t_ten));
            if let Some(VolQuote::SwaptionVol {
                convention: existing,
                ..
            }) = grouped_quotes
                .get(&key)
                .and_then(|bucket| bucket.first().copied())
                && existing != convention
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "swaption bucket mixes conventions '{existing}' and '{convention}'"
                )));
            }
            grouped_quotes.entry(key).or_default().push(vol_quote);
        }
    }

    if grouped_quotes.is_empty() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::TooFewPoints,
        ));
    }

    let vol_fit_tolerance = params.vol_tolerance.unwrap_or(0.0015);
    if !vol_fit_tolerance.is_finite() || vol_fit_tolerance <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "swaption vol_tolerance must be finite and positive in quoted volatility units".into(),
        ));
    }

    let mut sabr_params: BTreeMap<(u64, u64), SabrParameters> = BTreeMap::new();
    // Preserve each bucket's calibration forward for its ATM anchor.
    let mut calibration_forwards: BTreeMap<(u64, u64), f64> = BTreeMap::new();
    let mut bucket_errors: BTreeMap<(u64, u64), String> = BTreeMap::new();
    let mut count = 0;
    let mut total_iterations = 0;
    let mut sabr_winning_starts = Vec::new();
    let mut sabr_winning_iterations = Vec::new();
    let mut sabr_residual_evaluations = Vec::new();
    let mut sabr_bound_hits = Vec::new();
    let mut expiries_axis = Vec::new();
    let mut tenors_axis = Vec::new();

    for ((kb_exp, kb_ten), bucket_quotes) in &grouped_quotes {
        // Use conventions from a representative quote for this (expiry, tenor) bucket.
        // Market-standard: forward/par rate depends on schedule, DC, BDC, and calendar.
        let representative =
            bucket_quotes
                .first()
                .copied()
                .ok_or(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::TooFewPoints,
                ))?;
        let VolQuote::SwaptionVol { expiry, .. } = representative else {
            return Err(finstack_quant_core::Error::Validation(
                "Expected a SwaptionVol quote in the calibration bucket".into(),
            ));
        };
        let t_exp =
            day_count.year_fraction(params.base_date, *expiry, DayCountContext::default())?;
        let leg_conv = resolve_leg_conventions(params, representative)?;

        // Calculate the exact quote-defined underlying forward with the
        // registered settlement, calendar, business-day, and leg conventions.
        let (swap_start, swap_end) = resolve_underlying_dates(representative, &leg_conv)?;
        let t_ten = contractual_swap_tenor_years(swap_start, swap_end)?;
        let fwd_rate =
            calculate_forward_swap_rate_dates(params, swap_start, swap_end, &leg_conv, context)?;

        let mut strikes = Vec::new();
        let mut vols = Vec::new();
        let mut quote_error: Option<String> = None;

        for q in bucket_quotes {
            if let VolQuote::SwaptionVol {
                strike,
                vol,
                quote_type,
                ..
            } = q
            {
                strikes.push(*strike);
                match validate_quoted_vol(*vol, *quote_type, params.vol_convention) {
                    Ok(v) => vols.push(v),
                    Err(e) => {
                        quote_error = Some(format!(
                            "Invalid swaption vol quote at strike={:.12}: {}",
                            strike, e
                        ));
                        break;
                    }
                }
            }
        }

        if let Some(err) = quote_error {
            bucket_errors.insert((*kb_exp, *kb_ten), err);
            continue;
        }

        if strikes.len() < 3 {
            bucket_errors.insert(
                (*kb_exp, *kb_ten),
                format!(
                    "Need at least 3 strikes to calibrate SABR; got {}",
                    strikes.len()
                ),
            );
            continue;
        }

        // Map the absolute quote budget into a dimensionless necessary
        // bound for the shared solver. Final reporting checks each raw
        // quote error in the configured convention, without vega scaling.
        let min_quote = vols.iter().copied().fold(f64::INFINITY, f64::min);
        let sabr_calibrator = SabrCalibrator::new()
            .with_tolerance(vol_fit_tolerance / min_quote)
            .with_max_iterations(config.solver.max_iterations());

        let res = match params.vol_convention {
            SwaptionVolConvention::Normal => sabr_calibrator.with_atm_pinning(true).calibrate(
                fwd_rate,
                &strikes,
                &vols,
                t_exp,
                params.sabr_beta,
                VolQuoteType::Normal,
            ),
            SwaptionVolConvention::Lognormal => {
                sabr_calibrator.with_shift(SabrShift::None).calibrate(
                    fwd_rate,
                    &strikes,
                    &vols,
                    t_exp,
                    params.sabr_beta,
                    VolQuoteType::BlackLognormal,
                )
            }
            SwaptionVolConvention::ShiftedLognormal { shift } => {
                if !shift.is_finite() {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "Shifted lognormal convention requires a finite shift; got {}",
                        shift
                    )))
                } else {
                    sabr_calibrator
                        .with_shift(SabrShift::Fixed(shift))
                        .calibrate(
                            fwd_rate,
                            &strikes,
                            &vols,
                            t_exp,
                            params.sabr_beta,
                            VolQuoteType::ShiftedBlackLognormal,
                        )
                }
            }
        };

        match res {
            Ok(outcome) => {
                total_iterations += outcome.total_iterations;
                let bucket = format!("T={t_exp:.6},tenor={:.6}", *kb_ten as f64 / 10_000.0);
                sabr_winning_starts.push(format!(
                    "{bucket}:alpha={:.8},nu={:.8},rho={:.8}",
                    outcome.winning_start[0], outcome.winning_start[1], outcome.winning_start[2]
                ));
                sabr_winning_iterations.push(format!("{bucket}:{}", outcome.winning_iterations));
                sabr_residual_evaluations
                    .push(format!("{bucket}:{}", outcome.residual_evaluations));
                if !outcome.parameters_at_bounds.is_empty() {
                    sabr_bound_hits.push(format!(
                        "{bucket}:{}",
                        outcome.parameters_at_bounds.join("|")
                    ));
                }
                let p = outcome.parameters;
                sabr_params.insert((*kb_exp, *kb_ten), p);
                calibration_forwards.insert((*kb_exp, *kb_ten), fwd_rate);
                expiries_axis.push(t_exp);
                tenors_axis.push(t_ten);

                count += 1;
            }
            Err(e) => {
                bucket_errors.insert((*kb_exp, *kb_ten), e.to_string());
            }
        }
    }

    // Build grid (ATM vols on the target expiry–tenor grid).
    let target_expiries = params.target_expiries.clone();
    let target_tenors = params.target_tenors.clone();

    let extrap_policy = params.sabr_extrapolation;
    let allow_missing = params.allow_sabr_missing_bucket_fallback;

    // Bucket keys are rounded only for grouping. All model, interpolation,
    // and published coordinates retain the exact contractual year fractions.
    expiries_axis.sort_by(f64::total_cmp);
    expiries_axis.dedup();
    tenors_axis.sort_by(f64::total_cmp);
    tenors_axis.dedup();
    let expiry_bounds = expiries_axis
        .first()
        .copied()
        .zip(expiries_axis.last().copied());
    let tenor_bounds = tenors_axis
        .first()
        .copied()
        .zip(tenors_axis.last().copied());

    // Validate out-of-bounds behavior explicitly (no hidden extrapolation rules).
    if extrap_policy == SurfaceExtrapolationPolicy::Error && !sabr_params.is_empty() {
        if let Some((min_exp, max_exp)) = expiry_bounds {
            for &t in &target_expiries {
                if t < min_exp || t > max_exp {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Swaption target expiry {:.6} is out of bounds for calibrated expiries [{:.6}, {:.6}]. \
Set params.sabr_extrapolation='clamp' to allow flat extrapolation.",
                        t, min_exp, max_exp
                    )));
                }
            }
        }
        if let Some((min_ten, max_ten)) = tenor_bounds {
            for &t in &target_tenors {
                if t < min_ten || t > max_ten {
                    let failed = bucket_errors
                        .iter()
                        .filter(|((_, tenor), _)| ((*tenor as f64 / 10_000.0) - t).abs() < 1e-4)
                        .map(|(_, error)| error.as_str())
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Swaption target tenor {t:.6} is out of bounds for calibrated tenors \
                         [{min_ten:.6}, {max_ten:.6}]. Failed target bucket: {failed}. \
                         Set params.sabr_extrapolation='clamp' to allow flat extrapolation."
                    )));
                }
            }
        }
    }

    let mut interpolated_points = 0usize;
    let mut extrapolated_points = 0usize;

    let mut cube_params: Vec<SabrParameterData> = Vec::new();
    let mut cube_forwards: Vec<f64> = Vec::new();
    for &texp in &target_expiries {
        for &tten in &target_tenors {
            let key = (to_basis_points(texp), to_basis_points(tten));

            let is_clamped = if extrap_policy == SurfaceExtrapolationPolicy::Clamp {
                let mut clamped = false;
                if let Some((min_exp, max_exp)) = expiry_bounds
                    && (texp < min_exp || texp > max_exp)
                {
                    clamped = true;
                }
                if let Some((min_ten, max_ten)) = tenor_bounds
                    && (tten < min_ten || tten > max_ten)
                {
                    clamped = true;
                }
                clamped
            } else {
                false
            };

            let p = sabr_params.get(&key).cloned().or_else(|| {
                let p = interpolate_sabr_params_bilinear(
                    texp,
                    tten,
                    &sabr_params,
                    (&expiries_axis, &tenors_axis),
                    extrap_policy,
                    allow_missing,
                );
                if p.is_some() {
                    interpolated_points += 1;
                }
                p
            });

            if is_clamped {
                extrapolated_points += 1;
            }

            if let Some(p) = p {
                // Interpolated points fall back to index-default conventions.
                let f = if let Some(&cal_fwd) = calibration_forwards.get(&key) {
                    cal_fwd
                } else {
                    let leg_conv = default_leg_conventions(params)?;
                    calculate_forward_swap_rate_years(params, texp, tten, &leg_conv, context)?
                };

                let core_params = SabrParameterData::new(p.alpha, p.beta, p.nu, p.rho, p.shift)?;

                cube_params.push(core_params);
                cube_forwards.push(f);
            } else {
                // Market-standard: fail with context rather than silently returning a
                // placeholder cube.
                let available: Vec<String> = sabr_params
                    .keys()
                    .map(|(e, t)| {
                        format!("({:.4},{:.4})", *e as f64 / 10000.0, *t as f64 / 10000.0)
                    })
                    .collect();
                let bucket_hint = bucket_errors
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| "no bucket-specific error recorded".to_string());
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Swaption SABR params missing for target (expiry={texp:.4}, tenor={tten:.4}); \
                     available={:?}; bucket_error={}",
                    available, bucket_hint
                )));
            };
        }
    }

    let cube = VolCube::from_grid(
        &params.vol_surface_id,
        &target_expiries,
        &target_tenors,
        &cube_params,
        &cube_forwards,
    )?
    .with_expiry_clock(finstack_quant_core::dates::VolatilityClock::try_new(
        params.base_date,
        day_count,
        Default::default(),
    )?);

    // Acceptance must describe the artifact used by pricing. Resampling
    // onto target axes can discard a successfully calibrated smile.
    let mut residuals = BTreeMap::new();
    for quote in quotes {
        if let MarketQuote::Vol(
            vol_quote @ VolQuote::SwaptionVol {
                expiry,
                strike,
                vol,
                ..
            },
        ) = quote
        {
            let conventions = resolve_leg_conventions(params, vol_quote)?;
            let (start, end) = resolve_underlying_dates(vol_quote, &conventions)?;
            let expiry =
                day_count.year_fraction(params.base_date, *expiry, DayCountContext::default())?;
            let tenor = contractual_swap_tenor_years(start, end)?;
            let fitted = match params.vol_convention {
                SwaptionVolConvention::Normal => {
                    finstack_quant_models::volatility::get_cube_vol_clamped(
                        &cube,
                        expiry,
                        tenor,
                        *strike,
                        finstack_quant_models::volatility::VolatilityConvention::Normal,
                    )
                }
                SwaptionVolConvention::Lognormal
                | SwaptionVolConvention::ShiftedLognormal { .. } => {
                    finstack_quant_models::volatility::get_cube_vol_clamped(
                        &cube,
                        expiry,
                        tenor,
                        *strike,
                        finstack_quant_models::volatility::VolatilityConvention::Lognormal,
                    )
                }
            }?;
            residuals.insert(vol_quote.id().to_string(), fitted - vol);
        }
    }

    let vol_tolerance = vol_fit_tolerance;

    let mut report = CalibrationReport::for_type_with_tolerance(
        "swaption_vol",
        residuals,
        total_iterations,
        vol_tolerance,
    )
    .with_solver_method(crate::report::SolverMethod::PerSliceLeastSquares)
    .with_residual_units(crate::report::ResidualUnits::QuotedVolatility);
    report.update_metadata(
        "sabr_extrapolation_policy",
        match extrap_policy {
            SurfaceExtrapolationPolicy::Error => "error",
            SurfaceExtrapolationPolicy::Clamp => "clamp",
        },
    );
    report.update_metadata(
        "allow_sabr_missing_bucket_fallback",
        allow_missing.to_string(),
    );
    report.update_metadata(
        "interpolated_target_points",
        interpolated_points.to_string(),
    );
    report.update_metadata("clamped_target_points", extrapolated_points.to_string());
    report.update_metadata("sabr_winning_starts", sabr_winning_starts.join(";"));
    report.update_metadata("sabr_winning_iterations", sabr_winning_iterations.join(";"));
    report.update_metadata(
        "sabr_residual_evaluations",
        sabr_residual_evaluations.join(";"),
    );
    report.update_metadata("sabr_parameters_at_bounds", sabr_bound_hits.join(";"));

    // Item 3: a failed SABR expiry/tenor bucket must fail the surface calibration.
    //
    // Each bucket either calibrates (incrementing `count` and contributing per-strike
    // residuals) or fails (recorded in `bucket_errors` and `continue`d). The success
    // report was previously built from `residuals` alone — so a surface where most
    // buckets failed to fit still reported success because the survivors' residuals
    // were all in tolerance. A partially-fitted vol surface is NOT a calibrated
    // surface: mark the report failed whenever any bucket failed.
    report.update_metadata("calibrated_buckets", count.to_string());
    report.update_metadata("failed_buckets", bucket_errors.len().to_string());
    if !bucket_errors.is_empty() {
        let total_buckets = count + bucket_errors.len();
        // Summarise the failed buckets (expiry,tenor in years) with their errors.
        let mut failures: Vec<String> = bucket_errors
            .iter()
            .map(|((kb_exp, kb_ten), err)| {
                format!(
                    "(expiry={:.4},tenor={:.4}): {}",
                    *kb_exp as f64 / 10000.0,
                    *kb_ten as f64 / 10000.0,
                    err
                )
            })
            .collect();
        failures.sort();
        let summary = format!(
            "Swaption vol surface calibration failed: {} of {} SABR bucket(s) did not \
             calibrate. Failed buckets: [{}]",
            bucket_errors.len(),
            total_buckets,
            failures.join("; "),
        );
        report.success = false;
        report.validation_passed = false;
        report.validation_error = Some(summary.clone());
        report.convergence_reason = summary;
    }

    report.update_solver_config(config.solver.clone());

    Ok((cube, report))
}

// Market-standard forward/par swap rate + SABR parameter interpolation

/// Resolve swaption leg conventions from quote and plan parameters.
pub(crate) fn resolve_leg_conventions<'a>(
    params: &'a SwaptionVolParams,
    quote: &'a VolQuote,
) -> Result<SwaptionLegConventions<'a>> {
    let mut conventions = resolve_quote_leg_conventions(quote)?;
    if let Some(day_count) = params.fixed_day_count {
        conventions.fixed_day_count = day_count;
    }
    if let Some(calendar_id) = params.calendar_id.as_deref() {
        conventions.calendar_id = calendar_id;
    }
    Ok(conventions)
}

pub(crate) fn resolve_quote_leg_conventions(
    quote: &VolQuote,
) -> Result<SwaptionLegConventions<'_>> {
    let VolQuote::SwaptionVol {
        convention: swaption_conv_id,
        ..
    } = quote
    else {
        return Err(finstack_quant_core::Error::Validation(
            "Expected SwaptionVol quote".into(),
        ));
    };
    let registry = ConventionRegistry::try_global()?;
    let swaption_conv = registry.require_swaption(swaption_conv_id)?;
    let index_id = finstack_quant_core::types::IndexId::new(&swaption_conv.float_leg_index);
    let index_conv = registry.require_rate_index(&index_id)?;

    Ok(SwaptionLegConventions {
        currency: index_conv.currency,
        fixed_frequency: swaption_conv.fixed_leg_frequency,
        float_frequency: index_conv.default_payment_frequency,
        fixed_day_count: swaption_conv.fixed_leg_day_count,
        float_day_count: index_conv.day_count,
        fixed_business_day_convention: swaption_conv.business_day_convention,
        float_business_day_convention: index_conv.market_business_day_convention,
        calendar_id: swaption_conv.calendar_id.as_str(),
        settlement_days: swaption_conv.settlement_days,
        fixed_payment_lag_days: index_conv.default_payment_lag_days,
        float_payment_lag_days: index_conv.default_payment_lag_days,
        float_reset_lag_days: index_conv.default_reset_lag_days,
        float_compounding: index_conv
            .ois_compounding
            .unwrap_or(FloatingLegCompounding::Simple),
    })
}

pub(crate) fn resolve_underlying_dates(
    quote: &VolQuote,
    conventions: &SwaptionLegConventions<'_>,
) -> Result<(
    finstack_quant_core::dates::Date,
    finstack_quant_core::dates::Date,
)> {
    let VolQuote::SwaptionVol {
        expiry, maturity, ..
    } = quote
    else {
        return Err(finstack_quant_core::Error::Validation(
            "Expected SwaptionVol quote".into(),
        ));
    };
    let start = adjusted_swap_start(*expiry, conventions)?;
    if *maturity <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption maturity {maturity} must be after convention-adjusted swap start {start}"
        )));
    }
    Ok((start, *maturity))
}

pub(super) fn adjusted_swap_start(
    expiry: finstack_quant_core::dates::Date,
    conventions: &SwaptionLegConventions<'_>,
) -> Result<finstack_quant_core::dates::Date> {
    crate::build::spot_date::resolve_spot_date(
        expiry,
        conventions.calendar_id,
        conventions.settlement_days,
        conventions.fixed_business_day_convention,
    )
}

pub(super) fn default_leg_conventions<'a>(
    params: &'a SwaptionVolParams,
) -> Result<SwaptionLegConventions<'a>> {
    let idx = params.swap_index.as_ref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Swaption vol interpolation requires SwaptionVolParams.swap_index to be set"
                .to_string(),
        )
    })?;
    let idx_str = idx.as_str();
    let index_id = finstack_quant_core::types::IndexId::new(idx_str);
    let idx_conv = ConventionRegistry::try_global()?.require_rate_index(&index_id)?;

    Ok(SwaptionLegConventions {
        currency: idx_conv.currency,
        fixed_frequency: idx_conv.default_fixed_leg_frequency,
        float_frequency: idx_conv.default_payment_frequency,
        fixed_day_count: params
            .fixed_day_count
            .unwrap_or(idx_conv.default_fixed_leg_day_count),
        float_day_count: idx_conv.day_count,
        fixed_business_day_convention: idx_conv.market_business_day_convention,
        float_business_day_convention: idx_conv.market_business_day_convention,
        calendar_id: params
            .calendar_id
            .as_deref()
            .unwrap_or(idx_conv.market_calendar_id.as_str()),
        settlement_days: idx_conv.market_settlement_days,
        fixed_payment_lag_days: idx_conv.default_payment_lag_days,
        float_payment_lag_days: idx_conv.default_payment_lag_days,
        float_reset_lag_days: idx_conv.default_reset_lag_days,
        float_compounding: idx_conv
            .ois_compounding
            .unwrap_or(FloatingLegCompounding::Simple),
    })
}

pub(super) fn calculate_forward_swap_rate_years(
    params: &SwaptionVolParams,
    expiry_years: f64,
    tenor_years: f64,
    leg_conv: &SwaptionLegConventions<'_>,
    context: &MarketContext,
) -> Result<f64> {
    // Use month rounding to avoid float drift (e.g. 0.25*12=2.9999).
    let expiry_months = (expiry_years * 12.0).round();
    let tenor_months = (tenor_years * 12.0).round();
    if [expiry_months, tenor_months]
        .iter()
        .any(|months| !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(months))
    {
        return Err(finstack_quant_core::Error::Validation(
            "swaption expiry or tenor exceeds supported month-offset range".into(),
        ));
    }
    let expiry_date = params.base_date.add_months(expiry_months as i32)?;
    let swap_start = adjusted_swap_start(expiry_date, leg_conv)?;
    let maturity_date = swap_start.add_months(tenor_months as i32)?;
    calculate_forward_swap_rate_dates(params, swap_start, maturity_date, leg_conv, context)
}

fn calculate_forward_swap_rate_dates(
    params: &SwaptionVolParams,
    swap_start: finstack_quant_core::dates::Date,
    swap_end: finstack_quant_core::dates::Date,
    leg_conv: &SwaptionLegConventions<'_>,
    context: &MarketContext,
) -> Result<f64> {
    let disc = context.get_discount(&params.discount_curve_id)?;

    // PV01/annuity using a proper fixed-leg schedule.
    let pv01 = calculate_pv01_proper(swap_start, swap_end, leg_conv, disc.as_ref())?;
    if !pv01.is_finite() || pv01 <= 1e-16 {
        return Err(finstack_quant_core::Error::Validation(
            "swaption annuity (PV01) must be finite and positive".to_string(),
        ));
    }

    let float_periods = build_float_leg_periods(swap_start, swap_end, leg_conv)?;
    if float_periods.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "swaption floating leg produced no periods".to_string(),
        ));
    }
    let forward = params
        .forward_id
        .as_ref()
        .map(|id| context.get_forward(id))
        .transpose()?;
    let mut float_pv = 0.0;
    for period in float_periods {
        let t_pay_disc = disc.day_count().year_fraction(
            disc.base_date(),
            period.payment_date,
            DayCountContext::default(),
        )?;
        let coupon = if let Some(fwd) = &forward {
            fwd.as_ref().rate_between_dates(
                period.accrual_start,
                period.accrual_end,
                period.accrual_year_fraction,
            )? * period.accrual_year_fraction
        } else {
            if period.accrual_start < disc.base_date() {
                return Err(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::InvalidDateRange,
                ));
            }
            // The floating coupon accrues to the period end but pays on
            // its contractual payment date. Telescoping discount factors
            // is valid only when those dates coincide for every coupon.
            1.0 / disc.df_between_dates(period.accrual_start, period.accrual_end)? - 1.0
        };
        float_pv += coupon * disc.df(t_pay_disc)?;
    }
    Ok(float_pv / pv01)
}

pub(crate) fn build_float_leg_periods(
    start: finstack_quant_core::dates::Date,
    end: finstack_quant_core::dates::Date,
    leg_conv: &SwaptionLegConventions<'_>,
) -> Result<Vec<finstack_quant_cashflows::builder::SchedulePeriod>> {
    finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start,
            end,
            frequency: leg_conv.float_frequency,
            stub: StubKind::ShortBack,
            business_day_convention: leg_conv.float_business_day_convention,
            calendar_id: leg_conv.calendar_id,
            end_of_month: false,
            day_count: leg_conv.float_day_count,
            payment_lag_days: leg_conv.float_payment_lag_days,
            reset_lag_days: Some(leg_conv.float_reset_lag_days),
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
}

pub(crate) fn build_fixed_leg_periods(
    start: finstack_quant_core::dates::Date,
    end: finstack_quant_core::dates::Date,
    leg_conv: &SwaptionLegConventions<'_>,
) -> Result<Vec<finstack_quant_cashflows::builder::SchedulePeriod>> {
    finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start,
            end,
            frequency: leg_conv.fixed_frequency,
            stub: StubKind::ShortBack,
            business_day_convention: leg_conv.fixed_business_day_convention,
            calendar_id: leg_conv.calendar_id,
            end_of_month: false,
            day_count: leg_conv.fixed_day_count,
            payment_lag_days: leg_conv.fixed_payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
}

pub(super) fn calculate_pv01_proper(
    start: finstack_quant_core::dates::Date,
    end: finstack_quant_core::dates::Date,
    leg_conv: &SwaptionLegConventions<'_>,
    disc: &dyn finstack_quant_core::market_data::traits::Discounting,
) -> Result<f64> {
    let periods = build_fixed_leg_periods(start, end, leg_conv)?;
    if periods.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "swaption fixed leg produced no periods".to_string(),
        ));
    }

    let mut pv01 = 0.0_f64;
    for period in periods {
        let dcf = period.accrual_year_fraction;
        let t = disc.day_count().year_fraction(
            disc.base_date(),
            period.payment_date,
            DayCountContext::default(),
        )?;
        pv01 += disc.df(t)? * dcf;
    }
    Ok(pv01)
}

/// Find the indices of the interval bracketing a target point on an axis.
fn bracket_axis(
    axis: &[f64],
    target: f64,
    extrapolation: SurfaceExtrapolationPolicy,
) -> Option<(usize, usize)> {
    if axis.is_empty() {
        return None;
    }
    if axis.len() == 1 {
        let only = axis[0];
        if (target - only).abs() < 1e-12 {
            return Some((0, 0));
        }
        return match extrapolation {
            SurfaceExtrapolationPolicy::Clamp => Some((0, 0)),
            SurfaceExtrapolationPolicy::Error => None,
        };
    }

    if target < axis[0] {
        return match extrapolation {
            SurfaceExtrapolationPolicy::Clamp => Some((0, 0)),
            SurfaceExtrapolationPolicy::Error => None,
        };
    }
    if target > axis[axis.len() - 1] {
        let last = axis.len() - 1;
        return match extrapolation {
            SurfaceExtrapolationPolicy::Clamp => Some((last, last)),
            SurfaceExtrapolationPolicy::Error => None,
        };
    }

    for i in 0..axis.len() - 1 {
        if target >= axis[i] && target <= axis[i + 1] {
            return Some((i, i + 1));
        }
    }
    Some((axis.len() - 1, axis.len() - 1))
}

/// Interpolate SABR parameters across the 2D (expiry, tenor) grid.
pub(super) fn interpolate_sabr_params_bilinear(
    target_expiry: f64,
    target_tenor: f64,
    sabr_params: &BTreeMap<(u64, u64), SabrParameters>,
    axes: (&[f64], &[f64]),
    extrapolation: SurfaceExtrapolationPolicy,
    allow_missing_bucket_fallback: bool,
) -> Option<SabrParameters> {
    if sabr_params.is_empty() {
        return None;
    }

    let (expiries, tenors) = axes;
    if expiries.is_empty() || tenors.is_empty() {
        return None;
    }

    let (ei_lo, ei_hi) = bracket_axis(expiries, target_expiry, extrapolation)?;
    let (ti_lo, ti_hi) = bracket_axis(tenors, target_tenor, extrapolation)?;

    let e_lo = expiries[ei_lo];
    let e_hi = expiries[ei_hi];
    let t_lo = tenors[ti_lo];
    let t_hi = tenors[ti_hi];

    let fetch = |e: f64, t: f64| -> Option<&SabrParameters> {
        let key = (to_basis_points(e), to_basis_points(t));
        sabr_params.get(&key)
    };

    if ei_lo == ei_hi && ti_lo == ti_hi {
        return fetch(e_lo, t_lo).cloned();
    }

    // 1D tenor interpolation at a single expiry.
    if ei_lo == ei_hi && ti_lo != ti_hi {
        let p_lo = fetch(e_lo, t_lo)?;
        let p_hi = if allow_missing_bucket_fallback {
            fetch(e_lo, t_hi).unwrap_or(p_lo)
        } else {
            fetch(e_lo, t_hi)?
        };
        let wy = if (t_hi - t_lo).abs() > 0.0 {
            (target_tenor - t_lo) / (t_hi - t_lo)
        } else {
            0.0
        };
        return Some(interpolate_sabr_linear(p_lo, p_hi, wy));
    }

    // 1D expiry interpolation at a single tenor.
    if ti_lo == ti_hi && ei_lo != ei_hi {
        let p_lo = fetch(e_lo, t_lo)?;
        let p_hi = if allow_missing_bucket_fallback {
            fetch(e_hi, t_lo).unwrap_or(p_lo)
        } else {
            fetch(e_hi, t_lo)?
        };
        let wx = if (e_hi - e_lo).abs() > 0.0 {
            (target_expiry - e_lo) / (e_hi - e_lo)
        } else {
            0.0
        };
        return Some(interpolate_sabr_linear(p_lo, p_hi, wx));
    }

    // Full bilinear (with deterministic fallbacks for missing corners).
    let p_00 = fetch(e_lo, t_lo)?;
    let p_10 = if allow_missing_bucket_fallback {
        fetch(e_hi, t_lo).unwrap_or(p_00)
    } else {
        fetch(e_hi, t_lo)?
    };
    let p_01 = if allow_missing_bucket_fallback {
        fetch(e_lo, t_hi).unwrap_or(p_00)
    } else {
        fetch(e_lo, t_hi)?
    };
    let p_11 = if allow_missing_bucket_fallback {
        fetch(e_hi, t_hi).unwrap_or(p_10)
    } else {
        fetch(e_hi, t_hi)?
    };

    let wx = if (e_hi - e_lo).abs() > 0.0 {
        (target_expiry - e_lo) / (e_hi - e_lo)
    } else {
        0.0
    };
    let wy = if (t_hi - t_lo).abs() > 0.0 {
        (target_tenor - t_lo) / (t_hi - t_lo)
    } else {
        0.0
    };

    Some(interpolate_sabr_bilinear(p_00, p_10, p_01, p_11, wx, wy))
}

fn interpolate_sabr_linear(p0: &SabrParameters, p1: &SabrParameters, w: f64) -> SabrParameters {
    let w = w.clamp(0.0, 1.0);

    // Preserve positivity with log-space interpolation.
    let log_alpha0 = p0.alpha.max(1e-16).ln();
    let log_alpha1 = p1.alpha.max(1e-16).ln();
    let log_nu0 = p0.nu.max(1e-16).ln();
    let log_nu1 = p1.nu.max(1e-16).ln();

    let alpha = (log_alpha0 * (1.0 - w) + log_alpha1 * w).exp();
    let nu = (log_nu0 * (1.0 - w) + log_nu1 * w).exp();

    let rho_raw = p0.rho * (1.0 - w) + p1.rho * w;
    let rho = rho_raw.clamp(-0.999, 0.999);

    SabrParameters {
        alpha,
        beta: p0.beta,
        nu,
        rho,
        shift: p0.shift,
    }
}

fn interpolate_sabr_bilinear(
    p_00: &SabrParameters,
    p_10: &SabrParameters,
    p_01: &SabrParameters,
    p_11: &SabrParameters,
    wx: f64,
    wy: f64,
) -> SabrParameters {
    let wx = wx.clamp(0.0, 1.0);
    let wy = wy.clamp(0.0, 1.0);

    let p0 = interpolate_sabr_linear(p_00, p_10, wx);
    let p1 = interpolate_sabr_linear(p_01, p_11, wx);
    interpolate_sabr_linear(&p0, &p1, wy)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SwaptionLegConventions<'a> {
    pub(crate) currency: finstack_quant_core::currency::Currency,
    pub(crate) fixed_frequency: Tenor,
    pub(crate) float_frequency: Tenor,
    fixed_day_count: DayCount,
    pub(crate) float_day_count: DayCount,
    fixed_business_day_convention: BusinessDayConvention,
    pub(crate) float_business_day_convention: BusinessDayConvention,
    pub(crate) calendar_id: &'a str,
    pub(crate) settlement_days: i32,
    fixed_payment_lag_days: i32,
    pub(crate) float_payment_lag_days: i32,
    pub(crate) float_reset_lag_days: i32,
    pub(crate) float_compounding: FloatingLegCompounding,
}

pub(super) fn to_basis_points(value: f64) -> u64 {
    (value * 10000.0).round() as u64
}
