//! Shared hazard curve bumping logic.

use super::BumpRequest;
use crate::calibration::api::schema::{HazardCurveParams, StepParams};
use crate::calibration::config::CalibrationMethod;
use crate::calibration::step_runtime;
use crate::calibration::CalibrationConfig;
use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
use crate::market::conventions::ids::CdsDocClause;
use crate::market::quotes::cds::CdsQuote;
use crate::market::quotes::market_quote::MarketQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::ParInterp;
use finstack_quant_core::market_data::term_structures::Seniority;
use finstack_quant_core::types::CurveId;

struct HazardParRecalibration<'a> {
    hazard: &'a HazardCurve,
    context: &'a MarketContext,
    discount_id: &'a CurveId,
    recovery_rate: f64,
    doc_clause: CdsDocClause,
    cds_valuation_convention: Option<CdsValuationConvention>,
    quote_id_prefix: &'static str,
    spread_bump: Option<&'a BumpRequest>,
}

fn require_discount_id(discount_id: Option<&CurveId>) -> finstack_quant_core::Result<&CurveId> {
    discount_id.ok_or_else(|| {
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: "discount curve for hazard recalibration".to_string(),
        })
    })
}

fn snapped_cds_tenor_months(tenor_years: f64) -> u32 {
    let raw_months = (tenor_years * 12.0).round().max(1.0) as i32;
    const STD_MONTHS: [i32; 11] = [3, 6, 12, 24, 36, 60, 84, 120, 180, 240, 360];
    let mut snapped_months = raw_months;
    if let Some(best) = STD_MONTHS
        .iter()
        .copied()
        .min_by(|a, b| (raw_months - a).abs().cmp(&(raw_months - b).abs()))
    {
        if (raw_months - best).abs() <= 2 {
            snapped_months = best;
        } else {
            // Fallback: nearest quarter-year multiple.
            snapped_months = ((raw_months as f64 / 3.0).round() as i32).max(1) * 3;
        }
    }
    snapped_months as u32
}

fn bumped_spread(spread_bp: f64, tenor_years: f64, bump: Option<&BumpRequest>) -> f64 {
    let mut bumped = spread_bp;
    match bump {
        Some(BumpRequest::Parallel(bp)) => {
            bumped += bp;
        }
        Some(BumpRequest::Tenors(targets)) => {
            for (target_t, bp) in targets {
                // Match against the original curve tenor, not the snapped CDS
                // schedule tenor, so bucketed reports preserve irregular pillars.
                if (tenor_years - target_t).abs() < 0.1 {
                    bumped += bp;
                }
            }
        }
        None => {}
    }
    bumped
}

fn recalibrate_from_par_spreads(
    request: HazardParRecalibration<'_>,
) -> finstack_quant_core::Result<HazardCurve> {
    let par_points: Vec<(f64, f64)> = request.hazard.par_spread_points().collect();
    if par_points.is_empty() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::TooFewPoints,
        ));
    }

    let base_date = request.hazard.base_date();
    let currency = request.hazard.currency().unwrap_or(Currency::USD);
    let seniority = request.hazard.seniority.unwrap_or(Seniority::Senior);
    let issuer = request
        .hazard
        .issuer()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());

    let mut quotes = Vec::with_capacity(par_points.len());
    for (tenor_years, spread_bp) in par_points {
        quotes.push(CdsQuote::CdsParSpread {
            id: format!("{}-{}-{:.4}", request.quote_id_prefix, issuer, tenor_years).into(),
            entity: issuer.clone(),
            // Use tenor pillars so CDS schedule generation can snap to market-standard
            // IMM maturities. Using ad-hoc `Date` pillars can create invalid ranges.
            pillar: crate::market::quotes::ids::Pillar::Tenor(Tenor::new(
                snapped_cds_tenor_months(tenor_years),
                TenorUnit::Months,
            )),
            spread_bp: bumped_spread(spread_bp, tenor_years, request.spread_bump),
            recovery_rate: request.recovery_rate,
            convention: crate::market::conventions::ids::CdsConventionKey {
                currency,
                doc_clause: request.doc_clause,
            },
        });
    }

    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Cds).collect();
    let params = HazardCurveParams {
        curve_id: request.hazard.id().clone(),
        entity: issuer,
        seniority,
        currency,
        base_date,
        discount_curve_id: request.discount_id.clone(),
        recovery_rate: request.recovery_rate,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        // Preserve the base curve's survival interpolation style so the
        // bumped curve is built with the same convention.
        interpolation: request.hazard.survival_interp_style(),
        par_interp: ParInterp::Linear,
        doc_clause: Some(request.doc_clause.as_str().to_string()),
        cds_valuation_convention: request.cds_valuation_convention,
    };

    // Re-calibration uses the default CalibrationConfig — see the "Calibration
    // config — known limitation" note in this module's docs (`bumps/mod.rs`).
    let cfg = CalibrationConfig::default();
    let step = StepParams::Hazard(params.clone());
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, request.context, &cfg)?;
    let new_curve = ctx.get_hazard(params.curve_id.as_str())?.as_ref().clone();
    Ok(new_curve)
}

