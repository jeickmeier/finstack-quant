//! Pre-flight validation for calibration steps.
//!
//! This module performs validation before step execution:
//! - Quote availability and consistency
//! - Parameter validity
//! - Cross-curve dependency checks
//!
//! Pre-flight validation runs before any solver is invoked, catching configuration
//! errors early with descriptive messages.

use crate::calibration::api::schema::{CalibrationStep, StepParams};
use crate::calibration::config::CalibrationConfig;
use crate::calibration::targets::util::curve_day_count_from_quotes;
use crate::market::quotes::cds_tranche::CDSTrancheQuote;
use crate::market::quotes::market_quote::{ExtractQuotes, MarketQuote};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

/// Perform "pre-flight" validation of a calibration step before execution.
///
/// This checks for:
/// - Quote availability (at least one quote of the expected type)
/// - Parameter consistency (positive notionals, valid rates)
/// - Cross-curve dependencies (referenced curves exist in context)
/// - Quote-parameter alignment (entity, currency, recovery rate match)
///
/// # Arguments
/// * `step` - The calibration step to validate
/// * `quotes` - Available market quotes for this step
/// * `context` - Current market context with existing curves
/// * `global_config` - Global calibration configuration used for validation tolerances
///
/// # Errors
/// Returns an error if validation fails, with a descriptive message.
///
/// # Examples
/// ```ignore
/// use finstack_valuations::calibration::validation::preflight_step;
///
/// // Validate before execution
/// preflight_step(&step, &quotes, &context, &config)?;
/// // Safe to proceed with execution
/// ```
pub(crate) fn preflight_step(
    step: &CalibrationStep,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<()> {
    match &step.params {
        StepParams::Discount(_p) => validate_discount_step(quotes),
        StepParams::Forward(p) => validate_forward_step(p, quotes, context),
        StepParams::Hazard(p) => validate_hazard_step(p, quotes, context, global_config),
        StepParams::Inflation(p) => validate_inflation_step(p, quotes, context),
        StepParams::VolSurface(p) => validate_vol_surface_step(p, context),
        StepParams::SwaptionVol(p) => validate_swaption_vol_step(p, context),
        StepParams::BaseCorrelation(p) => {
            validate_base_correlation_step(p, quotes, context, global_config)
        }
        StepParams::StudentT(p) => validate_student_t_step(p, quotes, context),
        StepParams::HullWhite(_) | StepParams::CapFloorHullWhite(_) => Ok(()), // HW1F calibration validates quotes at execution time
        StepParams::SviSurface(p) => validate_svi_surface_step(p, context),
        StepParams::XccyBasis(p) => validate_xccy_basis_step(p, context),
        StepParams::Parametric(_) => Ok(()), // Parametric curves validate at execution time
    }
}

fn validate_student_t_step(
    p: &crate::calibration::api::schema::StudentTParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
) -> Result<()> {
    if !p.initial_df.is_finite() || p.initial_df <= 2.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Student-t initial_df must be finite and > 2.0; got {}",
            p.initial_df
        )));
    }
    if !p.df_bounds.0.is_finite()
        || !p.df_bounds.1.is_finite()
        || p.df_bounds.0 <= 2.0
        || p.df_bounds.0 >= p.df_bounds.1
    {
        return Err(finstack_core::Error::Validation(format!(
            "Student-t df_bounds must satisfy 2.0 < lo < hi; got ({}, {})",
            p.df_bounds.0, p.df_bounds.1
        )));
    }

    let tranche_quotes: Vec<CDSTrancheQuote> = quotes.extract_quotes();
    let tranche_quote = tranche_quotes
        .iter()
        .find(|quote| quote.id().as_str() == p.tranche_instrument_id)
        .ok_or_else(|| {
            finstack_core::Error::Input(finstack_core::InputError::NotFound {
                id: format!("CDS tranche quote '{}'", p.tranche_instrument_id),
            })
        })?;

    let index_id = match tranche_quote {
        CDSTrancheQuote::CDSTranche { index, .. } => index,
    };

    let _ = context.get_base_correlation(&p.base_correlation_curve_id)?;
    let _ = context.get_credit_index(index_id)?;

    if let Some(curve_id) = &p.discount_curve_id {
        let _ = context.get_discount(curve_id)?;
    } else {
        let mut discount_curves = context.curves_of_type("Discount");
        if discount_curves.next().is_none() {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::NotFound {
                    id: "discount curve".to_string(),
                },
            ));
        }
        if discount_curves.next().is_some() {
            return Err(finstack_core::Error::Validation(
                "Student-t calibration requires discount_curve_id when multiple discount curves are present"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_svi_surface_step(
    p: &crate::calibration::api::schema::SviSurfaceParams,
    context: &MarketContext,
) -> Result<()> {
    if p.target_expiries.is_empty() {
        return Err(finstack_core::Error::Validation(
            "SVI surface step requires non-empty target_expiries".to_string(),
        ));
    }
    if p.target_strikes.len() < 5 {
        return Err(finstack_core::Error::Validation(
            "SVI surface step requires at least five target_strikes".to_string(),
        ));
    }
    for (idx, expiry) in p.target_expiries.iter().enumerate() {
        if !expiry.is_finite() || *expiry <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "SVI surface step target_expiries[{idx}] must be finite and positive; got {expiry}"
            )));
        }
    }
    for (idx, strike) in p.target_strikes.iter().enumerate() {
        if !strike.is_finite() || *strike <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "SVI surface step target_strikes[{idx}] must be finite and positive; got {strike}"
            )));
        }
    }
    if let Some(spot_override) = p.spot_override {
        if !spot_override.is_finite() || spot_override <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "SVI surface step spot_override must be finite and positive; got {spot_override}"
            )));
        }
    }
    if let Some(discount_curve_id) = &p.discount_curve_id {
        let _ = context.get_discount(discount_curve_id)?;
    }
    Ok(())
}

