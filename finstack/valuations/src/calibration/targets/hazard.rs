use crate::calibration::api::schema::HazardCurveParams;
use crate::calibration::config::{
    CalibrationConfig, CalibrationMethod, HazardCurveSolveConfig, ResidualWeightingScheme,
};
use crate::calibration::constants::{TOLERANCE_DUP_KNOTS, WEIGHT_MIN_FLOOR};
use crate::calibration::prepared::CalibrationQuote;
use crate::calibration::solver::bootstrap::SequentialBootstrapper;
use crate::calibration::solver::global::GlobalFitOptimizer;
use crate::calibration::solver::traits::{BootstrapTarget, GlobalSolveTarget};
use crate::calibration::CalibrationReport;
use crate::instruments::credit_derivatives::cds::CdsConventionResolved;
use crate::market::build::context::BuildCtx;
use crate::market::quotes::market_quote::{ExtractQuotes, MarketQuote};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::HashMap;
use finstack_core::Result;
use std::cell::RefCell;

/// Bootstrapper for hazard curves from CDS quotes.
///
/// Implements sequential bootstrapping of hazard curves (survival probabilities)
/// using market CDS quotes with varying maturities. It derives standard ISDA
/// conventions (e.g., North American, European, Asian) from the currency and
/// prices synthetic CDS instruments to solve for the hazard rate at each knot.
///
/// # Invariants
/// - Hazard rates must be non-negative (to ensure non-increasing survival).
/// - Knot times must be strictly increasing.
///
/// # See Also
/// - [`crate::instruments::cds`] for details on the underlying instruments.
pub(crate) struct HazardCurveTarget {
    /// Parameters defining the hazard curve structure and IDs.
    pub(crate) params: HazardCurveParams,
    /// CDS market conventions resolved from (currency, doc_clause).
    pub(crate) cds_conventions: &'static CdsConventionResolved,
    /// Market context providing discount curves for PV calculations.
    pub(crate) base_context: MarketContext,
    /// Global calibration settings (used for solver controls and weights).
    pub(crate) config: CalibrationConfig,
    /// Optional reusable context for sequential solvers to reduce memory pressure.
    reuse_context: Option<RefCell<MarketContext>>,
}

/// Build the log-spaced hazard bracketing scan grid for a spread-implied
/// `initial_guess`, bounded by `cfg.hazard_hard_min` / `cfg.hazard_hard_max`.
///
/// # Item 6: distressed / jump-to-default upside resolution
///
/// The grid has two parts:
///
///  * A *primary window* `[log_center − 4, log_center + 2]` decades around the
///    spread-implied guess. The asymmetric `+2 / −4` shape concentrates resolution in
///    the typical low-hazard regime.
///
///  * A *distressed/JTD upside augmentation*: a coarser log-spaced set spanning
///    `(log_center + 2, log10(hazard_hard_max)]`. The previous grid had only the lone
///    `hazard_hard_max` anchor above the primary window, so a distressed name — whose
///    true hazard can sit far above a moderate `spread/(1−R)` guess when the CDS curve
///    is steep or inverted — could at best be bracketed against an extremely wide
///    `[10^(log_center+2), hazard_hard_max]` interval with no interior resolution.
///
/// The augmentation is **purely additive**: every primary-window point is retained
/// unchanged. The bracketing solver selects the sign-change bracket whose midpoint is
/// closest to the initial guess, so a normal (non-distressed) name — whose root lies
/// inside the primary window — gets the *same* bracket as before, keeping bit-stable
/// Bloomberg golden fixtures unaffected. Only a distressed name, whose root is above
/// the primary window, benefits from the added upper-region resolution.
///
/// Genuinely unbracketable cases (true hazard above `hazard_hard_max`) are a hard
/// error downstream: `resolve_no_bracket` fails loud when no sign-change bracket is
/// found rather than pinning the knot to the cap (item 10).
fn hazard_scan_grid(cfg: &HazardCurveSolveConfig, initial_guess: f64) -> Vec<f64> {
    let hazard_min = cfg.hazard_hard_min;
    let max_h = cfg.hazard_hard_max;
    let min_positive = 1e-10_f64;

    let center = if initial_guess.is_finite() {
        initial_guess.clamp(hazard_min, max_h)
    } else {
        0.01_f64
    };

    let mut pts = Vec::with_capacity(80);
    pts.push(hazard_min);
    pts.push(center);
    pts.push(max_h);

    let center_pos = center.max(min_positive);
    let log_center = center_pos.log10();
    let low_exp = (log_center - 4.0).max(min_positive.log10());
    let max_h_exp = max_h.max(min_positive).log10();
    let high_exp = (log_center + 2.0).min(max_h_exp);

    // Primary window: dense log-spaced grid over `[low_exp, high_exp]`.
    const N: usize = 48;
    if (high_exp - low_exp).abs() > 1e-12 {
        for i in 0..N {
            let t = i as f64 / (N - 1) as f64;
            let exp = low_exp + t * (high_exp - low_exp);
            let v = 10f64.powf(exp);
            if v.is_finite() && v >= hazard_min && v <= max_h {
                pts.push(v);
            }
        }
    } else {
        pts.push(center_pos);
    }

    // Distressed/JTD upside augmentation: coarse log-spaced points spanning the region
    // above the primary window, `(high_exp, max_h_exp]`. Additive only — does not
    // perturb the primary-window points used by normal-name calibrations.
    const N_UPPER: usize = 16;
    if max_h_exp - high_exp > 1e-9 {
        for i in 1..=N_UPPER {
            let t = i as f64 / N_UPPER as f64;
            let exp = high_exp + t * (max_h_exp - high_exp);
            let v = 10f64.powf(exp);
            if v.is_finite() && v >= hazard_min && v <= max_h {
                pts.push(v);
            }
        }
    }

    pts.sort_by(|a, b| a.total_cmp(b));
    pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    pts
}

