//! Bootstrapper for base correlation curves built from tranche quotes.

use crate::api::schema::BaseCorrelationParams;
use crate::build::cds::resolve_cds_tenor_maturity;
use crate::build::cds_tranche::{build_cds_tranche_instrument, CdsTrancheBuildOverrides};
use crate::build::context::BuildCtx;
use crate::build::prepared::PreparedQuote;
use crate::config::CalibrationConfig;
use crate::prepared::{CalibrationQuote, CdsTrancheCalibrationQuote};
use crate::quotes::cds_tranche::CdsTrancheQuote;
use crate::quotes::market_quote::{ExtractQuotes, MarketQuote};
use crate::solver::bootstrap::SequentialBootstrapper;
use crate::solver::traits::BootstrapTarget;
use crate::targets::util::ContextScratch;
use crate::CalibrationReport;
use finstack_quant_cashflows::builder::specs::RollRule;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranchePricer;
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use std::sync::Arc;

/// Validate that all detachment points in params are valid.
fn validate_detachment_points(points: &[f64]) -> Result<()> {
    for d in points {
        if !d.is_finite() || *d <= 0.0 || *d > 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "detachment point {d} must be in (0, 100]"
            )));
        }
    }
    Ok(())
}

/// Validate that a quote's index matches the expected index.
fn validate_quote_index(quote_index: &str, expected_index: &str) -> Result<()> {
    if quote_index != expected_index {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Tranche quote index '{quote_index}' does not match params.index_id '{expected_index}'"
        )));
    }
    Ok(())
}

/// Validate that a detachment point is in the expected set (if non-empty).
fn validate_detachment_in_expected(detachment_pct: f64, expected: &[f64]) -> Result<()> {
    if expected.is_empty() {
        return Ok(());
    }
    let found = expected.iter().any(|d| (d - detachment_pct).abs() <= 1e-8);
    if !found {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Tranche detachment {detachment_pct} not in params.detachment_points {expected:?}"
        )));
    }
    Ok(())
}

/// Validate the contractual maturity against the requested whole-month tenor.
fn validate_maturity(
    maturity: Date,
    base_date: Date,
    maturity_years: f64,
    roll_rule: RollRule,
) -> Result<()> {
    let months = maturity_years * 12.0;
    if !months.is_finite()
        || months < 1.0
        || months > f64::from(i32::MAX)
        || (months - months.round()).abs() > 1e-8
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "maturity_years must be positive and represent a whole number of months, got {maturity_years}"
        )));
    }
    let months = months.round() as i32;
    let expected = match roll_rule {
        RollRule::CdsImm => {
            resolve_cds_tenor_maturity(base_date, &Tenor::new(months as u32, TenorUnit::Months)?)?
        }
        RollRule::None => base_date.add_months(months)?,
        other => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS tranche roll_rule must be cds_imm or none, got {other:?}"
            )));
        }
    };
    if maturity != expected {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Tranche maturity {maturity} does not match the {months}M contractual maturity {expected} for {roll_rule:?}"
        )));
    }
    Ok(())
}

/// Validate that all expected detachments were seen in the quotes.
fn validate_all_detachments_seen(expected: &[f64], seen: &[f64]) -> Result<()> {
    for exp in expected {
        if !seen.iter().any(|d| (d - exp).abs() <= 1e-8) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Missing tranche detachment {exp} from quotes"
            )));
        }
    }
    Ok(())
}

/// Normalize a value to percentage (0-100 scale).
fn normalize_pct(value: f64) -> f64 {
    if (0.0..=1.0).contains(&value) {
        value * 100.0
    } else {
        value
    }
}

// Main Bootstrapper

/// Bootstrapper that calibrates a [`BaseCorrelationCurve`] from tranche quotes.
pub(crate) struct BaseCorrelationTarget {
    /// Calibration inputs (curve IDs, schedule conventions, detachment points).
    pub params: BaseCorrelationParams,
    /// Reusable sequential bootstrap scratch context.
    scratch: ContextScratch,
    /// Reusable tranche pricer retaining lazy copula and quadrature caches.
    pricer: CdsTranchePricer,
}

