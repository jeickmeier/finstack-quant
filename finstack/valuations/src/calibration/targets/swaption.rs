use crate::calibration::api::schema::{
    SabrInterpolationMethod, SurfaceExtrapolationPolicy, SwaptionVolConvention, SwaptionVolParams,
};
use crate::calibration::config::CalibrationConfig;
use crate::calibration::CalibrationReport;
use crate::instruments::common_impl::models::{SABRCalibrator, SABRModel, SABRParameters};
use crate::market::conventions::registry::ConventionRegistry;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::vol::VolQuote;
use finstack_core::dates::{
    BusinessDayConvention, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::surfaces::VolCube;
use finstack_core::math::volatility::sabr::SabrParams;
use finstack_core::Result;
use std::collections::BTreeMap;

#[cfg(test)]
use crate::market::conventions::ids::SwaptionConventionId;

/// Bootstrapper for calibrating swaption volatility surfaces.
///
/// Calibrates volatility surfaces from swaption quotes using the SABR model.
/// Groups quotes by expiry and tenor, calibrates SABR parameters per group,
/// and builds a volatility surface grid.
pub(crate) struct SwaptionVolTarget;

impl SwaptionVolTarget {
    /// Convert a quoted swaption vol to internal model units (decimal).
    ///
    /// Internal contract:
    /// - Normal vols are absolute (rate) vols as decimals (e.g., 50bp -> 0.0050)
    /// - Lognormal/shifted-lognormal vols are Black vols as decimals (e.g., 20% -> 0.20)
    fn normalize_quoted_vol(quoted: f64, convention: SwaptionVolConvention) -> Result<f64> {
        if !quoted.is_finite() || quoted < 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Swaption vol must be finite and non-negative; got {}",
                quoted
            )));
        }

        let normalized = match convention {
            SwaptionVolConvention::Normal => quoted / 10_000.0, // bp -> decimal
            SwaptionVolConvention::Lognormal => quoted / 100.0, // percent -> decimal
            SwaptionVolConvention::ShiftedLognormal { .. } => quoted / 100.0, // percent -> decimal
        };

        if !normalized.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Swaption vol normalization produced non-finite value; quoted={} convention={:?}",
                quoted, convention
            )));
        }

        // Sanity-range check on the normalized vol. Catches off-by-100 unit
        // errors (e.g., a 50bp Normal vol entered as `50` rather than `0.50`,
        // which divides to 0.005 — well below market norms — or a 20% LN vol
        // entered as `20.0` and treated as `0.20` correctly but `2000`
        // treated as `20.0`, off the chart). Range [1e-4, 2.0] covers both
        // Normal (100bp = 0.01, ~1% extreme) and Lognormal (1% to 200%)
        // regimes including most stress scenarios.
        if !(1e-4..=2.0).contains(&normalized) {
            return Err(finstack_core::Error::Validation(format!(
                "normalized swaption vol {normalized:.6} (quoted={quoted}, convention={convention:?}) \
                 is outside plausible market range [1e-4, 2.0] — check that the quoted vol's units \
                 match the declared convention",
            )));
        }

        Ok(normalized)
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
        let mut grouped_quotes: QuotesByExpiryTenor<'_> = BTreeMap::new();
        let dc = if let Some(dc) = params.fixed_day_count {
            dc
        } else {
            let mut idx_from_quotes = None;
            for quote in quotes {
                let MarketQuote::Vol(VolQuote::SwaptionVol { convention, .. }) = quote else {
                    continue;
                };
                let registry = ConventionRegistry::try_global()?;
                let swaption_conv = registry.require_swaption(convention)?;
                idx_from_quotes = Some(crate::market::conventions::ids::IndexId::new(
                    &swaption_conv.float_leg_index,
                ));
                break;
            }
            let idx_key = params
                .swap_index
                .as_ref()
                .map(|core_idx| crate::market::conventions::ids::IndexId::new(core_idx.as_str()))
                .or(idx_from_quotes)
                .ok_or_else(|| {
                    finstack_core::Error::Validation(
                    "Swaption vol calibration requires either SwaptionVolParams.fixed_day_count \
                     or SwaptionVolParams.swap_index (or per-quote convention)"
                        .to_string(),
                )
                })?;
            ConventionRegistry::try_global()?
                .require_rate_index(&idx_key)?
                .default_fixed_leg_day_count
        };

        for q in quotes {
            if let MarketQuote::Vol(VolQuote::SwaptionVol {
                expiry, maturity, ..
            }) = q
            {
                if *maturity < *expiry {
                    return Err(finstack_core::Error::Validation(format!(
                        "Swaption maturity {} must be on/after expiry {}",
                        maturity, expiry
                    )));
                }
                let t_exp =
                    dc.year_fraction(params.base_date, *expiry, DayCountContext::default())?;
                let t_ten = dc.year_fraction(*expiry, *maturity, DayCountContext::default())?;
                let key = (to_basis_points(t_exp), to_basis_points(t_ten));

                if let MarketQuote::Vol(vq) = q {
                    grouped_quotes.entry(key).or_default().push(vq);
                }
            }
        }

        if grouped_quotes.is_empty() {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
            ));
        }

        let vol_fit_tolerance = params.vol_tolerance.unwrap_or(0.0015);
        let sabr_solver_tolerance = params.sabr_tolerance.unwrap_or(1e-6);
        let sabr_calibrator = SABRCalibrator::new()
            .with_tolerance(sabr_solver_tolerance)
            .with_max_iterations(config.solver.max_iterations());

        let mut sabr_params: SABRParamsByExpiryTenor = BTreeMap::new();
        let mut residuals = BTreeMap::new();
        let mut bucket_errors: BTreeMap<(u64, u64), String> = BTreeMap::new();
        let mut count = 0;

        for ((kb_exp, kb_ten), bucket_quotes) in &grouped_quotes {
            let t_exp = *kb_exp as f64 / 10000.0;
            let t_ten = *kb_ten as f64 / 10000.0;

            // Use conventions from a representative quote for this (expiry, tenor) bucket.
            // Market-standard: forward/par rate depends on schedule, DC, BDC, and calendar.
            let representative =
                bucket_quotes
                    .first()
                    .copied()
                    .ok_or(finstack_core::Error::Input(
                        finstack_core::InputError::TooFewPoints,
                    ))?;
            let leg_conv = Self::resolve_leg_conventions(params, representative)?;

            // Calculate forward swap rate (exact PV01 schedule; multi-curve supported).
            let fwd_rate =
                Self::calculate_forward_swap_rate_years(params, t_exp, t_ten, &leg_conv, context)?;

            let mut strikes = Vec::new();
            let mut vols = Vec::new();
            let mut quote_error: Option<String> = None;

            for q in bucket_quotes {
                if let VolQuote::SwaptionVol { strike, vol, .. } = q {
                    strikes.push(*strike);
                    match Self::normalize_quoted_vol(*vol, params.vol_convention) {
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

            // Calibrate
            // Need to handle conventions (normal/lognormal).
            // Simplified: assume lognormal if beta != 0, normal if beta == 0
            // Params has explicit convention.

            let res = match params.vol_convention {
                SwaptionVolConvention::Normal => sabr_calibrator
                    .calibrate_with_atm_pinning(fwd_rate, &strikes, &vols, t_exp, 0.0),
                SwaptionVolConvention::Lognormal => sabr_calibrator.calibrate_auto_shift(
                    fwd_rate,
                    &strikes,
                    &vols,
                    t_exp,
                    params.sabr_beta,
                ),
                SwaptionVolConvention::ShiftedLognormal { shift } => {
                    if !shift.is_finite() || shift <= 0.0 {
                        Err(finstack_core::Error::Validation(format!(
                            "Shifted lognormal convention requires a finite, positive shift; got {}",
                            shift
                        )))
                    } else {
                        sabr_calibrator.calibrate_shifted(
                            fwd_rate,
                            &strikes,
                            &vols,
                            t_exp,
                            params.sabr_beta,
                            shift,
                        )
                    }
                }
            };

            match res {
                Ok(p) => {
                    sabr_params.insert((*kb_exp, *kb_ten), p.clone());

                    let model = SABRModel::new(p);
                    for (i, k) in strikes.iter().enumerate() {
                        let v = model.implied_volatility(fwd_rate, *k, t_exp)?;
                        residuals.insert(
                            format!("swpt_{}_{}_{}", kb_exp, kb_ten, i),
                            (v - vols[i]).abs(),
                        );
                    }
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

        let (expiries_axis, tenors_axis) = Self::sabr_grid_axes(&sabr_params);
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
                        return Err(finstack_core::Error::Validation(format!(
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
                        return Err(finstack_core::Error::Validation(format!(
                            "Swaption target tenor {:.6} is out of bounds for calibrated tenors [{:.6}, {:.6}]. \
Set params.sabr_extrapolation='clamp' to allow flat extrapolation.",
                            t, min_ten, max_ten
                        )));
                    }
                }
            }
        }

        let mut interpolated_points = 0usize;
        let mut extrapolated_points = 0usize;

        let mut cube_params: Vec<SabrParams> = Vec::new();
        let mut cube_forwards: Vec<f64> = Vec::new();
        for &texp in &target_expiries {
            for &tten in &target_tenors {
                let key = (to_basis_points(texp), to_basis_points(tten));

                let is_clamped = if extrap_policy == SurfaceExtrapolationPolicy::Clamp {
                    let mut clamped = false;
                    if let Some((min_exp, max_exp)) = expiry_bounds {
                        if texp < min_exp || texp > max_exp {
                            clamped = true;
                        }
                    }
                    if let Some((min_ten, max_ten)) = tenor_bounds {
                        if tten < min_ten || tten > max_ten {
                            clamped = true;
                        }
                    }
                    clamped
                } else {
                    false
                };

                let p = sabr_params.get(&key).cloned().or_else(|| {
                    let p = match params.sabr_interpolation {
                        SabrInterpolationMethod::Bilinear => {
                            Self::interpolate_sabr_params_bilinear(
                                texp,
                                tten,
                                &sabr_params,
                                extrap_policy,
                                allow_missing,
                            )
                        }
                    };
                    if p.is_some() {
                        interpolated_points += 1;
                    }
                    p
                });

                if is_clamped {
                    extrapolated_points += 1;
                }

                if let Some(p) = p {
                    let leg_conv = Self::default_leg_conventions(params)?;
                    let f = Self::calculate_forward_swap_rate_years(
                        params, texp, tten, &leg_conv, context,
                    )?;

                    let core_params = SabrParams {
                        alpha: p.alpha,
                        beta: p.beta,
                        rho: p.rho,
                        nu: p.nu,
                        shift: p.shift,
                    };

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
                    return Err(finstack_core::Error::Validation(format!(
                        "Swaption SABR params missing for target (expiry={texp:.4}, tenor={tten:.4}); \
                         available={:?}; bucket_error={}",
                        available, bucket_hint
                    )));
                };
            }
        }

        let cube = VolCube::from_grid(
            &params.surface_id,
            &target_expiries,
            &target_tenors,
            &cube_params,
            &cube_forwards,
        )?;

        let vol_tolerance = vol_fit_tolerance;

        let mut report = CalibrationReport::for_type_with_tolerance(
            "swaption_vol",
            residuals,
            count,
            vol_tolerance,
        );
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

    // =========================================================================
    // Market-standard forward/par swap rate + SABR parameter interpolation
    // =========================================================================

    /// Resolve swaption leg conventions from quote and plan parameters.
    fn resolve_leg_conventions<'a>(
        params: &'a SwaptionVolParams,
        quote: &'a VolQuote,
    ) -> Result<SwaptionLegConventions<'a>> {
        let swaption_conv_id = match quote {
            VolQuote::SwaptionVol { convention, .. } => convention,
            _ => {
                return Err(finstack_core::Error::Validation(
                    "Expected SwaptionVol quote".into(),
                ))
            }
        };
        let swaption_conv = ConventionRegistry::try_global()?.require_swaption(swaption_conv_id)?;

        let idx_key = crate::market::conventions::ids::IndexId::new(&swaption_conv.float_leg_index);
        let idx_conv = ConventionRegistry::try_global()?.require_rate_index(&idx_key)?;

        // Strict conventions: We use exactly what's in the registry.
        let fixed_freq = idx_conv.default_fixed_leg_frequency;
        let float_freq = idx_conv.default_payment_frequency;

        let fixed_day_count = params
            .fixed_day_count
            .unwrap_or(idx_conv.default_fixed_leg_day_count);
        let float_day_count = idx_conv.day_count;

        let bdc = swaption_conv.business_day_convention; // Or index market BDC? usually swaption follows index but overrides might happen.
                                                         // Actually, let's use the index convention for swap details, as swaption convention is for the OPTION part usually?
                                                         // But SwaptionConventions likely points to the underlying swap index.
                                                         // Let's stick to index conventions for swap leg details.

        let calendar_id = params
            .calendar_id
            .as_deref()
            .or(Some(swaption_conv.calendar_id.as_str()));

        Ok(SwaptionLegConventions {
            fixed_freq,
            float_freq,
            fixed_day_count,
            float_day_count,
            fixed_bdc: bdc,
            float_bdc: bdc,
            calendar_id,
        })
    }

    fn default_leg_conventions<'a>(
        params: &'a SwaptionVolParams,
    ) -> Result<SwaptionLegConventions<'a>> {
        let idx = params.swap_index.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Swaption vol interpolation requires SwaptionVolParams.swap_index to be set"
                    .to_string(),
            )
        })?;
        let idx_str = idx.as_str();
        let index_id = crate::market::conventions::ids::IndexId::new(idx_str);
        let idx_conv = ConventionRegistry::try_global()?.require_rate_index(&index_id)?;

        Ok(SwaptionLegConventions {
            fixed_freq: idx_conv.default_fixed_leg_frequency,
            float_freq: idx_conv.default_payment_frequency,
            fixed_day_count: params
                .fixed_day_count
                .unwrap_or(idx_conv.default_fixed_leg_day_count),
            float_day_count: idx_conv.day_count,
            fixed_bdc: idx_conv.market_business_day_convention,
            float_bdc: idx_conv.market_business_day_convention,
            calendar_id: params
                .calendar_id
                .as_deref()
                .or(Some(idx_conv.market_calendar_id.as_str())),
        })
    }

    fn calculate_forward_swap_rate_years(
        params: &SwaptionVolParams,
        expiry_years: f64,
        tenor_years: f64,
        leg_conv: &SwaptionLegConventions<'_>,
        context: &MarketContext,
    ) -> Result<f64> {
        // Use month rounding to avoid float drift (e.g. 0.25*12=2.9999).
        let expiry_months = (expiry_years * 12.0).round() as i32;
        let tenor_months = (tenor_years * 12.0).round() as i32;
        let expiry_date = params.base_date.add_months(expiry_months);
        let maturity_date = expiry_date.add_months(tenor_months);
        Self::calculate_forward_swap_rate_dates(
            params,
            expiry_date,
            maturity_date,
            leg_conv,
            context,
        )
    }

    fn calculate_forward_swap_rate_dates(
        params: &SwaptionVolParams,
        swap_start: finstack_core::dates::Date,
        swap_end: finstack_core::dates::Date,
        leg_conv: &SwaptionLegConventions<'_>,
        context: &MarketContext,
    ) -> Result<f64> {
        let disc = context.get_discount(&params.discount_curve_id)?;

        // PV01/annuity using a proper fixed-leg schedule.
        let pv01 = Self::calculate_pv01_proper(swap_start, swap_end, leg_conv, disc.as_ref())?;
        if !pv01.is_finite() || pv01 <= 1e-16 {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::Invalid,
            ));
        }

        // Multi-curve mode: use forward curve for the floating leg PV if configured.
        if let Some(ref forward_id) = params.forward_id {
            let fwd = context.get_forward(forward_id)?;

            let float_sched = crate::cashflow::builder::date_generation::build_dates(
                swap_start,
                swap_end,
                leg_conv.float_freq,
                StubKind::None,
                leg_conv.float_bdc,
                false,
                0,
                leg_conv
                    .calendar_id
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            )?;
            if float_sched.periods.is_empty() {
                return Err(finstack_core::Error::Input(
                    finstack_core::InputError::Invalid,
                ));
            }

            let mut float_pv = 0.0_f64;
            for period in float_sched.periods {
                let accrual = leg_conv.float_day_count.year_fraction(
                    period.accrual_start,
                    period.accrual_end,
                    DayCountContext::default(),
                )?;

                let t_pay_disc = disc.day_count().year_fraction(
                    disc.base_date(),
                    period.payment_date,
                    DayCountContext::default(),
                )?;

                let t_prev_fwd = fwd.day_count().year_fraction(
                    fwd.base_date(),
                    period.accrual_start,
                    DayCountContext::default(),
                )?;
                let t_pay_fwd = fwd.day_count().year_fraction(
                    fwd.base_date(),
                    period.accrual_end,
                    DayCountContext::default(),
                )?;

                let forward_rate = fwd.rate_period(t_prev_fwd, t_pay_fwd);
                float_pv += forward_rate * accrual * disc.df(t_pay_disc);
            }

            Ok(float_pv / pv01)
        } else {
            // Single-curve mode: (DF_start - DF_end) / PV01 with consistent curve day-count.
            let t_start = disc.day_count().year_fraction(
                disc.base_date(),
                swap_start,
                DayCountContext::default(),
            )?;
            let t_end = disc.day_count().year_fraction(
                disc.base_date(),
                swap_end,
                DayCountContext::default(),
            )?;
            if t_start < 0.0 || t_end <= t_start {
                return Err(finstack_core::Error::Input(
                    finstack_core::InputError::InvalidDateRange,
                ));
            }
            let df_start = disc.df(t_start);
            let df_end = disc.df(t_end);
            Ok((df_start - df_end) / pv01)
        }
    }

    fn calculate_pv01_proper(
        start: finstack_core::dates::Date,
        end: finstack_core::dates::Date,
        leg_conv: &SwaptionLegConventions<'_>,
        disc: &dyn finstack_core::market_data::traits::Discounting,
    ) -> Result<f64> {
        let sched = crate::cashflow::builder::date_generation::build_dates(
            start,
            end,
            leg_conv.fixed_freq,
            StubKind::None,
            leg_conv.fixed_bdc,
            false,
            0,
            leg_conv
                .calendar_id
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
        )?;
        if sched.periods.is_empty() {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::Invalid,
            ));
        }

        let mut pv01 = 0.0_f64;
        for period in sched.periods {
            let dcf = leg_conv.fixed_day_count.year_fraction(
                period.accrual_start,
                period.accrual_end,
                DayCountContext::default(),
            )?;
            let t = disc.day_count().year_fraction(
                disc.base_date(),
                period.payment_date,
                DayCountContext::default(),
            )?;
            pv01 += disc.df(t) * dcf;
        }
        Ok(pv01)
    }

    /// Extract unique expiry and tenor axes from a parameter map.
    fn sabr_grid_axes(sabr_params: &SABRParamsByExpiryTenor) -> (Vec<f64>, Vec<f64>) {
        let mut expiries_bp = Vec::new();
        let mut tenors_bp = Vec::new();

        for (key, _) in sabr_params.iter() {
            let (exp_bp, ten_bp) = *key;
            expiries_bp.push(exp_bp);
            tenors_bp.push(ten_bp);
        }

        expiries_bp.sort_unstable();
        expiries_bp.dedup();
        tenors_bp.sort_unstable();
        tenors_bp.dedup();

        let expiries = expiries_bp
            .into_iter()
            .map(|bp| bp as f64 / 10000.0)
            .collect();
        let tenors = tenors_bp
            .into_iter()
            .map(|bp| bp as f64 / 10000.0)
            .collect();

        (expiries, tenors)
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
    fn interpolate_sabr_params_bilinear(
        target_expiry: f64,
        target_tenor: f64,
        sabr_params: &SABRParamsByExpiryTenor,
        extrapolation: SurfaceExtrapolationPolicy,
        allow_missing_bucket_fallback: bool,
    ) -> Option<SABRParameters> {
        if sabr_params.is_empty() {
            return None;
        }

        let (expiries, tenors) = Self::sabr_grid_axes(sabr_params);
        if expiries.is_empty() || tenors.is_empty() {
            return None;
        }

        let (ei_lo, ei_hi) = Self::bracket_axis(&expiries, target_expiry, extrapolation)?;
        let (ti_lo, ti_hi) = Self::bracket_axis(&tenors, target_tenor, extrapolation)?;

        let e_lo = expiries[ei_lo];
        let e_hi = expiries[ei_hi];
        let t_lo = tenors[ti_lo];
        let t_hi = tenors[ti_hi];

        let fetch = |e: f64, t: f64| -> Option<&SABRParameters> {
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
            return Some(Self::interpolate_sabr_linear(p_lo, p_hi, wy));
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
            return Some(Self::interpolate_sabr_linear(p_lo, p_hi, wx));
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

        Some(Self::interpolate_sabr_bilinear(
            p_00, p_10, p_01, p_11, wx, wy,
        ))
    }

    fn interpolate_sabr_linear(p0: &SABRParameters, p1: &SABRParameters, w: f64) -> SABRParameters {
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

        SABRParameters {
            alpha,
            beta: p0.beta,
            nu,
            rho,
            shift: p0.shift,
        }
    }

    fn interpolate_sabr_bilinear(
        p_00: &SABRParameters,
        p_10: &SABRParameters,
        p_01: &SABRParameters,
        p_11: &SABRParameters,
        wx: f64,
        wy: f64,
    ) -> SABRParameters {
        let wx = wx.clamp(0.0, 1.0);
        let wy = wy.clamp(0.0, 1.0);

        let p0 = Self::interpolate_sabr_linear(p_00, p_10, wx);
        let p1 = Self::interpolate_sabr_linear(p_01, p_11, wx);
        Self::interpolate_sabr_linear(&p0, &p1, wy)
    }
}

type QuotesByExpiryTenor<'a> = BTreeMap<(u64, u64), Vec<&'a VolQuote>>;
type SABRParamsByExpiryTenor = BTreeMap<(u64, u64), SABRParameters>;

#[derive(Debug, Clone, Copy)]
struct SwaptionLegConventions<'a> {
    fixed_freq: Tenor,
    float_freq: Tenor,
    fixed_day_count: DayCount,
    float_day_count: DayCount,
    fixed_bdc: BusinessDayConvention,
    float_bdc: BusinessDayConvention,
    calendar_id: Option<&'a str>,
}