/// Bump hazard curve by shocking par spreads and re-calibrating.
///
/// This is the standard "Risk Re-calibration" approach. It extracts par
/// points from the current curve, applies shocks, and builds a new
/// credit-quote set to solve for a new hazard curve.
///
/// This function is strictly recalibration-only; callers that want a
/// model hazard shift should call [`bump_hazard_shift`] explicitly.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve from which par CDS spreads are implied
///   before applying the shock and re-bootstrap.
/// * `context` - Market context supplying the discount curve and calibration
///   dependencies required for CDS repricing.
/// * `bump` - Parallel or tenor-specific CDS spread shock in [`BumpRequest`]
///   basis point units.
/// * `discount_id` - Optional discount curve ID; `None` is rejected because
///   spread re-calibration must choose a discounting curve.
pub fn bump_hazard_spreads(
    hazard: &HazardCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    discount_id: Option<&CurveId>,
) -> finstack_quant_core::Result<HazardCurve> {
    bump_hazard_spreads_with_doc_clause(hazard, context, bump, discount_id, None)
}

/// Bump hazard curve by shocking par spreads and re-calibrating with an
/// explicit CDS documentation clause.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve from which par CDS spreads are implied.
/// * `context` - Market context supplying discounting and other dependencies.
/// * `bump` - Parallel or tenor-specific CDS spread shock in basis point units.
/// * `discount_id` - Optional discount curve ID; `None` is rejected.
/// * `doc_clause` - Optional ISDA CDS documentation clause; `None` uses the
///   canonical North American clause.
pub fn bump_hazard_spreads_with_doc_clause(
    hazard: &HazardCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
) -> finstack_quant_core::Result<HazardCurve> {
    bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard,
        context,
        bump,
        discount_id,
        doc_clause,
        None,
    )
}

/// Bump hazard curve by shocking par spreads and re-calibrating with an
/// explicit CDS documentation clause and valuation convention.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve from which par CDS spreads are implied.
/// * `context` - Market context supplying discounting and other dependencies.
/// * `bump` - Parallel or tenor-specific CDS spread shock in basis point units.
/// * `discount_id` - Optional discount curve ID; `None` is rejected.
/// * `doc_clause` - Optional ISDA CDS documentation clause; `None` selects
///   the canonical North American clause.
/// * `cds_valuation_convention` - Optional premium-leg/accrual convention;
///   `None` uses the calibration runtime default.
pub fn bump_hazard_spreads_with_doc_clause_and_valuation_convention(
    hazard: &HazardCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    let discount_id = require_discount_id(discount_id)?;
    recalibrate_from_par_spreads(HazardParRecalibration {
        hazard,
        context,
        discount_id,
        recovery_rate: hazard.recovery_rate(),
        doc_clause: doc_clause.unwrap_or(CdsDocClause::IsdaNa),
        cds_valuation_convention,
        quote_id_prefix: "BUMP",
        spread_bump: Some(bump),
    })
}