impl HazardCurveTarget {
    fn validate_hazard_bounds(
        params: &HazardCurveParams,
        config: &CalibrationConfig,
    ) -> Result<()> {
        let hazard_min = config.hazard_curve.hazard_hard_min;
        let hazard_max = config.hazard_curve.hazard_hard_max;
        if !hazard_min.is_finite()
            || !hazard_max.is_finite()
            || hazard_min < 0.0
            || hazard_max <= 0.0
            || hazard_min >= hazard_max
        {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Invalid hazard bounds for {}: hazard_hard_min={} hazard_hard_max={} (expected finite values with 0 <= min < max)",
                    params.curve_id, hazard_min, hazard_max
                ),
                category: "bootstrapping".to_string(),
            });
        }
        Ok(())
    }

    /// Creates a new hazard curve bootstrapper.
    ///
    /// # Arguments
    ///
    /// * `params` - Parameters defining the hazard curve structure
    /// * `base_context` - Market context containing discount curves
    /// * `config` - Global calibration settings (solver controls and weights)
    ///
    /// # Returns
    ///
    /// A new `HazardCurveTarget` instance ready for calibration.
    ///
    /// # Note
    ///
    /// CDS conventions are automatically derived from the currency:
    /// - USD/CAD: ISDA North American
    /// - EUR/GBP/CHF: ISDA European
    /// - JPY/HKD/SGD/AUD/NZD: ISDA Asian
    pub(crate) fn new(
        params: HazardCurveParams,
        base_context: MarketContext,
        config: CalibrationConfig,
    ) -> Result<Self> {
        Self::validate_hazard_bounds(&params, &config)?;
        let cds_conventions =
            crate::instruments::credit_derivatives::cds::resolve_market_conventions(
                params.currency,
                params.doc_clause.as_deref(),
            )?;

        let reuse_context = if matches!(config.calibration_method, CalibrationMethod::Bootstrap)
            || !config.use_parallel
        {
            Some(RefCell::new(base_context.clone()))
        } else {
            None
        };

        Ok(Self {
            params,
            cds_conventions,
            base_context,
            config,
            reuse_context,
        })
    }

    /// Execute the full calibration for a hazard curve step.
    pub(crate) fn solve(
        params: &HazardCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        let cds_quotes: Vec<crate::market::quotes::cds::CdsQuote> = quotes.extract_quotes();

        if cds_quotes.is_empty() {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
            ));
        }

        let mut config = global_config.clone();
        if cds_quotes.iter().any(|quote| match quote {
            crate::market::quotes::cds::CdsQuote::CdsParSpread { spread_bp, .. }
            | crate::market::quotes::cds::CdsQuote::CdsUpfront {
                running_spread_bp: spread_bp,
                ..
            } => *spread_bp >= 1_000.0,
        }) {
            config.hazard_curve.hazard_hard_max = config.hazard_curve.hazard_hard_max.max(100.0);
            config.hazard_curve.validation_tolerance =
                config.hazard_curve.validation_tolerance.max(1e-6);
            config.validation.max_hazard_rate = config.validation.max_hazard_rate.max(2.0);
        }
        config.calibration_method = params.method.clone();

        let target = HazardCurveTarget::new(params.clone(), context.clone(), config.clone())?;

        let mut prepared_quotes: Vec<CalibrationQuote> = Vec::with_capacity(cds_quotes.len());
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), params.discount_curve_id.to_string());
        curve_ids.insert("credit".to_string(), params.curve_id.to_string());
        let build_ctx = BuildCtx::new(params.base_date, params.notional, curve_ids)
            .with_cds_valuation_convention(params.cds_valuation_convention);
        let t_day_count = target.cds_conventions.day_count;

        for (i, q) in cds_quotes.into_iter().enumerate() {
            let prepared = crate::market::build::prepared::prepare_cds_quote(
                q.clone(),
                &build_ctx,
                t_day_count,
                params.base_date,
            )
            .map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Failed to build credit instrument {}: {}",
                    i, e
                ))
            })?;

            prepared_quotes.push(CalibrationQuote::Cds(prepared));
        }

        // Target-specific validation tolerance for hazard curves.
        let success_tolerance = Some(config.hazard_curve.validation_tolerance);

        let (curve, report) = match params.method {
            CalibrationMethod::Bootstrap => SequentialBootstrapper::bootstrap(
                &target,
                &prepared_quotes,
                Vec::new(),
                &config,
                success_tolerance,
                None,
            )?,
            CalibrationMethod::GlobalSolve { .. } => {
                GlobalFitOptimizer::optimize(&target, &prepared_quotes, &config, success_tolerance)?
            }
        };

        let report = report
            .with_model_version(finstack_core::versions::ISDA_STANDARD_MODEL)
            .with_metadata("calibration_type", "hazard_curve")
            .with_metadata("curve_id", params.curve_id.as_str())
            .with_metadata("entity", &params.entity);
        let mut report = report;
        report.update_solver_config(config.solver.clone());

        // Attach par spread points from the calibration quotes to the bootstrapped
        // curve so that downstream quote-bump CS01 metrics can re-bootstrap from
        // par spreads. The pillar_time values come from the prepared quotes and
        // reflect the actual IMM-rolled maturity dates used during calibration.
        //
        // Quote-type handling:
        //   * `CdsParSpread` — `spread_bp` is already the par spread by definition.
        //   * `CdsUpfront`  — `running_spread_bp` is the *fixed coupon* (e.g.
        //     100bp for CDX IG), NOT the implied par spread. Storing the
        //     coupon would corrupt downstream par-spread CS01 because a
        //     1bp shock to a coupon is not the same as a 1bp shock to the
        //     par spread. We therefore reprice each upfront pillar against
        //     the freshly bootstrapped hazard curve and back out the implied
        //     par spread via `CDSPricer::par_spread()`. If the implied solve
        //     fails for any reason (e.g. zero risky annuity), we drop that
        //     pillar from the sidecar rather than persisting a misleading
        //     value.
        let par_points: Vec<(f64, f64)> = {
            // The hazard curve we just bootstrapped lives under `params.curve_id`;
            // the discount curve was already provided in `context`. Insert by
            // id-overwrite so par_spread sees the new hazard knots.
            let bumped_ctx = context.clone().insert(curve.clone());
            let mut points: Vec<(f64, f64)> = Vec::with_capacity(prepared_quotes.len());
            for q in prepared_quotes.iter() {
                let pq = match q {
                    CalibrationQuote::Cds(pq) => pq,
                    _ => continue,
                };
                let spread_bp = match pq.quote.as_ref() {
                    crate::market::quotes::cds::CdsQuote::CdsParSpread { spread_bp, .. } => {
                        *spread_bp
                    }
                    crate::market::quotes::cds::CdsQuote::CdsUpfront { .. } => {
                        // W7: dropped pillars must surface in observability,
                        // not silently. Downstream CS01 quote-bump will refuse
                        // to re-bootstrap from a missing par pillar, and the
                        // operator needs to know the sidecar is incomplete.
                        let Some(cds) = pq.instrument.as_any().downcast_ref::<
                            crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
                        >() else {
                            tracing::warn!(
                                pillar_time = pq.pillar_time,
                                curve = %params.curve_id.as_str(),
                                "CdsUpfront pillar dropped from par_points sidecar: \
                                 instrument not downcastable to CreditDefaultSwap"
                            );
                            continue;
                        };
                        let disc = match bumped_ctx.get_discount(&cds.premium.discount_curve_id) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::warn!(
                                    pillar_time = pq.pillar_time,
                                    curve = %params.curve_id.as_str(),
                                    discount_curve = %cds.premium.discount_curve_id.as_ref(),
                                    error = %e,
                                    "CdsUpfront pillar dropped: discount curve lookup failed"
                                );
                                continue;
                            }
                        };
                        let surv = match bumped_ctx.get_hazard(&cds.protection.credit_curve_id) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::warn!(
                                    pillar_time = pq.pillar_time,
                                    curve = %params.curve_id.as_str(),
                                    credit_curve = %cds.protection.credit_curve_id.as_ref(),
                                    error = %e,
                                    "CdsUpfront pillar dropped: hazard curve lookup failed"
                                );
                                continue;
                            }
                        };
                        let pricer = crate::instruments::credit_derivatives::cds::pricer::CDSPricer::with_config(
                            crate::instruments::credit_derivatives::cds::pricer::CDSPricerConfig::from_cds(cds),
                        );
                        match pricer.par_spread(cds, disc.as_ref(), surv.as_ref(), params.base_date)
                        {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(
                                    pillar_time = pq.pillar_time,
                                    curve = %params.curve_id.as_str(),
                                    error = %e,
                                    "CdsUpfront pillar dropped: par_spread back-out failed \
                                     (likely zero risky annuity); downstream par-spread CS01 \
                                     for this pillar will be unavailable"
                                );
                                continue;
                            }
                        }
                    }
                };
                points.push((pq.pillar_time, spread_bp));
            }
            points
        };

        let curve = if !par_points.is_empty() {
            let id = curve.id().to_string();
            curve
                .to_builder_with_id(id)
                .par_spreads(par_points)
                .par_interp(params.par_interp)
                .build()?
        } else {
            curve
        };

        let new_context = context.clone().insert(curve);
        Ok((new_context, report))
    }

    fn quote_hazard_guess(&self, quote: &CalibrationQuote) -> Option<f64> {
        let pq = match quote {
            CalibrationQuote::Cds(pq) => pq,
            _ => return None,
        };

        // W6: prefer the *curve's* recovery (`params.recovery_rate`) for the
        // initial guess. The quote-level `recovery_rate` is the protection
        // seller's assumption at quote time; the curve-level recovery is what
        // actually drives the protection-leg PV during calibration. Using
        // them inconsistently biases the spread-implied λ ≈ S/(1-R) guess
        // when the two values disagree (e.g. quote has the desk's standard
        // 0.4 but the curve was overridden to 0.25 for a stressed name).
        // Fall back to the quote recovery if the curve recovery is missing
        // or sentinel (NaN).
        let curve_recovery = self.params.recovery_rate;
        let quote_spread_bp = match pq.quote.as_ref() {
            crate::market::quotes::cds::CdsQuote::CdsParSpread { spread_bp, .. } => *spread_bp,
            crate::market::quotes::cds::CdsQuote::CdsUpfront {
                running_spread_bp, ..
            } => *running_spread_bp,
        };
        let recovery = if curve_recovery.is_finite() {
            curve_recovery
        } else {
            match pq.quote.as_ref() {
                crate::market::quotes::cds::CdsQuote::CdsParSpread { recovery_rate, .. }
                | crate::market::quotes::cds::CdsQuote::CdsUpfront { recovery_rate, .. } => {
                    *recovery_rate
                }
            }
        };

        let min_lgd = self.config.validation.minimum_lgd_for_hazard_guess;
        let loss_given_default = (1.0 - recovery).max(min_lgd);
        let guess = (quote_spread_bp / 10_000.0) / loss_given_default;
        let hazard_min = self.config.hazard_curve.hazard_hard_min;
        let hazard_max = self.config.hazard_curve.hazard_hard_max;
        if guess.is_finite() && guess >= 0.0 {
            Some(guess.clamp(hazard_min, hazard_max))
        } else {
            None
        }
    }

    fn with_temp_context<F, T>(&self, curve: &HazardCurve, op: F) -> Result<T>
    where
        F: FnOnce(&MarketContext) -> Result<T>,
    {
        if let Some(ctx_cell) = &self.reuse_context {
            let mut ctx = ctx_cell.borrow_mut();
            ctx.insert_mut(curve.clone());
            // Sync CreditIndex if it exists (so pricer sees trial curve)
            if let Ok(idx) = ctx.get_credit_index(self.params.curve_id.as_str()) {
                let mut updated = idx.as_ref().clone();
                updated.index_credit_curve = std::sync::Arc::new(curve.clone());
                ctx.insert_credit_index_mut(self.params.curve_id.as_str(), updated);
            }
            op(&ctx)
        } else {
            let mut temp_context = self.base_context.clone();
            temp_context.insert_mut(curve.clone());
            // Sync CreditIndex if it exists
            if let Ok(idx) = temp_context.get_credit_index(self.params.curve_id.as_str()) {
                let mut updated = idx.as_ref().clone();
                updated.index_credit_curve = std::sync::Arc::new(curve.clone());
                temp_context.insert_credit_index_mut(self.params.curve_id.as_str(), updated);
            }
            op(&temp_context)
        }
    }
}