fn to_basis_points(value: f64) -> u64 {
    (value * 10000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::quotes::ids::QuoteId;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::types::CurveId;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn params(base_date: Date) -> SwaptionVolParams {
        SwaptionVolParams {
            surface_id: "USD-SWPTN".to_string(),
            base_date,
            discount_curve_id: CurveId::from("USD-OIS"),
            forward_id: None,
            currency: Currency::USD,
            vol_convention: SwaptionVolConvention::Lognormal,
            atm_convention: crate::calibration::api::schema::AtmStrikeConvention::SwapRate,
            sabr_beta: 0.5,
            target_expiries: vec![1.0, 2.0],
            target_tenors: vec![5.0, 10.0],
            sabr_interpolation: crate::calibration::api::schema::SabrInterpolationMethod::Bilinear,
            calendar_id: None,
            fixed_day_count: Some(DayCount::Act365F),
            swap_index: Some("USD-SOFR-3M".into()),
            vol_tolerance: None,
            sabr_tolerance: None,
            sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
            allow_sabr_missing_bucket_fallback: false,
        }
    }

    #[test]
    fn normalize_quoted_vol_converts_units_to_decimals() {
        let normal = SwaptionVolTarget::normalize_quoted_vol(50.0, SwaptionVolConvention::Normal)
            .expect("normal");
        assert!((normal - 0.005).abs() < 1e-12);

        let ln = SwaptionVolTarget::normalize_quoted_vol(20.0, SwaptionVolConvention::Lognormal)
            .expect("lognormal");
        assert!((ln - 0.20).abs() < 1e-12);

        let shifted = SwaptionVolTarget::normalize_quoted_vol(
            20.0,
            SwaptionVolConvention::ShiftedLognormal { shift: 0.01 },
        )
        .expect("shifted");
        assert!((shifted - 0.20).abs() < 1e-12);
    }

    #[test]
    fn calibrate_normal_quotes_in_bp_preserves_atm_vol_in_model_units() {
        let base_date = date(2024, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, 0.20)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let expiry_years: f64 = 1.0;
        let tenor_years: f64 = 5.0;
        let expiry_date = base_date.add_months((expiry_years * 12.0).round() as i32);
        let maturity_date = expiry_date.add_months((tenor_years * 12.0).round() as i32);
        let t_exp_raw = DayCount::Act365F
            .year_fraction(base_date, expiry_date, DayCountContext::default())
            .expect("t_exp");
        let t_ten_raw = DayCount::Act365F
            .year_fraction(expiry_date, maturity_date, DayCountContext::default())
            .expect("t_ten");
        let t_exp = to_basis_points(t_exp_raw) as f64 / 10_000.0;
        let t_ten = to_basis_points(t_ten_raw) as f64 / 10_000.0;

        let mut p = params(base_date);
        p.vol_convention = SwaptionVolConvention::Normal;
        p.sabr_beta = 0.0;
        p.target_expiries = vec![t_exp];
        p.target_tenors = vec![t_ten];
        p.vol_tolerance = Some(0.0020);

        let leg = SwaptionVolTarget::default_leg_conventions(&p).expect("leg conventions");
        let fwd = SwaptionVolTarget::calculate_forward_swap_rate_years(
            &p,
            expiry_years,
            tenor_years,
            &leg,
            &ctx,
        )
        .expect("forward");

        let true_alpha = 0.0050;
        let sabr_true = SABRParameters {
            alpha: true_alpha,
            beta: 0.0,
            nu: 0.60,
            rho: -0.20,
            shift: None,
        };
        let model = SABRModel::new(sabr_true);

        let strikes = vec![fwd - 0.005, fwd, fwd + 0.005, fwd + 0.010, fwd - 0.010];

        let mut quotes = Vec::new();
        for &k in &strikes {
            let vol_dec = model.implied_volatility(fwd, k, t_exp).expect("true vol");
            let vol_bp = vol_dec * 10_000.0;
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("USD-SWPTN-VOL-1Yx5Y-{k}")),
                expiry: expiry_date,
                maturity: maturity_date,
                strike: k,
                vol: vol_bp,
                quote_type: "implied_vol".to_string(),
                convention: SwaptionConventionId::new("USD-Annual"),
            }));
        }

        let config = CalibrationConfig::default();
        let (cube, _report) = SwaptionVolTarget::solve(&p, &quotes, &ctx, &config).expect("solve");

        // VolCube stores SABR params; verify calibrated alpha matches ground truth.
        // For beta=0 (normal SABR), alpha IS the ATM normal vol.
        let calibrated = cube.params_at(0, 0);
        assert!(
            (calibrated.alpha - true_alpha).abs() <= 0.0005,
            "alpha mismatch: calibrated={} true={}",
            calibrated.alpha,
            true_alpha
        );
    }

    #[test]
    fn calibrate_lognormal_quotes_in_percent_preserves_atm_vol_in_model_units() {
        let base_date = date(2024, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, 0.20)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let expiry_years: f64 = 1.0;
        let tenor_years: f64 = 5.0;
        let expiry_date = base_date.add_months((expiry_years * 12.0).round() as i32);
        let maturity_date = expiry_date.add_months((tenor_years * 12.0).round() as i32);
        let t_exp_raw = DayCount::Act365F
            .year_fraction(base_date, expiry_date, DayCountContext::default())
            .expect("t_exp");
        let t_ten_raw = DayCount::Act365F
            .year_fraction(expiry_date, maturity_date, DayCountContext::default())
            .expect("t_ten");
        let t_exp = to_basis_points(t_exp_raw) as f64 / 10_000.0;
        let t_ten = to_basis_points(t_ten_raw) as f64 / 10_000.0;

        let mut p = params(base_date);
        p.vol_convention = SwaptionVolConvention::Lognormal;
        p.sabr_beta = 0.5;
        p.target_expiries = vec![t_exp];
        p.target_tenors = vec![t_ten];
        p.vol_tolerance = Some(0.0020);

        let leg = SwaptionVolTarget::default_leg_conventions(&p).expect("leg conventions");
        let fwd = SwaptionVolTarget::calculate_forward_swap_rate_years(
            &p,
            expiry_years,
            tenor_years,
            &leg,
            &ctx,
        )
        .expect("forward");

        let sabr_true = SABRParameters {
            alpha: 0.020,
            beta: p.sabr_beta,
            nu: 0.30,
            rho: -0.20,
            shift: None,
        };
        let model = SABRModel::new(sabr_true);

        let strikes = vec![fwd - 0.010, fwd - 0.005, fwd, fwd + 0.005, fwd + 0.010];

        let mut quotes = Vec::new();
        for &k in &strikes {
            let vol_dec = model.implied_volatility(fwd, k, t_exp).expect("true vol");
            let vol_pct = vol_dec * 100.0;
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("USD-SWPTN-VOL-LN-1Yx5Y-{k}")),
                expiry: expiry_date,
                maturity: maturity_date,
                strike: k,
                vol: vol_pct,
                quote_type: "implied_vol".to_string(),
                convention: SwaptionConventionId::new("USD-Annual"),
            }));
        }

        let config = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_max_iterations(500),
            ..CalibrationConfig::default()
        };
        let (cube, _report) = SwaptionVolTarget::solve(&p, &quotes, &ctx, &config).expect("solve");

        let fitted_atm = cube.vol(t_exp, t_ten, fwd).expect("cube vol");
        let true_atm = model.implied_volatility(fwd, fwd, t_exp).expect("true atm");

        assert!(
            (fitted_atm - true_atm).abs() <= 0.0005,
            "atm mismatch: fitted={} true={}",
            fitted_atm,
            true_atm
        );
    }

    /// Item 3: a failed SABR expiry/tenor bucket must fail the whole surface report.
    ///
    /// One bucket (1Y×5Y) is given five well-behaved strikes and calibrates cleanly.
    /// A second bucket (2Y×5Y) is given only two strikes — below the three-strike SABR
    /// minimum — so it lands in `bucket_errors` and is `continue`d. The target grid
    /// points only at the good bucket, so the `VolCube` still builds. Pre-fix the report
    /// was assembled from the surviving bucket's residuals alone and reported
    /// `success = true`; post-fix the non-empty `bucket_errors` must force
    /// `success = false`.
    #[test]
    fn item3_failed_sabr_bucket_fails_surface_report() {
        let base_date = date(2024, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, 0.20)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        // --- Good bucket: 1Y x 5Y, five strikes. ---
        let good_expiry_years = 1.0_f64;
        let tenor_years = 5.0_f64;
        let good_expiry_date = base_date.add_months((good_expiry_years * 12.0).round() as i32);
        let good_maturity_date = good_expiry_date.add_months((tenor_years * 12.0).round() as i32);
        let good_t_exp = to_basis_points(
            DayCount::Act365F
                .year_fraction(base_date, good_expiry_date, DayCountContext::default())
                .expect("t_exp"),
        ) as f64
            / 10_000.0;
        let good_t_ten = to_basis_points(
            DayCount::Act365F
                .year_fraction(
                    good_expiry_date,
                    good_maturity_date,
                    DayCountContext::default(),
                )
                .expect("t_ten"),
        ) as f64
            / 10_000.0;

        let mut p = params(base_date);
        p.vol_convention = SwaptionVolConvention::Lognormal;
        p.sabr_beta = 0.5;
        // Target only the good bucket so the cube can be built from it alone.
        p.target_expiries = vec![good_t_exp];
        p.target_tenors = vec![good_t_ten];
        p.vol_tolerance = Some(0.0020);

        let leg = SwaptionVolTarget::default_leg_conventions(&p).expect("leg conventions");
        let good_fwd = SwaptionVolTarget::calculate_forward_swap_rate_years(
            &p,
            good_expiry_years,
            tenor_years,
            &leg,
            &ctx,
        )
        .expect("forward");
        let sabr_true = SABRParameters {
            alpha: 0.020,
            beta: p.sabr_beta,
            nu: 0.30,
            rho: -0.20,
            shift: None,
        };
        let model = SABRModel::new(sabr_true);

        let mut quotes = Vec::new();
        for &k in &[
            good_fwd - 0.010,
            good_fwd - 0.005,
            good_fwd,
            good_fwd + 0.005,
            good_fwd + 0.010,
        ] {
            let vol_dec = model
                .implied_volatility(good_fwd, k, good_t_exp)
                .expect("true vol");
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("GOOD-1Yx5Y-{k}")),
                expiry: good_expiry_date,
                maturity: good_maturity_date,
                strike: k,
                vol: vol_dec * 100.0,
                quote_type: "implied_vol".to_string(),
                convention: SwaptionConventionId::new("USD-Annual"),
            }));
        }

        // --- Bad bucket: 2Y x 5Y, only TWO strikes (< 3 required for SABR). ---
        let bad_expiry_date = base_date.add_months(24);
        let bad_maturity_date = bad_expiry_date.add_months((tenor_years * 12.0).round() as i32);
        for &k in &[good_fwd, good_fwd + 0.005] {
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("BAD-2Yx5Y-{k}")),
                expiry: bad_expiry_date,
                maturity: bad_maturity_date,
                strike: k,
                vol: 20.0,
                quote_type: "implied_vol".to_string(),
                convention: SwaptionConventionId::new("USD-Annual"),
            }));
        }

        let config = CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default()
                .with_max_iterations(500),
            ..CalibrationConfig::default()
        };

        let (_cube, report) =
            SwaptionVolTarget::solve(&p, &quotes, &ctx, &config).expect("solve builds the cube");

        assert!(
            !report.success,
            "Item 3: a surface with a failed SABR bucket must NOT report success \
             (convergence_reason: {})",
            report.convergence_reason,
        );
        assert!(
            !report.validation_passed,
            "validation_passed must be false when a bucket failed"
        );
        assert_eq!(
            report.metadata.get("failed_buckets").map(String::as_str),
            Some("1"),
            "exactly one bucket should be recorded as failed"
        );
        assert!(
            report
                .validation_error
                .as_deref()
                .is_some_and(|e| e.contains("did not calibrate")),
            "validation_error should describe the failed bucket(s): {:?}",
            report.validation_error,
        );
    }

    #[test]
    fn shifted_lognormal_uses_explicit_shift_and_does_not_auto_shift() {
        let base_date = date(2024, Month::January, 2);
        // Negative forwards via an explicit forward curve (discount curve remains standard).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, 0.20)])
            .build()
            .expect("discount curve");
        let fwd_curve =
            finstack_core::market_data::term_structures::ForwardCurve::builder("USD-FWD", 0.25)
                .base_date(base_date)
                .day_count(DayCount::Act365F)
                .knots([(0.0, -0.01), (30.0, -0.01)])
                .build()
                .expect("forward curve");
        let ctx = MarketContext::new().insert(disc).insert(fwd_curve);

        let expiry_years: f64 = 1.0;
        let tenor_years: f64 = 5.0;
        let expiry_date = base_date.add_months((expiry_years * 12.0).round() as i32);
        let maturity_date = expiry_date.add_months((tenor_years * 12.0).round() as i32);
        let t_exp_raw = DayCount::Act365F
            .year_fraction(base_date, expiry_date, DayCountContext::default())
            .expect("t_exp");
        let t_ten_raw = DayCount::Act365F
            .year_fraction(expiry_date, maturity_date, DayCountContext::default())
            .expect("t_ten");
        let t_exp = to_basis_points(t_exp_raw) as f64 / 10_000.0;
        let t_ten = to_basis_points(t_ten_raw) as f64 / 10_000.0;

        let mut p = params(base_date);
        p.forward_id = Some("USD-FWD".to_string());
        p.vol_convention = SwaptionVolConvention::ShiftedLognormal { shift: 1e-6 };
        p.sabr_beta = 0.5;
        p.target_expiries = vec![t_exp];
        p.target_tenors = vec![t_ten];

        let leg = SwaptionVolTarget::default_leg_conventions(&p).expect("leg conventions");
        let fwd = SwaptionVolTarget::calculate_forward_swap_rate_years(
            &p,
            expiry_years,
            tenor_years,
            &leg,
            &ctx,
        )
        .expect("forward");
        assert!(fwd < 0.0, "expected negative forward for test; got {}", fwd);

        let strikes = vec![fwd - 0.002, fwd, fwd + 0.002, fwd + 0.005, fwd - 0.005];
        let mut quotes = Vec::new();
        for &k in &strikes {
            // Percent-quoted; exact values don't matter for this check (shift is insufficient).
            quotes.push(MarketQuote::Vol(VolQuote::SwaptionVol {
                id: QuoteId::new(format!("USD-SWPTN-VOL-SLN-1Yx5Y-{k}")),
                expiry: expiry_date,
                maturity: maturity_date,
                strike: k,
                vol: 20.0,
                quote_type: "implied_vol".to_string(),
                convention: SwaptionConventionId::new("USD-Annual"),
            }));
        }

        let config = CalibrationConfig::default();
        let err = SwaptionVolTarget::solve(&p, &quotes, &ctx, &config)
            .expect_err("insufficient explicit shift should not be auto-adjusted");
        assert!(
            err.to_string().contains("Swaption SABR params missing"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn forward_swap_rate_single_curve_matches_df_formula_with_pv01_schedule() {
        let base_date = date(2024, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, 0.20)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let p = params(base_date);
        let leg = SwaptionVolTarget::default_leg_conventions(&p).expect("leg conventions");

        let expiry_years: f64 = 1.0;
        let tenor_years: f64 = 5.0;
        let expiry_date = base_date.add_months((expiry_years * 12.0).round() as i32);
        let maturity_date = expiry_date.add_months((tenor_years * 12.0).round() as i32);

        let disc_ref = ctx
            .get_discount(p.discount_curve_id.as_ref())
            .expect("disc");
        let pv01 = SwaptionVolTarget::calculate_pv01_proper(
            expiry_date,
            maturity_date,
            &leg,
            disc_ref.as_ref(),
        )
        .expect("pv01");
        let t_start = disc_ref
            .day_count()
            .year_fraction(
                disc_ref.base_date(),
                expiry_date,
                DayCountContext::default(),
            )
            .expect("t_start");
        let t_end = disc_ref
            .day_count()
            .year_fraction(
                disc_ref.base_date(),
                maturity_date,
                DayCountContext::default(),
            )
            .expect("t_end");
        let expected = (disc_ref.df(t_start) - disc_ref.df(t_end)) / pv01;

        let actual = SwaptionVolTarget::calculate_forward_swap_rate_years(
            &p,
            expiry_years,
            tenor_years,
            &leg,
            &ctx,
        )
        .expect("forward");

        assert!(
            (actual - expected).abs() < 1e-12,
            "forward mismatch: actual={} expected={}",
            actual,
            expected
        );
    }

    #[test]
    fn sabr_param_bilinear_interpolation_interpolates_in_log_space_for_positive_params() {
        let mut grid: SABRParamsByExpiryTenor = BTreeMap::new();
        let p00 = SABRParameters {
            alpha: 0.01,
            beta: 0.5,
            nu: 0.20,
            rho: -0.20,
            shift: Some(0.0),
        };
        let p10 = SABRParameters {
            alpha: 0.02,
            beta: 0.5,
            nu: 0.40,
            rho: 0.00,
            shift: Some(0.0),
        };
        let p01 = SABRParameters {
            alpha: 0.02,
            beta: 0.5,
            nu: 0.40,
            rho: -0.40,
            shift: Some(0.0),
        };
        let p11 = SABRParameters {
            alpha: 0.04,
            beta: 0.5,
            nu: 0.80,
            rho: 0.20,
            shift: Some(0.0),
        };

        grid.insert((to_basis_points(1.0), to_basis_points(5.0)), p00);
        grid.insert((to_basis_points(2.0), to_basis_points(5.0)), p10);
        grid.insert((to_basis_points(1.0), to_basis_points(10.0)), p01);
        grid.insert((to_basis_points(2.0), to_basis_points(10.0)), p11);

        let mid = SwaptionVolTarget::interpolate_sabr_params_bilinear(
            1.5,
            7.5,
            &grid,
            SurfaceExtrapolationPolicy::Error,
            false,
        )
        .expect("interpolated params");

        assert!(mid.alpha.is_finite() && mid.alpha > 0.0);
        assert!(mid.nu.is_finite() && mid.nu > 0.0);
        assert!(mid.rho > -1.0 && mid.rho < 1.0);
        assert!((mid.beta - 0.5).abs() < 1e-12);
    }
}
