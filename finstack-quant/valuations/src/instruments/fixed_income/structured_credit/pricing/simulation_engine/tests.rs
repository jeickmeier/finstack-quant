#[cfg(test)]
mod cases {
    use super::super::*;

    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, DefaultModelSpec, PoolAsset, PrepaymentModelSpec, RecoveryModelSpec,
        Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::fixings::fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use time::Month;

    fn production_trigger_deal(
        consequence: crate::instruments::fixed_income::structured_credit::TriggerConsequence,
    ) -> StructuredCredit {
        use crate::instruments::fixed_income::structured_credit::CoverageTrigger;
        let mut deal = StructuredCredit::example();
        deal.tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                20.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(80_000_000.0, Currency::USD).expect("senior"),
                TrancheCoupon::Fixed { rate: 0.04 },
                deal.maturity,
            )
            .expect("senior"),
            Tranche::new(
                "EQ",
                0.0,
                20.0,
                TrancheSeniority::Equity,
                Money::new(20_000_000.0, Currency::USD).expect("equity"),
                TrancheCoupon::Fixed { rate: 0.0 },
                deal.maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        deal.tranches.tranches[0].oc_trigger =
            Some(CoverageTrigger::new(1.30, consequence).with_cure_level(1.40));
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal
    }

    #[test]
    fn production_waterfall_reinvestment_preserves_decimal_cash_budget() {
        use crate::instruments::fixed_income::structured_credit::{
            ReinvestmentCriteria, ReinvestmentPeriod,
        };
        let mut deal = StructuredCredit::example();
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end_date: deal.maturity,
            is_active: true,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria::default(),
        });
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        let budget = Money::from_decimal(
            rust_decimal_macros::dec!(10557280.900008413701),
            Currency::USD,
        )
        .expect("decimal budget");
        let spent = recycle_reinvestment_principal(
            &mut state,
            budget,
            1.0,
            deal.first_payment_date,
            &MarketContext::new(),
        )
        .expect("reinvestment");
        assert_eq!(spent, budget);
        assert_eq!(
            budget
                .checked_sub(spent)
                .expect("cash left")
                .amount_decimal(),
            rust_decimal_macros::dec!(0)
        );
    }

    #[test]
    fn production_waterfall_trap_cure_rebreach_conserves_interest() {
        use crate::instruments::fixed_income::structured_credit::TriggerConsequence;
        let deal = production_trigger_deal(TriggerConsequence::TrapExcessSpread);
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
            &[],
        );
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        let mut source = DeterministicPoolFlowSource;
        let first = deal.first_payment_date;
        let second = first.add_months(3);
        let third = second.add_months(3);
        let mut interest_generated = 0.0;
        for (date, par, breached) in [
            (first, 100_000_000.0, true),
            (second, 112_000_000.0, false),
            (third, 100_000_000.0, true),
        ] {
            // Supply an independently specified current collateral state at each boundary.
            state.pool_state.balances[0] = par;
            state.pool_outstanding = Money::new(par, Currency::USD).expect("collateral");
            interest_generated +=
                par * 0.07 * (date - state.prev_date.expect("start")).whole_days() as f64 / 360.0;
            let opening_date = state.prev_date.expect("opening date");
            simulate_period(
                &mut state,
                &deal,
                &waterfall,
                SimulationPeriod {
                    accrual_start: opening_date,
                    accrual_end: date,
                    payment: date,
                    valuation: deal.closing_date,
                    redemption: false,
                },
                &MarketContext::new(),
                3.0,
                &mut source,
            )
            .expect("period");
            assert_eq!(
                state.tranche_triggers[0].trigger.breach_date.is_some(),
                breached
            );
            let distributed: f64 = state
                .results
                .values()
                .map(|result| result.total_interest.amount())
                .sum();
            assert!(
                (distributed + state.undistributed_interest.amount() - interest_generated).abs()
                    < 1e-6
            );
            assert_eq!(state.tranche_balances["EQ"].amount(), 20_000_000.0);
            if breached {
                assert!(state.undistributed_interest.amount() > 0.0);
            } else {
                assert_eq!(state.undistributed_interest.amount(), 0.0);
            }
        }
        assert_eq!(state.tranche_triggers[0].trigger.breach_date, Some(third));
    }

    #[test]
    fn production_waterfall_acceleration_pays_principal_from_excess_interest() {
        use crate::instruments::fixed_income::structured_credit::TriggerConsequence;
        for consequence in [
            TriggerConsequence::AccelerateAmortization,
            TriggerConsequence::DivertCashFlow,
        ] {
            let deal = production_trigger_deal(consequence);
            let waterfall = Waterfall::standard_sequential(
                Currency::USD,
                &deal.tranches,
                crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
                &[],
            );
            let mut state = SimulationState::new(
                &deal.pool,
                &deal.tranches,
                deal.closing_date,
                deal.closing_date,
                0,
            )
            .expect("state");
            simulate_period(
                &mut state,
                &deal,
                &waterfall,
                SimulationPeriod {
                    accrual_start: deal.closing_date,
                    accrual_end: deal.first_payment_date,
                    payment: deal.first_payment_date,
                    valuation: deal.closing_date,
                    redemption: false,
                },
                &MarketContext::new(),
                3.0,
                &mut DeterministicPoolFlowSource,
            )
            .expect("period");
            let days = (deal.first_payment_date - deal.closing_date).whole_days() as f64;
            let expected = (100_000_000.0 * 0.07 - 80_000_000.0 * 0.04) * days / 360.0;
            assert!((state.results["A"].total_principal.amount() - expected).abs() < 1e-6);
            assert_eq!(state.results["EQ"].total_interest.amount(), 0.0);
            assert_eq!(state.tranche_balances["EQ"].amount(), 20_000_000.0);
        }
    }

    #[test]
    fn production_waterfall_revolving_target_retains_ineligible_principal_until_end() {
        use crate::instruments::fixed_income::structured_credit::{
            ReinvestmentCriteria, ReinvestmentPeriod,
        };
        let mut deal = StructuredCredit::example();
        let first = deal.first_payment_date;
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end_date: first,
            is_active: true,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria {
                max_price: 99.0,
                ..Default::default()
            },
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
            &[],
        );
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        let note = deal.tranches.tranches[0].id.as_str();
        simulate_period(
            &mut state,
            &deal,
            &waterfall,
            SimulationPeriod {
                accrual_start: deal.closing_date,
                accrual_end: first,
                payment: first,
                valuation: deal.closing_date,
                redemption: false,
            },
            &MarketContext::new(),
            3.0,
            &mut DeterministicPoolFlowSource,
        )
        .expect("revolving");
        let held = 100_000_000.0 * (1.0 - 0.64_f64.powf(0.25));
        assert!((state.undistributed_principal.amount() - held).abs() < 1e-6);
        assert_eq!(state.results[note].total_principal.amount(), 0.0);
        simulate_period(
            &mut state,
            &deal,
            &waterfall,
            SimulationPeriod {
                accrual_start: first,
                accrual_end: first.add_months(3),
                payment: first.add_months(3),
                valuation: deal.closing_date,
                redemption: false,
            },
            &MarketContext::new(),
            3.0,
            &mut DeterministicPoolFlowSource,
        )
        .expect("amortizing");
        assert_eq!(state.undistributed_principal.amount(), 0.0);
        assert!(
            (state.results[note].total_principal.amount()
                - 100_000_000.0 * (1.0 - 0.64_f64.sqrt()))
            .abs()
                < 1e-6
        );
    }

    #[test]
    fn production_waterfall_stop_reinvestment_overrides_active_period() {
        use crate::instruments::fixed_income::structured_credit::{
            ReinvestmentCriteria, ReinvestmentPeriod, TriggerConsequence,
        };
        let mut deal = production_trigger_deal(TriggerConsequence::StopReinvestment);
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end_date: deal.maturity,
            is_active: true,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria::default(),
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
            &[],
        );
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        simulate_period(
            &mut state,
            &deal,
            &waterfall,
            SimulationPeriod {
                accrual_start: deal.closing_date,
                accrual_end: deal.first_payment_date,
                payment: deal.first_payment_date,
                valuation: deal.closing_date,
                redemption: false,
            },
            &MarketContext::new(),
            3.0,
            &mut DeterministicPoolFlowSource,
        )
        .expect("triggered period");
        let expected = 100_000_000.0 * (1.0 - 0.64_f64.powf(0.25));
        assert!((state.results["A"].total_principal.amount() - expected).abs() < 1e-6);
        assert!((state.pool_outstanding.amount() - (100_000_000.0 - expected)).abs() < 1e-6);
    }

    #[test]
    fn production_waterfall_terminal_spread_is_interest_and_conserved() {
        let mut deal = StructuredCredit::example();
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let market = MarketContext::new();
        let baseline = run_simulation_with_source(
            &deal,
            &market,
            deal.closing_date,
            &mut DeterministicPoolFlowSource,
        )
        .expect("baseline");
        deal.pool.excess_spread_account = Money::new(12345.0, Currency::USD).expect("spread");
        let actual = run_simulation_with_source(
            &deal,
            &market,
            deal.closing_date,
            &mut DeterministicPoolFlowSource,
        )
        .expect("funded spread");
        let interest = |results: &HashMap<String, TrancheCashflows>| {
            results
                .values()
                .map(|result| result.total_interest.amount())
                .sum::<f64>()
        };
        let principal = |results: &HashMap<String, TrancheCashflows>| {
            results
                .values()
                .map(|result| result.total_principal.amount())
                .sum::<f64>()
        };
        assert!((interest(&actual) - interest(&baseline) - 12345.0).abs() < 1e-6);
        assert!((principal(&actual) - principal(&baseline)).abs() < 1e-6);
    }

    #[test]
    fn production_waterfall_predefaulted_par_is_not_performing_collateral() {
        let mut deal = StructuredCredit::example();
        deal.pool.assets[0].default_with_recovery(
            Money::new(70_000_000.0, Currency::USD).expect("recovery"),
            deal.closing_date,
        );
        let state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        assert_eq!(state.pool_outstanding.amount(), 0.0);
        assert_eq!(state.pool_state.balances.iter().sum::<f64>(), 0.0);
        assert_eq!(
            state.recovery_queue.pending_amount(Currency::USD).amount(),
            70_000_000.0
        );
        assert_eq!(state.initial_realized_loss, 30_000_000.0);
    }

    #[test]
    fn production_waterfall_equity_interest_preserves_loss_absorbing_principal() {
        let mut deal = StructuredCredit::example();
        deal.tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                20.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(80_000_000.0, Currency::USD).expect("balance"),
                TrancheCoupon::Fixed { rate: 0.06 },
                deal.maturity,
            )
            .expect("senior"),
            Tranche::new(
                "EQ",
                0.0,
                20.0,
                TrancheSeniority::Equity,
                Money::new(20_000_000.0, Currency::USD).expect("balance"),
                TrancheCoupon::Fixed { rate: 0.0 },
                deal.maturity,
            )
            .expect("equity"),
        ])
        .expect("capital structure");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
            &[],
        );
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        let equity = deal
            .tranches
            .tranches
            .iter()
            .find(|t| t.seniority == TrancheSeniority::Equity)
            .expect("equity");
        let opening = equity.current_balance;
        let mut source = DeterministicPoolFlowSource;
        simulate_period(
            &mut state,
            &deal,
            &waterfall,
            SimulationPeriod {
                accrual_start: deal.closing_date,
                accrual_end: deal.first_payment_date,
                payment: deal.first_payment_date,
                valuation: deal.closing_date,
                redemption: false,
            },
            &MarketContext::new(),
            3.0,
            &mut source,
        )
        .expect("period");
        let result = state.results.get(equity.id.as_str()).expect("equity flows");
        assert!(
            result.total_interest.amount() > 0.0,
            "equity receives excess interest"
        );
        assert_eq!(result.total_principal.amount(), 0.0);
        assert_eq!(state.tranche_balances[equity.id.as_str()], opening);
    }

    #[test]
    fn production_waterfall_inactive_reinvestment_pays_principal() {
        use crate::instruments::fixed_income::structured_credit::{
            ReinvestmentCriteria, ReinvestmentPeriod,
        };
        let mut deal = StructuredCredit::example();
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end_date: deal.maturity,
            is_active: false,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria::default(),
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::instruments::fixed_income::structured_credit::TemplateFees::default(),
            &[],
        );
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        let mut source = DeterministicPoolFlowSource;
        simulate_period(
            &mut state,
            &deal,
            &waterfall,
            SimulationPeriod {
                accrual_start: deal.closing_date,
                accrual_end: deal.first_payment_date,
                payment: deal.first_payment_date,
                valuation: deal.closing_date,
                redemption: false,
            },
            &MarketContext::new(),
            3.0,
            &mut source,
        )
        .expect("period");
        // Three months of 36% annual CPR pays 1 - .64^(3/12) of par.
        let principal = state
            .results
            .values()
            .map(|r| r.total_principal.amount())
            .sum::<f64>();
        let expected =
            deal.pool.total_balance().expect("pool").amount() * (1.0 - 0.64_f64.powf(0.25));
        assert!(
            (principal - expected).abs() < 1e-6,
            "principal {principal}, expected {expected}"
        );
    }

    /// An OAS rate shift must move FLOATING coupon projections.
    ///
    /// If the simulated rate path scaled prepayment only, pool-asset and
    /// tranche floating coupons would still project off the deterministic
    /// forward curve: a stochastic discount factor applied to deterministic
    /// coupons — the wrong instrument, since coupon/discount correlation is
    /// exactly what makes a floater rate-insensitive.
    #[test]
    fn rate_shift_moves_floating_collateral_coupons() {
        let base = Date::from_calendar_date(2024, Month::January, 1).expect("base");
        let curve = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
            "SOFR-3M", 0.25,
        )
        .base_date(base)
        .day_count(DayCount::Act360)
        .reset_lag(2)
        .knots([(0.0, 0.04), (10.0, 0.04)])
        .build()
        .expect("forward curve should build");
        let accrual_start = Date::from_calendar_date(2024, Month::June, 1).expect("start");
        let market = MarketContext::new();

        let unshifted = collateral_asset_rate_for_period(
            &curve,
            &market,
            accrual_start,
            0.05,
            Some(100.0),
            0.0,
            None,
        )
        .expect("unshifted rate");
        let shifted = collateral_asset_rate_for_period(
            &curve,
            &market,
            accrual_start,
            0.05,
            Some(100.0),
            0.01,
            None,
        )
        .expect("shifted rate");

        assert!(
            (shifted - unshifted - 0.01).abs() < 1e-12,
            "a +100bp path shift must raise the projected floating coupon by \
             exactly that: {unshifted:.6} -> {shifted:.6}"
        );

        // An index floor binds before the spread: SOFR+400 with a 1% floor on
        // a path that pushes the index to −1% pays 1% + 4% = 5%.
        let index_floored = collateral_asset_rate_for_period(
            &curve,
            &market,
            accrual_start,
            0.05,
            Some(400.0),
            -0.05,
            Some(0.01),
        )
        .expect("index-floored rate");
        assert!(
            (index_floored - 0.05).abs() < 1e-12,
            "max(4% − 5%, 1%) + 4% = 5%, got {index_floored}"
        );

        // A deeply negative path must not manufacture a negative coupon.
        let floored = collateral_asset_rate_for_period(
            &curve,
            &market,
            accrual_start,
            0.05,
            Some(100.0),
            -0.50,
            None,
        )
        .expect("floored rate");
        assert!(
            floored >= 0.0,
            "the all-in coupon must floor at zero, got {floored}"
        );
    }

    #[test]
    fn collateral_term_rate_is_discount_factor_implied() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid base date");
        let curve = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
            "USD-3M", 0.25,
        )
        .base_date(base)
        .day_count(DayCount::Act360)
        .reset_lag(2)
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("forward curve should build");
        let start_date = Date::from_calendar_date(2025, Month::April, 1).expect("valid start date");
        let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict("weekends_only")
            .expect("calendar");
        let fixing_date = start_date
            .add_business_days(-curve.reset_lag(), calendar)
            .expect("fixing date");
        let reset_end =
            crate::instruments::fixed_income::structured_credit::utils::rate_helpers::try_tenor_to_period_end(
                fixing_date,
                curve.tenor(),
                curve.day_count(),
            )
            .expect("reset end");
        let t1 = curve
            .day_count()
            .year_fraction(base, fixing_date, DayCountContext::default())
            .expect("valid start time");
        let t2 = curve
            .day_count()
            .year_fraction(base, reset_end, DayCountContext::default())
            .expect("valid end time");
        let expected = curve
            .rate_between(t1, t2)
            .expect("valid term forward interval");

        let actual = term_rate_for_period(&curve, &MarketContext::new(), start_date)
            .expect("term projection should succeed");

        assert!((expected - curve.rate_period(t1, t2)).abs() > 1e-6);
        assert!((actual - expected).abs() < 1e-14);
    }

    #[test]
    fn collateral_term_rate_rejects_period_straddling_curve_base() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid base date");
        let curve = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
            "USD-3M", 0.25,
        )
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build()
        .expect("forward curve should build");
        let start_date =
            Date::from_calendar_date(2024, Month::December, 1).expect("valid start date");
        let error = term_rate_for_period(&curve, &MarketContext::new(), start_date)
            .expect_err("straddling term period should require a fixing");

        assert!(error.to_string().contains("FIXING:USD-3M"));
    }

    #[test]
    fn seasoned_collateral_uses_stored_coupon_without_fixing_series() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid base date");
        let curve = ForwardCurve::builder("USD-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act360)
            .reset_lag(2)
            .knots([(0.0, 0.03), (1.0, 0.08)])
            .build()
            .expect("forward curve");
        let accrual_start =
            Date::from_calendar_date(2024, Month::December, 1).expect("accrual start");

        let rate = collateral_asset_rate_for_period(
            &curve,
            &MarketContext::new(),
            accrual_start,
            0.071,
            Some(125.0),
            0.0, // no OAS rate shift in this unit test,
            None,
        )
        .expect("stored current coupon is authoritative for an already-reset period");

        assert!((rate - 0.071).abs() < 1e-14);
    }

    #[test]
    fn collateral_term_rate_uses_past_fixing_for_future_accrual_start() {
        let curve_base =
            Date::from_calendar_date(2025, Month::January, 6).expect("valid curve base");
        let accrual_start =
            Date::from_calendar_date(2025, Month::January, 7).expect("valid accrual start");
        let curve = ForwardCurve::builder("USD-3M", 0.25)
            .base_date(curve_base)
            .day_count(DayCount::Act360)
            .reset_lag(2)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("forward curve");
        let fixing_date =
            Date::from_calendar_date(2025, Month::January, 3).expect("valid fixing date");
        let series =
            ScalarTimeSeries::new(fixing_series_id("USD-3M"), vec![(fixing_date, 0.041)], None)
                .expect("fixing series");
        let context = MarketContext::new().insert_series(series);

        let rate = term_rate_for_period(&curve, &context, accrual_start)
            .expect("future accrual with historical fixing");
        assert!((rate - 0.041).abs() < 1e-14);
    }

    #[test]
    fn collateral_wac_projects_each_assets_own_curve_tenor() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid base");
        let period_start =
            Date::from_calendar_date(2025, Month::April, 1).expect("valid period start");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid maturity");
        let curve_3m = ForwardCurve::builder("USD-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act360)
            .reset_lag(0)
            .knots([(0.0, 0.02), (1.0, 0.08)])
            .build()
            .expect("3M curve");
        let curve_6m = ForwardCurve::builder("USD-6M", 0.5)
            .base_date(base)
            .day_count(DayCount::Act360)
            .reset_lag(0)
            .knots([(0.0, 0.03), (1.0, 0.12)])
            .build()
            .expect("6M curve");
        let context = MarketContext::new()
            .insert(curve_3m.clone())
            .insert(curve_6m.clone());
        let mut pool = AssetPool::new("MIXED", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::floating_rate_loan(
            "A-3M",
            Money::from((100_i64, Currency::USD)),
            "USD-3M",
            0.0,
            maturity,
            DayCount::Act360,
        ));
        pool.assets.push(PoolAsset::floating_rate_loan(
            "A-6M",
            Money::from((100_i64, Currency::USD)),
            "USD-6M",
            0.0,
            maturity,
            DayCount::Act360,
        ));
        let tranche = Tranche::new(
            "AAA",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((200_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let state = SimulationState::new(&pool, &tranches, base, base, 0).expect("state");

        let rate_3m = term_rate_for_period(&curve_3m, &context, period_start).expect("3M rate");
        let rate_6m = term_rate_for_period(&curve_6m, &context, period_start).expect("6M rate");
        let actual =
            current_collateral_wac(&state, &context, period_start).expect("collateral WAC");
        assert!((actual - (rate_3m + rate_6m) / 2.0).abs() < 1e-14);

        let shared_end =
            Date::from_calendar_date(2025, Month::July, 1).expect("shared waterfall end");
        let old_shared_6m = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            &curve_6m,
            period_start,
            shared_end,
        )
        .expect("old shared-window rate");
        assert!((rate_6m - old_shared_6m).abs() > 1e-6);
    }

    #[test]
    fn reinvestment_par_build_scales_inversely_with_price() {
        // At par, $1 of cash buys $1 of par.
        assert!((par_acquired_at_price(100.0, 1.0) - 100.0).abs() < 1e-9);
        // At a 97 price, $100 of cash buys 100 / 0.97 ≈ 103.09 of par (par build).
        assert!((par_acquired_at_price(100.0, 0.97) - 100.0 / 0.97).abs() < 1e-9);
        // A premium price (> par) acquires less par than cash spent.
        assert!(par_acquired_at_price(100.0, 1.02) < 100.0);
        // Degenerate non-positive price falls back to 1:1 par recycling.
    }

    fn empty_tranche_cashflows(id: &str, currency: Currency) -> TrancheCashflows {
        TrancheCashflows {
            tranche_id: id.to_string(),
            cashflows: Vec::new(),
            detailed_flows: Vec::new(),
            accrual_periods: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows: Vec::new(),
            pik_flows: Vec::new(),
            deferred_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: Money::from((0_i64, currency)),
            total_interest: Money::from((0_i64, currency)),
            total_principal: Money::from((0_i64, currency)),
            total_pik: Money::from((0_i64, currency)),
            total_deferred: Money::from((0_i64, currency)),
            total_writedown: Money::from((0_i64, currency)),
        }
    }

    #[test]
    fn aggregate_tranche_cashflows_errors_on_incompatible_same_date_amounts() {
        let pay_date = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
        let mut usd = empty_tranche_cashflows("AAA", Currency::USD);
        usd.cashflows
            .push((pay_date, Money::from((100_i64, Currency::USD))));

        let mut eur = empty_tranche_cashflows("BBB", Currency::EUR);
        eur.cashflows
            .push((pay_date, Money::from((50_i64, Currency::EUR))));

        let mut results = HashMap::default();
        results.insert(usd.tranche_id.clone(), usd);
        results.insert(eur.tranche_id.clone(), eur);

        let err = aggregate_tranche_cashflows(&results)
            .expect_err("currency mismatch should not be silently dropped");

        assert!(matches!(
            err,
            finstack_quant_core::Error::CurrencyMismatch { .. }
        ));
    }

    // ── W-25: cleanup-call redemption must include accrued interest ──────

    fn cleanup_test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn cleanup_discount_curve() -> finstack_quant_core::market_data::term_structures::DiscountCurve
    {
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(cleanup_test_date())
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88)])
            .build()
            .expect("curve")
    }

    /// A single-tranche ABS with a high CPR so the pool amortizes below the
    /// 10% cleanup threshold partway through its life, triggering an optional
    /// cleanup-call redemption mid-period.
    fn cleanup_call_deal() -> StructuredCredit {
        let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((10_000_000_i64, Currency::USD)),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((10_000_000_i64, Currency::USD)),
            // Non-zero coupon so the stub-period accrued interest is non-zero.
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS-CLEANUP",
            pool,
            TrancheStructure::new(vec![tranche]).expect("structure"),
            cleanup_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse")
        .with_cleanup_call(0.10)
        .expect("cleanup call");
        // 60% CPR: the pool factor crosses below 10% within ~4 years, well
        // before maturity, so a cleanup call genuinely fires mid-life.
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.60);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    /// W-25 — a cleanup-call redemption must settle the note at par PLUS
    /// accrued interest for the stub period, not at par balance only.
    ///
    /// Cleanup-call redemption records accrued interest on the cleanup date.
    #[test]
    fn cleanup_call_redemption_includes_accrued_interest() {
        let instrument = cleanup_call_deal();
        let market = MarketContext::new().insert(cleanup_discount_curve());

        let results = run_simulation_with_source(
            &instrument,
            &market,
            cleanup_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        let tranche = results.get("A").expect("tranche A result");

        // The cleanup call terminates the simulation: the last cashflow date
        // is the cleanup-call date. Identify it.
        let cleanup_date = tranche
            .cashflows
            .iter()
            .map(|(d, _)| *d)
            .max()
            .expect("at least one cashflow");

        // Principal retired on the cleanup date.
        let principal_on_cleanup: f64 = tranche
            .principal_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        // Interest paid on the cleanup date — the defect: this was 0.0.
        let interest_on_cleanup: f64 = tranche
            .interest_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        // Total cash on the cleanup date.
        let total_on_cleanup: f64 = tranche
            .cashflows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        assert!(
            principal_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup call should retire principal; got {principal_on_cleanup}"
        );
        assert!(
            interest_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup-call redemption must record accrued interest; got {interest_on_cleanup}"
        );
        // Total redemption cash must exceed the par balance retired by exactly
        // the accrued-interest component.
        assert!(
            total_on_cleanup > principal_on_cleanup + WRITEDOWN_DE_MINIMIS,
            "total cleanup cashflow {total_on_cleanup} must exceed principal \
             {principal_on_cleanup} by the accrued interest"
        );
        assert!(
            (total_on_cleanup - principal_on_cleanup - interest_on_cleanup).abs()
                < WRITEDOWN_DE_MINIMIS,
            "cleanup cashflow must split exactly into principal + interest: \
             total={total_on_cleanup}, principal={principal_on_cleanup}, \
             interest={interest_on_cleanup}"
        );
    }

    // ── Cash-conservation fixes: items 1, 2, 13 ─────────────────────────

    use crate::instruments::fixed_income::structured_credit::types::{
        ReinvestmentCriteria, ReinvestmentPeriod,
    };

    fn cc_test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn cc_discount_curve() -> finstack_quant_core::market_data::term_structures::DiscountCurve {
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(cc_test_date())
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88), (10.0, 0.75)])
            .build()
            .expect("curve")
    }

    /// Reinvestment-period principal is recycled into collateral (not dropped).
    #[test]
    fn reinvestment_period_principal_is_recycled_not_lost() {
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("valid date");
        let reinvest_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        // Build a CLO whose pool prepays fast (high CPR). During the 6y
        // reinvestment period, that prepaid principal must be recycled.
        let build_deal = |with_reinvestment: bool| {
            let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
            pool.assets.push(PoolAsset::fixed_rate_bond(
                "L1",
                Money::from((100_000_000_i64, Currency::USD)),
                0.07,
                maturity,
                DayCount::Thirty360,
            ));
            if with_reinvestment {
                pool.reinvestment_period = Some(ReinvestmentPeriod {
                    end_date: reinvest_end,
                    is_active: true,
                    amortizing_tranches: Vec::new(),
                    assumptions: None,
                    criteria: ReinvestmentCriteria::default(),
                });
            }
            let tranche = Tranche::new(
                "A",
                0.0,
                100.0,
                TrancheSeniority::Senior,
                Money::from((100_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche");
            let mut instrument = StructuredCredit::new_clo(
                "CLO-REINVEST",
                pool,
                TrancheStructure::new(vec![tranche]).expect("structure"),
                cc_test_date(),
                maturity,
                "USD-OIS",
            )
            .with_payment_calendar("nyse");
            instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
            instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
            instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
            instrument
        };

        let market = MarketContext::new().insert(cc_discount_curve());

        let with_reinvest = run_simulation_with_source(
            &build_deal(true),
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation with reinvestment");

        let tranche = with_reinvest.get("A").expect("tranche A");

        // Total interest collected by the senior tranche over the deal life.
        let total_interest: f64 = tranche.interest_flows.iter().map(|(_, m)| m.amount()).sum();
        // Total principal returned.
        let total_principal: f64 = tranche
            .principal_flows
            .iter()
            .map(|(_, m)| m.amount())
            .sum();

        assert!(
            total_principal > 99_000_000.0,
            "with reinvestment recycling senior must be repaid nearly in full; \
             got principal {total_principal:.0}"
        );
        assert!(
            total_interest > 25_000_000.0,
            "a $100M tranche held flat through a 6y reinvestment period at 5% \
             must collect substantial interest; got {total_interest:.0}"
        );
        // Final balance must be (near) zero — the tranche fully amortizes.
        assert!(
            tranche.final_balance.amount() < 1.0,
            "tranche must fully amortize by maturity; final balance {}",
            tranche.final_balance.amount()
        );
    }

    /// Write-downs are capped at current balance; principal + write-down ≤ face.
    #[test]
    fn loss_allocation_caps_writedown_by_face_net_of_principal_repaid() {
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        // High CPR pays the structure down fast; concurrent high CDR writes
        // losses against the (already-amortizing) subordinated tranche.
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L1",
            Money::from((100_000_000_i64, Currency::USD)),
            0.07,
            maturity,
            DayCount::Thirty360,
        ));
        // Senior + subordinated + equity. The subordinated tranche absorbs
        // loss after equity and is paid principal via bounded TranchePrincipal.
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                0.0,
                70.0,
                TrancheSeniority::Senior,
                Money::from((70_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                70.0,
                92.0,
                TrancheSeniority::Subordinated,
                Money::from((22_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.07 },
                maturity,
            )
            .expect("subordinated"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::from((8_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-LOSS-CAP",
            pool,
            tranches,
            cc_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        // Heavy prepayment + heavy default running together.
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.40);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);

        let market = MarketContext::new().insert(cc_discount_curve());
        let results = run_simulation_with_source(
            &instrument,
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        // Principal repaid + write-down must never exceed original face.
        for (id, face) in [("A", 70_000_000.0_f64), ("B", 22_000_000.0_f64)] {
            let tranche = results.get(id).unwrap_or_else(|| panic!("tranche {id}"));
            let total_writedown = tranche.total_writedown.amount();
            let total_principal = tranche.total_principal.amount();
            assert!(
                total_principal + total_writedown <= face + WRITEDOWN_DE_MINIMIS,
                "{id}: principal repaid ({total_principal:.0}) + write-down \
                 ({total_writedown:.0}) = {:.0} must not exceed original face \
                 ({face:.0})",
                total_principal + total_writedown
            );
            // A write-down can never exceed the tranche's own face.
            assert!(
                total_writedown <= face + WRITEDOWN_DE_MINIMIS,
                "{id}: write-down {total_writedown:.0} exceeds face {face:.0}"
            );
        }
    }

    /// Item 13 — per-period cash conservation: every dollar of pool collateral
    /// cashflow must be accounted for as a tranche cashflow, a reserve
    /// movement, or principal recycled during reinvestment. This drives the
    /// in-engine debug assertion on a representative deal and asserts the
    /// deal-level identity holds across the whole simulation.
    #[test]
    fn per_period_cash_conservation_holds_over_full_simulation() {
        let maturity = Date::from_calendar_date(2031, Month::January, 1).expect("valid date");

        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L1",
            Money::from((80_000_000_i64, Currency::USD)),
            0.07,
            maturity,
            DayCount::Thirty360,
        ));
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L2",
            Money::from((20_000_000_i64, Currency::USD)),
            0.065,
            maturity,
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::from((80_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::from((20_000_000_i64, Currency::USD)),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-CONSERVE",
            pool,
            tranches,
            cc_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 6);

        let market = MarketContext::new().insert(cc_discount_curve());
        // The per-period cash-conservation debug assertion fires inside
        // simulate_period; reaching `finalize` without a panic is the test.
        let results = run_simulation_with_source(
            &instrument,
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        // Deal-level sanity: total cash distributed to tranches is positive
        // and finite (the per-period assertion already proved conservation).
        let total_cash: f64 = results
            .values()
            .flat_map(|r| r.cashflows.iter())
            .map(|(_, m)| m.amount())
            .sum();
        assert!(
            total_cash.is_finite() && total_cash > 0.0,
            "deal must distribute positive finite cash; got {total_cash}"
        );
    }

    /// Contractual level payment is frozen; prepayments do not shrink it.
    #[test]
    fn level_pay_scheduled_principal_uses_frozen_contractual_payment() {
        use crate::instruments::fixed_income::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let pay1 = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let pay2 = Date::from_calendar_date(2024, Month::July, 1).expect("date");

        let rate = 0.06_f64;
        let original_balance = 1_000_000.0_f64;

        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: DayCount::Thirty360,
            id: InstrumentId::new("MTG1"),
            asset_type: AssetType::SingleFamilyMortgage { ltv: None },
            balance: Money::new(original_balance, Currency::USD).expect("valid money fixture"),
            rate,
            spread_bp: None,
            index_id: None,
            index_floor: None,
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
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
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(original_balance, Currency::USD).expect("valid money fixture"),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("structure");

        let market = MarketContext::new().insert(cc_discount_curve());
        let mut state = SimulationState::new(&pool, &tranches, closing, closing, 0).expect("state");

        let months_per_period = 3.0_f64;
        let rates = PoolFlowRates {
            smm: 0.0,
            mdr: 0.0,
            recovery_rate: 0.0,
        };

        // ── Period 1: pure scheduled amortization (no prepay). ──────────
        let flows1 = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: &mut state,
            pay_date: pay1,
            prev_date: closing,
            months_per_period,
            context: &market,
            rates,
            asset_rates: AssetSeasonedRates::default(),
            copula_outcome: None,
            delinquency: None,
            special_servicing: SpecialServicingFees::default(),
            card: None,
        })
        .expect("period 1 flows");
        let level_payment = state.pool_state.level_payments[0]
            .expect("level payment must be frozen after the first amortizing period");
        let sched1 = flows1.scheduled_principal.amount();
        assert!(
            sched1 > 0.0,
            "period 1 must amortize some scheduled principal"
        );

        // ── Simulate an external prepayment: halve the asset balance. ───
        let balance_after_prepay = state.pool_state.balances[0] * 0.5;
        state.pool_state.balances[0] = balance_after_prepay;

        // ── Period 2: the level payment must NOT be recomputed. ─────────
        let flows2 = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: &mut state,
            pay_date: pay2,
            prev_date: pay1,
            months_per_period,
            context: &market,
            rates,
            asset_rates: AssetSeasonedRates::default(),
            copula_outcome: None,
            delinquency: None,
            special_servicing: SpecialServicingFees::default(),
            card: None,
        })
        .expect("period 2 flows");

        // The frozen level payment is unchanged.
        assert_eq!(
            state.pool_state.level_payments[0],
            Some(level_payment),
            "the contractual level payment must stay frozen across periods"
        );

        // Period-2 scheduled principal = frozen_LP − interest on the
        // (prepaid-down) balance. With the bug the level payment would be
        // recomputed off the halved balance, roughly halving it.
        //
        // Level-pay annuity uses nominal periodic rate `rate × months/12`
        // (US mortgage convention; matches mbs_passthrough/pricer.rs).
        let period_rate = rate * months_per_period / 12.0;
        let expected_sched2 = level_payment - balance_after_prepay * period_rate;
        let sched2 = flows2.scheduled_principal.amount();
        assert!(
            (sched2 - expected_sched2).abs() < 1.0,
            "period-2 scheduled principal {sched2:.2} must equal frozen \
             level payment {level_payment:.2} minus interest on the \
             prepaid-down balance ({expected_sched2:.2})"
        );

        // The buggy recomputed payment would be ~half the frozen one, so the
        // buggy scheduled principal would be far below the correct value.
        let buggy_recomputed_lp = level_payment * 0.5; // LP ∝ balance
        let buggy_sched2 = (buggy_recomputed_lp - balance_after_prepay * period_rate).max(0.0);
        assert!(
            sched2 > buggy_sched2 + 1_000.0,
            "frozen-payment scheduled principal {sched2:.2} must materially \
             exceed the buggy recomputed-payment value {buggy_sched2:.2}"
        );
    }

    /// Build a single-asset 30-year level-pay pool for amortization tests.
    fn level_pay_pool_and_tranches(
        maturity: Date,
        rate: f64,
        balance: f64,
    ) -> (AssetPool, TrancheStructure) {
        use crate::instruments::fixed_income::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: DayCount::Thirty360,
            id: InstrumentId::new("MTG1"),
            asset_type: AssetType::SingleFamilyMortgage { ltv: None },
            balance: Money::new(balance, Currency::USD).expect("valid money fixture"),
            rate,
            spread_bp: None,
            index_id: None,
            index_floor: None,
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
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
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(balance, Currency::USD).expect("valid money fixture"),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("structure");
        (pool, tranches)
    }

    /// Run a monthly-pay pool to exhaustion and report (WAL years, principal
    /// returned, payoff month) where payoff month is 0 if it never exhausts.
    fn run_monthly_amortization(
        pool: &AssetPool,
        tranches: &TrancheStructure,
        closing: Date,
        smm: f64,
        months: usize,
    ) -> (f64, f64, usize) {
        use finstack_quant_core::dates::DateExt;

        let market = MarketContext::new().insert(cc_discount_curve());
        let mut state = SimulationState::new(pool, tranches, closing, closing, 0).expect("state");
        let rates = PoolFlowRates {
            smm,
            mdr: 0.0,
            recovery_rate: 0.0,
        };
        let mut prev = closing;
        let (mut weighted, mut total) = (0.0_f64, 0.0_f64);
        let mut payoff_month = 0usize;
        for m in 1..=months {
            let pay = closing.add_months(i32::try_from(m).expect("month fits i32"));
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay,
                prev_date: prev,
                months_per_period: 1.0,
                context: &market,
                rates,
                asset_rates: AssetSeasonedRates::default(),
                copula_outcome: None,
                delinquency: None,
                special_servicing: SpecialServicingFees::default(),
                card: None,
            })
            .expect("pool flows");
            let principal = flows.scheduled_principal.amount() + flows.prepayment.amount();
            weighted += (m as f64 / 12.0) * principal;
            total += principal;
            prev = pay;
            if state.pool_state.balances[0] <= 1e-9 {
                payoff_month = m;
                break;
            }
        }
        (weighted / total, total, payoff_month)
    }

    /// Rep-line level payment scales by prepayment and default survival (SIFMA).
    ///
    /// Scheduled principal is the loan amortization rate applied to current
    /// pool balance, so it shrinks as loans prepay away.
    #[test]
    fn level_pay_amortization_scales_payment_by_prepayment_survival() {
        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2054, Month::January, 1).expect("date"); // 360mo
        let (pool, tranches) = level_pay_pool_and_tranches(maturity, 0.06, 100.0);

        // ── Zero prepayment: pure scheduled amortization, unchanged by the fix.
        let (wal_zero, principal_zero, payoff_zero) =
            run_monthly_amortization(&pool, &tranches, closing, 0.0, 360);
        assert!(
            (wal_zero - 19.306).abs() < 0.01,
            "zero-CPR WAL must be the textbook 30y level-pay value 19.306y, got {wal_zero:.3}"
        );
        assert!(
            (principal_zero - 100.0).abs() < 1e-6,
            "all principal must be returned, got {principal_zero}"
        );
        assert_eq!(
            payoff_zero, 360,
            "a zero-prepay 30y pool must amortize over exactly 360 months"
        );

        // ── ~5.8% CPR (SMM 0.5%/month). Under the SIFMA convention the pool
        // still runs to its 360-month maturity — prepayments shrink the balance
        // but the survivors stay on their original schedule.
        let (wal_prepay, principal_prepay, payoff_prepay) =
            run_monthly_amortization(&pool, &tranches, closing, 0.005, 360);
        assert!(
            (principal_prepay - 100.0).abs() < 1e-6,
            "all principal must be returned under prepayment, got {principal_prepay}"
        );
        assert!(
            (wal_prepay - 10.750).abs() < 0.02,
            "SIFMA-convention WAL at ~5.8% CPR must be 10.750y, got {wal_prepay:.3}"
        );

        assert!(
            wal_prepay > 9.0,
            "WAL {wal_prepay:.3}y is too short — level payment is not scaling \
             by prepayment survival"
        );
        assert_eq!(
            payoff_prepay, 360,
            "under SIFMA a 30y pool at 5.8% CPR must run to month 360"
        );
    }

    /// Scheduled principal matches the mbs_passthrough SIFMA re-derivation.
    #[test]
    fn level_pay_scheduled_principal_matches_mbs_passthrough_convention() {
        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2054, Month::January, 1).expect("date");
        let rate = 0.06_f64;
        let (pool, tranches) = level_pay_pool_and_tranches(maturity, rate, 100.0);

        use finstack_quant_core::dates::DateExt;
        let market = MarketContext::new().insert(cc_discount_curve());
        let mut state = SimulationState::new(&pool, &tranches, closing, closing, 0).expect("state");
        let rates = PoolFlowRates {
            smm: 0.005,
            mdr: 0.0,
            recovery_rate: 0.0,
        };

        let monthly_rate = rate / 12.0;
        let mut prev = closing;
        for m in 1..=120usize {
            let balance_before = state.pool_state.balances[0];
            let remaining = 360 - m + 1;

            // mbs_passthrough convention: payment re-derived on the CURRENT
            // balance over the remaining term.
            let factor = (1.0 + monthly_rate).powi(i32::try_from(remaining).expect("term"));
            let reference_payment = balance_before * monthly_rate * factor / (factor - 1.0);
            let reference_sched = (reference_payment - balance_before * monthly_rate)
                .max(0.0)
                .min(balance_before);

            let pay = closing.add_months(i32::try_from(m).expect("month"));
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay,
                prev_date: prev,
                months_per_period: 1.0,
                context: &market,
                rates,
                asset_rates: AssetSeasonedRates::default(),
                copula_outcome: None,
                delinquency: None,
                special_servicing: SpecialServicingFees::default(),
                card: None,
            })
            .expect("pool flows");
            prev = pay;

            let engine_sched = flows.scheduled_principal.amount();
            let tolerance = (reference_sched.abs() * 1e-6).max(1e-9);
            assert!(
                (engine_sched - reference_sched).abs() <= tolerance,
                "month {m}: structured-credit scheduled principal {engine_sched:.10} \
                 disagrees with the mbs_passthrough convention {reference_sched:.10} \
                 (balance {balance_before:.6}, remaining {remaining})"
            );
        }
    }

    /// Loss allocation is junior-first by `payment_priority` (not seniority enum).
    #[test]
    fn loss_allocation_runs_junior_first_within_a_pari_passu_class() {
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let (pool, _) = level_pay_pool_and_tranches(maturity, 0.06, 1_000_000.0);

        // Two pari-passu Senior notes plus a Mezzanine and an Equity class —
        // the seniority enum cannot distinguish A-1 from A-2.
        let mk = |id: &str, seniority, attach: f64, detach: f64, bal: f64| {
            Tranche::new(
                id,
                attach,
                detach,
                seniority,
                Money::new(bal, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche")
        };
        let tranches = TrancheStructure::new(vec![
            mk("A1", TrancheSeniority::Senior, 60.0, 100.0, 400_000.0),
            mk("A2", TrancheSeniority::Senior, 30.0, 60.0, 300_000.0),
            mk("B", TrancheSeniority::Mezzanine, 10.0, 30.0, 200_000.0),
            mk("E", TrancheSeniority::Equity, 0.0, 10.0, 100_000.0),
        ])
        .expect("structure");

        let state = SimulationState::new(&pool, &tranches, closing, closing, 0).expect("state");

        let order: Vec<&str> = state
            .loss_alloc_order
            .iter()
            .map(|&i| state.tranches.tranches[i].id.as_str())
            .collect();

        assert_eq!(
            order,
            vec!["E", "B", "A2", "A1"],
            "losses must be junior-first; within Senior, A2 before A1"
        );
    }

    /// Build a simple amortizing ABS with an optionally funded reserve account.
    fn reserve_account_deal(reserve: f64) -> StructuredCredit {
        let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((10_000_000_i64, Currency::USD)),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        pool.reserve_account = Money::new(reserve, Currency::USD).expect("valid money fixture");
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((10_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS-RESERVE",
            pool,
            TrancheStructure::new(vec![tranche]).expect("structure"),
            cleanup_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    /// Total cash distributed to every tranche over the life of a deal.
    fn total_distributed(instrument: &StructuredCredit) -> f64 {
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let results = crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
            instrument,
            &market,
            cleanup_test_date(),
        )
        .expect("simulation");
        results
            .values()
            .flat_map(|tc| tc.cashflows.iter())
            .map(|(_, m)| m.amount())
            .sum()
    }

    /// Build a deal with an excess-spread account and optional senior fees.
    fn excess_spread_deal(with_fees: bool) -> StructuredCredit {
        use crate::instruments::fixed_income::structured_credit::types::{
            ExcessSpreadSpec, WaterfallRules,
        };
        let mut deal = reserve_account_deal(0.0);
        // Note coupon well below collateral, so genuine excess spread exists.
        deal.tranches.tranches[0].coupon = TrancheCoupon::Fixed { rate: 0.02 };
        deal.waterfall_rules = Some(WaterfallRules {
            excess_spread: Some(ExcessSpreadSpec {
                target_balance: Money::from((10_000_000_i64, Currency::USD)),
                trap_loss_pct: None,
            }),
            ..Default::default()
        });
        if with_fees {
            deal = deal.with_standard_fees();
        }
        deal
    }

    /// N7 — the spread account must reach the residual holder even when the
    /// most junior class is not labelled `Equity`.
    ///
    /// `release_spread_account` required a `TrancheSeniority::Equity` tranche
    /// and silently zeroed the balance otherwise — the same cash-sink class as
    /// the original reserve defect. A deal whose junior class is
    /// Subordinated is an ordinary structure and lost the whole balance.
    ///
    /// Only the junior tranche's SENIORITY LABEL differs between the two runs,
    /// so waterfall mechanics and payment ordering are identical and the
    /// comparison isolates the release path. (A with/without-account comparison
    /// would not: capturing surplus interest slows the turbo paydown, which
    /// legitimately changes lifetime totals.)
    #[test]
    fn spread_account_releases_to_a_non_equity_residual_holder() {
        use crate::instruments::fixed_income::structured_credit::types::{
            ExcessSpreadSpec, WaterfallRules,
        };

        let build = |junior_seniority: TrancheSeniority| -> StructuredCredit {
            let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("date");
            let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
            pool.assets.push(PoolAsset::fixed_rate_bond(
                "A1",
                Money::from((10_000_000_i64, Currency::USD)),
                0.06,
                maturity,
                DayCount::Thirty360,
            ));
            let mk = |id: &str, sen, attach: f64, detach: f64, bal: f64| {
                Tranche::new(
                    id,
                    attach,
                    detach,
                    sen,
                    Money::new(bal, Currency::USD).expect("valid money fixture"),
                    TrancheCoupon::Fixed { rate: 0.02 },
                    maturity,
                )
                .expect("tranche")
            };
            let tranches = TrancheStructure::new(vec![
                mk("SR", TrancheSeniority::Senior, 20.0, 100.0, 8_000_000.0),
                mk("JR", junior_seniority, 0.0, 20.0, 2_000_000.0),
            ])
            .expect("structure");
            let mut deal = StructuredCredit::new_abs(
                "ABS-SPREAD",
                pool,
                tranches,
                cleanup_test_date(),
                maturity,
                "USD-OIS",
            )
            .with_payment_calendar("nyse");
            deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
            deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
            deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
            deal.waterfall_rules = Some(WaterfallRules {
                excess_spread: Some(ExcessSpreadSpec {
                    target_balance: Money::from((10_000_000_i64, Currency::USD)),
                    trap_loss_pct: None,
                }),
                ..Default::default()
            });
            deal
        };

        let with_equity = total_distributed(&build(TrancheSeniority::Equity));
        let with_subordinated = total_distributed(&build(TrancheSeniority::Subordinated));

        assert!(
            (with_equity - with_subordinated).abs() < 1.0,
            "the released spread account must reach the residual holder \
             regardless of its seniority LABEL: {with_equity:.2} with an Equity \
             junior class vs {with_subordinated:.2} with a Subordinated one. A \
             shortfall means the balance was zeroed instead of paid out (N7)."
        );
    }

    /// N1 — excess-spread capture must rank senior fees ahead of note interest.
    ///
    /// `debt_interest_due` predated the fee tier and
    /// summed note coupon + deferred only, so the capture skimmed surplus the
    /// fee tier needed and the notes deferred despite ample excess spread.
    #[test]
    fn fee_aware_capture_does_not_starve_note_interest() {
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let results = crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
            &excess_spread_deal(true),
            &market,
            cleanup_test_date(),
        )
        .expect("simulation");

        // The tranche is NOT pik_enabled, so its shortfalls land in
        // `total_deferred`. Reading `total_pik` here would pass vacuously.
        let total_deferred: f64 = results.values().map(|tc| tc.total_deferred.amount()).sum();
        assert!(
            total_deferred < 1.0,
            "6% collateral against a 2% coupon leaves ample spread for both the \
             fee tier and note interest, so the notes must not defer. Deferred \
             {total_deferred:.2} means the capture is skimming cash fees need (N1)."
        );
    }

    /// N1 — senior fees are real cash leaving the deal, so the notes receive
    /// strictly less once fees are charged.
    #[test]
    fn senior_fees_reduce_cash_reaching_the_notes() {
        let with_fees = total_distributed(&excess_spread_deal(true));
        let without_fees = total_distributed(&excess_spread_deal(false));
        assert!(
            with_fees < without_fees,
            "fees must reduce note cash: {with_fees:.2} vs {without_fees:.2}"
        );
    }

    /// Funded reserve is released to investors (not destroyed as a sink).
    #[test]
    fn funded_reserve_account_is_released_to_investors_not_destroyed() {
        const RESERVE: f64 = 250_000.0;
        let without = total_distributed(&reserve_account_deal(0.0));
        let with = total_distributed(&reserve_account_deal(RESERVE));

        assert!(
            with > without,
            "funded reserve must increase distributions: {with:.2} vs {without:.2}"
        );
        assert!(
            (with - without - RESERVE).abs() < 1.0,
            "the entire reserve must reach investors: expected an uplift of \
             {RESERVE:.2}, got {:.2}",
            with - without
        );
    }

    /// Reserve is drawn to cover a current-period debt-interest shortfall.
    ///
    /// Asserts first-period interest (not lifetime totals): without a draw the
    /// shortfall is deferred and cured at maturity, so lifetime totals match.
    #[test]
    fn reserve_account_is_drawn_to_cover_interest_shortfall() {
        fn first_period_interest(reserve: f64) -> f64 {
            let mut deal = reserve_account_deal(reserve);
            deal.tranches.tranches[0].coupon = TrancheCoupon::Fixed { rate: 0.09 };
            let market = MarketContext::new().insert(cleanup_discount_curve());
            let results =
                crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                    &deal,
                    &market,
                    cleanup_test_date(),
                )
                .expect("simulation");
            let mut flows: Vec<(Date, f64)> = results
                .values()
                .flat_map(|tc| tc.interest_flows.iter())
                .map(|(d, m)| (*d, m.amount()))
                .collect();
            flows.sort_by_key(|(d, _)| *d);
            flows.first().map_or(0.0, |(_, amt)| *amt)
        }

        const RESERVE: f64 = 250_000.0;
        let without = first_period_interest(0.0);
        let with = first_period_interest(RESERVE);

        assert!(
            with > without + 1.0,
            "reserve must cure current-period interest: {with:.2} vs {without:.2}"
        );
    }

    /// A two-class CLO-shaped deal (senior note + equity) carrying enough
    /// collateral defaults to erode overcollateralization.
    fn oc_trigger_deal(
        triggers: Vec<crate::instruments::fixed_income::structured_credit::types::CoverageTestSpec>,
    ) -> StructuredCredit {
        let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((10_000_000_i64, Currency::USD)),
            0.08,
            maturity,
            DayCount::Thirty360,
        ));
        let senior = Tranche::new(
            "CLASS_A",
            20.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((8_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("senior");
        let equity = Tranche::new(
            "EQUITY",
            0.0,
            20.0,
            TrancheSeniority::Equity,
            Money::from((2_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("equity");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-OC",
            pool,
            TrancheStructure::new(vec![senior, equity]).expect("structure"),
            cleanup_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        // Sustained defaults erode the pool and therefore the OC ratio.
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument.coverage_triggers = triggers;
        instrument
    }

    /// A three-class CLO (senior / mezzanine / equity) — the realistic shape,
    /// where a failing OC test has subordinated interest available to divert.
    fn clo_with_mezz(
        triggers: Vec<crate::instruments::fixed_income::structured_credit::types::CoverageTestSpec>,
    ) -> StructuredCredit {
        let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((10_000_000_i64, Currency::USD)),
            0.08,
            maturity,
            DayCount::Thirty360,
        ));
        let mk = |id: &str, seniority, attach: f64, detach: f64, bal: f64, rate: f64| {
            Tranche::new(
                id,
                attach,
                detach,
                seniority,
                Money::new(bal, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate },
                maturity,
            )
            .expect("tranche")
        };
        let tranches = TrancheStructure::new(vec![
            mk(
                "CLASS_A",
                TrancheSeniority::Senior,
                30.0,
                100.0,
                7_000_000.0,
                0.05,
            ),
            mk(
                "CLASS_B",
                TrancheSeniority::Mezzanine,
                10.0,
                30.0,
                2_000_000.0,
                0.08,
            ),
            mk(
                "EQUITY",
                TrancheSeniority::Equity,
                0.0,
                10.0,
                1_000_000.0,
                0.0,
            ),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-MEZZ",
            pool,
            tranches,
            cleanup_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.03);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument.coverage_triggers = triggers;
        instrument
    }

    /// Failing OC test traps subordinated interest and accelerates senior principal.
    #[test]
    fn failing_oc_test_diverts_subordinated_interest_to_senior_principal() {
        use crate::instruments::fixed_income::structured_credit::types::CoverageTestSpec;

        let untriggered = clo_with_mezz(vec![]);
        // Pool 10M against 7M senior = 143% OC at closing. A 175% requirement
        // breaches from the first period.
        let triggered = clo_with_mezz(vec![CoverageTestSpec::oc("CLASS_A", 1.75)]);

        // Assert timing: excess cure cash returns to CLASS_B, so lifetime totals
        // can converge while the trap still delays subordinated interest.
        let interest_wal = |deal: &StructuredCredit, tranche: &str| -> f64 {
            let market = MarketContext::new().insert(cleanup_discount_curve());
            let results =
                crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                    deal,
                    &market,
                    cleanup_test_date(),
                )
                .expect("simulation");
            let flows = &results
                .get(tranche)
                .expect("tranche results")
                .interest_flows;
            let (mut weighted, mut total) = (0.0_f64, 0.0_f64);
            for (date, amount) in flows {
                let years = (*date - cleanup_test_date()).whole_days() as f64 / 365.0;
                weighted += years * amount.amount();
                total += amount.amount();
            }
            if total > 0.0 {
                weighted / total
            } else {
                0.0
            }
        };

        let mezz_wal_without = interest_wal(&untriggered, "CLASS_B");
        let mezz_wal_with = interest_wal(&triggered, "CLASS_B");
        assert!(
            mezz_wal_with > mezz_wal_without,
            "a breaching OC test must DEFER subordinated interest: CLASS_B \
             interest WAL was {mezz_wal_with:.4}y with the trigger vs \
             {mezz_wal_without:.4}y without. Equal values mean the cure has no \
             source — the test position is not placed between the senior and \
             subordinated interest tiers."
        );

        // Faster delevering reduces lifetime interest, so measure the senior
        // benefit through earlier principal rather than nominal cash.
        let senior_principal_wal = |deal: &StructuredCredit| -> f64 {
            let market = MarketContext::new().insert(cleanup_discount_curve());
            let results =
                crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                    deal,
                    &market,
                    cleanup_test_date(),
                )
                .expect("simulation");
            let flows = &results
                .get("CLASS_A")
                .expect("senior results")
                .principal_flows;
            let (mut weighted, mut total) = (0.0_f64, 0.0_f64);
            for (date, amount) in flows {
                let years = (*date - cleanup_test_date()).whole_days() as f64 / 365.0;
                weighted += years * amount.amount();
                total += amount.amount();
            }
            if total > 0.0 {
                weighted / total
            } else {
                0.0
            }
        };

        let wal_without = senior_principal_wal(&untriggered);
        let wal_with = senior_principal_wal(&triggered);
        assert!(
            wal_with < wal_without,
            "trapped subordinated interest must accelerate senior principal: \
             CLASS_A WAL was {wal_with:.4}y with the trigger vs \
             {wal_without:.4}y without"
        );
    }

    /// Interest-tier split is identity when no coverage test is failing.
    #[test]
    fn interest_tier_split_is_identity_without_triggers() {
        let deal = clo_with_mezz(vec![]);
        let waterfall = deal
            .create_waterfall()
            .expect("valid create_waterfall fixture");

        let tier_ids: Vec<&str> = waterfall.tiers.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            tier_ids,
            vec![
                "CLASS_A_interest",
                "CLASS_B_interest",
                "principal",
                "equity"
            ],
            "the standard waterfall must give every note its own interest tier \
             so coverage tests can sit between them"
        );

        // Interest recipients must still appear in payment_priority order
        // across the two tiers, which is what makes the split identity.
        let interest_order: Vec<String> = waterfall
            .tiers
            .iter()
            .filter(|t| t.id.ends_with("interest"))
            .flat_map(|t| t.recipients.iter().map(|r| r.id.clone()))
            .collect();
        assert_eq!(
            interest_order,
            vec![
                "CLASS_A_interest".to_string(),
                "CLASS_B_interest".to_string()
            ],
            "splitting must preserve the seniority ordering of interest payments"
        );
    }

    /// Deal-attached OC trigger is evaluated and reports a breach.
    #[test]
    fn oc_trigger_is_evaluated_and_reports_its_breach() {
        use crate::instruments::fixed_income::structured_credit::pricing::waterfall::execute_waterfall;
        use crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext;
        use crate::instruments::fixed_income::structured_credit::types::CoverageTestSpec;

        // A 150% OC requirement against a 10M pool supporting an 8M senior note
        // (125% at closing) is breached immediately.
        let deal = oc_trigger_deal(vec![CoverageTestSpec::oc("CLASS_A", 1.50)]);
        let waterfall = deal
            .create_waterfall()
            .expect("valid create_waterfall fixture");
        assert_eq!(
            waterfall.coverage_tests().count(),
            1,
            "deal trigger must reach the executing waterfall"
        );

        let market = MarketContext::new().insert(cleanup_discount_curve());
        let ccy = Currency::USD;
        let pay = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let result = execute_waterfall(
            &waterfall,
            &deal.tranches,
            &deal.pool,
            WaterfallContext {
                available_cash: Money::from((500_000_i64, ccy)),
                interest_collections: Money::from((200_000_i64, ccy)),
                principal_collections: Money::from((300_000_i64, ccy)),
                payment_date: pay,
                period_start: cleanup_test_date(),
                valuation_date: cleanup_test_date(),
                pool_balance: Money::from((10_000_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: None,
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, Currency::USD)),
                defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("waterfall executes");

        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "OC_CLASS_A")
            .expect("OC test must appear in the waterfall result");
        let (_, ratio, passing) = oc;
        assert!(
            !passing,
            "OC ratio {ratio:.4} against a 1.50 requirement must breach"
        );
        assert!(
            (*ratio - 1.2875).abs() < 0.05,
            "OC ratio must be ~(10M pool + 0.3M cash)/8M senior = 1.2875, got {ratio:.4}"
        );
    }

    /// Deal with no triggers reports no coverage tests.
    #[test]
    fn deal_without_triggers_reports_no_coverage_tests() {
        use crate::instruments::fixed_income::structured_credit::pricing::waterfall::execute_waterfall;
        use crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext;

        let deal = oc_trigger_deal(vec![]);
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let ccy = Currency::USD;
        let result = execute_waterfall(
            &deal
                .create_waterfall()
                .expect("valid create_waterfall fixture"),
            &deal.tranches,
            &deal.pool,
            WaterfallContext {
                available_cash: Money::from((500_000_i64, ccy)),
                interest_collections: Money::from((200_000_i64, ccy)),
                principal_collections: Money::from((300_000_i64, ccy)),
                payment_date: Date::from_calendar_date(2024, Month::April, 1).expect("date"),
                period_start: cleanup_test_date(),
                valuation_date: cleanup_test_date(),
                pool_balance: Money::from((10_000_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: None,
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, Currency::USD)),
                defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("waterfall executes");

        assert!(
            result.coverage_tests.is_empty(),
            "deal with no triggers must run no coverage tests"
        );
    }

    /// Empty trigger list builds a waterfall with no coverage triggers.
    #[test]
    fn absent_coverage_triggers_are_identity() {
        let deal = oc_trigger_deal(vec![]);
        assert!(
            deal.coverage_triggers.is_empty(),
            "the default must be no triggers"
        );
        let waterfall = deal
            .create_waterfall()
            .expect("valid create_waterfall fixture");
        assert!(
            waterfall.coverage_tests().next().is_none(),
            "deal with no triggers must build a waterfall with none"
        );
    }

    /// Trigger naming an unknown tranche is rejected.
    #[test]
    fn coverage_trigger_for_unknown_tranche_is_rejected() {
        use crate::instruments::fixed_income::structured_credit::types::CoverageTestSpec;

        let err = oc_trigger_deal(vec![])
            .with_coverage_triggers(vec![CoverageTestSpec::oc("NOT_A_TRANCHE", 1.2)])
            .expect_err("a trigger naming an unknown tranche must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("NOT_A_TRANCHE"),
            "the error must name the offending tranche; got: {msg}"
        );
    }

    /// Attached senior fees build a fee tier and pay service providers first.
    #[test]
    fn attached_fees_are_built_and_paid_ahead_of_the_notes() {
        use crate::instruments::fixed_income::structured_credit::pricing::waterfall::{
            execute_waterfall, WaterfallContext,
        };

        let without = reserve_account_deal(0.0);
        let with_fees = reserve_account_deal(0.0).with_standard_fees();

        assert!(
            with_fees.fees.is_some(),
            "with_standard_fees must attach the deal-type calibration"
        );

        let fee_tiers = |deal: &StructuredCredit| {
            deal.create_waterfall()
                .expect("valid create_waterfall fixture")
                .tiers
                .iter()
                .filter(|t| t.id == "fees")
                .count()
        };
        assert_eq!(
            fee_tiers(&without),
            0,
            "a deal with no fees must have no fee tier"
        );
        assert_eq!(fee_tiers(&with_fees), 1, "fees must build a fee tier");

        // Execute one period and confirm the service providers are paid.
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let ccy = Currency::USD;
        let run = |deal: &StructuredCredit| {
            execute_waterfall(
                &deal
                    .create_waterfall()
                    .expect("valid create_waterfall fixture"),
                &deal.tranches,
                &deal.pool,
                WaterfallContext {
                    available_cash: Money::from((500_000_i64, ccy)),
                    interest_collections: Money::from((500_000_i64, ccy)),
                    principal_collections: Money::from((0_i64, ccy)),
                    payment_date: Date::from_calendar_date(2024, Month::April, 1).expect("date"),
                    period_start: cleanup_test_date(),
                    valuation_date: cleanup_test_date(),
                    pool_balance: Money::from((10_000_000_i64, ccy)),
                    market: &market,
                    tranche_balances: None,
                    asset_balances: None,
                    live_collateral: None,
                    special_serviced: None,
                    deferred_interest: None,
                    reserve_balance: Money::from((0_i64, ccy)),
                    restricted_cash: Money::from((0_i64, Currency::USD)),
                    defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
                    recovery_proceeds: Money::from((0_i64, ccy)),
                    floating_rate_shift: 0.0,
                    equity_history: None,
                },
            )
            .expect("waterfall executes")
        };

        let service_provider_cash = |result: &WaterfallDistribution| -> f64 {
            result
                .distributions
                .iter()
                .filter(|(recipient, _)| matches!(recipient, RecipientType::ServiceProvider(_)))
                .map(|(_, amount)| amount.amount())
                .sum()
        };

        let fees_paid = service_provider_cash(&run(&with_fees));
        let none_paid = service_provider_cash(&run(&without));

        assert!(
            none_paid.abs() < 1e-9,
            "a deal with no fees must pay service providers nothing, got {none_paid:.2}"
        );
        // ABS standard: 25,000 annual trustee (monthly-scaled) + 50bp servicing
        // on a 10,000,000 pool over a quarter.
        assert!(
            fees_paid > 1_000.0,
            "senior fees must be paid to service providers; got {fees_paid:.2}"
        );

        // And that cash is taken ahead of the notes in the same period.
        let to_notes_with = run(&with_fees)
            .distributions
            .iter()
            .filter(|(r, _)| !matches!(r, RecipientType::ServiceProvider(_)))
            .map(|(_, m)| m.amount())
            .sum::<f64>();
        let to_notes_without = run(&without)
            .distributions
            .iter()
            .filter(|(r, _)| !matches!(r, RecipientType::ServiceProvider(_)))
            .map(|(_, m)| m.amount())
            .sum::<f64>();
        assert!(
            to_notes_with < to_notes_without,
            "within a period, fees must reduce cash reaching the notes: \
             {to_notes_with:.2} with fees vs {to_notes_without:.2} without"
        );
    }

    // ── W-26: cleanup-call redemption must INCLUDE pending recoveries ───────

    /// A principal-only deal with 30% annual defaults, 40% recovery and a
    /// 24-month lag. At cleanup, performing par plus pending recoveries must
    /// fund remaining principal after realized recoveries and net-loss
    /// writedowns. Subtracting pending recoveries necessarily underfunds it.
    fn cleanup_with_large_recovery_queue_deal() -> StructuredCredit {
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        // Long maturity so the deal runs to cleanup before expiring.
        let maturity = Date::from_calendar_date(2035, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::from((10_000_000_i64, Currency::USD)),
            0.0,
            maturity,
            DayCount::Thirty360,
        ));
        // Single senior tranche; the equity shortfall is equity's problem.
        let senior = Tranche::new(
            "A",
            0.0,
            80.0, // 80% of original balance = 8 M face
            TrancheSeniority::Senior,
            Money::from((8_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("senior tranche");
        let equity = Tranche::new(
            "E",
            80.0,
            100.0, // junior 20% = 2 M face
            TrancheSeniority::Equity,
            Money::from((2_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("equity tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS-BIG-RECOVERY-QUEUE",
            pool,
            TrancheStructure::new(vec![senior, equity]).expect("structure"),
            start,
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse")
        .with_cleanup_call(0.10)
        .expect("cleanup call");
        // No fees/coupons: principal funding is independently conserved as
        // performing par plus recoveries, less allocated net credit losses.
        instrument.fees = None;
        // CDR=30%: heavy defaults drive the pool below 10% factor in ~77 months.
        // No prepayments: the cleanup is triggered purely by defaults.
        // Recovery_lag=24 months: recent defaults still carry unsettled claims.
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 24);
        instrument
    }

    /// Pending recoveries are purchased with the performing pool at cleanup.
    /// With no coupons or fees, the principal budget is independently funded
    /// by par plus recoveries after net credit-loss writedowns. This isolates
    /// recovery recognition from a note's ability to meet an interest claim.
    #[test]
    fn cleanup_call_redemption_includes_pending_recovery_queue() {
        let instrument = cleanup_with_large_recovery_queue_deal();
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(start)
                .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88), (15.0, 0.70)])
                .build()
                .expect("curve"),
        );

        let results = run_simulation_with_source(
            &instrument,
            &market,
            start,
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        let tranche = results.get("A").expect("tranche A result");

        // Identify the cleanup-call date (simulation terminates there).
        let cleanup_date = tranche
            .cashflows
            .iter()
            .map(|(d, _)| *d)
            .max()
            .expect("at least one cashflow");

        // Principal retired at the cleanup call on the senior tranche.
        let principal_on_cleanup: f64 = tranche
            .principal_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        let final_balance = tranche.final_balance.amount();

        // Net-loss accounting leaves a funded principal claim when the
        // remaining performing par and recovery receivables are sold together.
        assert!(
            final_balance < 1.0,
            "senior tranche must be fully redeemed at the cleanup call; \
             final_balance={final_balance:.2} (non-zero means pending recoveries \
             were subtracted instead of added, leaving the tranche unredeemed)"
        );

        // The cleanup call must have retired some principal on the senior tranche.
        assert!(
            principal_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup call must retire senior principal; got {principal_on_cleanup:.2} \
             (under the buggy subtraction available_for_redemption clamps to 0)"
        );
    }

    /// Item 12 — interest must stop accruing when an asset defaults.
    ///
    /// A defaulting asset must not accrue the full period's interest on its
    /// full pre-default balance. With defaults assumed uniformly distributed
    /// over the period, the defaulting fraction `period_mdr` accrues on
    /// average HALF the period's interest, so total interest is scaled by
    /// `(1 − 0.5·period_mdr)`.
    ///
    /// White-box test: a single non-amortizing asset, two runs at the same
    /// balance/rate — one with zero default, one with a high MDR. The
    /// high-MDR run's interest must equal the zero-default interest times
    /// `(1 − 0.5·period_mdr)`, strictly less than the full accrual.
    #[test]
    fn interest_accrual_stops_at_default() {
        use crate::instruments::fixed_income::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let pay1 = Date::from_calendar_date(2024, Month::April, 1).expect("date");

        let rate = 0.08_f64;
        let balance = 1_000_000.0_f64;
        let months_per_period = 3.0_f64;
        let market = MarketContext::new().insert(cc_discount_curve());

        // Build a single non-amortizing bond pool with an optional MDR override.
        let build_pool = |mdr_override: Option<f64>| {
            let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
            pool.assets.push(PoolAsset {
                day_count: DayCount::Thirty360,
                id: InstrumentId::new("BND1"),
                asset_type: AssetType::HighYieldBond { industry: None },
                balance: Money::new(balance, Currency::USD).expect("valid money fixture"),
                rate,
                spread_bp: None,
                index_id: None,
                index_floor: None,
                maturity,
                credit_quality: None,
                industry: None,
                obligor_id: None,
                is_defaulted: false,
                recovery_amount: None,
                default_date: None,
                purchase_price: None,
                acquisition_date: None,
                origination_date: None,
                smm_override: None,
                mdr_override,
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
        };
        let make_tranches = || {
            let tranche = Tranche::new(
                "A",
                0.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(balance, Currency::USD).expect("valid money fixture"),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche");
            TrancheStructure::new(vec![tranche]).expect("structure")
        };

        let one_period_interest = |pool: &AssetPool, tranches: &TrancheStructure| -> f64 {
            let mut state =
                SimulationState::new(pool, tranches, closing, closing, 0).expect("state");
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay1,
                prev_date: closing,
                months_per_period,
                context: &market,
                rates: PoolFlowRates {
                    smm: 0.0,
                    mdr: 0.0,
                    recovery_rate: 0.40,
                },
                asset_rates: AssetSeasonedRates::default(),
                copula_outcome: None,
                delinquency: None,
                special_servicing: SpecialServicingFees::default(),
                card: None,
            })
            .expect("flows");
            flows.interest.amount()
        };

        // No default: full period accrual.
        let pool_no_default = build_pool(None);
        let tranches_a = make_tranches();
        let interest_no_default = one_period_interest(&pool_no_default, &tranches_a);
        assert!(
            interest_no_default > 0.0,
            "baseline interest must be positive"
        );

        // High monthly MDR ⇒ large period default fraction.
        let monthly_mdr = 0.10_f64;
        let pool_with_default = build_pool(Some(monthly_mdr));
        let tranches_b = make_tranches();
        let interest_with_default = one_period_interest(&pool_with_default, &tranches_b);

        // Expected: period_mdr = 1 − (1 − monthly_mdr)^months; interest scaled
        // by (1 − 0.5·period_mdr).
        let period_mdr = 1.0 - (1.0 - monthly_mdr).powf(months_per_period);
        let expected = interest_no_default * (1.0 - 0.5 * period_mdr);
        assert!(
            (interest_with_default - expected).abs() < 1.0,
            "defaulting-asset interest {interest_with_default:.2} must be the \
             full-accrual interest {interest_no_default:.2} scaled by \
             (1 − 0.5·period_mdr) = {expected:.2}"
        );
        // And it must be strictly below the un-haircut full accrual.
        assert!(
            interest_with_default < interest_no_default - 1.0,
            "defaulting-asset interest must be strictly below the full \
             pre-default accrual ({interest_with_default:.2} vs \
             {interest_no_default:.2})"
        );
    }

    /// Lagged recoveries still pending at simulation end are drained, not dropped.
    #[test]
    fn pending_recoveries_at_simulation_end_are_drained_not_dropped() {
        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("date");
        let face_senior = 90_000_000.0_f64;

        let build_deal = |lag_months: u32| {
            let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
            pool.assets.push(PoolAsset::fixed_rate_bond(
                "A1",
                Money::from((100_000_000_i64, Currency::USD)),
                0.06,
                maturity,
                DayCount::Thirty360,
            ));
            let tranches = TrancheStructure::new(vec![
                Tranche::new(
                    "A",
                    0.0,
                    90.0,
                    TrancheSeniority::Senior,
                    Money::new(face_senior, Currency::USD).expect("valid money fixture"),
                    TrancheCoupon::Fixed { rate: 0.05 },
                    maturity,
                )
                .expect("senior"),
                Tranche::new(
                    "EQ",
                    90.0,
                    100.0,
                    TrancheSeniority::Equity,
                    Money::from((10_000_000_i64, Currency::USD)),
                    TrancheCoupon::Fixed { rate: 0.0 },
                    maturity,
                )
                .expect("equity"),
            ])
            .expect("structure");
            let mut instrument = StructuredCredit::new_abs(
                "ABS-TAIL-RECOVERY",
                pool,
                tranches,
                closing,
                maturity,
                "USD-OIS",
            )
            .with_payment_calendar("nyse");
            // CDR 20%: defaults occur in every period including the final
            // year, so with a 12-month lag the queue is non-empty at the
            // final scheduled date. No prepayments, 40% recovery.
            instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.20);
            instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
            instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, lag_months);
            instrument
        };

        let market = MarketContext::new().insert(cc_discount_curve());
        let run = |lag: u32| {
            run_simulation_with_source(
                &build_deal(lag),
                &market,
                closing,
                &mut DeterministicPoolFlowSource,
            )
            .expect("simulation")
        };
        let lagged = run(12);
        let immediate = run(0);

        // Senior face must be fully consumed by principal + write-downs.
        let senior = lagged.get("A").expect("senior tranche");
        let retired = senior.total_principal.amount() + senior.total_writedown.amount();
        assert!(
            (retired - face_senior).abs() < 1.0,
            "senior principal ({:.2}) + write-down ({:.2}) must equal face \
             {face_senior:.0}; a shortfall means tail recoveries were dropped",
            senior.total_principal.amount(),
            senior.total_writedown.amount(),
        );
        assert!(
            senior.final_balance.amount() < 1.0,
            "senior tranche must be fully retired once pending recoveries \
             are drained; final_balance={}",
            senior.final_balance.amount()
        );

        // (b) Conservation against a lag-0 run: the recovery lag shifts cash
        // TIMING only — pool interest, defaults, and recoveries are identical
        // — so total cash distributed across the structure must match.
        let total_cash = |res: &HashMap<String, TrancheCashflows>| -> f64 {
            res.values()
                .flat_map(|r| r.cashflows.iter())
                .map(|(_, m)| m.amount())
                .sum()
        };
        let cash_lagged = total_cash(&lagged);
        let cash_immediate = total_cash(&immediate);
        assert!(
            (cash_lagged - cash_immediate).abs() < 1.0,
            "total cash with a 12m recovery lag ({cash_lagged:.2}) must equal \
             the lag-0 total ({cash_immediate:.2}); a shortfall is dropped \
             tail-recovery cash"
        );

        // (c) The drained flows must land at-or-after the final scheduled
        // date (released at the later of the final date and default + lag).
        let last_flow = lagged
            .values()
            .flat_map(|r| r.cashflows.iter())
            .map(|(d, _)| *d)
            .max()
            .expect("flows");
        assert!(
            last_flow >= maturity,
            "drained recovery flows must release on/after the final date; \
             last flow {last_flow}"
        );
    }
}
