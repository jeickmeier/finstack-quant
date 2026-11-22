use super::collector::{PathTrancheMetrics, ScenarioCollector, expected_shortfall};
use super::factors::PREPAY_FACTOR_SEED_SALT;
use super::*;
use crate::structured_credit::pricing::stochastic::calibrations::{
    clo_default_spec, rmbs_default_spec, rmbs_prepay_spec,
};
use finstack_quant_core::dates::DayCount;
use finstack_quant_models::correlation::{LatentFactorSpec, RecoverySpec};
use finstack_quant_models::credit::pool::{StochasticDefaultSpec, StochasticPrepaySpec};
use finstack_quant_models::monte_carlo::estimate::independent_estimator_stats;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::RandomStream;
use finstack_quant_valuations_core::results::structured_credit::{
    StochasticPricingResult, StructuredCreditPricingMode,
};

mod core_tests {
    use super::*;
    use crate::structured_credit::pricing::stochastic::pricer::ScenarioTreeConfig;
    use crate::structured_credit::{
        AssetPool, DealType, PoolAsset, Tranche, TrancheCashflows, TrancheStructure,
    };
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_cashflows::builder::{DefaultModelSpec, RecoveryModelSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DateExt};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::results::structured_credit::TrancheSeniority;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    /// Minimal single-asset/single-tranche deal for tests that need a
    /// structurally valid instrument to prepare a simulation against.
    fn simple_deal(id: &str) -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((1_000_000_i64, Currency::USD)),
            0.06,
            Date::from_calendar_date(2029, Month::January, 1).expect("valid date"),
            finstack_quant_core::dates::DayCount::Thirty360,
        ));
        let senior = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        )
        .expect("senior tranche");
        let mut deal = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Abs,
            crate::structured_credit::StandardDealParams {
                id: id.into(),
                pool,
                tranches: TrancheStructure::new(vec![senior]).expect("structure"),
                closing_date: test_date(),
                maturity: Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates");
        deal.calendar_id = Some("nyse".into());
        deal
    }

    fn test_discount_curve() -> std::sync::Arc<DiscountCurve> {
        std::sync::Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(test_date())
                .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("curve"),
        )
    }

    #[test]
    fn path_metrics_wal_matches_the_canonical_kernel() {
        let as_of = test_date();
        let first = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let second = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let principal_flows = vec![
            (first, Money::from((40_i64, Currency::USD))),
            (second, Money::from((60_i64, Currency::USD))),
        ];
        let zero = Money::from((0_i64, Currency::USD));
        let cashflows = TrancheCashflows {
            tranche_id: "A".to_string(),
            cashflows: principal_flows.clone(),
            detailed_flows: Vec::new(),
            accrual_periods: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows,
            pik_flows: Vec::new(),
            deferred_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: zero,
            total_interest: zero,
            total_principal: Money::from((100_i64, Currency::USD)),
            total_pik: zero,
            total_deferred: zero,
            total_writedown: zero,
        };

        let metrics =
            PathTrancheMetrics::from_cashflows(&cashflows, as_of, test_discount_curve().as_ref())
                .expect("path metrics");
        let expected = finstack_quant_cashflows::aggregation::weighted_average_life_from_principal(
            cashflows.principal_flows.iter().copied(),
            as_of,
        )
        .expect("canonical WAL");

        assert_eq!(metrics.wal, expected);
    }

    fn test_instrument() -> StructuredCredit {
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((1_000_000_i64, Currency::USD)),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Abs,
            crate::structured_credit::StandardDealParams {
                id: "ABS".into(),
                pool,
                tranches: TrancheStructure::new(vec![tranche]).expect("structure"),
                closing_date: test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    #[test]
    fn monte_carlo_two_estimators_price_waterfall_cashflows() {
        let instrument = test_instrument();
        let market = MarketContext::new()
            .try_insert((*test_discount_curve()).clone())
            .expect("insert market curve");
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(
                test_date()
                    .months_until(instrument.maturity)
                    .expect("valid maturity range") as usize,
            ),
            StructuredCreditPricingMode::MonteCarlo {
                num_paths: 2,
                antithetic: false,
            },
        );
        let pricer = StochasticPricer::new(config);

        let result = pricer.price(&instrument, &market).expect("price");

        assert_eq!(result.num_paths, 2);
        assert_eq!(result.tranche_results.len(), 1);
        assert!(result.npv.amount().is_finite());
    }

    #[test]
    fn scenario_collector_small_sample_interval_uses_independent_estimator_count() {
        let instrument = test_instrument();
        for antithetic in [false, true] {
            // The independent observations are [1, 3] in either mode:
            // sample variance = 2, standard error = 1, and t_1 is Cauchy.
            let pvs: &[f64] = if antithetic {
                &[0.0, 2.0, 2.0, 4.0]
            } else {
                &[1.0, 3.0]
            };
            let mut collector = ScenarioCollector::new(&instrument, pvs.len(), antithetic, false)
                .expect("collector");
            for &pv in pvs {
                collector.record_deal(pv, 0.0).expect("finite test pv");
            }
            let result = collector
                .finalize(StructuredCreditPricingMode::MonteCarlo {
                    num_paths: 2,
                    antithetic,
                })
                .expect("two independent estimators define sampling uncertainty");
            let critical = 1.0 / (std::f64::consts::PI * 0.025).tan();
            assert_eq!(result.npv.amount(), 2.0);
            assert_eq!(result.pv_std_error, 1.0);
            assert!((result.pv_confidence_interval.0 - (2.0 - critical)).abs() < 1e-10);
            assert!((result.pv_confidence_interval.1 - (2.0 + critical)).abs() < 1e-10);
        }
    }

    #[test]
    fn deal_pv_statistics_reject_insufficient_or_incomplete_estimators() {
        assert!(independent_estimator_stats(&[], false).is_err());
        assert!(independent_estimator_stats(&[1.0], false).is_err());
        assert!(independent_estimator_stats(&[1.0, 3.0], true).is_err());
        assert!(independent_estimator_stats(&[1.0, 3.0, 2.0], true).is_err());
    }

    /// Regression test: catastrophic cancellation in MC variance accumulation.
    ///
    /// The `E[X²] - E[X]²` form (`sq_sum / paths - mean * mean`) suffers
    /// catastrophic cancellation when `delta² ≪ ULP(mean²)`.  For `mean = 5e7`
    /// the ULP of `mean²` is `≈ 0.555` (since `2^−52 · (5e7)² ≈ 0.555`).
    /// When `delta = 0.05` (`delta² = 0.0025 ≪ 0.555`) the two terms in the
    /// subtraction are identical in f64, so the naive form returns **exactly
    /// zero**, collapsing `pv_std_error` to zero even though the true value is
    /// `0.05 / √999 ≈ 0.00158` after the sample-variance correction.
    ///
    /// This test drives `ScenarioCollector` directly with synthetic path outputs
    /// whose population variance is known exactly, then verifies that the
    /// computed `pv_std_error` is accurate.
    ///
    /// For the buggy `E[X²] - E[X]²` form the computed variance is exactly
    /// **0.0** (collapsed), making `pv_std_error = 0.0` even though the true
    /// variance is `0.0025`.  The Welford fix recovers the correct value.
    #[test]
    fn scenario_collector_variance_no_catastrophic_cancellation() {
        let instrument = test_instrument();
        let n = 1000usize;
        let mut collector =
            ScenarioCollector::new(&instrument, n, false, false).expect("collector");

        // Synthetic PVs: alternating mean ± delta where delta is tiny relative to mean.
        // True population variance = delta² = 0.0025.
        // True population std      = 0.05.
        //
        // At mean = 5e7, ULP(mean²) ≈ 0.555.  delta² = 0.0025 ≪ 0.555, so the
        // naive sq_sum/n − mean² subtraction cancels completely to 0.0 in f64.
        let mean_pv: f64 = 50_000_000.0; // $50 M — large enough for cancellation
        let delta: f64 = 0.05; // $0.05 spread → sigma/mean = 1e-9, delta² ≪ ULP(mean²)

        for i in 0..n {
            let pv = if i % 2 == 0 {
                mean_pv + delta
            } else {
                mean_pv - delta
            };
            // Feed as deal-level output (no tranche sub-paths needed here).
            collector.record_deal(pv, 0.0).expect("finite test pv");
        }

        // Extract the deal-level variance directly via the finalize path.
        let result = collector
            .finalize(StructuredCreditPricingMode::MonteCarlo {
                num_paths: n,
                antithetic: false,
            })
            .expect("valid pricing result");

        // True population variance = delta² = 0.0025
        // Bessel-corrected standard error = 0.05 / sqrt(999).
        let true_pop_var: f64 = delta * delta; // = 0.0025
        let true_std_error = true_pop_var.sqrt() / ((n - 1) as f64).sqrt();

        // The E[X²]-E[X]² form collapses sq_sum/n − mean² to exactly 0.0 here:
        // delta² = 0.0025 is below the ~0.555 ULP of mean², so the subtraction
        // rounds to zero, making pv_std_error = 0. The Welford form is immune.
        assert!(
            result.pv_std_error > 0.0,
            "pv_std_error must be strictly positive (true value ≈ {true_std_error:.8}); \
             got {}. Catastrophic cancellation in sq_sum/n - mean² collapses to 0 \
             when delta²={:.6} ≪ ULP(mean²)≈0.555.",
            result.pv_std_error,
            delta * delta,
        );

        // Relative error must be small (< 0.5%). The E[X²]-E[X]² form
        // produces 100% relative error here; Welford is accurate to rounding.
        let rel_err = (result.pv_std_error - true_std_error).abs() / true_std_error;
        assert!(
            rel_err < 0.005,
            "pv_std_error relative error {rel_err:.4} exceeds 0.5%: \
             computed={}, true={true_std_error:.8}. \
             This indicates the E[X²]-E[X]² form is being used instead of Welford.",
            result.pv_std_error
        );
    }

    /// W-23 — antithetic pairs are negatively-correlated draws, not i.i.d.
    /// samples; the deal-PV std-error must be computed over the `n/2` pair
    /// means, not over the `n` per-path values.
    ///
    /// Pathology: per-path PVs alternate `mean ± delta`. Each antithetic pair
    /// `(2k, 2k+1)` therefore averages to *exactly* `mean` — the pair-mean
    /// variance is zero even though the per-path population variance is
    /// `delta²`. The plain i.i.d. estimator `√(delta²/n)` is non-zero and
    /// hence wrong; the pair-aware estimator must report `SE = 0`.
    #[test]
    fn antithetic_std_error_uses_pair_means_not_per_path() {
        let pvs: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0e7 + 5.0 } else { 1.0e7 - 5.0 })
            .collect();

        // Plain i.i.d. estimator treats all 1000 paths as independent: it
        // sees per-path population variance = 25 and reports a non-zero SE.
        let iid_se = independent_estimator_stats(&pvs, false)
            .expect("valid observations")
            .get_stderr()
            .expect("finite stderr")
            .expect("1000 samples");
        assert!(
            iid_se > 0.0,
            "i.i.d. estimator should see per-path dispersion (got {iid_se})"
        );

        // Pair-aware estimator: every pair averages to exactly 1e7, so the
        // pair-mean variance — and therefore the SE — is zero.
        let pair_se = independent_estimator_stats(&pvs, true)
            .expect("complete pairs")
            .get_stderr()
            .expect("finite stderr")
            .expect("500 pairs");
        assert!(
            pair_se.abs() < 1e-9,
            "antithetic SE must be ~0 when every pair averages identically; \
             got {pair_se} (i.i.d. estimator would wrongly report {iid_se})"
        );

        // The two estimators must genuinely disagree on this pathology — the
        // whole point of the fix.
        assert!(
            iid_se > 1e-3,
            "the i.i.d. and pair-aware estimators must differ materially here"
        );
    }

    /// W-23 — the pair-aware std-error must match an independent recomputation
    /// over the pair-mean sample, and antithetic mode must not *increase* the
    /// reported estimator variance versus the i.i.d. interpretation when the
    /// pairs carry genuine negative correlation.
    #[test]
    fn antithetic_std_error_matches_pair_mean_recomputation() {
        // Negatively-correlated pairs: within each pair the two paths move in
        // opposite directions about a slowly-drifting pair mean. This is the
        // regime antithetic sampling targets.
        let n_pairs = 500usize;
        let mut pvs = Vec::with_capacity(2 * n_pairs);
        for k in 0..n_pairs {
            let pair_mean = 1.0e7 + (k as f64) * 0.01;
            let spread = 100.0; // large per-path swing, cancels within the pair
            pvs.push(pair_mean + spread);
            pvs.push(pair_mean - spread);
        }

        let reported = independent_estimator_stats(&pvs, true)
            .expect("complete pairs")
            .get_stderr()
            .expect("finite stderr")
            .expect("500 pairs");

        // Independent recomputation: collapse each pair to its mean, then take
        // the plain SE over the n/2 pair means.
        let pair_means: Vec<f64> = pvs.chunks(2).map(|p| (p[0] + p[1]) / 2.0).collect();
        let mean = pair_means.iter().sum::<f64>() / n_pairs as f64;
        let sample_var =
            pair_means.iter().map(|pv| (pv - mean).powi(2)).sum::<f64>() / (n_pairs - 1) as f64;
        let expected = (sample_var / n_pairs as f64).sqrt();
        // Welford and the two-pass reference accumulate different rounding
        // at a 1e7 PV level; require seven significant digits in the SE.
        assert!(
            (reported - expected).abs() / expected < 1e-7,
            "pair-aware SE {reported} must match pair-mean recomputation {expected}"
        );

        // The i.i.d. estimator sees the huge ±100 per-path swing and reports a
        // far larger SE; antithetic mode must NOT inflate variance beyond it.
        let iid = independent_estimator_stats(&pvs, false)
            .expect("valid observations")
            .get_stderr()
            .expect("finite stderr")
            .expect("1000 samples");
        assert!(
            reported <= iid,
            "antithetic SE {reported} must not exceed the i.i.d. SE {iid}"
        );
    }

    /// M2.16 — `evolved_factors` must implement the exact stationary AR(1)/OU
    /// recursion `Z_m = φ·Z_{m−1} + √(1−φ²)·ε_m` with `φ = e^{−κ/12}`.
    #[test]
    fn evolved_factors_applies_stationary_ar1_recursion() {
        let kappa = 1.5_f64;
        let phi = (-kappa / 12.0).exp();
        let scale = (1.0 - phi * phi).sqrt();
        let innovations = [0.3, -1.2, 0.7];

        let evolved = StochasticPricer::evolved_factors(&innovations, kappa);

        assert!((evolved[0] - 0.3).abs() < 1e-15, "Z_1 = ε_1");
        let expected_1 = phi * evolved[0] + scale * innovations[1];
        assert!((evolved[1] - expected_1).abs() < 1e-15);
        let expected_2 = phi * evolved[1] + scale * innovations[2];
        assert!((evolved[2] - expected_2).abs() < 1e-15);

        // Antithetic linearity: negated innovations give the negated path.
        let negated: Vec<f64> = innovations.iter().map(|e| -e).collect();
        let evolved_neg = StochasticPricer::evolved_factors(&negated, kappa);
        for (a, b) in evolved.iter().zip(&evolved_neg) {
            assert!((a + b).abs() < 1e-15, "AR(1) must commute with negation");
        }
    }

    /// Configured `mean_reversion` must change Monte Carlo path statistics.
    #[test]
    fn mean_reversion_changes_path_statistics() {
        let instrument = test_instrument();
        let market = MarketContext::new()
            .try_insert((*test_discount_curve()).clone())
            .expect("insert market curve");
        let price_with_kappa = |kappa: f64| {
            let mut tree_config = ScenarioTreeConfig::new(24);
            tree_config.factor_spec =
                LatentFactorSpec::single_factor(1.0, kappa).expect("valid model inputs");
            tree_config.default_spec =
                StochasticDefaultSpec::intensity_process(0.10, 1.0, 0.5, 0.8);
            let config = StochasticPricerConfig::new(
                test_date(),
                test_discount_curve(),
                tree_config,
                StructuredCreditPricingMode::MonteCarlo {
                    num_paths: 64,
                    antithetic: false,
                },
            );
            StochasticPricer::new(config)
                .price(&instrument, &market)
                .expect("MC price")
        };

        let iid = price_with_kappa(0.0);
        let persistent = price_with_kappa(2.0);

        assert!(
            (iid.npv.amount() - persistent.npv.amount()).abs() > 0.0
                || (iid.pv_std_error - persistent.pv_std_error).abs() > 0.0,
            "mean_reversion must change MC path statistics: \
             npv {} vs {}, std_error {} vs {}",
            iid.npv.amount(),
            persistent.npv.amount(),
            iid.pv_std_error,
            persistent.pv_std_error
        );
    }

    /// M2.13 — canonical sign convention invariant for the MC engine: under
    /// the shipped RMBS/CLO default and recovery calibrations, the two must
    /// co-move NEGATIVELY across systematic-factor realizations (stress = low
    /// factor ⇒ high MDR and low recovery).
    #[test]
    fn mc_engine_defaults_and_recoveries_co_move_negatively() {
        let mut rmbs = ScenarioTreeConfig::new(24);
        rmbs.default_spec = rmbs_default_spec();
        rmbs.recovery_spec =
            RecoverySpec::market_standard_stochastic().expect("valid model inputs");

        let mut clo = ScenarioTreeConfig::new(24);
        clo.default_spec = clo_default_spec();
        clo.recovery_spec = RecoverySpec::MarketCorrelated {
            mean_recovery: 0.40,
            recovery_volatility: 0.30,
            factor_correlation: 0.50,
        };

        for (label, tree_config) in [("rmbs", rmbs), ("clo", clo)] {
            let config = StochasticPricerConfig::new(
                test_date(),
                test_discount_curve(),
                tree_config,
                StructuredCreditPricingMode::default(),
            );
            let pricer = StochasticPricer::new(config);

            let zs = [-2.0, -1.0, 0.0, 1.0, 2.0];
            let deal = simple_deal("CO-MOVE-DEAL");
            let prepared = pricer
                .prepare_run(&deal, &MarketContext::new())
                .expect("prepared run");
            let shocks: Vec<_> = zs
                .iter()
                .map(|&z| {
                    pricer
                        .monthly_shock(
                            &prepared,
                            36,
                            z,
                            z,
                            &mut pricer.prepayment_path(&prepared, (0, false)),
                        )
                        .expect("valid monthly shock")
                })
                .collect();
            let mdrs: Vec<f64> = shocks.iter().map(|s| s.mdr).collect();
            let recoveries: Vec<f64> = shocks.iter().map(|s| s.recovery_rate).collect();

            let corr = pearson(&mdrs, &recoveries);
            assert!(
                corr < 0.0,
                "{label}: corr(MDR, recovery) across factor realizations must be \
                 negative, got {corr} (mdrs {mdrs:?}, recoveries {recoveries:?})"
            );
        }
    }

    fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
        let n = xs.len() as f64;
        let mean_x = xs.iter().sum::<f64>() / n;
        let mean_y = ys.iter().sum::<f64>() / n;
        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;
        for (x, y) in xs.iter().zip(ys) {
            cov += (x - mean_x) * (y - mean_y);
            var_x += (x - mean_x).powi(2);
            var_y += (y - mean_y).powi(2);
        }
        cov / (var_x.sqrt() * var_y.sqrt()).max(f64::MIN_POSITIVE)
    }
}