/// Validate cross-currency basis curve calibration step.
fn validate_xccy_basis_step(
    p: &crate::calibration::api::schema::XccyBasisParams,
    context: &MarketContext,
) -> Result<()> {
    let _ = context.get_discount(&p.domestic_discount_id)?;
    if !p.fx_spot.is_finite() || p.fx_spot <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "XCCY basis step fx_spot must be finite and positive; got {}",
            p.fx_spot
        )));
    }
    Ok(())
}

/// Validate discount curve calibration step.
fn validate_discount_step(quotes: &[MarketQuote]) -> Result<()> {
    let rates_quotes: Vec<crate::market::quotes::rates::RateQuote> = quotes.extract_quotes();
    if rates_quotes.is_empty() {
        return Err(finstack_core::Error::Input(
            finstack_core::InputError::TooFewPoints,
        ));
    }
    let _curve_dc = curve_day_count_from_quotes(&rates_quotes)?;
    Ok(())
}

/// Validate forward curve calibration step.
fn validate_forward_step(
    p: &crate::calibration::api::schema::ForwardCurveParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
) -> Result<()> {
    // Ensure referenced discount curve exists (forward curve depends on it for pricing).
    let _ = context.get_discount(&p.discount_curve_id)?;

    let rates_quotes: Vec<crate::market::quotes::rates::RateQuote> = quotes.extract_quotes();
    if rates_quotes.is_empty() {
        return Err(finstack_core::Error::Input(
            finstack_core::InputError::TooFewPoints,
        ));
    }
    let _curve_dc = curve_day_count_from_quotes(&rates_quotes)?;
    Ok(())
}

