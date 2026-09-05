//! Unified pricing engine for revolving credit facilities.
//!
//! Provides a single pricer that handles both deterministic and stochastic modes:
//! - **Deterministic**: Prices using pre-defined draw/repay events
//! - **Stochastic**: Generates 3-factor MC paths and prices each path deterministically
//!
//! # Architecture
//!
//! Stochastic pricing is implemented as averaging many deterministic path pricings,
//! ensuring consistency between modes and enabling full path capture for distribution analysis.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::revolving_credit::types::{DrawRepaySpec, RevolvingCredit};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

pub use super::results::{EnhancedMonteCarloResult, PathResult};

/// Default utilization–credit correlation for the auto-synthesized `McConfig`.
///
/// When a stochastic facility carries a hazard curve but no explicit
/// `mc_config`, the pricer synthesizes one with this moderate positive
/// correlation between the utilization and credit-spread factors. This is the
/// default adverse-selection ("run on the bank") assumption: as a borrower's
/// credit deteriorates, drawn exposure rises, so exposure-at-default exceeds
/// the unconditional expected drawn balance. Simulating utilization
/// independently of the credit state (the previous default) understates EAD
/// and overstates lender PV for risky borrowers.
///
/// The empirical EAD/LEQ literature consistently finds materially higher
/// drawdown as borrowers approach default:
///
/// - Asarnow, E., & Marker, J. (1995). "Historical Performance of the U.S.
///   Corporate Loan Market: 1988–1993." *Journal of Commercial Lending*,
///   77(7), 13–32 — loan-equivalent usage of revolving commitments rises
///   sharply with deteriorating ratings.
/// - Jiménez, G., Lopez, J. A., & Saurina, J. (2009). "Empirical Analysis of
///   Corporate Credit Lines." *Review of Financial Studies*, 22(12),
///   5069–5098 — defaulting firms draw down their credit lines materially
///   more than non-defaulting firms in the years leading up to default.
///
/// Override by supplying an explicit `McConfig` (any `util_credit_corr` or a
/// full `correlation_matrix`); pass `util_credit_corr: Some(0.0)` to disable
/// adverse selection entirely.
pub(crate) const DEFAULT_UTIL_CREDIT_CORR: f64 = 0.3;

/// Default fractional implied volatility for the auto-synthesized
/// market-anchored credit-spread process.
///
/// Utilization–credit dependence enters the model **only** through the factor
/// correlation of the Brownian shocks (see `monte_carlo_process`), so
/// [`DEFAULT_UTIL_CREDIT_CORR`] has a pricing effect only when the credit
/// factor genuinely diffuses. The previously synthesized `implied_vol = 1e-10`
/// froze the credit factor, which would leave a default correlation silently
/// inert. 30% is at the low end of typical single-name credit-spread implied
/// volatilities and keeps the mean-anchored CIR comfortably inside the Feller
/// region (2κθ > σ²) for the synthesized `kappa = 0.1`.
///
/// Override by supplying an explicit `McConfig` with a custom
/// `credit_spread_process` (e.g. `implied_vol` near zero for deterministic
/// credit).
pub(crate) const DEFAULT_CREDIT_SPREAD_IMPLIED_VOL: f64 = 0.3;
/// Unified pricer for revolving credit facilities.
///
/// Handles both deterministic and stochastic pricing using a single implementation.
/// Stochastic pricing generates paths and applies deterministic pricing to each path.
pub struct RevolvingCreditPricer {
    model: ModelKey,
}

impl Default for RevolvingCreditPricer {
    fn default() -> Self {
        Self {
            model: ModelKey::Discounting,
        }
    }
}

impl RevolvingCreditPricer {
    /// Create a pricer for the given registered model key.
    ///
    /// # Arguments
    ///
    /// * `model` - `Discounting` for deterministic facilities or
    ///   `MonteCarloGBM` for stochastic utilization paths.
    pub fn new(model: ModelKey) -> Self {
        Self { model }
    }
    /// Main pricing entry point.
    ///
    /// Automatically dispatches to deterministic or stochastic pricing based on
    /// the facility's `draw_repay_spec`.
    ///
    /// # Arguments
    ///
    /// * `facility` - The revolving credit facility
    /// * `market` - Market context with curves
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value as `Money`
    pub(crate) fn price(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        match &facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => Self::price_deterministic(facility, market, as_of),
            DrawRepaySpec::Stochastic(_) => {
                let enhanced = Self::price_monte_carlo(facility, market, as_of)?;
                Ok(enhanced.mc_result.estimate.mean)
            }
        }
    }
}