impl BootstrapTarget for HazardCurveTarget {
    type Quote = crate::calibration::prepared::CalibrationQuote;
    type Curve = HazardCurve;

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        match quote {
            crate::calibration::prepared::CalibrationQuote::Cds(pq) => Ok(pq.pillar_time),
            _ => Err(finstack_core::Error::Input(
                finstack_core::InputError::Invalid,
            )),
        }
    }

    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        HazardCurve::builder(self.params.curve_id.to_string())
            .base_date(self.params.base_date)
            .day_count(self.cds_conventions.day_count)
            .issuer(self.params.entity.clone())
            .seniority(self.params.seniority)
            .currency(self.params.currency)
            .recovery_rate(self.params.recovery_rate)
            .knots(knots.to_vec())
            // Par spread interpolation is for *reporting* quoted spreads on the calibrated curve.
            // Positivity / no-arbitrage for survival is enforced via λ>=0; the survival
            // interpolation style between pillars comes from `params.interpolation`
            // (default LogLinear, i.e. piecewise-constant hazard).
            .par_interp(self.params.par_interp)
            .interp(self.params.interpolation)
            .build()
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        let pq = match quote {
            crate::calibration::prepared::CalibrationQuote::Cds(pq) => pq,
            _ => {
                return Err(finstack_core::Error::Input(
                    finstack_core::InputError::Invalid,
                ))
            }
        };
        let base_date = self.params.base_date;
        self.with_temp_context(curve, |ctx| {
            let npv = pq.instrument.value_raw(ctx, base_date)?;
            Ok(npv / self.params.notional)
        })
    }

    fn initial_guess(&self, quote: &Self::Quote, previous_knots: &[(f64, f64)]) -> Result<f64> {
        let hazard_min = self.config.hazard_curve.hazard_hard_min;
        let hazard_max = self.config.hazard_curve.hazard_hard_max;

        // Prefer a spread-implied guess: λ ≈ spread / (1 − R)
        if let Some(spread_guess) = self.quote_hazard_guess(quote) {
            return Ok(spread_guess.clamp(hazard_min, hazard_max));
        }

        let guess = previous_knots.last().map(|&(_, v)| v).unwrap_or(0.01);
        if guess.is_finite() {
            Ok(guess.clamp(hazard_min, hazard_max))
        } else {
            Ok(0.01)
        }
    }

    fn scan_points(&self, _quote: &Self::Quote, initial_guess: f64) -> Result<Vec<f64>> {
        // The scan grid is maturity- and quote-agnostic; it depends only on the
        // spread-implied `initial_guess` and the configured hazard bounds. The grid
        // construction is factored into the free function `hazard_scan_grid` so it can
        // be unit-tested directly (item 6) without fabricating a `CalibrationQuote`.
        Ok(hazard_scan_grid(&self.config.hazard_curve, initial_guess))
    }

    fn validate_knot(&self, time: f64, value: f64) -> Result<()> {
        let hazard_min = self.config.hazard_curve.hazard_hard_min;
        let hazard_max = self.config.hazard_curve.hazard_hard_max;

        if !time.is_finite() || time <= 0.0 {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Invalid hazard knot time for {}: t={}",
                    self.params.curve_id, time
                ),
                category: "bootstrapping".to_string(),
            });
        }
        if !value.is_finite() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Non-finite hazard rate for {} at t={:.6}",
                    self.params.curve_id, time
                ),
                category: "bootstrapping".to_string(),
            });
        }
        if value < hazard_min {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Negative hazard rate for {} at t={:.6}: {:.6}",
                    self.params.curve_id, time, value
                ),
                category: "bootstrapping".to_string(),
            });
        }
        if value > hazard_max {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Hazard rate out of bounds for {} at t={:.6}: {:.6} (max {:.6})",
                    self.params.curve_id, time, value, hazard_max
                ),
                category: "bootstrapping".to_string(),
            });
        }
        Ok(())
    }
}