impl BaseCorrelationTarget {
    /// Create a new base correlation bootstrapper.
    pub(crate) fn new(params: BaseCorrelationParams, base_context: MarketContext) -> Self {
        Self {
            params,
            scratch: ContextScratch::new(base_context),
            pricer: CdsTranchePricer::new(),
        }
    }

    fn validate_monotone_and_bounds(knots: &[(f64, f64)]) -> Result<()> {
        if knots.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        for (detachment, corr) in knots {
            if !detachment.is_finite() || *detachment <= 0.0 || *detachment > 100.0 {
                return Err(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::Invalid,
                ));
            }
            if !corr.is_finite() || *corr < 0.0 || *corr > 1.0 {
                return Err(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::Invalid,
                ));
            }
        }
        Ok(())
    }

    fn build_ctx(&self) -> BuildCtx {
        let mut curve_ids = HashMap::default();
        curve_ids.insert(
            "discount".to_string(),
            self.params.discount_curve_id.to_string(),
        );
        curve_ids.insert("credit".to_string(), self.params.index_id.clone());
        BuildCtx::new(self.params.base_date, self.params.notional, curve_ids)
    }

    fn build_overrides(&self) -> CdsTrancheBuildOverrides {
        CdsTrancheBuildOverrides {
            frequency: self.params.frequency,
            day_count: self.params.day_count,
            business_day_convention: self.params.business_day_convention,
            calendar_id: self.params.calendar_id.clone(),
            roll_rule: self.params.roll_rule,
        }
    }

    fn normalized_expected_detachments(&self) -> Vec<f64> {
        self.params
            .detachment_points
            .iter()
            .map(|d| normalize_pct(*d))
            .collect()
    }

    /// Build a single calibration quote from a tranche quote.
    fn build_calibration_quote(
        &self,
        quote: &CdsTrancheQuote,
        build_ctx: &BuildCtx,
        overrides: &CdsTrancheBuildOverrides,
        time_day_count: DayCount,
    ) -> Result<CalibrationQuote> {
        // Retain the contractual upfront so pricing discounts it to settlement
        // using the same cashflow rules as the instrument exposed to callers.
        let instrument =
            build_cds_tranche_instrument(quote, build_ctx, overrides).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to build tranche instrument: {e}"
                ))
            })?;

        let pillar_time = time_day_count.year_fraction(
            self.params.base_date,
            quote.maturity,
            DayCountContext::default(),
        )?;

        let prepared_quote = PreparedQuote::new(
            Arc::new(quote.clone()),
            Arc::<dyn finstack_quant_valuations::instruments::Instrument>::from(instrument),
            quote.maturity,
            pillar_time,
        );

        let detachment_pct = normalize_pct(quote.detachment);
        Ok(CalibrationQuote::CdsTranche(CdsTrancheCalibrationQuote {
            prepared: prepared_quote,
            detachment_pct,
        }))
    }

    fn prepare_quotes(&self, quotes: Vec<CdsTrancheQuote>) -> Result<Vec<CalibrationQuote>> {
        if !self.params.detachment_points.is_empty() {
            validate_detachment_points(&self.params.detachment_points)?;
        }

        let expected_detachments = self.normalized_expected_detachments();
        let time_day_count = self.params.day_count.unwrap_or(DayCount::Act365F);
        let build_ctx = self.build_ctx();
        let overrides = self.build_overrides();

        let mut prepared = Vec::with_capacity(quotes.len());
        let mut seen_detachments = Vec::new();

        for q in quotes {
            validate_quote_index(q.index.as_str(), &self.params.index_id)?;
            if q.series != self.params.series {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Tranche quote series {} does not match params.series {}",
                    q.series, self.params.series
                )));
            }
            ConventionRegistry::try_global()?.resolve_cds(&q.convention)?;

            let detachment_pct = normalize_pct(q.detachment);
            validate_detachment_in_expected(detachment_pct, &expected_detachments)?;
            validate_maturity(
                q.maturity,
                self.params.base_date,
                self.params.maturity_years,
                self.params.roll_rule,
            )?;

            let calib_quote =
                self.build_calibration_quote(&q, &build_ctx, &overrides, time_day_count)?;
            seen_detachments.push(detachment_pct);
            prepared.push(calib_quote);
        }

        // Validate all expected detachments were seen
        if !expected_detachments.is_empty() {
            validate_all_detachments_seen(&expected_detachments, &seen_detachments)?;
        }

        Ok(prepared)
    }

    /// Prepare quotes and run the sequential bootstrap for base correlation.
    pub(crate) fn solve(
        params: &BaseCorrelationParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        let tranche_quotes: Vec<CdsTrancheQuote> = quotes.extract_quotes();
        if tranche_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let target = BaseCorrelationTarget::new(params.clone(), context.clone());
        let prepared_quotes = target.prepare_quotes(tranche_quotes)?;

        // Base-correlation bootstrapping fits a correlation to each tranche's market
        // upfront. Unlike a discount-curve bootstrap (which reprices par instruments to
        // machine precision), tranche repricing carries its own loss-distribution /
        // numerical-integration error, and a market upfront need not correspond exactly
        // to any correlation in the model-admissible range. Holding it to the
        // discount-curve machine-precision tolerance is unrealistic; use a
        // tranche-appropriate validation tolerance. ~10 bp of discounted upfront
        // per unit of tranche notional — generous
        // relative to tranche-pricing precision, but still rejects a materially
        // miscalibrated knot.
        const BASE_CORRELATION_VALIDATION_TOLERANCE: f64 = 1e-3;
        let success_tolerance = BASE_CORRELATION_VALIDATION_TOLERANCE;

        let (curve, mut report) = SequentialBootstrapper::bootstrap(
            &target,
            &prepared_quotes,
            Vec::new(),
            global_config,
            success_tolerance,
        )?;

        report = report.with_metadata("residual_units", "discounted_upfront_fraction");
        report.update_solver_config(global_config.solver.clone());

        let mut new_context = context.clone().insert(curve.clone());
        if let Ok(idx) = new_context.get_credit_index(params.index_id.as_str()) {
            let mut updated = idx.as_ref().clone();
            updated.base_correlation_curve = Arc::new(curve);
            new_context = new_context.insert_credit_index(params.index_id.as_str(), updated)?;
        }

        Ok((new_context, report))
    }
}