/// Validate hazard curve calibration step.
fn validate_hazard_step(
    p: &crate::calibration::api::schema::HazardCurveParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<()> {
    let recovery_rate_abs_tolerance = global_config.validation.recovery_rate_abs_tolerance;
    // Ensure referenced discount curve exists.
    let _ = context.get_discount(&p.discount_curve_id)?;

    if !p.notional.is_finite() || p.notional <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Hazard calibration notional must be positive; got {}",
            p.notional
        )));
    }

    let cds_quotes: Vec<crate::market::quotes::cds::CdsQuote> = quotes.extract_quotes();
    if cds_quotes.is_empty() {
        return Err(finstack_core::Error::Input(
            finstack_core::InputError::TooFewPoints,
        ));
    }

    for q in &cds_quotes {
        q.validate_market_conventions()?;
        match q {
            crate::market::quotes::cds::CdsQuote::CdsParSpread {
                entity,
                recovery_rate,
                convention,
                spread_bp,
                ..
            } => {
                if *spread_bp <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "CDS spread_bp must be positive; got {}",
                        spread_bp
                    )));
                }
                if entity != &p.entity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step entity mismatch: params.entity='{}' but quote.entity='{}'",
                        p.entity, entity
                    )));
                }
                if convention.currency != p.currency {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step currency mismatch: params.currency='{}' but quote.convention.currency='{}'",
                        p.currency, convention.currency
                    )));
                }
                if (recovery_rate - p.recovery_rate).abs() > recovery_rate_abs_tolerance {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step recovery mismatch: params.recovery_rate={} but quote.recovery_rate={}",
                        p.recovery_rate, recovery_rate
                    )));
                }
            }
            crate::market::quotes::cds::CdsQuote::CdsUpfront {
                entity,
                recovery_rate,
                convention,
                running_spread_bp,
                ..
            } => {
                if *running_spread_bp <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "CDS running_spread_bp must be positive; got {}",
                        running_spread_bp
                    )));
                }
                if entity != &p.entity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step entity mismatch: params.entity='{}' but quote.entity='{}'",
                        p.entity, entity
                    )));
                }
                if convention.currency != p.currency {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step currency mismatch: params.currency='{}' but quote.convention.currency='{}'",
                        p.currency, convention.currency
                    )));
                }
                if (recovery_rate - p.recovery_rate).abs() > recovery_rate_abs_tolerance {
                    return Err(finstack_core::Error::Validation(format!(
                        "Hazard step recovery mismatch: params.recovery_rate={} but quote.recovery_rate={}",
                        p.recovery_rate, recovery_rate
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Validate inflation curve calibration step.
fn validate_inflation_step(
    p: &crate::calibration::api::schema::InflationCurveParams,
    _quotes: &[MarketQuote],
    context: &MarketContext,
) -> Result<()> {
    let _ = context.get_discount(&p.discount_curve_id)?;
    if !p.notional.is_finite() || p.notional <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Inflation calibration notional must be positive; got {}",
            p.notional
        )));
    }
    if !p.base_cpi.is_finite() || p.base_cpi <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Inflation base_cpi must be positive; got {}",
            p.base_cpi
        )));
    }

    // Validate observation lag string (used when no InflationIndex fixings are provided).
    let lag = p.observation_lag.trim();
    if !lag.is_empty() {
        let upper = lag.to_ascii_uppercase();
        let valid = upper == "NONE"
            || upper == "0"
            || upper == "0M"
            || upper == "0D"
            || upper
                .strip_suffix('M')
                .and_then(|n| n.trim().parse::<u8>().ok())
                .is_some()
            || upper
                .strip_suffix('D')
                .and_then(|n| n.trim().parse::<u16>().ok())
                .is_some();
        if !valid {
            return Err(finstack_core::Error::Validation(format!(
                "Invalid observation_lag '{}': expected like '3M' or '90D'",
                p.observation_lag
            )));
        }
    }

    // If an InflationIndex fixings series is provided, enforce consistency:
    // - currency match
    // - lag match
    // - base CPI match (including any seasonality applied by the index)
    if let Ok(index) = context.get_inflation_index(p.curve_id.as_str()) {
        if index.currency != p.currency {
            return Err(finstack_core::Error::Validation(format!(
                "Inflation step currency mismatch: params.currency='{}' but InflationIndex.currency='{}'",
                p.currency, index.currency
            )));
        }

        // Parse observation lag and require it to match the index lag.
        let parsed_lag = {
            let upper = p.observation_lag.trim().to_ascii_uppercase();
            if upper == "NONE" || upper == "0" || upper == "0M" || upper == "0D" {
                finstack_core::market_data::scalars::InflationLag::None
            } else if let Some(num) = upper.strip_suffix('M') {
                let months: u8 = num.trim().parse().map_err(|_| {
                    finstack_core::Error::Validation(format!(
                        "Invalid observation_lag '{}': expected like '3M'",
                        p.observation_lag
                    ))
                })?;
                finstack_core::market_data::scalars::InflationLag::Months(months)
            } else if let Some(num) = upper.strip_suffix('D') {
                let days: u16 = num.trim().parse().map_err(|_| {
                    finstack_core::Error::Validation(format!(
                        "Invalid observation_lag '{}': expected like '90D'",
                        p.observation_lag
                    ))
                })?;
                finstack_core::market_data::scalars::InflationLag::Days(days)
            } else {
                return Err(finstack_core::Error::Validation(format!(
                    "Invalid observation_lag '{}': expected like '3M' or '90D'",
                    p.observation_lag
                )));
            }
        };

        if parsed_lag != index.lag() {
            return Err(finstack_core::Error::Validation(format!(
                "Inflation step lag mismatch: params.observation_lag='{}' but InflationIndex.lag={:?}",
                p.observation_lag,
                index.lag()
            )));
        }

        let expected_base = index.value_on(p.base_date).map_err(|e| {
            finstack_core::Error::Validation(format!(
                "Failed to resolve base CPI from InflationIndex '{}': {}",
                p.curve_id.as_str(),
                e
            ))
        })?;
        let abs_tol = 1e-8_f64.max(1e-10_f64 * expected_base.abs());
        if (expected_base - p.base_cpi).abs() > abs_tol {
            return Err(finstack_core::Error::Validation(format!(
                "Inflation base_cpi mismatch: params.base_cpi={} but InflationIndex.value_on(base_date)={}",
                p.base_cpi, expected_base
            )));
        }
    }
    Ok(())
}