impl Pricer for RevolvingCreditPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::RevolvingCredit, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        use crate::pricer::expect_inst;

        let facility: &RevolvingCredit = expect_inst(instrument, InstrumentType::RevolvingCredit)?;

        let ctx = PricingErrorContext::new()
            .instrument_id(facility.id.as_str())
            .instrument_type(InstrumentType::RevolvingCredit)
            .model(self.model);

        // Route to appropriate pricing method based on model
        let result_pv = match self.model {
            ModelKey::Discounting => {
                // For discounting, we use the unified price method which handles
                // deterministic specs (and errs on stochastic if MC not enabled/used)
                Self::price(facility, market, as_of)
                    .map_err(|e| PricingError::from_core(e, ctx.clone()))?
            }

            ModelKey::MonteCarloGBM => {
                // For MC, we ensure we're using the MC path
                let enhanced = Self::price_with_paths(facility, market, as_of)
                    .map_err(|e| PricingError::from_core(e, ctx.clone()))?;
                enhanced.mc_result.estimate.mean
            }
            _ => {
                return Err(PricingError::model_failure_with_context(
                    format!("Unsupported model for RevolvingCredit: {}", self.model),
                    ctx,
                ));
            }
        };

        // Wrap in ValuationResult
        let mut result = ValuationResult::stamped(facility.id.as_str(), as_of, result_pv);
        result.measures.insert(
            crate::metrics::MetricId::custom("model"),
            self.model.to_string().parse().unwrap_or(0.0),
        ); // Just tagging
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::revolving_credit::ThreeFactorPathData;

    use crate::instruments::fixed_income::revolving_credit::{
        BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
        RevolvingCreditFees, StochasticUtilizationSpec, UtilizationProcess,
    };
    use finstack_quant_core::dates::DayCount;

    use finstack_quant_core::market_data::context::MarketContext;

    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    use finstack_quant_core::money::Money;

    use finstack_quant_core::{currency::Currency, dates::Tenor};
    use time::Month;

    /// Item 6 verification: the contractual leg (survival-weighted no-default
    /// cashflows) plus the recovery leg must NOT double-count LGD on
    /// principal.
    ///
    /// The audit flagged a suspected double-count (PV overstated ≈ R·drawn·PD).
    /// This test pins the exact decomposition: for a zero-coupon facility
    /// (principal only) with flat DF = 1, flat hazard, and recovery R, the
    /// correct risky PV of principal is `D·SP + R·D·(1-SP)` — full repayment
    /// if the borrower survives, recovery if it defaults. The priced PV must
    /// equal that exactly, NOT the double-counted `D·SP + 2·R·D·(1-SP)`.
    ///
    /// Conclusion (recorded as a regression guard): the current implementation
    /// is correct — the survival-weighted contractual leg represents the
    /// no-default state and the recovery leg adds the disjoint default-state
    /// value, exactly as a risky-cashflow decomposition should.
    #[test]
    fn recovery_leg_does_not_double_count_lgd_on_principal() {
        use crate::instruments::fixed_income::revolving_credit::RevolvingCreditFees;
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-RECOVERY-NODBL".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((1_000_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            // Zero coupon isolates the principal leg.
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        // Flat DF = 1 everywhere → arithmetic is exact (no discounting noise).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        // Flat hazard 20% → SP(1y) = exp(-0.2).
        let hz = HazardCurve::builder("USD-HZ")
            .base_date(start)
            .knots([(1.0, 0.20), (5.0, 0.20)])
            .recovery_rate(0.40)
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(disc).insert(hz);

        let pv = RevolvingCreditPricer::price(&facility, &market, start)
            .expect("price")
            .amount();

        let d = 1_000_000.0_f64;
        let sp = (-0.20_f64).exp();
        let r = 0.4_f64;
        // Correct: full repayment on survival + recovery on default.
        let correct = d * sp + r * d * (1.0 - sp);
        // Double-counted: an extra R·D·(1-SP) on top.
        let double_counted = correct + r * d * (1.0 - sp);

        assert!(
            (pv - correct).abs() < 1.0,
            "risky PV {pv} should equal D·SP + R·D·(1-SP) = {correct}, not the \
             double-counted {double_counted}"
        );
        assert!(
            (pv - double_counted).abs() > 1.0,
            "risky PV {pv} must NOT equal the double-counted value {double_counted}"
        );
    }

    /// M2.10: survival weighting must be conditioned on survival to `as_of`.
    ///
    /// A seasoned zero-coupon facility (flat DF = 1, flat hazard λ = 20%)
    /// priced one year into a two-year life must be worth
    /// `D·SP(as_of→T) + R·D·(1−SP(as_of→T))` with `SP(as_of→T) = e^{-0.2}`,
    /// NOT the unconditional `D·SP(0→T)/1 + …` which understates PV by the
    /// factor S(0→as_of) = e^{-0.2}.
    #[test]
    fn seasoned_facility_survival_is_conditioned_on_as_of() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let start = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-SEASONED".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((1_000_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let hz = HazardCurve::builder("USD-HZ")
            .base_date(start)
            .knots([(1.0, 0.20), (5.0, 0.20)])
            .recovery_rate(0.40)
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(disc).insert(hz);

        let pv = RevolvingCreditPricer::price(&facility, &market, as_of)
            .expect("price")
            .amount();

        let d = 1_000_000.0_f64;
        let r = 0.4_f64;
        // One year remains at λ = 20%, conditional on survival to as_of.
        let sp_cond = (-0.20_f64).exp();
        let correct = d * sp_cond + r * d * (1.0 - sp_cond);
        // Unconditional weighting multiplies the survival leg by an extra
        // S(0→as_of) = e^{-0.2} and scales the default leg the same way.
        let sp_uncond_t = (-0.40_f64).exp();
        let unconditional = d * sp_uncond_t + r * d * ((-0.20_f64).exp() - sp_uncond_t);

        assert!(
            (pv - correct).abs() < 1.0,
            "seasoned PV {pv} should equal conditional value {correct} \
             (unconditional would be {unconditional})"
        );
        assert!(
            (pv - unconditional).abs() > 1.0,
            "seasoned PV {pv} must NOT equal the unconditional value {unconditional}"
        );
    }

    /// M2.8: a deterministic draw/repay event dated on the commitment date is
    /// rejected — the position at commitment is defined by `drawn_amount`
    /// and a commitment-date event double-counted principal (interest on 2X,
    /// 2X terminal repayment).
    #[test]
    fn commitment_date_event_is_rejected() {
        use crate::instruments::fixed_income::revolving_credit::DrawRepayEvent;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let result = RevolvingCredit::builder()
            .id("RC-COMMIT-EVENT".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((400_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
                date: start,
                amount: Money::from((400_000_i64, Currency::USD)),
                is_draw: true,
            }]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build();

        assert!(
            result.is_err(),
            "construction must reject a commitment-date event"
        );
    }

    /// M2.9: Sobol QMC requires one coordinate per (step, factor). The
    /// weekly-refined grid of a one-year facility needs ~52×3 dimensions —
    /// far beyond the supported Sobol table — so `use_sobol_qmc` must be
    /// rejected rather than silently consuming a 3-dimensional sequence
    /// once per time step (van-der-Corput anti-correlated, biased paths).
    #[test]
    fn sobol_qmc_with_underdimensioned_schedule_is_rejected() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("date");

        let facility = RevolvingCredit::builder()
            .id("RC-SOBOL".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((400_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 8,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: true,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);

        let err = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect_err("Sobol with num_steps×num_factors > MAX_SOBOL_DIMENSION must error");
        assert!(
            err.to_string().contains("use_sobol_qmc"),
            "error should explain the Sobol dimension contract, got: {err}"
        );
    }

    #[test]
    fn test_compute_dynamic_survival() {
        let spreads = vec![0.01, 0.02, 0.015, 0.018];
        let times = vec![0.0, 0.25, 0.5, 0.75];
        let recovery = 0.4;
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let cashflow_dates = vec![
            start,
            Date::from_calendar_date(2025, Month::April, 1).expect("valid date"),
            Date::from_calendar_date(2025, Month::July, 1).expect("valid date"),
            Date::from_calendar_date(2025, Month::October, 1).expect("valid date"),
        ];

        let survivals = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &times,
            &cashflow_dates,
            recovery,
            start,
            DayCount::Act365F,
        )
        .expect("should succeed");

        assert_eq!(survivals.len(), 4);
        // Survival at t=0 should be 1.0
        assert!((survivals[0] - 1.0).abs() < 1e-10);
        // Survival should generally decrease over time (with positive spreads)
        // All survivals should be in (0, 1]
        for &s in &survivals {
            assert!(s > 0.0 && s <= 1.0);
        }
    }

    #[test]
    fn dynamic_survival_uses_trapezoidal_hazard_integration() {
        // A rising spread path distinguishes trapezoidal integration from the
        // left-Riemann sum (which would understate cumulative hazard and
        // overstate survival). Pin the exact trapezoid value.
        let day_count = DayCount::Act365F;
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mid = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let t_mid = day_count
            .year_fraction(start, mid, Default::default())
            .expect("year fraction");
        let t_end = day_count
            .year_fraction(start, end, Default::default())
            .expect("year fraction");

        let times = vec![0.0, t_mid, t_end];
        let spreads = vec![0.01, 0.03, 0.05];
        let recovery = 0.0; // hazard = spread

        let survivals = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &times,
            &[end],
            recovery,
            start,
            day_count,
        )
        .expect("should succeed");

        let expected_hazard = 0.5 * (0.01 + 0.03) * t_mid + 0.5 * (0.03 + 0.05) * (t_end - t_mid);
        let expected = (-expected_hazard).exp();
        assert!(
            (survivals[0] - expected).abs() < 1e-12,
            "trapezoidal survival mismatch: got {}, expected {expected}",
            survivals[0]
        );
    }

    #[test]
    fn test_day_count_consistency() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let dc_act360 = DayCount::Act360;

        // Create time points using Act360 (approx 1.0139 for 1 year)
        let t_end_act360 = dc_act360
            .year_fraction(start, end, Default::default())
            .expect("valid date range for year fraction");
        let time_points = vec![0.0, t_end_act360];

        // Spread path: 100bps constant
        let spreads = vec![0.01, 0.01];
        let recovery = 0.0; // Simple hazard = spread

        // We want to look up survival at 'end' date
        let cashflow_dates = vec![end];

        // 1. Correct: Pass Act360
        let survivals_correct = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &time_points,
            &cashflow_dates,
            recovery,
            start,
            dc_act360,
        )
        .expect("should succeed");

        // Should match exact calculation: exp(-hazard * t)
        // hazard = 0.01
        // t = t_end_act360
        let expected = (-0.01 * t_end_act360).exp();
        assert!(
            (survivals_correct[0] - expected).abs() < 1e-10,
            "Correct day count should yield exact match. Got {}, expected {}",
            survivals_correct[0],
            expected
        );

        // 2. Incorrect: Pass Act365F (simulating the bug)
        let survivals_mismatch = RevolvingCreditPricer::compute_dynamic_survival_at_dates(
            &spreads,
            &time_points,
            &cashflow_dates,
            recovery,
            start,
            DayCount::Act365F,
        )
        .expect("should succeed");

        assert!(
            (survivals_mismatch[0] - survivals_correct[0]).abs() > 1e-5,
            "Mismatching day counts should yield different results"
        );
    }

    #[test]
    fn test_price_with_paths_uses_moneyestimate_defaults() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-UNIFIED-PATHS".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((400_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 8,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility should build");

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("curve should build");
        let market = MarketContext::new().insert(disc_curve);

        let result = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect("should price");

        assert_eq!(result.mc_result.estimate.num_paths, 8);
        assert_eq!(result.path_results.len(), 8);
        assert!(result.mc_result.estimate.std_dev.is_none());
        assert!(result.mc_result.estimate.median.is_none());
        assert!(result.mc_result.estimate.percentile_25.is_none());
        assert!(result.mc_result.estimate.percentile_75.is_none());
        assert!(result.mc_result.estimate.min.is_none());
        assert!(result.mc_result.estimate.max.is_none());
    }

    /// `num_paths < 2` must be rejected: a single path has no variance
    /// estimate (previously produced NaN std error downstream).
    #[test]
    fn single_path_mc_is_rejected() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let result = RevolvingCredit::builder()
            .id("RC-ONE-PATH".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((400_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.05,
                    },
                    num_paths: 1,
                    seed: Some(7),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: None,
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build();

        let err = result.expect_err("num_paths = 1 must be rejected at construction");
        assert!(
            err.to_string().contains("num_paths"),
            "error should mention num_paths, got: {err}"
        );
    }

    /// Zero utilization volatility must freeze ONLY the utilization factor:
    /// the credit-spread (and rate) factors keep their own dynamics. The
    /// previous behavior skipped the discretization step entirely, silently
    /// freezing all three factors.
    #[test]
    fn zero_util_vol_freezes_only_the_utilization_factor() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-ZEROVOL".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((400_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 0.75,
                        volatility: 0.0, // zero utilization vol
                    },
                    num_paths: 4,
                    seed: Some(11),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: Some(McConfig {
                        recovery_rate: 0.4,
                        // Genuinely stochastic credit spread.
                        credit_spread_process: CreditSpreadProcessSpec::Cir {
                            kappa: 0.5,
                            theta: 0.02,
                            sigma: 0.05,
                            initial: 0.02,
                        },
                        interest_rate_process: None,
                        correlation_matrix: None,
                        util_credit_corr: None,
                    }),
                },
            )))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(disc);

        let result = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
            .expect("should price");
        let path = result.path_results[0]
            .path_data
            .as_ref()
            .expect("path data");

        // Utilization frozen at its initial value across the whole path…
        let u0 = path.utilization_path[0];
        assert!(
            path.utilization_path
                .iter()
                .all(|&u| (u - u0).abs() < 1e-12),
            "utilization must be frozen with zero vol: {:?}",
            path.utilization_path
        );
        // …while the credit spread still diffuses.
        let s0 = path.credit_spread_path[0];
        assert!(
            path.credit_spread_path
                .iter()
                .any(|&s| (s - s0).abs() > 1e-6),
            "credit spread must keep stepping with zero util vol: {:?}",
            path.credit_spread_path
        );
    }

    /// `ThreeFactorPathData::validate` must reject length mismatches with an
    /// error instead of letting downstream indexing panic.
    #[test]
    fn path_data_validation_rejects_mismatched_lengths() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let d2 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

        let bad = ThreeFactorPathData {
            utilization_path: vec![0.4, 0.5],
            short_rate_path: vec![0.03], // wrong length
            credit_spread_path: vec![0.01, 0.01],
            time_points: vec![0.0, 0.5],
            payment_dates: vec![start, d2],
            stochastic_rates: false,
        };
        let err = bad.validate().expect_err("length mismatch must error");
        assert!(err.to_string().contains("short_rate_path"), "got: {err}");

        let non_monotone = ThreeFactorPathData {
            utilization_path: vec![0.4, 0.5],
            short_rate_path: vec![0.03, 0.03],
            credit_spread_path: vec![0.01, 0.01],
            time_points: vec![0.5, 0.0],
            payment_dates: vec![start, d2],
            stochastic_rates: false,
        };
        assert!(
            non_monotone.validate().is_err(),
            "non-increasing time_points must error"
        );
    }

    /// Regression test for parallel MC determinism.
    ///
    /// After parallelizing the Philox path loop with rayon, two runs with the
    /// same seed must produce bit-identical PVs and per-path values regardless
    /// of how many cores rayon uses to execute. This guards against any RNG
    /// substream initialization regressions or accidental order-dependence in
    /// the parallel `collect()` pipeline.
    #[test]
    fn parallel_mc_is_deterministic_across_runs() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let make_facility = || {
            RevolvingCredit::builder()
                .id("RC-DETERMINISM".into())
                .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
                .drawn_amount(Money::from((400_000_i64, Currency::USD)))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
                .day_count(DayCount::Act360)
                .frequency(Tenor::quarterly())
                .fees(RevolvingCreditFees::default())
                .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                    StochasticUtilizationSpec {
                        utilization_process: UtilizationProcess::MeanReverting {
                            target_rate: 0.5,
                            speed: 0.75,
                            volatility: 0.05,
                        },
                        num_paths: 64,
                        seed: Some(123_456_789),
                        antithetic: true,
                        use_sobol_qmc: false,
                        mc_config: Some(McConfig {
                            recovery_rate: 0.4,
                            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
                            interest_rate_process: None,
                            correlation_matrix: None,
                            util_credit_corr: None,
                        }),
                    },
                )))
                .discount_curve_id("USD-OIS".into())
                .recovery_rate(0.4)
                .build()
                .expect("facility should build")
        };

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("curve should build");
        let market = MarketContext::new().insert(disc_curve);

        let r1 = RevolvingCreditPricer::price_with_paths(&make_facility(), &market, start)
            .expect("first run should price");
        let r2 = RevolvingCreditPricer::price_with_paths(&make_facility(), &market, start)
            .expect("second run should price");

        assert_eq!(r1.path_results.len(), r2.path_results.len());
        // Mean PV must be bit-identical (same seed → same paths → same PVs).
        let m1 = r1.mc_result.estimate.mean.amount();
        let m2 = r2.mc_result.estimate.mean.amount();
        assert_eq!(
            m1.to_bits(),
            m2.to_bits(),
            "parallel MC must be deterministic for fixed seed; got mean1={m1} mean2={m2}"
        );
        for (i, (p1, p2)) in r1
            .path_results
            .iter()
            .zip(r2.path_results.iter())
            .enumerate()
        {
            let v1 = p1.pv.amount();
            let v2 = p2.pv.amount();
            assert_eq!(
                v1.to_bits(),
                v2.to_bits(),
                "path {i} PV diverges between runs: {v1} vs {v2}"
            );
        }
    }

    /// The auto-synthesized default `McConfig` must embed adverse selection.
    ///
    /// For a risky borrower (hazard curve present) with no explicit
    /// `mc_config`, the synthesized config defaults to
    /// `util_credit_corr = DEFAULT_UTIL_CREDIT_CORR > 0`: paths where the
    /// credit spread widens also draw more, so exposure-at-default is higher
    /// than under independence and the lender PV must be LOWER than an
    /// otherwise-identical explicit config with `util_credit_corr = 0.0`
    /// (same seed, same synthesized credit process).
    #[test]
    fn default_config_embeds_adverse_selection_vs_explicit_zero_corr() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let end = Date::from_calendar_date(2027, Month::January, 1).expect("date");

        let make_facility = |id: &str, mc_config: Option<McConfig>| {
            RevolvingCredit::builder()
                .id(id.into())
                .commitment_amount(Money::from((10_000_000_i64, Currency::USD)))
                .drawn_amount(Money::from((5_000_000_i64, Currency::USD)))
                .commitment_date(start)
                .maturity(end)
                .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
                .day_count(DayCount::Act360)
                .frequency(Tenor::quarterly())
                .fees(RevolvingCreditFees::default())
                .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                    StochasticUtilizationSpec {
                        utilization_process: UtilizationProcess::MeanReverting {
                            target_rate: 0.6,
                            speed: 0.5,
                            volatility: 0.25,
                        },
                        num_paths: 4000,
                        seed: Some(42),
                        antithetic: true,
                        use_sobol_qmc: false,
                        mc_config,
                    },
                )))
                .discount_curve_id("USD-OIS".into())
                .credit_curve_id("BORROWER-HZ".into())
                .recovery_rate(0.4)
                .build()
                .expect("facility")
        };

        // Explicit config replicating the auto-synthesized default exactly,
        // except adverse selection is disabled (util_credit_corr = 0.0).
        let zero_corr_config = McConfig {
            correlation_matrix: None,
            recovery_rate: 0.4,
            credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                credit_curve_id: "BORROWER-HZ".into(),
                kappa: 0.1,
                implied_vol: DEFAULT_CREDIT_SPREAD_IMPLIED_VOL,
                tenor_years: None,
            },
            interest_rate_process: None,
            util_credit_corr: Some(0.0),
        };

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.03f64).exp()), (5.0, (-0.15f64).exp())])
            .build()
            .expect("curve");
        // Risky borrower: flat 5% hazard.
        let hz = HazardCurve::builder("BORROWER-HZ")
            .base_date(start)
            .recovery_rate(0.4)
            .day_count(DayCount::Act365F)
            .knots([(1.0, 0.05), (5.0, 0.05)])
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(disc).insert(hz);

        let pv_default = RevolvingCreditPricer::price_with_paths(
            &make_facility("RC-ADVSEL-DEFAULT", None),
            &market,
            start,
        )
        .expect("default-config pricing")
        .mc_result
        .estimate
        .mean
        .amount();
        let pv_zero_corr = RevolvingCreditPricer::price_with_paths(
            &make_facility("RC-ADVSEL-ZERO", Some(zero_corr_config)),
            &market,
            start,
        )
        .expect("zero-corr pricing")
        .mc_result
        .estimate
        .mean
        .amount();

        // Adverse selection increases expected drawn exposure at default, so
        // the lender's PV must be strictly lower under the default config.
        assert!(
            pv_default < pv_zero_corr,
            "default config (util_credit_corr = {DEFAULT_UTIL_CREDIT_CORR}) must \
             embed adverse selection and price BELOW the zero-correlation config: \
             default = {pv_default}, zero-corr = {pv_zero_corr}"
        );
    }
}
