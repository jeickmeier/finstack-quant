use super::allocation::{to_currency_units, water_fill_allocation};
use super::*;
use crate::structured_credit::types::{CoverageTestAction, RecipientType, WaterfallDistribution};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::market_data::context::MarketContext;

#[cfg(test)]
mod to_currency_units_tests {
    use super::to_currency_units;

    #[test]
    fn rejects_overflow_beyond_representable_units() {
        let scale = 100.0;
        let amount = (i64::MAX as f64) / scale + 1.0e6;
        assert!(to_currency_units(amount, scale).is_err());
    }

    #[test]
    fn rejects_invalid_scale() {
        assert!(to_currency_units(1.0, 0.0).is_err());
        assert!(to_currency_units(1.0, -1.0).is_err());
    }
}

#[cfg(test)]
mod coverage_position_tests {
    //! Positional coverage tests: a `PaymentType::CoverageTest` tier can only
    //! divert the interest still undistributed at its position, so a test on
    //! a junior class never traps a senior class's coupon. The IC cure-sizing
    //! rules (recipient order, out-of-denominator recipients) are covered at
    //! the `CoverageTest::calculate` level in `coverage_tests.rs`.
    use super::WaterfallContext;
    use super::execute_waterfall;
    use crate::structured_credit::types::{
        AllocationMode, AssetPool, AssetType, CoverageTestSpec, DealType, PaymentCalculation,
        PaymentType, PoolAsset, Recipient, RecipientType, Tranche, TrancheSeniority,
        TrancheStructure, Waterfall, WaterfallTier,
    };
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CreditRating, InstrumentId};
    use time::Month;

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("valid money fixture")
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2031, Month::January, 1).expect("date")
    }

    fn pool_with(balance: f64) -> AssetPool {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: finstack_quant_core::dates::DayCount::Act360,
            id: InstrumentId::new("ASSET_0"),
            asset_type: AssetType::FirstLienLoan {},
            balance: usd(balance),
            rate: 0.08,
            spread_bp: Some(400.0),
            forward_curve_id: None,
            index_floor_bp: None,
            reset_lag_days: None,
            maturity: maturity(),
            rating: Some(CreditRating::BB),
            industry: Some("Technology".into()),
            obligor_id: Some("OBLIGOR_0".into()),
            defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            origination_date: None,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
        });
        pool
    }

    fn tranche(id: &str, a: f64, d: f64, sen: TrancheSeniority, bal: f64, cpn: f64) -> Tranche {
        Tranche::new(
            id,
            a,
            d,
            sen,
            usd(bal),
            RateSpec::Fixed { rate: cpn },
            maturity(),
        )
        .expect("tranche")
    }

    fn context<'a>(
        market: &'a MarketContext,
        interest: f64,
        principal: f64,
        pool_balance: f64,
    ) -> WaterfallContext<'a> {
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        WaterfallContext {
            available_cash: usd(interest + principal),
            interest_collections: usd(interest),
            principal_collections: usd(principal),
            payment_date,
            accrual_end: payment_date,
            period_start,
            valuation_date: period_start,
            pool_balance: usd(pool_balance),
            market,
            tranche_balances: None,
            asset_balances: None,
            live_collateral: None,
            special_serviced: None,
            deferred_interest: None,
            reserve_balance: usd(0.0),
            restricted_cash: usd(0.0),
            defaulted_collateral_value: usd(0.0),
            recovery_proceeds: usd(0.0),
            floating_rate_shift: 0.0,
            equity_history: None,
        }
    }

    fn paid_to(result: &super::WaterfallDistribution, recipient_id: &str) -> f64 {
        result
            .payment_records
            .iter()
            .filter(|r| r.recipient_id == recipient_id)
            .map(|r| r.paid_amount.amount())
            .sum()
    }

    /// A test after Class B's interest can trap Class C's coupon and the
    /// residual, never Class A's or B's.
    #[test]
    fn test_after_a_junior_class_leaves_senior_coupons_intact() {
        let pool = pool_with(120_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche("A", 0.0, 60.0, TrancheSeniority::Senior, 60_000_000.0, 0.05),
            tranche(
                "B",
                60.0,
                80.0,
                TrancheSeniority::Mezzanine,
                20_000_000.0,
                0.07,
            ),
            tranche(
                "C",
                80.0,
                100.0,
                TrancheSeniority::Subordinated,
                20_000_000.0,
                0.10,
            ),
        ])
        .expect("structure");
        // Collateral 120M + 0 principal cash against A + B = 80M gives 1.50,
        // so a 1.60 test on B fails while everything is otherwise healthy.
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("a_interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("a_int", "A")),
            )
            .add_tier(
                WaterfallTier::new("b_interest", 2, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("b_int", "B")),
            )
            .add_tier(WaterfallTier::coverage_tests(
                "b_coverage",
                3,
                vec![CoverageTestSpec::oc("B", 1.60)],
            ))
            .add_tier(
                WaterfallTier::new("c_interest", 4, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("c_int", "C")),
            )
            .add_tier(
                WaterfallTier::new("principal", 5, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal("a_prin", "A", None))
                    .add_recipient(Recipient::tranche_principal("b_prin", "B", None))
                    .add_recipient(Recipient::tranche_principal("c_prin", "C", None)),
            )
            .add_tier(
                WaterfallTier::new("equity", 6, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 3_000_000.0, 0.0, 120_000_000.0),
        )
        .expect("waterfall execution");

        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "OC_B")
            .expect("OC_B evaluated");
        assert!(!oc.2, "the B test must fail (ratio {:?})", oc.1);
        assert!(result.had_diversions);

        let a_interest = paid_to(&result, "a_int");
        let b_interest = paid_to(&result, "b_int");
        let c_interest = paid_to(&result, "c_int");
        // Fixed coupons accrue ACT/360 over the 91-day Jan-1 to Apr-1 period.
        let accrual = 91.0 / 360.0;
        assert!(
            (a_interest - 60_000_000.0 * 0.05 * accrual).abs() < 1.0,
            "A coupon is paid in full above the test, got {a_interest}"
        );
        assert!(
            (b_interest - 20_000_000.0 * 0.07 * accrual).abs() < 1.0,
            "B coupon is paid in full above the test, got {b_interest}"
        );
        assert!(
            c_interest < 1.0,
            "C's coupon sits below the failing test and is diverted, got {c_interest}"
        );
        // Everything left after B's coupon went to A principal, up to the cure.
        let a_principal = paid_to(&result, "a_prin");
        let remaining_after_b = 3_000_000.0 - a_interest - b_interest;
        assert!(
            (a_principal - remaining_after_b).abs() < 1.0,
            "diverted interest {remaining_after_b} must pay down A, got {a_principal}"
        );
        assert!(
            (result.diverted_cash.amount() - a_principal).abs() < 1.0,
            "diverted cash {} must equal the senior paydown {a_principal}",
            result.diverted_cash.amount()
        );
    }

    /// W-21: an IC-only breach diverts the interest below the test position.
    #[test]
    fn ic_only_breach_diverts_interest_below_the_test() {
        let pool = pool_with(500_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                76.9,
                TrancheSeniority::Senior,
                100_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                76.9,
                100.0,
                TrancheSeniority::Subordinated,
                30_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("a_interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A")),
            )
            // OC 1.05 passes easily (500M collateral); IC 1.20 fails because
            // collections (1.4M) do not cover 1.2 × A's 1.25M coupon.
            .add_tier(WaterfallTier::coverage_tests(
                "a_coverage",
                2,
                vec![
                    CoverageTestSpec::oc("CLASS_A", 1.05),
                    CoverageTestSpec::ic("CLASS_A", 1.20),
                ],
            ))
            .add_tier(
                WaterfallTier::new("b_interest", 3, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            .add_tier(
                WaterfallTier::new("principal", 4, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        None,
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 5, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let principal_collections = 19_900_000.0;
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 1_400_000.0, principal_collections, 500_000_000.0),
        )
        .expect("waterfall execution");

        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "OC_CLASS_A")
            .expect("OC result present");
        let ic = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "IC_CLASS_A")
            .expect("IC result present");
        assert!(oc.2, "OC test should pass (ratio {:?})", oc.1);
        assert!(!ic.2, "IC test should fail (ratio {:?})", ic.1);

        let a_interest = paid_to(&result, "class_a_int");
        let diverted = result.diverted_cash.amount();
        assert!(
            (diverted - (1_400_000.0 - a_interest)).abs() < 1.0,
            "the interest left after A's coupon ({}) is diverted, got {diverted}",
            1_400_000.0 - a_interest
        );
        assert!(
            paid_to(&result, "class_b_int") < 1.0,
            "B's coupon sits below the failing test"
        );
        // The diverted interest pays A principal on top of the period's
        // principal collections, which A absorbs in full (no target balance).
        let a_principal = paid_to(&result, "class_a_prin");
        assert!(
            (a_principal - (principal_collections + diverted)).abs() < 1.0,
            "A principal must be the collections plus the diversion, got {a_principal}"
        );
        assert!(
            paid_to(&result, "class_b_prin") < 1.0,
            "no principal reaches B while A is outstanding"
        );
    }

    /// Two tests at one position share the senior balances they de-lever, so
    /// the binding cure is the maximum, not the sum.
    #[test]
    fn coverage_cures_at_one_position_are_not_summed() {
        let pool = pool_with(118_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                77.0,
                TrancheSeniority::Senior,
                100_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                77.0,
                100.0,
                TrancheSeniority::Subordinated,
                30_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // Numerator = 118M collateral + 20M principal cash = 138M.
            // CLASS_A denominator 100M (1.38) breaches 1.39; CLASS_B
            // denominator 130M (1.06) breaches 1.07 by more.
            .add_tier(WaterfallTier::coverage_tests(
                "coverage",
                2,
                vec![
                    CoverageTestSpec::oc("CLASS_A", 1.39),
                    CoverageTestSpec::oc("CLASS_B", 1.07),
                ],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(usd(99_000_000.0)),
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 25_000_000.0, 20_000_000.0, 118_000_000.0),
        )
        .expect("waterfall execution");

        let failing = result
            .coverage_tests
            .iter()
            .filter(|(_, _, passing)| !passing)
            .count();
        assert_eq!(
            failing, 2,
            "both OC tests must breach: {:?}",
            result.coverage_tests
        );

        // Diverting X of interest pays down X of the shared senior
        // denominator and leaves the numerator untouched: X = den − num / r.
        let numerator = 118_000_000.0_f64 + 20_000_000.0;
        let cure = |ratio: f64, denom: f64| denom - numerator / ratio;
        let cure_a = cure(1.39, 100_000_000.0);
        let cure_b = cure(1.07, 130_000_000.0);
        let binding = cure_a.max(cure_b);
        let summed = cure_a + cure_b;
        let interest_left =
            25_000_000.0 - paid_to(&result, "class_a_int") - paid_to(&result, "class_b_int");
        assert!(
            summed < interest_left,
            "fixture: both cures ({summed}) must be fundable from the interest left ({interest_left})"
        );
        let diverted = result.diverted_cash.amount();
        assert!(
            (diverted - binding).abs() < 1.0,
            "diverted {diverted} must equal the binding (max) cure {binding}, not the sum {summed}"
        );
    }

    /// Principal paid by a diversion is netted by the regular principal tier.
    #[test]
    fn diversion_never_over_pays_senior_principal() {
        let pool = pool_with(20_000_000.0);
        let class_a_balance = 3_000_000.0;
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                10.0,
                TrancheSeniority::Senior,
                class_a_balance,
                0.05,
            ),
            tranche(
                "CLASS_B",
                10.0,
                100.0,
                TrancheSeniority::Subordinated,
                27_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // Collateral 20M + 9M cash against the 30M stack breaches 1.20.
            .add_tier(WaterfallTier::coverage_tests(
                "coverage",
                2,
                vec![CoverageTestSpec::oc("CLASS_B", 1.20)],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        None,
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 1_000_000.0, 9_000_000.0, 20_000_000.0),
        )
        .expect("waterfall execution");

        assert!(result.had_diversions, "the OC breach must divert");
        assert!(
            result.diverted_cash.amount() > 0.0,
            "interest left after the coupons must reach A principal"
        );
        let class_a_principal_paid = paid_to(&result, "class_a_prin");
        assert!(
            class_a_principal_paid <= class_a_balance + 1e-6,
            "CLASS_A principal paid {class_a_principal_paid:.2} exceeds its \
             balance {class_a_balance:.2}: the regular tier did not net the diversion"
        );
        assert!(
            (class_a_principal_paid - class_a_balance).abs() < 1e-6,
            "with 9M of principal the senior is retired exactly once"
        );
    }

    /// A `Reinvest` test retains the diverted interest as principal proceeds
    /// instead of paying notes.
    #[test]
    fn reinvest_action_retains_diverted_interest_as_principal() {
        let pool = pool_with(100_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                80_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                80.0,
                100.0,
                TrancheSeniority::Subordinated,
                20_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // 100M against 100M of notes fails a 1.05 reinvestment OC test.
            .add_tier(WaterfallTier::coverage_tests(
                "reinvestment_oc",
                2,
                vec![CoverageTestSpec {
                    action: super::CoverageTestAction::Reinvest,
                    ..CoverageTestSpec::oc("CLASS_B", 1.05)
                }],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(usd(80_000_000.0)),
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 2_500_000.0, 0.0, 100_000_000.0),
        )
        .expect("waterfall execution");

        let coupons = paid_to(&result, "class_a_int") + paid_to(&result, "class_b_int");
        let retained = 2_500_000.0 - coupons;
        assert!(retained > 0.0, "fixture: excess interest must exist");
        assert!(
            (result.remaining_principal.amount() - retained).abs() < 1.0,
            "diverted interest is carried as principal: got {}, expected {retained}",
            result.remaining_principal.amount()
        );
        assert!(
            paid_to(&result, "class_a_prin") < 1.0,
            "a Reinvest test does not pay down notes this period"
        );
        assert!(
            result
                .distributions()
                .expect("distributions")
                .get(&RecipientType::Equity)
                .is_none_or(|m| m.amount() < 1.0),
            "nothing reaches equity below a failing test"
        );
        assert!(
            result
                .diverted_amounts
                .iter()
                .any(|record| record.target_tranche == "principal_account")
        );
    }

    /// A fee tier ranked behind note interest is junior to the IC claim and
    /// must not reduce its numerator.
    #[test]
    fn coverage_economics_late_junior_fee_is_not_deducted_from_ic_numerator() {
        let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        let tranches = TrancheStructure::new(vec![tranche(
            "CLASS_A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            100_000.0,
            0.05,
        )])
        .expect("structure");
        let build = |late_fee: Option<f64>| {
            let mut builder = Waterfall::builder(Currency::USD)
                .add_tier(
                    WaterfallTier::new("note_interest", 1, PaymentType::Interest)
                        .add_recipient(Recipient::tranche_interest("class_a_interest", "CLASS_A")),
                )
                .add_tier(WaterfallTier::coverage_tests(
                    "coverage",
                    2,
                    vec![CoverageTestSpec::ic("CLASS_A", 1.0)],
                ));
            if let Some(amount) = late_fee {
                builder = builder.add_tier(
                    WaterfallTier::new("junior_fee", 3, PaymentType::Fee).add_recipient(
                        Recipient::fixed_fee("junior_fee_recipient", "junior_manager", usd(amount)),
                    ),
                );
            }
            builder.build().expect("waterfall")
        };
        let market = MarketContext::new();
        let ratio = |late_fee| {
            execute_waterfall(
                &build(late_fee),
                &tranches,
                &pool,
                context(&market, 2_000.0, 0.0, 100_000.0),
            )
            .expect("waterfall execution")
            .coverage_tests
            .into_iter()
            .find(|(id, _, _)| id == "IC_CLASS_A")
            .expect("IC result")
            .1
            .expect("the IC test has a claim")
        };
        let without = ratio(None);
        let with = ratio(Some(500.0));
        assert!(
            (with - without).abs() < 1e-12,
            "a fee tier behind note interest must not reduce the IC numerator: \
             without={without}, with={with}"
        );
    }
}

#[cfg(test)]
mod water_fill_tests {
    use super::*;
    use crate::structured_credit::types::{
        AllocationMode, AssetPool, DealType, PaymentType, Recipient, Tranche, TrancheSeniority,
        TrancheStructure, Waterfall, WaterfallTier,
    };
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;
    use time::Month;

    /// The waterfall must report its OWN interest/principal split.
    ///
    /// `distributions` keys a tranche's interest and principal under the same
    /// `RecipientType::Tranche(id)`, so the engine's Step 5 had to reconstruct
    /// the split by assuming interest is satisfied first. That reclassified
    /// principal as interest whenever a tranche carried a shortfall — the exact
    /// state an OC cure addresses — leaving the balance unretired so the cure
    /// could not de-lever the ratio it was sized to fix.
    ///
    /// This pins the contract that removes the guesswork: for a tranche paid
    /// from BOTH an interest tier and a principal tier, `distributions` holds
    /// the total while `principal_distributions` holds only the principal.
    #[test]
    fn distribution_reports_the_principal_portion_separately() {
        let ccy = Currency::USD;
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2024, Month::January, 1).expect("date");

        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, ccy)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("date"),
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("structure");

        let waterfall = Waterfall::builder(ccy)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("A_int", "A")),
            )
            .add_tier(
                WaterfallTier::new("principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal("A_prin", "A", None)),
            )
            .build()
            .expect("valid waterfall");

        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &AssetPool::new("POOL", DealType::Clo, ccy),
            WaterfallContext {
                available_cash: Money::from((300_000_i64, ccy)),
                interest_collections: Money::from((13_000_i64, ccy)),
                principal_collections: Money::from((287_000_i64, ccy)),
                payment_date,
                accrual_end: payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::from((1_000_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: None,
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, ccy)),
                defaulted_collateral_value: Money::from((0_i64, ccy)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("waterfall executes");

        let key = RecipientType::Tranche("A".to_string());
        let total = result
            .distributions()
            .expect("distributions")
            .get(&key)
            .copied()
            .expect("tranche paid")
            .amount();
        let principal = result
            .principal_distributions()
            .expect("principal_distributions")
            .get(&key)
            .copied()
            .unwrap_or(Money::from((0_i64, ccy)))
            .amount();

        // A quarter's interest on 1,000,000 at 5% is ~12,500; the rest of the
        // 300,000 retires principal.
        assert!(
            total > 290_000.0,
            "the tranche should receive nearly all available cash, got {total:.2}"
        );
        assert!(
            principal > 250_000.0 && principal < total,
            "principal_distributions must hold ONLY the principal portion: got \
             {principal:.2} of {total:.2} total. Equal values mean interest was \
             misclassified as principal; zero means the split is not being \
             reported at all."
        );
        let interest = total - principal;
        assert!(
            (interest - 12_500.0).abs() < 500.0,
            "the interest remainder must be ~one quarter's coupon (12,500), \
             got {interest:.2}"
        );
    }

    use super::water_fill_allocation;

    /// The subordination-inversion case: a small-balance senior carrying a large
    /// weight must not leak its capped excess to the residual tier; the freed
    /// units water-fill onto the outstanding junior.
    #[test]
    fn capped_senior_excess_redistributes_to_junior() {
        // available = 100 units; senior weight 0.8 but cap 10; junior weight 0.2
        // cap 1000. Senior takes 10, junior absorbs the remaining 90.
        let alloc = water_fill_allocation(100, &[0.8, 0.2], &[10, 1000]);
        assert_eq!(alloc, vec![10, 90]);
        assert_eq!(alloc.iter().sum::<i64>(), 100);
    }

    /// Equal-weight pro-rata with unequal balances: the small recipient is
    /// capped and the large recipient takes the rest — nothing leaks.
    #[test]
    fn equal_weight_unequal_caps_no_leak() {
        let alloc = water_fill_allocation(100, &[1.0, 1.0], &[10, 1000]);
        assert_eq!(alloc, vec![10, 90]);
        assert_eq!(alloc.iter().sum::<i64>(), 100);
    }

    /// When every recipient's request is fully covered (available >= total
    /// requested), each receives exactly its cap and the rest stays unplaced
    /// (to flow to the next tier).
    #[test]
    fn all_requests_covered_leaves_residual() {
        let alloc = water_fill_allocation(100, &[1.0, 1.0], &[30, 40]);
        assert_eq!(alloc, vec![30, 40]);
        assert_eq!(alloc.iter().sum::<i64>(), 70); // 30 residual flows on
    }

    /// Conservation under tight caps: the allocation never exceeds total_units
    /// and never exceeds any per-recipient cap.
    #[test]
    fn conserves_and_respects_caps() {
        let caps = [13, 27, 5, 200];
        let alloc = water_fill_allocation(101, &[0.4, 0.3, 0.2, 0.1], &caps);
        let total: i64 = alloc.iter().sum();
        assert_eq!(total, 101, "all available units distributed");
        for (a, c) in alloc.iter().zip(caps.iter()) {
            assert!(*a <= *c, "allocation {a} exceeded cap {c}");
        }
    }

    /// Zero-weight recipients still receive an even split (degenerate fallback).
    #[test]
    fn zero_weights_split_evenly() {
        let alloc = water_fill_allocation(10, &[0.0, 0.0], &[100, 100]);
        assert_eq!(alloc.iter().sum::<i64>(), 10);
        assert!((alloc[0] - alloc[1]).abs() <= 1);
    }

    /// Cash conservation across an interest-only period (no tranche receives
    /// principal) and a sequential paydown period where Senior A receives
    /// principal while Mezzanine B receives zero principal (and only B has a
    /// carried deferred-interest entry).
    #[test]
    fn cash_conservation_when_some_tranches_receive_zero_principal() {
        let ccy = Currency::USD;
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let payment_date = Date::from_calendar_date(2025, Month::April, 1).expect("date");

        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("date");
        let tranche_a = Tranche::new(
            "A",
            20.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, ccy)),
            RateSpec::Fixed { rate: 0.04 },
            maturity,
        )
        .expect("tranche A");
        let tranche_b = Tranche::new(
            "B",
            0.0,
            20.0,
            TrancheSeniority::Mezzanine,
            Money::from((250_000_i64, ccy)),
            RateSpec::Fixed { rate: 0.08 },
            maturity,
        )
        .expect("tranche B");
        let tranches = TrancheStructure::new(vec![tranche_a, tranche_b]).expect("structure");

        let waterfall = Waterfall::builder(ccy)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("A_int", "A"))
                    .add_recipient(Recipient::tranche_interest("B_int", "B")),
            )
            .add_tier(
                WaterfallTier::new("principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal("A_prin", "A", None))
                    .add_recipient(Recipient::tranche_principal("B_prin", "B", None)),
            )
            .build()
            .expect("valid waterfall");

        let market = MarketContext::new();
        let pool = AssetPool::new("POOL", DealType::Clo, ccy);

        // Period 1: interest-only cashflow — neither A nor B receives principal.
        let io_cash = 15_000.0;
        let io_result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            WaterfallContext {
                available_cash: Money::from((15_000_i64, ccy)),
                interest_collections: Money::from((15_000_i64, ccy)),
                principal_collections: Money::from((0_i64, ccy)),
                payment_date,
                accrual_end: payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::from((1_250_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: None,
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, ccy)),
                defaulted_collateral_value: Money::from((0_i64, ccy)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("IO waterfall executes");

        let key_a = RecipientType::Tranche("A".to_string());
        let key_b = RecipientType::Tranche("B".to_string());
        let io_dists = io_result.distributions().expect("io distributions");
        let io_prins = io_result
            .principal_distributions()
            .expect("io principal distributions");
        let io_distributed: f64 = io_dists.values().map(|m| m.amount()).sum();
        assert!(
            (io_distributed + io_result.remaining_cash.amount() - io_cash).abs() < 1e-6,
            "IO period must conserve cash"
        );
        assert!(!io_prins.contains_key(&key_a));
        assert!(!io_prins.contains_key(&key_b));

        // Period 2: partial principal paydown — A receives principal, B receives
        // zero principal, and only B carries deferred interest (A is absent).
        let mut deferred = FxHashMap::default();
        deferred.insert("B".to_string(), Money::from((500_i64, ccy)));

        let p2_cash = 120_000.0;
        let p2_result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            WaterfallContext {
                available_cash: Money::from((120_000_i64, ccy)),
                interest_collections: Money::from((20_000_i64, ccy)),
                principal_collections: Money::from((100_000_i64, ccy)),
                payment_date,
                accrual_end: payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::from((1_250_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: Some(&deferred),
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, ccy)),
                defaulted_collateral_value: Money::from((0_i64, ccy)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("period 2 waterfall executes");

        let p2_dists = p2_result.distributions().expect("p2 distributions");
        let p2_prins = p2_result
            .principal_distributions()
            .expect("p2 principal distributions");
        let p2_distributed: f64 = p2_dists.values().map(|m| m.amount()).sum();
        assert!(
            (p2_distributed + p2_result.remaining_cash.amount() - p2_cash).abs() < 1e-6,
            "Period 2 must conserve cash: distributed={p2_distributed}, remaining={}",
            p2_result.remaining_cash.amount()
        );
        let a_prin = p2_prins
            .get(&key_a)
            .expect("senior A receives principal")
            .amount();
        assert!(a_prin > 90_000.0 && a_prin < 110_000.0, "a_prin={a_prin}");
        assert!(
            !p2_prins.contains_key(&key_b),
            "mezzanine B receives zero principal while senior A is still outstanding"
        );
        // B receives its quarterly coupon (250k * 8% * 90/360 = 5,000) + 500 carried deferred interest.
        let b_total = p2_dists.get(&key_b).expect("B interest paid").amount();
        assert!(
            (b_total - 5_500.0).abs() < 1e-6,
            "B total interest with deferred carry should be 5,500, got {b_total}"
        );
    }
}