impl BootstrapTarget for BaseCorrelationTarget {
    type Quote = CalibrationQuote;
    type Curve = BaseCorrelationCurve;

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        quote.quote_id().to_string()
    }

    /// Base-correlation calibration opts into best-effort knots: the objective
    /// (tranche upfront vs. correlation) is monotone, and a market upfront may
    /// sit just outside the model-reachable correlation range, in which case the
    /// closest admissible correlation is the correct calibrated value.
    fn allow_approximate_knots(&self) -> bool {
        true
    }

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        match quote {
            CalibrationQuote::CdsTranche(pq) => Ok(pq.detachment_pct),
            _ => Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            )),
        }
    }

    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        let mut sorted_knots = knots.to_vec();
        if sorted_knots.iter().any(|(d, _)| !d.is_finite()) {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        if sorted_knots.len() == 1 {
            let (k, v) = sorted_knots[0];
            let bump = 10.0;
            let k2 = if k + bump <= 100.0 {
                k + bump
            } else if k >= bump {
                k - bump
            } else {
                (k + 1.0).min(100.0)
            };
            if (k2 - k).abs() > 1e-12 {
                sorted_knots.push((k2, v));
            }
        }

        sorted_knots.sort_by(|a, b| a.0.total_cmp(&b.0));
        sorted_knots.dedup_by(|a, b| (a.0 - b.0).abs() <= 1e-12);

        if sorted_knots.len() < 2 {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        Self::validate_monotone_and_bounds(&sorted_knots)?;

        BaseCorrelationCurve::builder(format!("{}_CORR", self.params.index_id))
            .knots(sorted_knots)
            .build()
    }

    fn build_curve_final(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        let curve = self.build_curve(knots)?;
        let validation = curve.validate_shape();
        if !validation.is_monotonic {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Base correlation curve violates the calibration monotonicity policy: {:?}",
                validation.violations
            )));
        }
        Ok(curve)
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        let pq = match quote {
            CalibrationQuote::CdsTranche(pq) => &pq.prepared,
            _ => {
                return Err(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::Invalid,
                ))
            }
        };

        let tranche = pq
            .instrument
            .as_any()
            .downcast_ref::<finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche>()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Base correlation calibration requires a CdsTranche instrument".to_string(),
                )
            })?;
        let index_id = self.params.index_id.as_str();
        self.scratch.with_curve_then(
            curve,
            |ctx| {
                if let Ok(idx) = ctx.get_credit_index(index_id) {
                    let mut updated = idx.as_ref().clone();
                    updated.base_correlation_curve = Arc::new(curve.clone());
                    ctx.insert_credit_index_mut(index_id, updated)?;
                }
                Ok(())
            },
            |ctx| {
                // Residuals are discounted upfront fractions of tranche notional.
                // Portfolio-notional scaling would understate a thin tranche's
                // mispricing by the reciprocal of its attachment/detachment width.
                let npv = self
                    .pricer
                    .price_tranche(tranche, ctx, self.params.base_date)?;
                Ok(npv.amount() / tranche.notional.amount())
            },
        )
    }

    fn initial_guess(&self, _quote: &Self::Quote, previous_knots: &[(f64, f64)]) -> Result<f64> {
        // Return the previous knot's correlation as both the starting point and the
        // lower bound for the monotonicity constraint (β(K₂) ≥ β(K₁) for K₂ > K₁).
        // For the first quote, return 0.0 (no constraint from previous knots).
        let prev = previous_knots.last().map(|(_, v)| *v).unwrap_or(0.0);
        Ok(prev.clamp(0.0, 0.999))
    }

    fn scan_points(&self, _quote: &Self::Quote, initial_guess: f64) -> Result<Vec<f64>> {
        // Base correlation is a bounded parameter in [0, 1) with a monotonicity
        // constraint: β(K₂) ≥ β(K₁) for K₂ > K₁. The `initial_guess` provides the
        // previous knot's correlation, which is the lower bound for valid solutions.
        //
        // Generate a dense bounded scan grid in [low, hi] so the solver only
        // explores the feasible region, avoiding validation errors from
        // non-monotonic trial curves.
        let mut pts = Vec::with_capacity(64);
        let hi = 0.999_f64;

        // Lower bound from monotonicity: new correlation >= previous correlation.
        let low = initial_guess.clamp(0.0, hi);

        pts.push(low);
        pts.push(hi);

        // Linear grid across the feasible region [low, hi].
        const N: usize = 6;
        for i in 0..=N {
            let x = low + (i as f64) / (N as f64) * (hi - low);
            pts.push(x);
        }

        // Extra refinement around a central estimate.
        // Start searching from a point in the interior of the feasible region.
        let center = if low < 0.5 {
            (low + 0.30).min(0.85)
        } else {
            (low + hi) / 2.0
        };
        for dx in [1e-4, 5e-4, 1e-3, 5e-3, 1e-2, 0.05] {
            for s in [-1.0, 1.0] {
                let x = (center + s * dx).clamp(low, hi);
                pts.push(x);
            }
        }

        pts.retain(|x| x.is_finite());
        pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        Ok(pts)
    }

    fn supports_nearest_first_bracketing(&self) -> bool {
        true
    }

    fn validate_knot(&self, _time: f64, value: f64) -> Result<()> {
        if !value.is_finite() || !(0.0..=0.999).contains(&value) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Base correlation must be in [0, 0.999], got {}",
                value
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date};
    use time::Month;

    fn test_params() -> BaseCorrelationParams {
        BaseCorrelationParams {
            index_id: "CDX.NA.IG".to_string(),
            series: 42,
            maturity_years: 5.0,
            base_date: Date::from_calendar_date(2025, Month::January, 2).expect("base date"),
            discount_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            notional: 1_000_000.0,
            frequency: None,
            day_count: Some(DayCount::Act365F),
            business_day_convention: Some(BusinessDayConvention::Following),
            calendar_id: None,
            detachment_points: vec![3.0, 7.0],
            roll_rule: RollRule::None,
        }
    }

    fn test_target() -> BaseCorrelationTarget {
        BaseCorrelationTarget::new(test_params(), MarketContext::new())
    }

    #[test]
    fn detachment_and_maturity_validators_cover_boundary_cases() {
        assert_eq!(normalize_pct(0.03), 3.0);
        assert_eq!(normalize_pct(7.0), 7.0);
        assert!(validate_detachment_points(&[3.0, 7.0, 10.0]).is_ok());
        assert!(validate_detachment_points(&[0.0]).is_err());
        assert!(validate_detachment_points(&[101.0]).is_err());

        let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base date");
        let expected = base_date.add_months(60).expect("valid date shift");
        assert!(validate_maturity(expected, base_date, 5.0, RollRule::None).is_ok());
        assert!(validate_maturity(
            expected + time::Duration::days(1),
            base_date,
            5.0,
            RollRule::None
        )
        .is_err());
        for years in [0.0, -1.0, f64::NAN, f64::INFINITY, 5.1] {
            assert!(validate_maturity(expected, base_date, years, RollRule::None).is_err());
        }
        assert!(validate_maturity(
            base_date.add_months(6).expect("six months"),
            base_date,
            0.5,
            RollRule::None
        )
        .is_ok());
    }

    #[test]
    fn maturity_follows_semiannual_rolls_on_both_sides_of_the_roll_date() {
        for (month, day, maturity_year, maturity_month) in [
            (Month::March, 19, 2029, Month::December),
            (Month::March, 20, 2030, Month::June),
            (Month::March, 21, 2030, Month::June),
            (Month::September, 19, 2030, Month::June),
            (Month::September, 20, 2030, Month::December),
            (Month::September, 21, 2030, Month::December),
        ] {
            let trade = Date::from_calendar_date(2025, month, day).expect("trade date");
            let maturity = Date::from_calendar_date(maturity_year, maturity_month, 20)
                .expect("standard maturity");
            assert!(validate_maturity(maturity, trade, 5.0, RollRule::CdsImm).is_ok());
            assert!(validate_maturity(
                trade.add_months(60).expect("anniversary"),
                trade,
                5.0,
                RollRule::CdsImm
            )
            .is_err());
        }
    }

    #[test]
    fn validate_monotone_and_bounds_rejects_invalid_knots() {
        assert!(
            BaseCorrelationTarget::validate_monotone_and_bounds(&[(3.0, 0.25), (7.0, 0.45)])
                .is_ok()
        );
        assert!(
            BaseCorrelationTarget::validate_monotone_and_bounds(&[(7.0, 0.45), (3.0, 0.25)])
                .is_err()
        );
        assert!(
            BaseCorrelationTarget::validate_monotone_and_bounds(&[(0.0, 0.25), (7.0, 0.45)])
                .is_err()
        );
        assert!(
            BaseCorrelationTarget::validate_monotone_and_bounds(&[(3.0, 1.2), (7.0, 0.45)])
                .is_err()
        );
    }

    #[test]
    fn build_curve_sorts_dedups_and_synthesizes_single_knot() {
        let target = test_target();

        let single = target
            .build_curve(&[(7.0, 0.25)])
            .expect("single knot should be expanded");
        assert_eq!(single.detachment_points(), &[7.0, 17.0]);
        assert_eq!(single.correlations(), &[0.25, 0.25]);

        let unsorted = target
            .build_curve(&[(10.0, 0.4), (3.0, 0.2), (10.0 + 1e-13, 0.4)])
            .expect("unsorted near-duplicate knots should normalize");
        assert_eq!(unsorted.detachment_points(), &[3.0, 10.0]);
        assert_eq!(unsorted.correlations(), &[0.2, 0.4]);
    }

    #[test]
    fn build_curve_final_rejects_non_arbitrage_free_shape() {
        let err = test_target()
            .build_curve_final(&[(3.0, 0.7), (7.0, 0.4)])
            .expect_err("decreasing base correlation should fail final validation");
        assert!(err.to_string().contains("monotonicity policy"));
    }

    #[test]
    fn helper_accessors_and_validate_knot_respect_bounds() {
        let target = test_target();

        assert_eq!(target.normalized_expected_detachments(), vec![3.0, 7.0]);
        let ctx = target.build_ctx();
        assert_eq!(ctx.as_of(), target.params.base_date);
        assert!((ctx.notional() - 1_000_000.0).abs() < 1e-12);

        let overrides = target.build_overrides();
        assert_eq!(overrides.day_count, target.params.day_count);
        assert_eq!(
            overrides.business_day_convention,
            target.params.business_day_convention
        );
        assert_eq!(overrides.roll_rule, target.params.roll_rule);

        assert!(target.validate_knot(7.0, 0.5).is_ok());
        assert!(target.validate_knot(7.0, 1.1).is_err());
    }
}