impl GlobalSolveTarget for HazardCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = HazardCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        let seed_curve = self
            .base_context
            .get_hazard(self.params.curve_id.as_str())
            .ok();

        let hazard_min = self.config.hazard_curve.hazard_hard_min;
        let hazard_max = self.config.hazard_curve.hazard_hard_max;

        let mut entries = Vec::with_capacity(quotes.len());

        for quote in quotes {
            let t = self.quote_time(quote)?;
            if !t.is_finite() || t <= 0.0 {
                continue;
            }

            let guess = if let Some(curve) = seed_curve.as_ref() {
                curve.hazard_rate(t)
            } else {
                self.quote_hazard_guess(quote).unwrap_or(0.01)
            };

            let guess = if guess.is_finite() {
                guess.clamp(hazard_min, hazard_max)
            } else {
                0.01
            };

            entries.push((t, guess, quote.clone()));
        }

        entries.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut times = Vec::with_capacity(entries.len());
        let mut initials = Vec::with_capacity(entries.len());
        let mut active_quotes = Vec::with_capacity(entries.len());
        let mut last_time: Option<f64> = None;

        for (t, guess, quote) in entries {
            if let Some(prev) = last_time {
                if (t - prev).abs() <= TOLERANCE_DUP_KNOTS {
                    return Err(finstack_core::Error::Calibration {
                        message: format!(
                            "Duplicate or unsorted hazard knot times detected (prev={:.10}, new={:.10}). \
Ensure quotes map to strictly increasing year fractions.",
                            prev, t
                        ),
                        category: "global_solve".to_string(),
                    });
                }
            }
            last_time = Some(t);
            times.push(t);
            initials.push(guess);
            active_quotes.push(quote);
        }

        Ok((times, initials, active_quotes))
    }

    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        self.build_curve_for_solver_from_params(times, params)
    }

    fn build_curve_for_solver_from_params(
        &self,
        times: &[f64],
        params: &[f64],
    ) -> Result<Self::Curve> {
        if times.len() != params.len() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Global solve dimension mismatch: {} times vs {} params",
                    times.len(),
                    params.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        let mut knots = Vec::with_capacity(times.len());
        let mut last_t = 0.0;

        for (&t, &lambda) in times.iter().zip(params.iter()) {
            if t <= last_t {
                return Err(finstack_core::Error::Calibration {
                    message: format!(
                        "Non-increasing hazard knot time {:.10} detected (previous {:.10}). \
Global solve requires strictly increasing times.",
                        t, last_t
                    ),
                    category: "global_solve".to_string(),
                });
            }
            self.validate_knot(t, lambda)?;
            last_t = t;
            knots.push((t, lambda));
        }

        self.build_curve_for_solver(&knots)
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        if residuals.len() < quotes.len() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Global solve residuals buffer too small: got {} need {}",
                    residuals.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        self.with_temp_context(curve, |ctx| {
            for (i, quote) in quotes.iter().enumerate() {
                let pq = match quote {
                    CalibrationQuote::Cds(pq) => pq,
                    _ => {
                        return Err(finstack_core::Error::Input(
                            finstack_core::InputError::Invalid,
                        ))
                    }
                };
                let npv = pq.instrument.value_raw(ctx, self.params.base_date)?;
                residuals[i] = npv / self.params.notional;
            }
            Ok(())
        })
    }

    fn residual_key(&self, quote: &Self::Quote, idx: usize) -> String {
        let q = quote.get_instrument();
        format!("{}-{:03}", q.id(), idx)
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        if quotes.len() != weights_out.len() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Global solve requires weights.len() == quotes.len(); got {} vs {}.",
                    weights_out.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        for (i, quote) in quotes.iter().enumerate() {
            let t = self.quote_time(quote)?.max(1e-6);

            // Use hazard-curve-specific weighting scheme, not discount curve's.
            let weight = match self.config.hazard_curve.weighting_scheme {
                ResidualWeightingScheme::Equal => 1.0,
                ResidualWeightingScheme::LinearTime => t,
                ResidualWeightingScheme::SqrtTime => t.sqrt(),
                ResidualWeightingScheme::InverseDuration => 1.0 / t.max(0.1),
            };

            weights_out[i] = weight.max(WEIGHT_MIN_FLOOR);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::solver::traits::BootstrapTarget;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::term_structures::ParInterp;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::types::CurveId;
    use time::Month;

    fn base_params() -> HazardCurveParams {
        HazardCurveParams {
            curve_id: CurveId::new("TEST-HAZ".to_string()),
            entity: "ACME".to_string(),
            seniority: finstack_core::market_data::term_structures::Seniority::Senior,
            currency: Currency::USD,
            base_date: Date::from_calendar_date(2025, Month::January, 1).expect("valid base_date"),
            discount_curve_id: CurveId::new("USD-OIS".to_string()),
            recovery_rate: 0.4,
            notional: 1.0,
            method: crate::calibration::config::CalibrationMethod::Bootstrap,
            interpolation: InterpStyle::Linear,
            par_interp: ParInterp::Linear,
            doc_clause: None,
            cds_valuation_convention: None,
        }
    }

    #[test]
    fn validate_knot_rejects_negative_hazard() {
        let target = HazardCurveTarget::new(
            base_params(),
            MarketContext::default(),
            CalibrationConfig::default(),
        )
        .expect("target");
        let err = target
            .validate_knot(1.0, -1e-6)
            .expect_err("should reject negative hazard");
        assert!(err.to_string().to_lowercase().contains("negative hazard"));
    }

    #[test]
    fn validate_knot_rejects_hazard_above_max() {
        let config = CalibrationConfig::default();
        let hazard_max = config.hazard_curve.hazard_hard_max;
        let target = HazardCurveTarget::new(base_params(), MarketContext::default(), config)
            .expect("target");
        let err = target
            .validate_knot(1.0, hazard_max + 1e-6)
            .expect_err("should reject excessive hazard");
        assert!(err.to_string().to_lowercase().contains("out of bounds"));
    }

    #[test]
    fn new_rejects_inverted_hazard_bounds() {
        let mut config = CalibrationConfig::default();
        config.hazard_curve.hazard_hard_min = 0.05;
        config.hazard_curve.hazard_hard_max = 0.01;

        let err = HazardCurveTarget::new(base_params(), MarketContext::default(), config)
            .err()
            .expect("inverted hazard bounds should be rejected");
        assert!(err.to_string().to_lowercase().contains("hazard"));
    }

    /// Item 6: the hazard scan grid must give distressed / jump-to-default names
    /// resolution above the primary `+2`-decade window.
    ///
    /// For a moderate spread-implied guess (2% hazard), the primary window's upper edge
    /// is `10^(log10(0.02)+2) ≈ 2.0`. Pre-fix the *only* grid point in `(2.0, hazard_max]`
    /// was the lone `hazard_hard_max` anchor — no interior resolution for a distressed
    /// root sitting up there. Post-fix the upside augmentation adds many log-spaced
    /// points across that region. The primary window itself must be unchanged so
    /// bit-stable Bloomberg golden fixtures (normal IG/HY names) are unaffected.
    #[test]
    fn item6_hazard_scan_grid_has_distressed_upside_resolution() {
        let cfg = CalibrationConfig::default();
        let hcfg = &cfg.hazard_curve;
        let max_h = hcfg.hazard_hard_max; // 10.0 by default

        let guess = 0.02_f64; // 2% hazard — a moderate spread-implied guess
        let grid = hazard_scan_grid(hcfg, guess);

        // Upper edge of the primary +2-decade window.
        let primary_upper = 10f64.powf(guess.log10() + 2.0);
        assert!(
            primary_upper < max_h,
            "test fixture invalid: primary window must not already reach hazard_max \
             (primary_upper={primary_upper}, max_h={max_h})",
        );

        // Count grid points strictly inside the upper region (primary_upper, max_h).
        let upper_region_points = grid
            .iter()
            .filter(|&&v| v > primary_upper * (1.0 + 1e-9) && v < max_h * (1.0 - 1e-9))
            .count();
        assert!(
            upper_region_points >= 5,
            "Item 6: the scan grid must provide distressed/JTD resolution above the \
             primary window — found only {upper_region_points} point(s) in \
             ({primary_upper:.4}, {max_h:.4}); pre-fix there were none",
        );

        // The grid must still span the full range and stay within bounds.
        assert!(grid
            .iter()
            .all(|&v| v >= hcfg.hazard_hard_min && v <= max_h));
        assert!(
            grid.iter().any(|&v| (v - max_h).abs() < 1e-12),
            "hazard_hard_max anchor must still be present"
        );

        // Golden-safety: for a normal name the primary window is dense and the grid
        // contains the spread-implied guess itself as an anchor.
        assert!(
            grid.iter().any(|&v| (v - guess).abs() < 1e-12),
            "the spread-implied guess must remain a grid anchor"
        );
        // Strictly increasing & deduplicated.
        for w in grid.windows(2) {
            assert!(w[1] > w[0], "scan grid must be strictly increasing");
        }
    }

    #[test]
    fn build_curve_preserves_par_interp_and_monotone_survival() {
        let mut p = base_params();
        p.par_interp = ParInterp::LogLinear;
        let target =
            HazardCurveTarget::new(p, MarketContext::default(), CalibrationConfig::default())
                .expect("target");

        let curve = target
            .build_curve(&[(1.0, 0.02), (5.0, 0.03)])
            .expect("curve build should succeed");
        assert_eq!(curve.par_interp(), ParInterp::LogLinear);

        let s1 = curve.sp(1.0);
        let s5 = curve.sp(5.0);
        let s10 = curve.sp(10.0);
        assert!((0.0..=1.0).contains(&s1));
        assert!((0.0..=1.0).contains(&s5));
        assert!((0.0..=1.0).contains(&s10));
        assert!(s1 >= s5 && s5 >= s10);
    }
}