/// Validate volatility surface calibration step.
fn validate_vol_surface_step(
    p: &crate::calibration::api::schema::VolSurfaceParams,
    context: &MarketContext,
) -> Result<()> {
    let model = p.model.trim().to_ascii_lowercase();
    if model != "sabr" {
        return Err(finstack_core::Error::Validation(format!(
            "VolSurface model '{}' is not supported (currently supported: 'sabr')",
            p.model
        )));
    }
    let discount_id = p.discount_curve_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation("VolSurface step requires discount_curve_id".to_string())
    })?;
    let _ = context.get_discount(discount_id)?;
    Ok(())
}

/// Validate swaption volatility surface calibration step.
fn validate_swaption_vol_step(
    p: &crate::calibration::api::schema::SwaptionVolParams,
    context: &MarketContext,
) -> Result<()> {
    let _ = context.get_discount(&p.discount_curve_id)?;
    if let crate::calibration::api::schema::SwaptionVolConvention::ShiftedLognormal { shift } =
        p.vol_convention
    {
        if !shift.is_finite() || shift <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Shifted lognormal convention requires a finite, positive shift; got {}",
                shift
            )));
        }
    }
    Ok(())
}

/// Validate base correlation calibration step.
fn validate_base_correlation_step(
    p: &crate::calibration::api::schema::BaseCorrelationParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<()> {
    let recovery_rate_abs_tolerance = global_config.validation.recovery_rate_abs_tolerance;
    if !p.notional.is_finite() || p.notional <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "BaseCorrelation calibration notional must be positive; got {}",
            p.notional
        )));
    }

    // Base correlation calibration requires credit index data to be present in the context.
    let index_data = context.get_credit_index(&p.index_id)?;

    // Market-standard: ensure recovery/currency/series/index are consistent.
    let tranche_quotes: Vec<crate::market::quotes::cds_tranche::CDSTrancheQuote> =
        quotes.extract_quotes();
    if tranche_quotes.is_empty() {
        return Err(finstack_core::Error::Input(
            finstack_core::InputError::TooFewPoints,
        ));
    }
    let tranche_recovery: Option<f64> = None;

    for q in &tranche_quotes {
        match q {
            crate::market::quotes::cds_tranche::CDSTrancheQuote::CDSTranche {
                index,
                attachment,
                detachment,
                convention,
                ..
            } => {
                if index != &p.index_id {
                    continue;
                }

                if convention.currency != p.currency {
                    return Err(finstack_core::Error::Validation(format!(
                        "Base correlation tranche currency mismatch: params.currency='{}' but quote.convention.currency='{}'",
                        p.currency, convention.currency
                    )));
                }

                let normalize_pct = |value: f64| {
                    if (0.0..=1.0).contains(&value) {
                        value * 100.0
                    } else {
                        value
                    }
                };
                let attach_pct = normalize_pct(*attachment);
                let detach_pct = normalize_pct(*detachment);
                if !attach_pct.is_finite()
                    || !detach_pct.is_finite()
                    || attach_pct < 0.0
                    || !(0.0..=100.0).contains(&detach_pct)
                    || attach_pct >= detach_pct
                {
                    return Err(finstack_core::Error::Validation(format!(
                        "Invalid tranche attachment/detachment: attachment={}, detachment={} (expect 0 <= attachment < detachment <= 100, percent or fraction)",
                        attachment, detachment
                    )));
                }

                // Note: CDS tranche quotes don't have recovery_rate in the convention.
                // Recovery rate comes from the credit index data and is validated later.
            }
        }
    }

    if let Some(r) = tranche_recovery {
        if (r - index_data.recovery_rate).abs() > recovery_rate_abs_tolerance {
            return Err(finstack_core::Error::Validation(format!(
                "Tranche quote recovery_rate={} does not match credit index recovery_rate={}",
                r, index_data.recovery_rate
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::schema::{
        DiscountCurveParams, StepParams, StudentTParams, SviSurfaceParams,
    };
    use crate::calibration::config::CalibrationMethod;
    use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause, IndexId};
    use crate::market::quotes::cds_tranche::CDSTrancheQuote;
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, Tenor, TenorUnit};
    use finstack_core::market_data::term_structures::{
        BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
    };
    use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_core::types::CurveId;
    use std::sync::Arc;
    use time::Month;

    fn make_discount_step() -> CalibrationStep {
        CalibrationStep {
            id: "test".to_string(),
            quote_set: "rates".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: CurveId::from("USD-OIS"),
                currency: finstack_core::currency::Currency::USD,
                base_date: time::Date::from_calendar_date(2025, Month::January, 15)
                    .expect("valid test date"),
                method: CalibrationMethod::Bootstrap,
                interpolation: InterpStyle::Linear,
                extrapolation: ExtrapolationPolicy::FlatZero,
                pricing_discount_id: None,
                pricing_forward_id: None,
                conventions: Default::default(),
            }),
        }
    }

    fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
        DiscountCurve::builder(curve_id)
            .base_date(base_date)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .expect("flat discount curve should build")
    }

    #[test]
    fn preflight_rejects_empty_rates_quotes() {
        let step = make_discount_step();
        let config = CalibrationConfig::default();
        let ctx = MarketContext::new();
        let quotes: Vec<MarketQuote> = vec![];

        let result = preflight_step(&step, &quotes, &ctx, &config);
        assert!(result.is_err());
        let err = result.expect_err("expected error for empty quotes");
        assert!(
            matches!(err, finstack_core::Error::Input(_)),
            "Expected Input error, got: {:?}",
            err
        );
    }

    #[test]
    fn preflight_accepts_valid_discount_quotes() {
        let step = make_discount_step();
        let config = CalibrationConfig::default();
        let ctx = MarketContext::new();

        // Create a valid deposit quote
        let quote = MarketQuote::Rates(crate::market::quotes::rates::RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Tenor(Tenor::new(1, TenorUnit::Months)),
            rate: 0.05,
        });
        let quotes = vec![quote];

        let result = preflight_step(&step, &quotes, &ctx, &config);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
    }

    #[test]
    fn preflight_accepts_student_t_when_quote_and_market_are_present() {
        let base_date = Date::from_calendar_date(2025, Month::March, 20).expect("valid date");
        let step = CalibrationStep {
            id: "student-t".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::StudentT(StudentTParams {
                tranche_instrument_id: "TRANCHE-1".to_string(),
                base_correlation_curve_id: "CDX_CORR".to_string(),
                discount_curve_id: Some("USD-OIS".into()),
                initial_df: 5.0,
                df_bounds: (2.1, 50.0),
                correlation: 0.3,
            }),
        };
        let config = CalibrationConfig::default();
        let hazard = HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .recovery_rate(0.40)
            .knots([(1.0, 0.0010), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve");
        let base_corr = BaseCorrelationCurve::builder("CDX_CORR")
            .knots([(3.0, 0.25), (7.0, 0.35)])
            .build()
            .expect("base correlation");
        let credit_index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(base_corr.clone()))
            .build()
            .expect("credit index");
        let ctx = MarketContext::new()
            .insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"))
            .insert(hazard)
            .insert(base_corr)
            .insert_credit_index("CDX.NA.IG", credit_index);
        let quotes = vec![MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity: Date::from_calendar_date(2030, Month::March, 20).expect("valid maturity"),
            upfront_pct: -0.01,
            running_spread_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::Cr14,
            },
        })];

        let result = preflight_step(&step, &quotes, &ctx, &config);
        assert!(result.is_ok(), "unexpected error: {result:?}");
    }

    #[test]
    fn preflight_accepts_student_t_with_explicit_discount_curve_in_multi_curve_context() {
        let base_date = Date::from_calendar_date(2025, Month::March, 20).expect("valid date");
        let step = CalibrationStep {
            id: "student-t".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::StudentT(StudentTParams {
                tranche_instrument_id: "TRANCHE-1".to_string(),
                base_correlation_curve_id: "CDX_CORR".to_string(),
                discount_curve_id: Some("USD-OIS".into()),
                initial_df: 5.0,
                df_bounds: (2.1, 50.0),
                correlation: 0.3,
            }),
        };
        let config = CalibrationConfig::default();
        let hazard = HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .recovery_rate(0.40)
            .knots([(1.0, 0.0010), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve");
        let base_corr = BaseCorrelationCurve::builder("CDX_CORR")
            .knots([(3.0, 0.25), (7.0, 0.35)])
            .build()
            .expect("base correlation");
        let credit_index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(base_corr.clone()))
            .build()
            .expect("credit index");
        let ctx = MarketContext::new()
            .insert(build_flat_discount_curve(0.03, base_date, "USD-OIS"))
            .insert(build_flat_discount_curve(0.025, base_date, "USD-ALT"))
            .insert(hazard)
            .insert(base_corr)
            .insert_credit_index("CDX.NA.IG", credit_index);
        let quotes = vec![MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX.NA.IG".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity: Date::from_calendar_date(2030, Month::March, 20).expect("valid maturity"),
            upfront_pct: -0.01,
            running_spread_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::Cr14,
            },
        })];

        let result = preflight_step(&step, &quotes, &ctx, &config);
        assert!(result.is_ok(), "unexpected error: {result:?}");
    }

    #[test]
    fn preflight_rejects_svi_surface_without_discount_curve() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let step = CalibrationStep {
            id: "svi".to_string(),
            quote_set: "vols".to_string(),
            params: StepParams::SviSurface(SviSurfaceParams {
                surface_id: "SPX-SVI".to_string(),
                base_date,
                underlying_ticker: "SPX".to_string(),
                discount_curve_id: Some("USD-OIS".into()),
                target_expiries: vec![0.5, 1.0],
                target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
                spot_override: Some(100.0),
            }),
        };

        let err = preflight_step(
            &step,
            &[],
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect_err("SVI preflight should require the configured discount curve");
        assert!(
            err.to_string().contains("USD-OIS"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn preflight_rejects_svi_surface_non_positive_spot_override() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = SviSurfaceParams {
            surface_id: "SPX-SVI".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: None,
            target_expiries: vec![0.5, 1.0],
            target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(0.0),
        };

        let err = validate_svi_surface_step(&params, &MarketContext::new())
            .expect_err("non-positive spot override should fail preflight");
        assert!(err.to_string().to_lowercase().contains("spot"));
    }

    #[test]
    fn preflight_rejects_svi_surface_non_positive_target_strike() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let params = SviSurfaceParams {
            surface_id: "SPX-SVI".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: None,
            target_expiries: vec![0.5, 1.0],
            target_strikes: vec![0.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(100.0),
        };

        let err = validate_svi_surface_step(&params, &MarketContext::new())
            .expect_err("non-positive target strike should fail preflight");
        assert!(err.to_string().to_lowercase().contains("strike"));
    }
}