/// Bump hazard curve directly (model hazard shift), without recalibration.
///
/// # Arguments
///
/// * `hazard` - Existing hazard curve whose model hazard nodes are shifted
///   directly without recovering par spreads.
/// * `bump` - Parallel or tenor-specific hazard-rate shock in [`BumpRequest`]
///   basis point units.
pub fn bump_hazard_shift(
    hazard: &HazardCurve,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<HazardCurve> {
    bump_hazard_shift_fallback(hazard, bump)
}

/// Re-bootstrap a hazard curve with a *new* recovery assumption while holding
/// observed CDS par spreads constant.
///
/// This is the operation a Recovery01 metric needs in order to capture the
/// indirect effect of recovery changes on the survival probability term
/// structure. Because `h ≈ S / (1 - R)` to first order, raising `R` while
/// holding `S` constant requires the bootstrap to lift `h` (and vice versa).
/// A naive Recovery01 that bumps the instrument LGD without recalibrating
/// the hazard curve typically *understates* the true recovery sensitivity by
/// 2-5x, which matters materially for distressed credits.
///
/// # Errors
///
/// Returns the original error from `bump_hazard_spreads` if the curve has no
/// stored par-spread points (uncalibratable) or if the bootstrap fails.
///
/// # Arguments
///
/// * `hazard` — source curve carrying the observed CDS par spreads
/// * `new_recovery` — recovery rate to use during re-bootstrapping (clamped to `[0, 1)`)
/// * `context` — market context providing the discount curve referenced by `discount_id`
/// * `discount_id` — discount curve identifier used to value the protection leg during bootstrap
pub fn recalibrate_hazard_with_recovery(
    hazard: &HazardCurve,
    new_recovery: f64,
    context: &MarketContext,
    discount_id: Option<&CurveId>,
) -> finstack_quant_core::Result<HazardCurve> {
    recalibrate_hazard_with_recovery_and_doc_clause(
        hazard,
        new_recovery,
        context,
        discount_id,
        None,
    )
}

/// Re-bootstrap a hazard curve with a new recovery and explicit CDS
/// documentation clause.
pub fn recalibrate_hazard_with_recovery_and_doc_clause(
    hazard: &HazardCurve,
    new_recovery: f64,
    context: &MarketContext,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
) -> finstack_quant_core::Result<HazardCurve> {
    recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention(
        hazard,
        new_recovery,
        context,
        discount_id,
        doc_clause,
        None,
    )
}

/// Re-bootstrap a hazard curve with a new recovery, explicit CDS
/// documentation clause, and valuation convention.
pub fn recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention(
    hazard: &HazardCurve,
    new_recovery: f64,
    context: &MarketContext,
    discount_id: Option<&CurveId>,
    doc_clause: Option<CdsDocClause>,
    cds_valuation_convention: Option<CdsValuationConvention>,
) -> finstack_quant_core::Result<HazardCurve> {
    // Clamp recovery to a numerically safe range. R = 1 leaves zero LGD which
    // makes spreads non-identifying; we leave a small floor below 1.
    let new_recovery = new_recovery.clamp(0.0, 0.999_999);

    let discount_id = require_discount_id(discount_id)?;
    recalibrate_from_par_spreads(HazardParRecalibration {
        hazard,
        context,
        discount_id,
        recovery_rate: new_recovery,
        doc_clause: doc_clause.unwrap_or(CdsDocClause::IsdaNa),
        cds_valuation_convention,
        quote_id_prefix: "RECOVERY-RECALIB",
        spread_bump: None,
    })
}

/// Fallback: bump hazard rates directly (Model Sensitivity / Hazard Delta).
fn bump_hazard_shift_fallback(
    hazard: &HazardCurve,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<HazardCurve> {
    match bump {
        BumpRequest::Parallel(bp) => {
            // Convert bp to decimal
            let bump_decimal = bp * 1e-4;
            let temp_bumped = hazard.with_parallel_bump(bump_decimal)?;
            temp_bumped
                .to_builder_with_id(hazard.id().clone())
                .build()
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!("Failed to rebuild hazard curve after parallel bump: {e}"),
                    category: "bumps".to_string(),
                })
        }
        BumpRequest::Tenors(targets) => {
            // Sequential bumping for each target
            let mut current = hazard.clone();
            for (t, bp) in targets {
                current = with_key_rate_hazard_bump(&current, *t, *bp)?;
            }
            Ok(current)
        }
    }
}

/// Helper to apply a key-rate bump to a hazard curve at a specific tenor.
fn with_key_rate_hazard_bump(
    hazard: &HazardCurve,
    t_years: f64,
    bump_bp: f64,
) -> finstack_quant_core::Result<HazardCurve> {
    // Convert bump from bp to hazard rate units
    let bump_decimal = bump_bp * 1e-4;

    let knots: Vec<f64> = hazard.knot_points().map(|(t, _)| t).collect();
    let hazard_rates: Vec<f64> = hazard.knot_points().map(|(_, lambda)| lambda).collect();

    if knots.len() < 2 {
        return hazard.with_parallel_bump(bump_decimal);
    }

    // If the requested bucket is beyond the curve's supported maturity, treat as "no-op".
    // This avoids double-counting in bucketed CS01 when requesting standard buckets
    // beyond the last calibrated hazard knot.
    let last_knot = knots[knots.len() - 1];
    if t_years > last_knot + 1e-6 {
        return Ok(hazard.clone());
    }

    // If the request matches an existing knot, bump that knot directly.
    // Otherwise bump the segment that contains the target time.
    let eps = 1e-6;
    let mut target_idx = knots
        .iter()
        .position(|&k| (k - t_years).abs() <= eps)
        .unwrap_or(0);
    if target_idx == 0 {
        if t_years <= knots[0] {
            target_idx = 0;
        } else if t_years >= knots[knots.len() - 1] {
            target_idx = knots.len() - 1;
        } else {
            for i in 0..knots.len() - 1 {
                if t_years > knots[i] && t_years < knots[i + 1] {
                    target_idx = i;
                    break;
                }
            }
        }
    }

    let mut bumped_rates = hazard_rates;
    bumped_rates[target_idx] = (bumped_rates[target_idx] + bump_decimal).max(0.0);

    let bumped_points: Vec<(f64, f64)> = knots
        .iter()
        .zip(bumped_rates.iter())
        .map(|(&t, &lambda)| (t, lambda))
        .collect();

    let mut builder = HazardCurve::builder(hazard.id().clone())
        .base_date(hazard.base_date())
        .recovery_rate(hazard.recovery_rate())
        .day_count(hazard.day_count())
        .knots(bumped_points)
        .par_interp(hazard.par_interp())
        .par_spreads(hazard.par_spread_points());

    if let Some(issuer) = hazard.issuer() {
        builder = builder.issuer(issuer.to_string());
    }
    if let Some(seniority) = hazard.seniority {
        builder = builder.seniority(seniority);
    }
    if let Some(currency) = hazard.currency() {
        builder = builder.currency(currency);
    }

    builder
        .build()
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!("Failed to rebuild hazard curve after key-rate bump: {e}"),
            category: "bumps".to_string(),
        })
}