/// Tests for per-name copula default simulation (finite-pool Monte Carlo).
///
/// These exercise the path that replaced the pool-wide-MDR-applied-to-all-
/// names defect: the engine now realizes each pool asset's default
/// individually via the copula latent variable, with a documented LHP
/// fast-path for genuinely granular pools.
#[cfg(test)]
mod per_name_copula_tests {
    /// The expected-shortfall sort must use a TOTAL order.
    ///
    /// `partial_cmp(..).unwrap_or(Equal)` is not a total order when a NaN is
    /// present: NaN compares Equal to everything while the finite values keep
    /// their own ordering. Since Rust 1.81 the standard sort detects that and
    /// PANICS. A NaN can reach here via `convert_clamped`, so this was a live
    /// panic path in a pricing loop.
    ///
    /// Positive-sign NaNs are the representation emitted by the live
    /// `convert_clamped` corruption path. They must sort into the worst tail so
    /// the measure surfaces the corrupted path instead of returning a plausible
    /// finite value.
    #[test]
    fn expected_shortfall_places_nan_in_worst_tail() {
        let mut losses = vec![10.0, f64::NAN, 5.0, 100.0, 1.0, f64::NAN, 50.0];
        let es = expected_shortfall(&mut losses, 0.95);
        assert!(
            es.is_nan(),
            "a corrupted worst-tail path must surface as NaN, got {es}"
        );
        assert!(losses[..2].iter().all(|loss| loss.is_nan()));
        assert_eq!(&losses[2..], &[100.0, 50.0, 10.0, 5.0, 1.0]);
    }

