use super::credit_pv::*;
use super::periods::*;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;

use indexmap::IndexMap;

mod period_aggregation_tests {
    use super::*;

    #[test]
    fn exact_period_lookup_preserves_empty_buckets_and_currency_totals() {
        let totals: PeriodAggregation = serde_json::from_str(
            r#"{"2025Q1":{"USD":{"amount":"100.25","currency":"USD"},"EUR":{"amount":"40","currency":"EUR"}},"2025Q2":{}}"#,
        ).expect("valid aggregation");
        let first = totals.get_period("2025Q1").expect("present period");
        assert_eq!(first.len(), 2);
        assert_eq!(
            totals.get_amount("2025Q1", Currency::USD),
            first.get(&Currency::USD).copied()
        );
        assert!(
            totals
                .get_period("2025Q2")
                .expect("present empty period")
                .is_empty()
        );
        assert_eq!(totals.get_amount("2025Q2", Currency::USD), None);
        for missing in ["2025Q3", "2025q1", " 2025Q1", "nope", ""] {
            assert!(
                totals.get_period(missing).is_none(),
                "unexpected match for {missing:?}"
            );
        }
    }

    #[test]
    fn checked_map_construction_rejects_currency_mismatch() {
        let period = PeriodId::quarter(2025, 1).expect("valid period");
        let mut per_currency = IndexMap::new();
        per_currency.insert(Currency::USD, Money::from((100_i64, Currency::EUR)));
        let mut map = IndexMap::new();
        map.insert(period, per_currency);

        assert!(matches!(
            PeriodAggregation::try_from(map),
            Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: Currency::USD,
                actual: Currency::EUR,
            })
        ));
    }

    #[test]
    fn serde_rejects_currency_mismatch() {
        let wire = r#"{"2025Q1":{"USD":{"amount":"100","currency":"EUR"}}}"#;
        let error = serde_json::from_str::<PeriodAggregation>(wire)
            .expect_err("mislabeled currency must fail");
        assert!(error.to_string().contains("USD"));
        assert!(error.to_string().contains("EUR"));
    }

    #[test]
    fn checked_map_and_serde_preserve_valid_currency_totals() {
        let period = PeriodId::quarter(2025, 1).expect("valid period");
        let mut per_currency = IndexMap::new();
        per_currency.insert(Currency::USD, Money::from((100_i64, Currency::USD)));
        per_currency.insert(Currency::EUR, Money::from((200_i64, Currency::EUR)));
        let mut map = IndexMap::new();
        map.insert(period, per_currency);
        let totals = PeriodAggregation::try_from(map).expect("matching currencies");
        let wire = serde_json::to_string(&totals).expect("serialize totals");
        let restored: PeriodAggregation = serde_json::from_str(&wire).expect("valid totals");

        assert_eq!(restored, totals);
        assert_eq!(
            restored.rows(),
            vec![
                (period, Currency::USD, Money::from((100_i64, Currency::USD))),
                (period, Currency::EUR, Money::from((200_i64, Currency::EUR))),
            ]
        );
    }
}