    /// With clean input the measure is unchanged.
    #[test]
    fn expected_shortfall_is_unchanged_for_finite_losses() {
        let mut losses = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        // 95% confidence over 10 paths -> ceil(0.5) = 1 worst loss.
        let es = expected_shortfall(&mut losses, 0.95);
        assert!(
            (es - 10.0).abs() < 1e-12,
            "the 95% ES over 10 sorted losses is the single worst (10.0), got {es}"
        );
    }

    use super::*;
    use crate::structured_credit::pricing::stochastic::pricer::ScenarioTreeConfig;
    use crate::structured_credit::{AssetPool, DealType, PoolAsset, Tranche, TrancheStructure};
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_cashflows::builder::{DefaultModelSpec, RecoveryModelSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::credit::pool::PoolGranularity;
    use finstack_quant_valuations_core::results::structured_credit::TrancheSeniority;
    use time::Month;

    fn close() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2027, Month::January, 1).expect("valid date")
    }

    fn discount_curve() -> std::sync::Arc<DiscountCurve> {
        std::sync::Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(close())
                .knots([(0.0, 1.0), (1.0, 0.97), (3.0, 0.91), (5.0, 0.85)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("curve"),
        )
    }

    /// Build a CLO-style deal: `n_assets` identical fixed-rate loans summing
    /// to $100M, tranched senior (0-80%) / mezzanine (80-92%) / equity
    /// (92-100%). Larger `n_assets` ⇒ more granular pool.
    fn clo_deal(n_assets: usize) -> StructuredCredit {
        let total = 100_000_000.0;
        let per_asset = total / n_assets as f64;
        let mut pool = AssetPool::new("CLO-POOL", DealType::Abs, Currency::USD);
        for i in 0..n_assets {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("L{i}"),
                Money::new(per_asset, Currency::USD).expect("valid money fixture"),
                0.07,
                maturity(),
                DayCount::Thirty360,
            ));
        }
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(total * 0.80, Currency::USD).expect("valid money fixture"),
                RateSpec::Fixed { rate: 0.05 },
                maturity(),
            )
            .expect("senior"),
            Tranche::new(
                "MEZZ",
                80.0,
                92.0,
                TrancheSeniority::Mezzanine,
                Money::new(total * 0.12, Currency::USD).expect("valid money fixture"),
                RateSpec::Fixed { rate: 0.08 },
                maturity(),
            )
            .expect("mezz"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(total * 0.08, Currency::USD).expect("valid money fixture"),
                RateSpec::Fixed { rate: 0.0 },
                maturity(),
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut sc = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Abs,
            crate::structured_credit::StandardDealParams {
                id: "CLO-PER-NAME".into(),
                pool,
                tranches,
                closing_date: close(),
                maturity: maturity(),
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        sc
    }

    /// Build a pricer config with a Gaussian-copula default model.
    fn copula_config(
        base_cdr: f64,
        correlation: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        copula_config_with_spec(
            StochasticDefaultSpec::gaussian_copula(base_cdr, correlation),
            num_periods,
            granularity,
            num_paths,
        )
    }

    /// Build a copula config with antithetic Monte Carlo sampling enabled.
    fn antithetic_copula_config(
        base_cdr: f64,
        correlation: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        let mut tree_config = ScenarioTreeConfig::new(num_periods);
        tree_config.default_spec = StochasticDefaultSpec::gaussian_copula(base_cdr, correlation);
        StochasticPricerConfig::new(
            close(),
            discount_curve(),
            tree_config,
            StructuredCreditPricingMode::MonteCarlo {
                num_paths,
                antithetic: true,
            },
        )
        .with_pool_granularity(granularity)
    }

    /// Build a pricer config with a Student-t-copula default model.
    fn student_t_copula_config(
        base_cdr: f64,
        correlation: f64,
        degrees_of_freedom: f64,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        copula_config_with_spec(
            StochasticDefaultSpec::student_t_copula(base_cdr, correlation, degrees_of_freedom),
            num_periods,
            granularity,
            num_paths,
        )
    }

    /// Build a pricer config from an explicit copula default spec.
    fn copula_config_with_spec(
        default_spec: StochasticDefaultSpec,
        num_periods: usize,
        granularity: PoolGranularity,
        num_paths: usize,
    ) -> StochasticPricerConfig {
        let mut tree_config = ScenarioTreeConfig::new(num_periods);
        tree_config.default_spec = default_spec;
        StochasticPricerConfig::new(
            close(),
            discount_curve(),
            tree_config,
            StructuredCreditPricingMode::MonteCarlo {
                num_paths,
                antithetic: false,
            },
        )
        .with_pool_granularity(granularity)
    }

    fn tranche_pv(result: &StochasticPricingResult, id: &str) -> f64 {
        result
            .tranche_results
            .iter()
            .find(|t| t.tranche_id == id)
            .map(|t| t.npv.amount())
            .unwrap_or_else(|| panic!("tranche {id} missing"))
    }

    /// **LHP-limit parity** — the correctness anchor.
    ///
    /// A large, granular, homogeneous pool priced per-name must converge to
    /// the closed-form LHP result for the *same* pool: per-name → LHP as
    /// `N → ∞`. This both validates the per-name engine and shows the LHP
    /// fast-path is the genuine large-pool limit.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn large_granular_pool_per_name_converges_to_lhp() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        // 600 names ⇒ granular: per-name realized fraction ≈ LHP conditional.
        let deal = clo_deal(600);

        let per_name = StochasticPricer::new(copula_config(
            0.03,
            0.20,
            36,
            PoolGranularity::PerName,
            1_500,
        ))
        .price(&deal, &market)
        .expect("per-name pricing");
        let lhp = StochasticPricer::new(copula_config(
            0.03,
            0.20,
            36,
            PoolGranularity::LargeHomogeneous,
            1_500,
        ))
        .price(&deal, &market)
        .expect("LHP pricing");

        // Each tranche PV must agree within a few MC standard errors. The
        // per-name engine has its own idiosyncratic dispersion, but for a
        // 600-name pool that dispersion is small relative to the systematic
        // channel, so the means converge.
        for id in ["SR", "MEZZ", "EQ"] {
            let pn = tranche_pv(&per_name, id);
            let lh = tranche_pv(&lhp, id);
            let tol = (0.01 * lh.abs()).max(150_000.0);
            assert!(
                (pn - lh).abs() < tol,
                "{id}: per-name PV {pn:.0} should converge to LHP PV {lh:.0} \
                 (|diff|={:.0}, tol={tol:.0})",
                (pn - lh).abs()
            );
        }
    }

    /// Per-name simulation must be deterministic and bit-identical between
    /// repeated runs (seeded `PhiloxRng` substreams).
    #[test]
    fn per_name_pricing_is_deterministic() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(80);

        let run = || {
            StochasticPricer::new(copula_config(0.04, 0.25, 36, PoolGranularity::PerName, 500))
                .price(&deal, &market)
                .expect("per-name pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated per-name runs must produce bit-identical tranche PV"
            );
        }
    }

    /// Item 3 — per-name copula mask / asset-loop alignment.
    ///
    /// The default-indicator mask is sized by the builder from the
    /// performing-asset count at period start; the asset loop claims one
    /// entry per performing asset in pool-index order. With ≥2 defaults per
    /// period the loop mutates `defaulted` mid-iteration — the alignment
    /// must survive that. A misalignment is now a hard `Error` (the engine's
    /// pre-loop length guard), so a successfully-priced, deterministic run
    /// over a high-default scenario proves the mask stays index-aligned.
    ///
    /// This drives a concentrated pool with a high base CDR and high
    /// correlation so multiple names default in the same period across many
    /// paths; if the guard ever tripped the run would error rather than
    /// return a price.
    #[test]
    fn per_name_mask_stays_aligned_with_multiple_defaults_per_period() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        // 30 names ⇒ concentrated; high CDR + high correlation drives several
        // simultaneous defaults per period on a meaningful share of paths.
        let deal = clo_deal(30);

        let run = || {
            StochasticPricer::new(copula_config(
                0.12, // high base CDR
                0.45, // high correlation ⇒ clustered (multi-) defaults
                36,
                PoolGranularity::PerName,
                400,
            ))
            .price(&deal, &market)
            // `.expect` fails loudly if the mask-alignment guard ever errors.
            .expect("per-name pricing must not trip the mask-alignment guard")
        };
        let a = run();
        let b = run();

        // The run completed (guard never tripped) and is bit-reproducible.
        assert!(a.npv.amount().is_finite(), "priced NPV must be finite");
        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "per-name pricing under multi-default periods must be deterministic"
        );
        // The high-default scenario must actually realize losses — otherwise
        // the test would not be exercising the multi-default code path.
        let total_loss: f64 = a
            .tranche_results
            .iter()
            .map(|t| t.expected_loss.amount())
            .sum();
        assert!(
            total_loss > 0.0,
            "high-CDR per-name scenario must realize pool losses (got {total_loss}); \
             otherwise multi-default periods are not exercised"
        );
    }

    /// Item 10 — for a multi-month payment period the per-name copula
    /// systematic factor must aggregate ALL the period's monthly factors as
    /// `(Σ Zₘ)/√M`, not just the first month's. White-box test on
    /// `path_shocks_from_factors`: a quarterly deal with crafted month
    /// factors must produce a period systematic `Z` equal to the `√M`-scaled
    /// sum, so months 2..M are not silently ignored.
    #[test]
    fn multi_month_copula_systematic_factor_aggregates_all_months() {
        // Quarterly deal ⇒ 3 months per payment period.
        let mut deal = clo_deal(60);
        deal.frequency = finstack_quant_core::dates::Tenor::quarterly();

        let mut cfg = copula_config(
            0.05,
            0.30,
            12, // 12 monthly periods
            PoolGranularity::PerName,
            16,
        );
        // Fast mean reversion (κ = 6 ⇒ φ ≈ 0.61) so months within a period
        // stay distinct; at κ = 0 (φ = 1) later months equal month 1.
        cfg.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 6.0,
        };
        let pricer = StochasticPricer::new(cfg);

        // Craft 6 monthly factors covering two quarterly periods. The first
        // quarter is benign in month 1 but stressed in months 2 and 3 — a
        // month-1-only systematic factor would miss that stress entirely.
        let factors = vec![0.10_f64, -2.0, -1.5, 0.3, 0.4, 0.5];
        let prepared = pricer
            .prepare_run(&deal, &MarketContext::new())
            .expect("prepared run");
        let shocks = pricer
            .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
            .expect("path shocks");

        assert!(!shocks.is_empty(), "must produce at least one period shock");
        let period0 = shocks[0]
            .per_name
            .expect("per-name plan must be present for a copula deal");

        // Expected period factor: aggregate the evolved AR(1) path, not the
        // raw innovations, with the same scale the engine uses.
        let phi = pricer.factor_phi();
        let evolved = StochasticPricer::evolved_factors(&factors, pricer.factor_mean_reversion());
        let scale = StochasticPricer::period_factor_scale(3, phi);
        let expected_z = (evolved[0] + evolved[1] + evolved[2]) / scale;
        assert!(
            (period0.systematic_z - expected_z).abs() < 1e-9,
            "quarter-1 copula systematic factor {} must be the √M-scaled sum \
             of all 3 monthly factors ({expected_z}); a month-1-only factor \
             would be {}",
            period0.systematic_z,
            factors[0],
        );
        // The aggregated factor must be materially stressed (negative), unlike
        // the benign month-1 factor — proving months 2-3 are not ignored.
        assert!(
            period0.systematic_z < -1.0,
            "aggregated systematic factor {} must reflect the months-2-3 \
             stress, not the benign month-1 value {}",
            period0.systematic_z,
            factors[0],
        );
    }

    /// Aggregated period factor `Z_period = (Σ Zₘ)/s` stays `N(0,1)` at every κ.
    ///
    /// The copula barrier `Φ⁻¹(PD)` assumes a standard-normal factor, so
    /// `s` must equal `sqrt(Var(Σ Zₘ))` under the AR(1) autocorrelation.
    #[test]
    fn aggregated_period_factor_is_standard_normal_at_every_kappa() {
        const MONTHS: usize = 6;
        const PATHS: usize = 200_000;

        for kappa in [0.0_f64, 0.25, 1.0, 6.0, 50.0] {
            let phi = (-kappa / 12.0).exp();
            let scale = StochasticPricer::period_factor_scale(MONTHS, phi);

            let mut sum = 0.0_f64;
            let mut sum_sq = 0.0_f64;
            for path in 0..PATHS {
                let mut rng = PhiloxRng::new(20_260_719).substream(path as u64);
                let innovations: Vec<f64> = (0..MONTHS)
                    .map(|_| rng.next_std_normal().expect("normal draw"))
                    .collect();
                let evolved = StochasticPricer::evolved_factors(&innovations, kappa);
                let z_period: f64 = evolved.iter().sum::<f64>() / scale;
                sum += z_period;
                sum_sq += z_period * z_period;
            }

            let n = PATHS as f64;
            let mean = sum / n;
            let variance = sum_sq / n - mean * mean;

            assert!(
                mean.abs() < 0.02,
                "kappa={kappa}: aggregated period factor mean {mean:.4} must be ~0"
            );
            assert!(
                (variance - 1.0).abs() < 0.03,
                "kappa={kappa} (phi={phi:.4}): aggregated period factor variance \
                 {variance:.4} must be ~1, or the copula barrier Phi^-1(PD) is \
                 de-calibrated. Divisor used was {scale:.4}; the naive sqrt(M) \
                 would be {:.4}.",
                (MONTHS as f64).sqrt()
            );
        }
    }

    /// Persistent systematic factor (κ = 0) widens loss dispersion vs i.i.d.
    /// months (large κ), holding asset correlation fixed.
    ///
    /// Under the canonical single-factor copula one systematic draw governs the
    /// horizon, so defaults cluster; independent monthly factors dilute that.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn persistent_systematic_factor_widens_the_loss_distribution() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(60);

        let unexpected_loss_for = |kappa: f64| -> f64 {
            let mut cfg = copula_config(0.06, 0.30, 36, PoolGranularity::PerName, 6_000);
            cfg.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
                volatility: 1.0,
                mean_reversion: kappa,
            };
            StochasticPricer::new(cfg)
                .price(&deal, &market)
                .expect("per-name copula pricing")
                .unexpected_loss
                .amount()
        };

        // kappa = 0  -> phi = 1     -> one systematic draw for the horizon.
        // kappa = 600 -> phi ~ 0    -> independent monthly factors (the bug).
        let persistent = unexpected_loss_for(0.0);
        let independent = unexpected_loss_for(600.0);

        assert!(
            persistent > independent * 1.10,
            "persistent UL {persistent:.0} should exceed i.i.d. UL \
             {independent:.0} by more than 10%"
        );
    }

    /// Explicit deal `CorrelationStructure` override moves unexpected loss.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn explicit_correlation_structure_changes_the_price() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(60);

        let unexpected_loss_for = |override_rho: Option<f64>| -> f64 {
            let mut cfg = copula_config(0.06, 0.10, 36, PoolGranularity::PerName, 4_000);
            cfg.tree_config.asset_correlation_override = override_rho;
            StochasticPricer::new(cfg)
                .price(&deal, &market)
                .expect("per-name copula pricing")
                .unexpected_loss
                .amount()
        };

        // No override: the copula spec's own 10% correlation applies.
        let baseline = unexpected_loss_for(None);
        // An explicit structure at 45% must widen the loss distribution.
        let high = unexpected_loss_for(Some(0.45));

        assert!(
            high > baseline * 1.10,
            "UL at 45% override ({high:.0}) must exceed baseline at 10% \
             ({baseline:.0}) by more than 10%"
        );
    }

    /// The intensity model's `mean_reversion` must drive the factor.
    ///
    /// `IntensityProcessDefault` documents `dX = kappa(theta - X)dt + sigma dW`
    /// and cites Duffie-Singleton, but `kappa` was stored, clamped, exposed by
    /// a getter and used in NO computation — the model was a static lognormal
    /// shock. The systematic factor is now a genuine OU path, so
    /// sourcing its kappa from this spec realizes the documented model.
    #[test]
    fn intensity_mean_reversion_drives_the_systematic_factor() {
        let mut deal = clo_deal(20);
        deal.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::intensity_process(
            0.03, 0.8, 2.5, 0.4,
        ));

        let as_of = deal.closing_date;
        let tree_config = deal
            .build_scenario_tree_config(as_of)
            .expect("scenario config");

        match tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { mean_reversion, .. } => assert!(
                (mean_reversion - 2.5).abs() < 1e-12,
                "the factor's kappa must come from the intensity spec (2.5), \
                 got {mean_reversion}. Zero means the parameter is still inert \
                ."
            ),
            other => panic!("expected a single-factor spec, got {other:?}"),
        }
    }

    /// Regression: `TwoFactor { correlation: 0.0 }` means the prepayment factor
    /// is INDEPENDENT of credit. Collapsing it into the single-factor arm
    /// (`|rho| <= f64::EPSILON` returning `None`) would share one factor —
    /// implied correlation +1, the exact opposite of what was asked for.
    #[test]
    fn zero_correlation_is_independent_not_shared() {
        let mut two = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        two.tree_config.factor_spec =
            LatentFactorSpec::two_factor(0.20, 0.25, 0.0).expect("valid model inputs");
        assert_eq!(
            StochasticPricer::new(two)
                .factor_correlation()
                .expect("two-factor spec is supported"),
            Some(0.0),
            "rho = 0 must stay a two-factor spec, not collapse to single-factor"
        );

        let mut single = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        single.tree_config.factor_spec = LatentFactorSpec::SingleFactor {
            volatility: 0.20,
            mean_reversion: 0.0,
        };
        assert_eq!(
            StochasticPricer::new(single)
                .factor_correlation()
                .expect("single-factor spec is supported"),
            None,
            "only SingleFactor shares the credit factor"
        );
    }

    /// Prepayment and default must be driven by SEPARATE factors
    /// correlated at the configured level.
    ///
    /// `monthly_shock` built `let factors = [factor]` — one scalar handed to
    /// both `conditional_smm` and `conditional_mdr` — so the implied
    /// prepay/default correlation was +/-1 regardless of the -0.30 configured
    /// in the shipped RMBS/CLO calibrations, so a user calibrating a two-factor
    /// model silently got a single-factor one.
    ///
    /// The correlation is measured ACROSS PATHS, not across months. Under the
    /// canonical single-draw copula (kappa = 0 is a random walk) each
    /// path's factor is constant over the horizon, so a time-series
    /// correlation is degenerate — any two constant series correlate at 1.
    /// Cross-sectional is also the quantity that matters: it is the dependence
    /// between the two channels' realizations.
    #[test]
    fn prepay_factor_is_correlated_not_identical_to_the_credit_factor() {
        const RHO: f64 = -0.30;
        const PATHS: usize = 20_000;

        let mut cfg = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        cfg.tree_config.factor_spec =
            LatentFactorSpec::two_factor(0.20, 0.25, RHO).expect("valid model inputs");
        let pricer = StochasticPricer::new(cfg);

        let rho = pricer
            .factor_correlation()
            .expect("factor spec is supported")
            .expect("a two-factor spec must expose its correlation");
        assert!(
            (rho - RHO).abs() < 1e-12,
            "the configured correlation must reach the engine, got {rho}"
        );
        let scale = (1.0 - rho * rho).sqrt();

        // Reconstruct each path's period-0 credit and prepay factor exactly as
        // the engine builds them.
        let (mut sum_c, mut sum_p, mut sum_cc, mut sum_pp, mut sum_cp) =
            (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        for path in 0..PATHS as u64 {
            let mut credit_rng = PhiloxRng::new(pricer.config.tree_config.seed).substream(path);
            let credit = credit_rng.next_std_normal().expect("normal draw");

            let mut indep_rng =
                PhiloxRng::new(pricer.config.tree_config.seed ^ PREPAY_FACTOR_SEED_SALT)
                    .substream(path);
            let indep = indep_rng.next_std_normal().expect("normal draw");

            let prepay = rho * credit + scale * indep;

            sum_c += credit;
            sum_p += prepay;
            sum_cc += credit * credit;
            sum_pp += prepay * prepay;
            sum_cp += credit * prepay;
        }

        let n = PATHS as f64;
        let mean_c = sum_c / n;
        let mean_p = sum_p / n;
        let var_c = sum_cc / n - mean_c * mean_c;
        let var_p = sum_pp / n - mean_p * mean_p;
        let cov = sum_cp / n - mean_c * mean_p;
        let realized = cov / (var_c.sqrt() * var_p.sqrt());

        assert!(
            (realized - RHO).abs() < 0.02,
            "realized prepay/default factor correlation {realized:.4} must be \
             near the configured {RHO}. A value near +/-1 means both channels \
             still share one factor."
        );
        assert!(
            (var_p - 1.0).abs() < 0.05,
            "the prepay factor must keep a unit-variance marginal so the \
             prepayment model's calibration is untouched, got {var_p:.4}"
        );

        // The formula above is only meaningful if the ENGINE actually consumes
        // the second factor. Same deal, same seed, same credit factors — only
        // the factor spec differs, so any difference in realized prepayment
        // proves the prepay channel is reading its own series.
        let deal = clo_deal(40);
        let smm_for = |spec: LatentFactorSpec| -> f64 {
            let mut cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 16);
            cfg.tree_config.factor_spec = spec;
            // A STOCHASTIC prepay model, or the factor is ignored entirely and
            // the comparison is vacuous.
            cfg.tree_config.prepay_spec = rmbs_prepay_spec(0.06);
            let pricer = StochasticPricer::new(cfg);
            let prepared = pricer
                .prepare_run(&deal, &MarketContext::new())
                .expect("prepared run");
            let factors: Vec<f64> = (0..24).map(|m| ((m as f64) * 0.37).sin()).collect();
            pricer
                .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
                .expect("path shocks")
                .iter()
                .map(|s| s.smm)
                .sum()
        };

        let two_factor =
            smm_for(LatentFactorSpec::two_factor(0.20, 0.25, RHO).expect("valid model inputs"));
        let single_factor = smm_for(LatentFactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 0.0,
        });
        assert!(
            (two_factor - single_factor).abs() > 1e-9,
            "a two-factor spec must produce different prepayment from a \
             single-factor one on identical credit factors: {two_factor} vs \
             {single_factor}. Equal values mean the engine is still handing one \
             scalar to both channels."
        );
    }

    /// Antithetic pairs `(2k, 2k+1)` negate the credit factors, so the
    /// pair's prepayment factors `rho * Z_credit + sqrt(1 - rho^2) * Z_indep`
    /// must be exact negatives too: the independent draw has to come from
    /// the pair's shared substream and be negated on the odd member. Drawing
    /// it from `substream(path_index)` left the pair's prepayment channel
    /// independent.
    #[test]
    fn antithetic_pair_prepayment_factors_sum_to_zero() {
        let mut cfg = copula_config(0.06, 0.20, 12, PoolGranularity::PerName, 16);
        cfg.tree_config.factor_spec =
            LatentFactorSpec::two_factor(0.20, 0.25, -0.30).expect("valid model inputs");
        let pricer = StochasticPricer::new(cfg);
        let prepared = pricer
            .prepare_run(&clo_deal(10), &MarketContext::new())
            .expect("prepared run");
        let credit: Vec<f64> = (0..12).map(|m| ((m as f64) * 0.61).cos()).collect();
        let negated: Vec<f64> = credit.iter().map(|z| -z).collect();
        for pair in [0usize, 3] {
            let first = pricer
                .prepay_factors(&credit, (2 * pair, true), &prepared)
                .expect("normal draws")
                .expect("two-factor spec");
            let second = pricer
                .prepay_factors(&negated, (2 * pair + 1, true), &prepared)
                .expect("normal draws")
                .expect("two-factor spec");
            for (a, b) in first.iter().zip(&second) {
                assert!(a.abs() > 0.0);
                assert!((a + b).abs() < 1e-15, "pair {pair}: {a} + {b} != 0");
            }
        }
        // Without antithetic pairing, neighbouring paths stay independent.
        let first = pricer
            .prepay_factors(&credit, (0, false), &prepared)
            .expect("normal draws")
            .expect("two-factor spec");
        let second = pricer
            .prepay_factors(&negated, (1, false), &prepared)
            .expect("normal draws")
            .expect("two-factor spec");
        assert!(first.iter().zip(&second).any(|(a, b)| (a + b).abs() > 1e-3));
    }

    /// Burnout must ACCUMULATE across a path, not sit at 1.0.
    ///
    /// `update_burnout` had zero production callers and the engine passed a
    /// hard-coded 1.0, so the burnout channel of Richard-Roll was inert:
    /// seasoned pools that had already refinanced heavily were modelled with
    /// full prepayment propensity, while `has_burnout()` returned true.
    ///
    /// Burnout is PATH state — it accumulates as realized prepayment runs
    /// above or below expectation — so it must be threaded through the month
    /// loop rather than recomputed per month.
    #[test]
    fn burnout_accumulates_across_a_path() {
        let deal = clo_deal(60);
        let mut cfg = copula_config(0.06, 0.20, 36, PoolGranularity::PerName, 16);
        // Richard-Roll with a live burnout rate.
        cfg.tree_config.prepay_spec = rmbs_prepay_spec(0.06);
        let pricer = StochasticPricer::new(cfg);

        // Drive a strongly-prepaying path so realized runs above expected.
        let prepared = pricer
            .prepare_run(&deal, &MarketContext::new())
            .expect("prepared run");
        let factors: Vec<f64> = (0..24).map(|_| 1.5_f64).collect();
        let mut prepayment = pricer.prepayment_path(&prepared, (0, false));
        let _ = pricer
            .path_shocks_from_factors(&deal, &factors, (0, false), &prepared)
            .expect("path shocks");

        // Exercise the month loop directly so the state is observable.
        for month in 1..=24u32 {
            let _ = pricer
                .monthly_shock(&prepared, month, 1.5, 1.5, &mut prepayment)
                .expect("valid monthly shock");
        }

        let burnout = prepayment.burnout;
        assert!(
            burnout < 1.0,
            "after 24 months of above-expectation prepayment the burnout factor \
             must have decayed below 1.0, got {burnout}. A value pinned at 1.0 \
             means the channel is still inert."
        );
        assert!(burnout > 0.0, "burnout must stay in (0, 1], got {burnout}");
    }

    #[test]
    fn regime_switching_paths_preserve_monthly_markov_transitions() {
        let deal = clo_deal(24);
        let mut cfg = copula_config(0.06, 0.20, 24, PoolGranularity::PerName, 16);
        cfg.tree_config.seasoning_months = 0;
        cfg.tree_config.prepay_spec = StochasticPrepaySpec::RegimeSwitching {
            low_cpr: 0.04,
            high_cpr: 0.80,
            transition_up: 0.10,
            transition_down: 0.20,
            factor_loading: 0.4,
            cpr_volatility: 0.0,
        };
        let pricer = StochasticPricer::new(cfg);
        let prepared = pricer
            .prepare_run(&deal, &MarketContext::new())
            .expect("prepared run");
        let low = 1.0 - 0.96_f64.powf(1.0 / 12.0);
        let high = 1.0 - 0.20_f64.powf(1.0 / 12.0);
        let mut first_sum = 0.0;
        let mut high_months = 0;
        let mut high_to_high = 0;
        let paths = 4_000;
        for path_index in 0..paths {
            let mut prepayment = pricer.prepayment_path(&prepared, (path_index, false));
            let mut previous_high = false;
            for month in 1..=12 {
                let smm = pricer
                    .monthly_shock(&prepared, month, 0.0, 0.0, &mut prepayment)
                    .expect("valid monthly shock")
                    .smm;
                assert!((smm - low).abs() < 1e-14 || (smm - high).abs() < 1e-14);
                let current_high = (smm - high).abs() < 1e-14;
                if month == 1 {
                    first_sum += smm;
                } else if previous_high {
                    high_months += 1;
                    high_to_high += usize::from(current_high);
                }
                previous_high = current_high;
            }
        }
        let first_mean = first_sum / paths as f64;
        assert!((first_mean - (0.9 * low + 0.1 * high)).abs() < 0.002);
        assert!((high_to_high as f64 / high_months as f64 - 0.8).abs() < 0.02);

        // Path-indexed streams reproduce exactly and antithetic uniforms
        // remain complementary, including the initialization draw.
        let mut first = pricer.prepayment_path(&prepared, (8, true));
        let mut repeated = pricer.prepayment_path(&prepared, (8, true));
        let mut paired = pricer.prepayment_path(&prepared, (9, true));
        for _ in 0..12 {
            let uniform = first.next_uniform();
            assert_eq!(uniform, repeated.next_uniform());
            assert_eq!(uniform + paired.next_uniform(), 1.0);
        }
    }

    /// `period_factor_scale` hits its analytic limits at φ = 0 and φ = 1.
    #[test]
    fn period_factor_scale_matches_its_analytic_limits() {
        let m = 6usize;
        // phi = 0 (independent months): Var(sum) = M, so s = sqrt(M).
        let independent = StochasticPricer::period_factor_scale(m, 0.0);
        assert!(
            (independent - (m as f64).sqrt()).abs() < 1e-12,
            "at phi=0 the divisor must be sqrt(M)={}, got {independent}",
            (m as f64).sqrt()
        );
        // phi = 1 (fully persistent): Var(sum) = M^2, so s = M and Z_period = Z_1.
        let persistent = StochasticPricer::period_factor_scale(m, 1.0);
        assert!(
            (persistent - m as f64).abs() < 1e-9,
            "at phi=1 the divisor must be M={m}, got {persistent}"
        );
        // Single-month periods are the identity under either convention.
        assert!((StochasticPricer::period_factor_scale(1, 0.5) - 1.0).abs() < 1e-12);
    }

    /// Item 5 — antithetic per-name pricing must be deterministic and produce
    /// a finite, sensible result. The per-name idiosyncratic substreams are
    /// now paired antithetically (paired paths share a substream; the odd
    /// member negates `εᵢ`), so repeated runs must stay bit-identical.
    #[test]
    fn antithetic_per_name_pricing_is_deterministic() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(60);

        let run = || {
            StochasticPricer::new(antithetic_copula_config(
                0.05,
                0.30,
                36,
                PoolGranularity::PerName,
                200, // 200 estimators ⇒ 400 simulated antithetic paths
            ))
            .price(&deal, &market)
            .expect("antithetic per-name pricing")
        };
        let a = run();
        let b = run();

        assert!(a.npv.amount().is_finite(), "antithetic NPV must be finite");
        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated antithetic per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated antithetic per-name runs must produce \
                 bit-identical tranche PV"
            );
        }
        // The reported MC confidence interval must be a valid interval.
        let (lo, hi) = a.pv_confidence_interval;
        assert!(
            lo.is_finite() && hi.is_finite() && lo <= hi,
            "antithetic CI must be a valid finite interval: ({lo}, {hi})"
        );
    }

    /// Concentrated pools must carry strictly more loss dispersion than
    /// granular pools under per-name simulation: with fewer names, the same
    /// correlation, name-level lumpiness fattens the loss tail.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn concentration_increases_loss_dispersion() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");

        let granular = StochasticPricer::new(copula_config(
            0.05,
            0.20,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(600), &market)
        .expect("granular per-name pricing");
        let concentrated = StochasticPricer::new(copula_config(
            0.05,
            0.20,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(40), &market)
        .expect("concentrated per-name pricing");

        assert!(
            concentrated.unexpected_loss.amount() > granular.unexpected_loss.amount(),
            "concentrated pool (40 names) loss dispersion {:.0} must exceed \
             granular pool (600 names) dispersion {:.0}",
            concentrated.unexpected_loss.amount(),
            granular.unexpected_loss.amount()
        );
    }

    #[test]
    fn recovery_uses_raw_period_mean_while_default_uses_standardized_factor() {
        let mut config = copula_config(0.05, 0.3, 36, PoolGranularity::PerName, 20);
        config.tree_config.recovery_spec =
            RecoverySpec::market_correlated(0.4, 0.25, 0.4).expect("spec");
        config.tree_config.factor_spec =
            LatentFactorSpec::single_factor(1.0, 20.0).expect("factor");
        let pricer = StochasticPricer::new(config);
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(40);
        let prepared = pricer.prepare_run(&deal, &market).expect("prepared");
        let factors = [-1.0, 0.0, 0.4];
        let mut prepayment = pricer.prepayment_path(&prepared, (0, false));
        let shock = pricer
            .aggregate_monthly_shocks(&prepared, 0, &factors, &factors, &mut prepayment)
            .expect("shock");
        let mean = -0.2;
        assert!((shock.recovery_factor - mean).abs() < 1e-15);
        assert_eq!(
            shock.recovery_rate,
            prepared
                .recovery_model
                .conditional_recovery(shock.recovery_factor)
                .expect("mean")
        );
        assert!((pricer.period_systematic_z(&prepared, &factors) - mean).abs() > 0.1);
    }

    #[test]
    fn stochastic_recovery_is_repeatable_and_serial_parallel_identical() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(80);
        for mut config in [
            copula_config(0.05, 0.3, 36, PoolGranularity::PerName, 128),
            student_t_copula_config(0.05, 0.3, 6.0, 36, PoolGranularity::PerName, 128),
        ] {
            config.tree_config.recovery_spec =
                RecoverySpec::market_correlated(0.4, 0.25, 0.4).expect("spec");
            let pricer = StochasticPricer::new(config);
            let run = |threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("pool")
                    .install(|| {
                        serde_json::to_value(pricer.price(&deal, &market).expect("price"))
                            .expect("result")
                    })
            };
            assert_eq!(run(1), run(4));
            assert_eq!(run(4), run(4));
        }
    }

    #[test]
    #[ignore = "slow: stochastic recovery finite-pool/LHP convergence"]
    fn stochastic_recovery_large_pool_converges_to_lhp() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(600);
        for student_t in [false, true] {
            let run = |granularity| {
                let mut config = if student_t {
                    student_t_copula_config(0.05, 0.3, 6.0, 36, granularity, 2_000)
                } else {
                    copula_config(0.05, 0.3, 36, granularity, 2_000)
                };
                config.tree_config.recovery_spec =
                    RecoverySpec::market_correlated(0.4, 0.25, 0.4).expect("spec");
                StochasticPricer::new(config)
                    .price(&deal, &market)
                    .expect("price")
            };
            let finite = run(PoolGranularity::PerName);
            let lhp = run(PoolGranularity::LargeHomogeneous);
            let expected = deal_credit_loss(&lhp);
            assert!(
                (deal_credit_loss(&finite) - expected).abs()
                    < (0.05 * expected.abs()).max(250_000.0)
            );
            for id in ["SR", "MEZZ", "EQ"] {
                let reference = tranche_pv(&lhp, id);
                assert!(
                    (tranche_pv(&finite, id) - reference).abs()
                        < (0.015 * reference.abs()).max(250_000.0)
                );
            }
        }
    }

    /// Per-name idiosyncratic recovery dispersion widens deal-loss variance.
    ///
    /// The constant model is calibrated to the stochastic model mean; only
    /// idiosyncratic scatter differs (`ρ_R = 0`). Asset correlation is kept
    /// low (2%) so the systematic factor does not swamp the channel under test.
    #[test]
    fn per_name_recovery_dispersion_widens_loss_distribution() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(40); // concentrated pool — name-level scatter shows

        let mut constant_cfg = copula_config(0.06, 0.02, 36, PoolGranularity::PerName, 4_000);
        constant_cfg.tree_config.recovery_spec = RecoverySpec::Constant { rate: 0.40 };

        let mut dispersed_cfg = copula_config(0.06, 0.02, 36, PoolGranularity::PerName, 4_000);
        // Zero correlation gives a constant conditional mean, with residual
        // obligor variation around that mean.
        dispersed_cfg.tree_config.recovery_spec = RecoverySpec::market_correlated(0.40, 0.30, 0.0)
            .expect("valid dispersed recovery inputs");

        constant_cfg.tree_config.recovery_spec = RecoverySpec::constant(
            dispersed_cfg
                .tree_config
                .recovery_spec
                .build()
                .expect("model")
                .expected_recovery(),
        )
        .expect("matched mean");

        let constant = StochasticPricer::new(constant_cfg)
            .price(&deal, &market)
            .expect("constant-recovery per-name pricing");
        let dispersed = StochasticPricer::new(dispersed_cfg)
            .price(&deal, &market)
            .expect("dispersed-recovery per-name pricing");

        assert!(
            dispersed.unexpected_loss.amount() > constant.unexpected_loss.amount(),
            "per-name recovery dispersion must widen the loss distribution: \
             dispersed UL {:.0} should exceed constant-recovery UL {:.0}",
            dispersed.unexpected_loss.amount(),
            constant.unexpected_loss.amount(),
        );
    }

    /// Sum of realized pool credit losses across all tranches, used as a
    /// granularity-independent proxy for the pool's total default experience.
    fn deal_credit_loss(result: &StochasticPricingResult) -> f64 {
        result
            .tranche_results
            .iter()
            .map(|t| t.expected_loss.amount())
            .sum()
    }

    /// Student-t per-name pricing converges to LHP for a large granular pool.
    ///
    /// LHP must condition on the Student-t factor `M = Z/√W`, not the Gaussian `Z`.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn student_t_large_granular_pool_per_name_converges_to_lhp() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(600);

        let per_name = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.30,
            6.0,
            36,
            PoolGranularity::PerName,
            2_000,
        ))
        .price(&deal, &market)
        .expect("Student-t per-name pricing");
        let lhp = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.30,
            6.0,
            36,
            PoolGranularity::LargeHomogeneous,
            2_000,
        ))
        .price(&deal, &market)
        .expect("Student-t LHP pricing");

        let loss_pn = deal_credit_loss(&per_name);
        let loss_lhp = deal_credit_loss(&lhp);
        let loss_tol = (0.05 * loss_pn.abs()).max(250_000.0);
        assert!(
            (loss_pn - loss_lhp).abs() < loss_tol,
            "Student-t per-name credit loss {loss_pn:.0} should converge to \
             LHP credit loss {loss_lhp:.0} (|diff|={:.0}, tol={loss_tol:.0})",
            (loss_pn - loss_lhp).abs()
        );

        for id in ["SR", "MEZZ", "EQ"] {
            let pn = tranche_pv(&per_name, id);
            let lh = tranche_pv(&lhp, id);
            let tol = (0.015 * lh.abs()).max(250_000.0);
            assert!(
                (pn - lh).abs() < tol,
                "{id}: Student-t per-name PV {pn:.0} should converge to LHP \
                 PV {lh:.0} (|diff|={:.0}, tol={tol:.0})",
                (pn - lh).abs()
            );
        }
    }

    /// Student-t copula default tail dependence: at a fixed correlation, a
    /// concentrated pool priced per-name with a Student-t copula must carry
    /// strictly more loss dispersion than a granular one — the per-name
    /// engine must work end-to-end for the Student-t copula, not just
    /// Gaussian.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn student_t_concentration_increases_loss_dispersion() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");

        let granular = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.25,
            6.0,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(600), &market)
        .expect("Student-t granular per-name pricing");
        let concentrated = StochasticPricer::new(student_t_copula_config(
            0.05,
            0.25,
            6.0,
            36,
            PoolGranularity::PerName,
            3_000,
        ))
        .price(&clo_deal(40), &market)
        .expect("Student-t concentrated per-name pricing");

        assert!(
            concentrated.unexpected_loss.amount() > granular.unexpected_loss.amount(),
            "Student-t concentrated pool (40 names) loss dispersion {:.0} \
             must exceed granular pool (600 names) dispersion {:.0}",
            concentrated.unexpected_loss.amount(),
            granular.unexpected_loss.amount()
        );
    }

    /// Student-t per-name pricing must be deterministic and bit-identical
    /// between repeated runs — the shared mixing `W` is drawn from a seeded
    /// per-path stream.
    #[test]
    fn student_t_per_name_pricing_is_deterministic() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(80);

        let run = || {
            StochasticPricer::new(student_t_copula_config(
                0.04,
                0.25,
                6.0,
                36,
                PoolGranularity::PerName,
                500,
            ))
            .price(&deal, &market)
            .expect("Student-t per-name pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated Student-t per-name MC runs must be bit-identical"
        );
        for id in ["SR", "MEZZ", "EQ"] {
            assert_eq!(
                tranche_pv(&a, id),
                tranche_pv(&b, id),
                "{id}: repeated Student-t per-name runs must produce \
                 bit-identical tranche PV"
            );
        }
    }

    /// Student-t LHP pricing must be deterministic: the LHP fast-path now
    /// draws a shared mixing `W` per period from the seeded per-path stream,
    /// so repeated runs must stay bit-identical.
    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn student_t_lhp_pricing_is_deterministic() {
        let market = MarketContext::new()
            .try_insert((*discount_curve()).clone())
            .expect("insert market curve");
        let deal = clo_deal(600);

        let run = || {
            StochasticPricer::new(student_t_copula_config(
                0.04,
                0.25,
                6.0,
                36,
                PoolGranularity::LargeHomogeneous,
                500,
            ))
            .price(&deal, &market)
            .expect("Student-t LHP pricing")
        };
        let a = run();
        let b = run();

        assert_eq!(
            a.npv.amount(),
            b.npv.amount(),
            "repeated Student-t LHP MC runs must be bit-identical"
        );
    }
}