mod period_contract_tests {
    use super::*;
    use finstack_quant_core::cashflow::CashFlowParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, Period, PeriodId};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    #[test]
    fn get_amount_reads_one_period_and_currency_and_misses_quietly() {
        let periods = vec![Period {
            id: PeriodId::quarter(2025, 1).expect("valid period"),
            start: d(2025, 1, 1),
            end: d(2025, 4, 1),
            is_actual: true,
        }];
        let flows = vec![
            (d(2025, 2, 15), Money::from((100_i64, Currency::USD))),
            (d(2025, 3, 15), Money::from((40_i64, Currency::USD))),
        ];
        let aggregation = aggregate_by_period(&flows, &periods).expect("aggregates");

        assert_eq!(
            aggregation.get_amount("2025Q1", Currency::USD),
            Some(Money::from((140_i64, Currency::USD)))
        );
        assert_eq!(aggregation.get_amount("2025Q1", Currency::EUR), None);
        assert_eq!(aggregation.get_amount("2025Q2", Currency::USD), None);
        assert_eq!(aggregation.get_amount("not a period", Currency::USD), None);
    }

    fn period(id: PeriodId, start: Date, end: Date) -> Period {
        Period {
            id,
            start,
            end,
            is_actual: true,
        }
    }

    struct UnitDiscount {
        base: Date,
    }

    impl Discounting for UnitDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "unit".into());
            &ID
        }

        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> finstack_quant_core::Result<f64> {
            Ok(1.0)
        }
        fn instantaneous_forward(&self, _t: f64) -> finstack_quant_core::Result<f64> {
            Ok(0.0)
        }
    }

    /// Non-conforming discount curve that returns NaN for any positive time point.
    struct NanDiscount {
        base: Date,
    }

    impl Discounting for NanDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "nan".into());
            &ID
        }

        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, t: f64) -> finstack_quant_core::Result<f64> {
            Ok(if t > 0.0 { f64::NAN } else { 1.0 })
        }
        fn instantaneous_forward(&self, _t: f64) -> finstack_quant_core::Result<f64> {
            Ok(f64::NAN)
        }
    }

    #[test]
    fn aggregate_by_period_rejects_unsorted_periods() {
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("unsorted periods rejected");
        assert!(format!("{err}").contains("sorted"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_overlapping_periods() {
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 5, 1),
            ),
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("overlapping periods rejected");
        assert!(format!("{err}").contains("non-overlapping"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_duplicate_period_ids() {
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("duplicate ids rejected");
        assert!(format!("{err}").contains("duplicate"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_preserves_currency_separation() {
        // Two currencies through aggregate_by_period: per-currency map outputs
        // are separate and no cross-currency summation occurs.
        let flows = vec![
            (d(2025, 2, 1), Money::from((100_i64, Currency::USD))),
            (d(2025, 2, 15), Money::from((70_i64, Currency::EUR))),
            (d(2025, 3, 1), Money::from((50_i64, Currency::USD))),
        ];
        let periods = vec![period(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            d(2025, 1, 1),
            d(2025, 4, 1),
        )];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        let q1 = out
            .get(&PeriodId::quarter(2025, 1).expect("valid period fixture"))
            .expect("Q1 present");
        assert_eq!(q1.len(), 2, "one entry per currency");
        assert!((q1[&Currency::USD].amount() - 150.0).abs() < 1e-12);
        assert!((q1[&Currency::EUR].amount() - 70.0).abs() < 1e-12);
        assert_eq!(q1[&Currency::USD].currency(), Currency::USD);
        assert_eq!(q1[&Currency::EUR].currency(), Currency::EUR);
    }

    #[test]
    fn boundary_flow_buckets_into_next_period_half_open() {
        // A flow exactly on a period boundary belongs to the NEXT period
        // (half-open [start, end) convention).
        let boundary = d(2025, 4, 1);
        let flows = vec![(boundary, Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        assert!(!out.contains_key(&PeriodId::quarter(2025, 1).expect("valid period fixture")));
        assert!(
            (out[&PeriodId::quarter(2025, 2).expect("valid period fixture")][&Currency::USD]
                .amount()
                - 100.0)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn pv_by_period_errors_on_nan_discount_curve() {
        let base = d(2025, 1, 1);
        let flows = vec![
            CashFlowParams::new(
                d(2025, 6, 1),
                None,
                Money::from((100_i64, Currency::USD)),
                CFKind::Fixed,
                0.5,
                None,
            )
            .build()
            .expect("valid cashflow"),
        ];
        let periods = vec![period(
            PeriodId::annual(2025).expect("valid annual period"),
            base,
            d(2026, 1, 1),
        )];
        let disc = NanDiscount { base };

        let result = pv_by_period(&flows, &periods, &disc, None, None, base);
        let err = result.expect_err("NaN df must error, not panic");
        assert!(
            format!("{err}").contains("Invalid discount factor"),
            "got: {err}"
        );
    }

    #[test]
    fn pv_by_period_validates_periods_on_empty_flows() {
        let base = d(2025, 1, 1);
        let disc = NanDiscount { base };
        let periods = vec![period(
            PeriodId::annual(2025).expect("valid annual period"),
            d(2026, 1, 1),
            d(2025, 1, 1),
        )];
        let err = pv_by_period(&[], &periods, &disc, None, None, base)
            .expect_err("inverted period must be rejected even with no flows");
        assert!(
            format!("{err}").contains("strictly before end"),
            "got: {err}"
        );
    }

    #[test]
    fn pv_by_period_zeroes_historical_flows() {
        // Flows dated on or before the valuation base get zero PV (matching
        // the DataFrame convention); they still appear in nominal aggregation.
        let base = d(2025, 4, 1);
        let flows = vec![
            CashFlowParams::new(
                d(2025, 2, 1),
                None,
                Money::from((100_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )
            .build()
            .expect("valid cashflow"),
            CashFlowParams::new(
                base,
                None,
                Money::from((50_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )
            .build()
            .expect("valid cashflow"),
            CashFlowParams::new(
                d(2025, 6, 1),
                None,
                Money::from((200_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )
            .build()
            .expect("valid cashflow"),
        ];
        let periods = vec![period(
            PeriodId::annual(2025).expect("valid annual period"),
            d(2025, 1, 1),
            d(2026, 1, 1),
        )];
        let disc = UnitDiscount { base };

        let out = pv_by_period(&flows, &periods, &disc, None, None, base)
            .expect("pv aggregation succeeds");

        // With df = 1.0 only the future flow contributes PV.
        let pv =
            out[&PeriodId::annual(2025).expect("valid annual period")][&Currency::USD].amount();
        assert!((pv - 200.0).abs() < 1e-12, "expected 200, got {pv}");

        // Nominal aggregation still includes the historical flows.
        let dated: Vec<(Date, Money)> = flows
            .iter()
            .map(|cf| (cf.get_date(), cf.get_amount()))
            .collect();
        let nominal = aggregate_by_period(&dated, &periods).expect("nominal aggregation");
        let total =
            nominal[&PeriodId::annual(2025).expect("valid annual period")][&Currency::USD].amount();
        assert!((total - 350.0).abs() < 1e-12, "expected 350, got {total}");
    }
}

mod credit_pv_tests {
    use super::*;
    use finstack_quant_core::cashflow::CashFlowParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    struct FlatDiscount {
        base: Date,
    }

    impl Discounting for FlatDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "test".into());
            &ID
        }

        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> finstack_quant_core::Result<f64> {
            Ok(1.0)
        }
        fn instantaneous_forward(&self, _t: f64) -> finstack_quant_core::Result<f64> {
            Ok(0.0)
        }
    }

    fn flat_survival_at(base: Date, target: Date, sp: f64) -> HazardCurve {
        let t = DayCount::Act365F
            .year_fraction(base, target, DayCountContext::default())
            .expect("valid year fraction");
        HazardCurve::from_survival_probs("hzd", base, &[(t, sp)], 0.4, DayCount::Act365F)
            .expect("valid hazard curve")
    }

    fn annual_survival(base: Date) -> HazardCurve {
        HazardCurve::flat(
            "annual-survival",
            base,
            -0.8_f64.ln(),
            0.4,
            DayCount::Act365F,
        )
        .expect("valid hazard curve")
    }

    fn make_period(base: Date, end: Date) -> Period {
        Period {
            id: PeriodId::quarter(base.year(), 1).expect("valid period fixture"),
            start: base,
            end,
            is_actual: false,
        }
    }

    /// A fixture row; a cash principal row moves the balance by minus its cash
    /// unless the test states another explicit effect.
    fn flow(date: Date, amount: f64, kind: CFKind) -> CashFlow {
        let cash = Money::new(amount, Currency::USD).expect("valid money fixture");
        let row = CashFlowParams::new(date, None, cash, kind, 0.0, None);
        if kind.is_principal_like() && kind.is_cash_settlement() {
            row.with_principal_delta(cash.checked_neg())
                .with_principal_date(date)
        } else {
            row
        }
        .build()
        .expect("valid cashflow")
    }

    /// A fixture principal row with a stated delta and balance date.
    fn principal_row(date: Date, cash: Money, kind: CFKind, delta: Money, on: Date) -> CashFlow {
        CashFlowParams::new(date, None, cash, kind, 0.0, None)
            .with_principal_delta(delta)
            .with_principal_date(on)
            .build()
            .expect("valid cashflow")
    }

    /// A PIK capitalization on `date`: zero cash, explicit balance increase.
    fn pik_row(date: Date, amount: f64) -> CashFlow {
        CashFlowParams::new(
            date,
            None,
            Money::zero(Currency::USD),
            CFKind::Pik,
            0.0,
            None,
        )
        .with_principal_delta(Money::new(amount, Currency::USD).expect("valid PIK delta"))
        .with_principal_date(date)
        .build()
        .expect("valid cashflow")
    }

    fn funded_pvs(flows: &[CashFlow], base: Date) -> Vec<f64> {
        let hazard = annual_survival(base);
        let flat = FlatDiscount { base };
        let curves: Vec<&dyn Discounting> = vec![&flat; flows.len()];
        credit_adjusted_cashflow_pvs(flows, &curves, Some(&hazard), Some(0.4), base)
            .expect("funded default-time PVs")
    }

    #[test]
    fn forward_funding_has_no_recovery_before_advance_and_rows_match_periods() {
        let base = d(2025, 1, 1);
        let flows = vec![
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 100.0, CFKind::Notional),
        ];
        let pvs = funded_pvs(&flows, base);
        let expected = -100.0 * 0.8 + 100.0 * 0.64 + 0.4 * 100.0 * (0.8 - 0.64);
        assert!((pvs.iter().sum::<f64>() - expected).abs() < 1e-10);
        let periods = [make_period(base, d(2027, 1, 2))];
        let hazard = annual_survival(base);
        let aggregate = pv_by_period(
            &flows,
            &periods,
            &FlatDiscount { base },
            Some(&hazard),
            Some(0.4),
            base,
        )
        .expect("period PV");
        assert!((aggregate[&periods[0].id][&Currency::USD].amount() - expected).abs() < 1e-10);
    }

    #[test]
    fn funded_recovery_payment_discount_matches_independent_interval_formula() {
        struct ExponentialDiscount {
            base: Date,
        }

        impl Discounting for ExponentialDiscount {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> =
                    std::sync::LazyLock::new(|| "recovery-discount".into());
                &ID
            }

            fn base_date(&self) -> Date {
                self.base
            }

            fn df(&self, t: f64) -> finstack_quant_core::Result<f64> {
                Ok((-0.05 * t).exp())
            }

            fn instantaneous_forward(&self, _t: f64) -> finstack_quant_core::Result<f64> {
                Ok(0.05)
            }
        }

        let base = d(2025, 1, 1);
        // The curve starts one Act/365F year before valuation, exercising
        // relative discounting for both settlement dates and recovery payments.
        let discount = ExponentialDiscount {
            base: d(2024, 1, 2),
        };
        let flows = [
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 40.0, CFKind::Amortization),
            flow(d(2028, 1, 1), 60.0, CFKind::Notional),
        ];
        let periods = [make_period(base, d(2028, 1, 2))];
        let df = |years: f64| (-0.05 * years).exp();
        let settlement = -100.0 * df(1.0) * 0.8 + 40.0 * df(2.0) * 0.64 + 60.0 * df(3.0) * 0.512;
        let payment_recovery =
            0.4 * (40.0 * df(2.0) * (0.8 - 0.64) + 60.0 * df(3.0) * (0.8 - 0.512));
        let r = 0.05_f64;
        let lambda = -0.8_f64.ln();
        let continuous_lot = |n: f64, t_start: f64, t_end: f64| {
            0.4 * n
                * (lambda / (r + lambda))
                * ((-(r + lambda) * t_start).exp() - (-(r + lambda) * t_end).exp())
        };
        let analytic_recovery = continuous_lot(40.0, 1.0, 2.0) + continuous_lot(60.0, 1.0, 3.0);
        let hazard = annual_survival(base);
        let pv = pv_by_period(&flows, &periods, &discount, Some(&hazard), Some(0.4), base)
            .expect("funded recovery with nonzero discount rate");
        let actual = pv[&periods[0].id][&Currency::USD].amount();
        let expected = settlement + analytic_recovery;
        assert!((actual - expected).abs() < 2e-4, "{actual} != {expected}");
        assert!(
            actual > settlement + payment_recovery,
            "default-time recovery ({actual}) must exceed terminal-discounted recovery ({})",
            settlement + payment_recovery
        );
    }

    #[test]
    fn funded_amortization_draws_and_pik_preserve_dated_exposure() {
        let base = d(2025, 1, 1);
        let scenarios = [
            (
                vec![
                    flow(d(2024, 1, 1), -100.0, CFKind::Notional),
                    flow(d(2026, 1, 1), 40.0, CFKind::Amortization),
                    flow(d(2027, 1, 1), 60.0, CFKind::Notional),
                ],
                40.0 * (0.8 + 0.4 * 0.2) + 60.0 * (0.64 + 0.4 * 0.36),
            ),
            (
                vec![
                    flow(d(2026, 1, 1), -50.0, CFKind::RevolvingDraw),
                    flow(d(2027, 1, 1), 150.0, CFKind::RevolvingRepayment),
                ],
                -50.0 * 0.8 + 150.0 * 0.64 + 0.4 * (100.0 * 0.36 + 50.0 * 0.16),
            ),
            (
                vec![
                    pik_row(d(2026, 1, 1), 5.0),
                    flow(d(2027, 1, 1), 105.0, CFKind::Notional),
                ],
                105.0 * 0.64 + 0.4 * (100.0 * 0.36 + 5.0 * 0.16),
            ),
        ];
        for (flows, expected) in scenarios {
            {
                let actual = funded_pvs(&flows, base).iter().sum::<f64>();
                assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
            }
        }
    }

    #[test]
    fn funding_is_currency_separated_and_same_day_advances_can_be_repaid() {
        let base = d(2025, 1, 1);
        let euro = principal_row(
            d(2027, 1, 1),
            Money::from((100_i64, Currency::EUR)),
            CFKind::Notional,
            Money::from((-100_i64, Currency::EUR)),
            d(2027, 1, 1),
        );
        let flows = vec![
            flow(d(2026, 1, 1), 40.0, CFKind::Amortization),
            euro,
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 60.0, CFKind::Notional),
        ];
        let pvs = funded_pvs(&flows, base);
        assert!((pvs[0] - 32.0).abs() < 1e-10);
        assert!((pvs[1] - 78.4).abs() < 1e-10);
        assert!((pvs[2] + 80.0).abs() < 1e-10);
        assert!((pvs[3] - 42.24).abs() < 1e-10);
    }

    #[test]
    fn explicit_principal_deltas_determine_recovery_independently_of_cash_price() {
        let base = d(2025, 1, 1);
        for repayment_cash in [90.0, 110.0] {
            let flows = vec![
                principal_row(
                    d(2026, 1, 1),
                    Money::from((-90_i64, Currency::USD)),
                    CFKind::Notional,
                    Money::from((100_i64, Currency::USD)),
                    d(2026, 1, 1),
                ),
                principal_row(
                    d(2027, 1, 1),
                    Money::new(repayment_cash, Currency::USD).expect("valid money fixture"),
                    CFKind::Amortization,
                    Money::from((-100_i64, Currency::USD)),
                    d(2027, 1, 1),
                ),
            ];
            {
                let actual = funded_pvs(&flows, base).iter().sum::<f64>();
                let expected = -90.0 * 0.8 + repayment_cash * 0.64 + 100.0 * 0.4 * 0.16;
                assert!((actual - expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn early_paid_final_pik_has_no_recovery_before_capitalization() {
        let base = d(2025, 8, 1);
        let payment = d(2025, 8, 29);
        let capitalization = d(2025, 8, 31);
        // PIK is dated at its capitalization (record CORE-CONVENTIONS-015 #6).
        let pik = pik_row(capitalization, 5.0);
        let redemption = principal_row(
            payment,
            Money::from((105_i64, Currency::USD)),
            CFKind::Notional,
            Money::from((-105_i64, Currency::USD)),
            capitalization,
        );
        let pvs = funded_pvs(&[pik.clone(), redemption.clone()], base);
        let survival = 0.8_f64.powf(28.0 / 365.0);
        let expected = 105.0 * survival + 100.0 * 0.4 * (1.0 - survival);
        assert!((pvs.iter().sum::<f64>() - expected).abs() < 1e-10);
        let settled = funded_pvs(&[pik.clone(), redemption.clone()], d(2025, 8, 30));
        assert_eq!(settled, vec![0.0, 0.0]);
        let replay = [
            principal_row(
                capitalization,
                Money::zero(Currency::USD),
                CFKind::Notional,
                Money::from((5_i64, Currency::USD)),
                capitalization,
            ),
            principal_row(
                capitalization,
                Money::zero(Currency::USD),
                CFKind::Notional,
                Money::from((-105_i64, Currency::USD)),
                capitalization,
            ),
        ];
        assert_eq!(funded_pvs(&replay, d(2025, 8, 30)), vec![0.0, 0.0]);

        // An independent later claim must not absorb capitalization that
        // was extinguished by this already-paid early redemption.
        let later = flow(d(2026, 8, 30), 100.0, CFKind::Notional);
        let mut with_later = replay.to_vec();
        with_later.push(later.clone());
        let remaining = funded_pvs(&with_later, d(2025, 8, 30));
        assert!((remaining[2] - 88.0).abs() < 1e-10);
        let raw_remaining = funded_pvs(
            &[pik.clone(), redemption.clone(), later.clone()],
            d(2025, 8, 30),
        );
        assert_eq!(raw_remaining, remaining);
        let before_payment = funded_pvs(&[pik, redemption, later.clone()], base);
        let later_alone = funded_pvs(&[later], base);
        assert!((before_payment[2] - later_alone[0]).abs() < 1e-10);
    }

    #[test]
    fn pending_settlement_recovery_agrees_before_and_after_normalization() {
        let base = d(2025, 11, 5);
        for cash in [90.0, 110.0] {
            let raw = principal_row(
                d(2025, 11, 10),
                Money::new(cash, Currency::USD).expect("valid money fixture"),
                CFKind::Amortization,
                Money::from((-100_i64, Currency::USD)),
                d(2025, 11, 1),
            );
            let normalized = raw
                .with_principal(Money::from((0_i64, Currency::USD)), d(2025, 11, 1))
                .expect("valid cashflow");
            {
                let raw_pv = funded_pvs(std::slice::from_ref(&raw), base);
                let normalized_pv = funded_pvs(std::slice::from_ref(&normalized), base);
                assert_eq!(raw_pv, normalized_pv);
                let survival = 0.8_f64.powf(5.0 / 365.0);
                assert!((raw_pv[0] - cash * (survival + 0.4 * (1.0 - survival))).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn credit_adjusted_pv_zeroes_pik_flows() {
        // PIK is a capitalization event, not cash: its value is already carried
        // by the inflated notional redemption. Its PV is zero.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = flat_survival_at(base, d(2025, 6, 1), 0.95);
        let flows = vec![
            flow(d(2025, 6, 1), 50_000.0, CFKind::Fixed),
            pik_row(d(2025, 6, 1), 40_000.0),
        ];

        let out = pv_by_period(&flows, &periods, &disc, Some(&hazard), None, base)
            .expect("credit-adjusted prices");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        let expected = 50_000.0 * 0.95;
        assert!(
            (pv - expected).abs() < 1e-9,
            "PIK must carry zero PV under credit adjustment: got {pv}, expected {expected}"
        );
    }

    #[test]
    fn revolving_repayment_recovery_applies_under_payment_date_timing() {
        // RevolvingRepayment is a principal claim; the payment-date path must
        // apply the funded R·(1−SP) recovery term.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = flat_survival_at(base, d(2025, 6, 1), 0.95);
        let flows = vec![flow(d(2025, 6, 1), 100_000.0, CFKind::RevolvingRepayment)];

        let out = pv_by_period(&flows, &periods, &disc, Some(&hazard), Some(0.40), base)
            .expect("payment-date timing prices");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        let expected = 100_000.0 * (0.95 + 0.40 * 0.05);
        assert!(
            (pv - expected).abs() < 1e-9,
            "revolving repayment must earn recovery on default: got {pv}, expected {expected}"
        );
    }

    #[test]
    fn recovery_skips_negative_principal_draws() {
        // Future funding is survival-weighted cash, and recovery begins only
        // once that draw creates funded exposure.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let t1 = 151.0 / 365.0; // 2025-01-01 → 2025-06-01
        let t2 = 334.0 / 365.0; // 2025-01-01 → 2025-12-01
        let sp1 = 1.0 - 0.1 * t1;
        let sp2 = 1.0 - 0.1 * t2;
        let hazard = HazardCurve::from_survival_probs(
            "hzd-linear",
            base,
            &[(t1, sp1), (t2, sp2)],
            0.4,
            DayCount::Act365F,
        )
        .expect("valid hazard curve");
        let flows = vec![
            flow(d(2025, 6, 1), -500_000.0, CFKind::Notional),
            flow(d(2025, 12, 1), 500_000.0, CFKind::Amortization),
        ];

        let out = pv_by_period(&flows, &periods, &disc, Some(&hazard), Some(0.40), base)
            .expect("negative draws must not earn recovery");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        // Draw: survival-weighted cash PV only. Amortization: survival-weighted
        // PV plus recovery only over (draw date, repayment date].
        let expected = -500_000.0 * sp1 + 500_000.0 * sp2 + 0.40 * 500_000.0 * (sp1 - sp2);
        assert!(
            (pv - expected).abs() < 1e-6,
            "got {pv}, expected {expected}"
        );
    }

    #[test]
    fn aggregate_by_period_errors_on_decimal_overflow() {
        // Two near-Decimal-max flows in one period overflow the Decimal range
        // when materialized as Money; that must surface as Err, not a panic.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let flows: Vec<(Date, Money)> = vec![
            (
                d(2025, 3, 1),
                Money::new(7.0e28, Currency::USD).expect("valid money fixture"),
            ),
            (
                d(2025, 6, 1),
                Money::new(7.0e28, Currency::USD).expect("valid money fixture"),
            ),
        ];

        let res = aggregate_by_period(&flows, &periods);
        assert!(res.is_err(), "Decimal-overflow sums must error, not panic");
    }

    #[test]
    fn rejects_defaulted_notional_with_recovery_rate() {
        let base = d(2025, 1, 1);
        let flows = vec![
            // A noncash write-down: zero cash, explicit balance decrease.
            CashFlowParams::new(
                d(2025, 6, 1),
                None,
                Money::zero(Currency::USD),
                CFKind::DefaultedNotional,
                0.0,
                None,
            )
            .with_principal_delta(Money::from((-100_000_i64, Currency::USD)))
            .with_principal_date(d(2025, 6, 1))
            .build()
            .expect("valid cashflow"),
            {
                let cash = Money::from((900_000_i64, Currency::USD));
                CashFlowParams::new(d(2025, 12, 1), None, cash, CFKind::Amortization, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(d(2025, 12, 1))
                    .build()
                    .expect("valid cashflow")
            },
        ];
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = flat_survival_at(base, d(2025, 12, 1), 0.95);
        let ctx = base;

        let result = pv_by_period(&flows, &periods, &disc, Some(&hazard), Some(0.40), ctx);
        assert!(
            result.is_err(),
            "should reject DefaultedNotional + recovery_rate"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("DefaultedNotional"),
            "error message should mention DefaultedNotional: {}",
            err_msg
        );
    }

    #[test]
    fn allows_defaulted_notional_without_recovery_rate() {
        let base = d(2025, 1, 1);
        let flows = vec![
            // A noncash write-down: zero cash, explicit balance decrease.
            CashFlowParams::new(
                d(2025, 6, 1),
                None,
                Money::zero(Currency::USD),
                CFKind::DefaultedNotional,
                0.0,
                None,
            )
            .with_principal_delta(Money::from((-100_000_i64, Currency::USD)))
            .with_principal_date(d(2025, 6, 1))
            .build()
            .expect("valid cashflow"),
            {
                let cash = Money::from((900_000_i64, Currency::USD));
                CashFlowParams::new(d(2025, 12, 1), None, cash, CFKind::Amortization, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(d(2025, 12, 1))
                    .build()
                    .expect("valid cashflow")
            },
        ];
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = flat_survival_at(base, d(2025, 12, 1), 0.95);
        let ctx = base;

        let result = pv_by_period(&flows, &periods, &disc, Some(&hazard), None, ctx);
        assert!(
            result.is_ok(),
            "should allow DefaultedNotional without recovery_rate"
        );
    }

    #[test]
    fn credit_adjusted_period_pv_uses_matching_pv_after_skipped_flows() {
        let base = d(2025, 1, 1);
        let periods = vec![Period {
            id: PeriodId::quarter(2025, 3).expect("valid period fixture"),
            start: d(2025, 7, 1),
            end: d(2026, 1, 1),
            is_actual: false,
        }];
        let disc = FlatDiscount { base };
        let hazard = flat_survival_at(base, d(2025, 10, 1), 0.95);
        let flows = vec![
            flow(d(2025, 3, 1), 111.0, CFKind::Fixed),
            flow(d(2025, 10, 1), 1_000.0, CFKind::Fixed),
        ];

        let out = pv_by_period(&flows, &periods, &disc, Some(&hazard), None, base)
            .expect("credit-adjusted pricing");

        let pv = out
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(|m| m.amount())
            .expect("period pv");
        let expected = 1_000.0 * 0.95;
        assert!(
            (pv - expected).abs() < 1e-9,
            "expected {}, got {}",
            expected,
            pv
        );
    }
}

mod calendar_year_ladder_tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    #[test]
    fn calendar_year_ladder_splits_non_principal_and_principal_by_year() {
        let dates = [d(2027, 3, 15), d(2027, 9, 15), d(2034, 3, 15)];
        let kinds = [" fixed ", "FiXeD", "Notional"];
        let amounts = [212_500.0, 212_500.0, 10_000_000.0];
        let pvs = [208_000.0, 206_000.0, 7_100_000.0];
        let rows = calendar_year_ladder(&dates, &kinds, &amounts, &pvs).expect("ladder");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].year, 2027);
        assert!((rows[0].non_principal - 425_000.0).abs() < 1e-9);
        assert!((rows[0].principal).abs() < 1e-12);
        assert_eq!(rows[1].year, 2034);
        assert!((rows[1].principal - 10_000_000.0).abs() < 1e-9);
        assert!((rows[1].pv - 7_100_000.0).abs() < 1e-9);
    }

    #[test]
    fn calendar_year_ladder_counts_only_cash_settlement() {
        let dates = [d(2027, 1, 1), d(2027, 6, 1), d(2028, 1, 1)];
        let rows = calendar_year_ladder(
            &dates,
            &["fixed", "pik", "defaulted_notional"],
            &[10.0, 5.0, 40.0],
            &[9.0, 4.0, 30.0],
        )
        .expect("ladder");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].year, 2027);
        assert_eq!(rows[0].non_principal, 10.0);
        assert_eq!(rows[0].principal, 0.0);
        assert_eq!(rows[0].pv, 9.0);
    }

    #[test]
    fn calendar_year_ladder_rejects_length_mismatch() {
        let err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[1.0], &[])
            .expect_err("length mismatch");
        assert!(err.to_string().contains("equal lengths"));
    }

    #[test]
    fn calendar_year_ladder_rejects_unknown_kind() {
        for label in ["redemption", "coupon", "principal"] {
            let err = calendar_year_ladder(&[d(2027, 1, 1)], &[label], &[1.0], &[0.9])
                .expect_err("unknown kind");
            assert!(
                err.to_string().contains("unknown cashflow kind"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn calendar_year_ladder_rejects_non_finite_amount_or_pv() {
        let amount_err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[f64::NAN], &[1.0])
            .expect_err("nan amount");
        assert!(amount_err.to_string().contains("amount must be finite"));
        let pv_err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[1.0], &[f64::INFINITY])
            .expect_err("inf pv");
        assert!(pv_err.to_string().contains("pv must be finite"));
    }
}
