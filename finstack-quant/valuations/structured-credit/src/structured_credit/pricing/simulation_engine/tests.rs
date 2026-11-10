#[cfg(test)]
mod cases {
    use super::super::*;

    use crate::structured_credit::{AssetPool, DealType, PoolAsset, Tranche, TrancheStructure};
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::fixings::fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_valuations_core::results::structured_credit::TrancheSeniority;
    use time::Month;

    #[test]
    fn coupon_accrual_is_independent_of_payment_adjustment() {
        use time::macros::date;
        let mut deal = StructuredCredit::example().expect("example");
        deal.closing_date = date!(2024 - 03 - 30);
        deal.first_payment_date = date!(2024 - 06 - 30);
        deal.maturity = date!(2024 - 06 - 30);
        deal.pool.reinvestment_period = None;
        deal.cleanup_call_decimal = None;
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        for asset in &mut deal.pool.assets {
            asset.maturity = deal.maturity;
        }
        for tranche in &mut deal.tranches.tranches {
            tranche.maturity = deal.maturity;
        }
        let market = MarketContext::new();
        let run = |deal: &StructuredCredit| {
            simulate_with_source(
                deal,
                &market,
                deal.closing_date,
                Some(&mut DeterministicPoolFlowSource),
            )
            .expect("simulation")
        };
        deal.business_day_convention = Some(BusinessDayConvention::Unadjusted);
        let unadjusted = run(&deal);
        deal.business_day_convention = Some(BusinessDayConvention::ModifiedFollowing);
        let adjusted = run(&deal);
        for (id, expected) in unadjusted.tranches {
            let actual = &adjusted.tranches[&id];
            assert!(
                (actual.total_interest.amount() - expected.total_interest.amount()).abs() < 1e-6,
                "{id}: {} vs {}",
                actual.total_interest,
                expected.total_interest
            );
        }
    }

    #[test]
    fn contractual_maturity_pays_bullet_when_payment_rolls_backward() {
        use time::macros::date;
        let mut deal = StructuredCredit::example().expect("example");
        deal.closing_date = date!(2024 - 03 - 30);
        deal.first_payment_date = date!(2024 - 06 - 30);
        deal.maturity = date!(2024 - 06 - 30);
        deal.pool.reinvestment_period = None;
        deal.cleanup_call_decimal = None;
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        for asset in &mut deal.pool.assets {
            asset.maturity = deal.maturity;
        }
        for tranche in &mut deal.tranches.tranches {
            tranche.maturity = deal.maturity;
        }
        let run = simulate_with_source(
            &deal,
            &MarketContext::new(),
            deal.closing_date,
            Some(&mut DeterministicPoolFlowSource),
        )
        .expect("bullet simulation");
        let principal: f64 = run
            .tranches
            .values()
            .map(|tranche| tranche.total_principal.amount())
            .sum();
        let opening: f64 = deal
            .pool
            .assets
            .iter()
            .map(|asset| asset.balance.amount())
            .sum();
        assert!(
            (principal - opening).abs() < 1e-6,
            "{principal} vs {opening}"
        );
        for tranche in run.tranches.values() {
            assert!(
                tranche
                    .principal_flows
                    .iter()
                    .all(|(date, _)| *date == date!(2024 - 06 - 28))
            );
        }
    }

    fn production_trigger_deal(
        consequence: crate::structured_credit::TriggerConsequence,
    ) -> StructuredCredit {
        use crate::structured_credit::CoverageTrigger;
        let mut deal = StructuredCredit::example().expect("example");
        deal.tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                20.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(80_000_000.0, Currency::USD).expect("senior"),
                RateSpec::Fixed { rate: 0.04 },
                deal.maturity,
            )
            .expect("senior"),
            Tranche::new(
                "EQ",
                0.0,
                20.0,
                TrancheSeniority::Equity,
                Money::new(20_000_000.0, Currency::USD).expect("equity"),
                RateSpec::Fixed { rate: 0.0 },
                deal.maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        deal.tranches.tranches[0].oc_trigger = Some(CoverageTrigger {
            cure_level: Some(1.40),
            ..CoverageTrigger::new(1.30, consequence)
        });
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal
    }

    #[test]
    fn production_waterfall_reinvestment_preserves_decimal_cash_budget() {
        use crate::structured_credit::{ReinvestmentCriteria, ReinvestmentPeriod};
        let mut deal = StructuredCredit::example().expect("example");
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end: deal.maturity,
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
        );
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
        use crate::structured_credit::TriggerConsequence;
        let deal = production_trigger_deal(TriggerConsequence::TrapExcessSpread);
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::structured_credit::TemplateFees::default(),
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
        let second = first.add_months(3).expect("valid date shift");
        let third = second.add_months(3).expect("valid date shift");
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
                // A cured period releases the trap. Exact shortest-text Money
                // admission keeps the f64 waterfall's sub-cent residue, so this
                // uses the same 1e-6 tolerance as the conservation check above.
                assert!(state.undistributed_interest.amount().abs() < 1e-6);
            }
        }
        assert_eq!(state.tranche_triggers[0].trigger.breach_date, Some(third));
    }

    #[test]
    fn production_waterfall_acceleration_pays_principal_from_excess_interest() {
        use crate::structured_credit::TriggerConsequence;
        for consequence in [
            TriggerConsequence::AccelerateAmortization,
            TriggerConsequence::DivertCashFlow,
        ] {
            let deal = production_trigger_deal(consequence);
            let waterfall = Waterfall::standard_sequential(
                Currency::USD,
                &deal.tranches,
                crate::structured_credit::TemplateFees::default(),
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
        use crate::structured_credit::{ReinvestmentCriteria, ReinvestmentPeriod};
        let mut deal = StructuredCredit::example().expect("example");
        let first = deal.first_payment_date;
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end: first,
            is_active: true,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria {
                max_price_pct: 99.0,
                ..Default::default()
            },
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::structured_credit::TemplateFees::default(),
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
                accrual_end: first.add_months(3).expect("valid date shift"),
                payment: first.add_months(3).expect("valid date shift"),
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
        use crate::structured_credit::{
            ReinvestmentCriteria, ReinvestmentPeriod, TriggerConsequence,
        };
        let mut deal = production_trigger_deal(TriggerConsequence::StopReinvestment);
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end: deal.maturity,
            is_active: true,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria::default(),
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::structured_credit::TemplateFees::default(),
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
        let mut deal = StructuredCredit::example().expect("example");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let market = MarketContext::new();
        let baseline = simulate_with_source(
            &deal,
            &market,
            deal.closing_date,
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
        .expect("baseline");
        deal.pool.excess_spread_account = Money::new(12345.0, Currency::USD).expect("spread");
        let actual = simulate_with_source(
            &deal,
            &market,
            deal.closing_date,
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
        .expect("funded spread");
        let interest = |results: &FxHashMap<String, TrancheCashflows>| {
            results
                .values()
                .map(|result| result.total_interest.amount())
                .sum::<f64>()
        };
        let principal = |results: &FxHashMap<String, TrancheCashflows>| {
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
        let mut deal = StructuredCredit::example().expect("example");
        deal.pool.assets[0].defaulted = true;
        deal.pool.assets[0].recovery_amount =
            Some(Money::new(70_000_000.0, Currency::USD).expect("recovery"));
        deal.pool.assets[0].default_date = Some(deal.closing_date);
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
        let mut deal = StructuredCredit::example().expect("example");
        deal.tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                20.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(80_000_000.0, Currency::USD).expect("balance"),
                RateSpec::Fixed { rate: 0.06 },
                deal.maturity,
            )
            .expect("senior"),
            Tranche::new(
                "EQ",
                0.0,
                20.0,
                TrancheSeniority::Equity,
                Money::new(20_000_000.0, Currency::USD).expect("balance"),
                RateSpec::Fixed { rate: 0.0 },
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
            crate::structured_credit::TemplateFees::default(),
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
        use crate::structured_credit::{ReinvestmentCriteria, ReinvestmentPeriod};
        let mut deal = StructuredCredit::example().expect("example");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal.pool.reinvestment_period = Some(ReinvestmentPeriod {
            end: deal.maturity,
            is_active: false,
            amortizing_tranches: Vec::new(),
            assumptions: None,
            criteria: ReinvestmentCriteria::default(),
        });
        let waterfall = Waterfall::standard_sequential(
            Currency::USD,
            &deal.tranches,
            crate::structured_credit::TemplateFees::default(),
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
            period_helpers::CollateralCoupon {
                fallback_all_in_rate: 0.05,
                spread_bp: Some(100.0),
                day_count: DayCount::Act360,
                index_floor_rate: None,
            },
            0.0,
        )
        .expect("unshifted rate");
        let shifted = collateral_asset_rate_for_period(
            &curve,
            &market,
            accrual_start,
            period_helpers::CollateralCoupon {
                fallback_all_in_rate: 0.05,
                spread_bp: Some(100.0),
                day_count: DayCount::Act360,
                index_floor_rate: None,
            },
            0.01,
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
            period_helpers::CollateralCoupon {
                fallback_all_in_rate: 0.05,
                spread_bp: Some(400.0),
                day_count: DayCount::Act360,
                index_floor_rate: Some(0.01),
            },
            -0.05,
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
            period_helpers::CollateralCoupon {
                fallback_all_in_rate: 0.05,
                spread_bp: Some(100.0),
                day_count: DayCount::Act360,
                index_floor_rate: None,
            },
            -0.50,
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
        .day_count(DayCount::Act365F)
        .reset_lag(2)
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("forward curve should build");
        let start_date = Date::from_calendar_date(2025, Month::April, 1).expect("valid start date");
        let calendar =
            finstack_quant_core::dates::calendar_by_id_strict("weekends_only").expect("calendar");
        let fixing_date = start_date
            .add_business_days(-curve.reset_lag(), calendar)
            .expect("fixing date");
        let reset_end = crate::structured_credit::pricing::simulation_engine::period_helpers::try_tenor_to_period_end(
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
        let accrual = (reset_end - fixing_date).whole_days() as f64 / 360.0;
        let growth = curve.df(t1).expect("start projection DF")
            / curve.df(t2).expect("end projection DF")
            - 1.0;
        let expected = growth / accrual;

        let actual =
            term_rate_for_period(&curve, &MarketContext::new(), start_date, DayCount::Act360)
                .expect("term projection should succeed");

        assert!(
            (expected - curve.rate_period(t1, t2).expect("finite ordered period")).abs() > 1e-6
        );
        assert!((actual - expected).abs() < 1e-14);
        let curve_basis_rate = curve.rate_between(t1, t2).expect("curve-basis rate");
        assert!((actual - curve_basis_rate).abs() > 1e-5);
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
        let error =
            term_rate_for_period(&curve, &MarketContext::new(), start_date, DayCount::Act360)
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
            period_helpers::CollateralCoupon {
                fallback_all_in_rate: 0.071,
                spread_bp: Some(125.0),
                day_count: DayCount::Act360,
                index_floor_rate: None,
            },
            0.0, // no OAS rate shift in this unit test,
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

        let rate = term_rate_for_period(&curve, &context, accrual_start, DayCount::Act360)
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
            .day_count(DayCount::Act365F)
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
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let state = SimulationState::new(&pool, &tranches, base, base, 0).expect("state");

        let rate_3m = term_rate_for_period(&curve_3m, &context, period_start, DayCount::Act360)
            .expect("3M rate");
        let rate_6m = term_rate_for_period(&curve_6m, &context, period_start, DayCount::Act360)
            .expect("6M rate");
        let actual =
            current_collateral_wac(&state, &context, period_start).expect("collateral WAC");
        assert!((actual - (rate_3m + rate_6m) / 2.0).abs() < 1e-14);

        let shared_end =
            Date::from_calendar_date(2025, Month::July, 1).expect("shared waterfall end");
        let old_shared_6m = curve_6m
            .rate_between_dates(
                period_start,
                shared_end,
                DayCount::Act360
                    .year_fraction(period_start, shared_end, DayCountContext::default())
                    .expect("shared accrual"),
            )
            .expect("old shared-window rate");
        assert!((rate_6m - old_shared_6m).abs() > 1e-6);
    }

    /// `AssetPool::wac` and the simulated `PeriodDiagnostics::wac` average
    /// one population (performing fixed-rate collateral), so on the closing
    /// state they agree bit for bit. Independent reference: the hand WAC of
    /// the two performing fixed rows, (10·6% + 20·9%) / 30 = 8%. The floating
    /// row (400 bp) and the defaulted 12% row are outside the population; the
    /// pre-unification `AssetPool` WAC averaged all four rows over the full
    /// 65-unit balance instead (≈ 5.815%).
    #[test]
    fn pool_wac_matches_period_diagnostics_population() {
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("maturity");
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("base");
        let usd = |amount: i64| Money::from((amount, Currency::USD));
        let mut pool = AssetPool::new("WAC", DealType::Clo, Currency::USD);
        for (id, balance, rate) in [("F1", 10, 0.06), ("F2", 20, 0.09)] {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                id,
                usd(balance),
                rate,
                maturity,
                DayCount::Thirty360,
            ));
        }
        pool.assets.push(PoolAsset::floating_rate_loan(
            "L1",
            usd(30),
            "USD-3M",
            400.0,
            maturity,
            DayCount::Act360,
        ));
        let mut defaulted =
            PoolAsset::fixed_rate_bond("D1", usd(5), 0.12, maturity, DayCount::Thirty360);
        defaulted.defaulted = true;
        pool.assets.push(defaulted);
        let tranche = Tranche::new(
            "AAA",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            usd(60),
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let state = SimulationState::new(&pool, &tranches, base, base, 0).expect("state");

        let (diagnostics_wac, _, _) = state::pool_composition(&state).expect("composition");
        // Rounding only: two products and one division on exact inputs.
        let wac = pool.wac().expect("fixed-rate balance");
        assert!((wac - 0.08).abs() < 1e-15, "{wac}");
        assert_eq!(Some(wac), diagnostics_wac);
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
            RateSpec::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit {
            cleanup_call_decimal: Some(0.10),
            ..StructuredCredit::new_standard(
                crate::structured_credit::DealType::Abs,
                crate::structured_credit::StandardDealParams {
                    id: "ABS-CLEANUP".into(),
                    pool,
                    tranches: TrancheStructure::new(vec![tranche]).expect("structure"),
                    closing_date: cleanup_test_date(),
                    maturity,
                    discount_curve_id: "USD-OIS".into(),
                    calendar_id: None,
                },
            )
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse")
        };
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

        let results = simulate_with_source(
            &instrument,
            &market,
            cleanup_test_date(),
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
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

    use crate::structured_credit::types::{ReinvestmentCriteria, ReinvestmentPeriod};

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
                    end: reinvest_end,
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
                RateSpec::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche");
            let mut instrument = StructuredCredit::new_standard(
                crate::structured_credit::DealType::Clo,
                crate::structured_credit::StandardDealParams {
                    id: "CLO-REINVEST".into(),
                    pool,
                    tranches: TrancheStructure::new(vec![tranche]).expect("structure"),
                    closing_date: cc_test_date(),
                    maturity,
                    discount_curve_id: "USD-OIS".into(),
                    calendar_id: None,
                },
            )
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse");
            instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
            instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
            instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
            instrument
        };

        let market = MarketContext::new().insert(cc_discount_curve());

        let with_reinvest = simulate_with_source(
            &build_deal(true),
            &market,
            cc_test_date(),
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
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
                RateSpec::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                70.0,
                92.0,
                TrancheSeniority::Subordinated,
                Money::from((22_000_000_i64, Currency::USD)),
                RateSpec::Fixed { rate: 0.07 },
                maturity,
            )
            .expect("subordinated"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::from((8_000_000_i64, Currency::USD)),
                RateSpec::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Clo,
            crate::structured_credit::StandardDealParams {
                id: "CLO-LOSS-CAP".into(),
                pool,
                tranches,
                closing_date: cc_test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        // Heavy prepayment + heavy default running together.
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.40);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);

        let market = MarketContext::new().insert(cc_discount_curve());
        let results = simulate_with_source(
            &instrument,
            &market,
            cc_test_date(),
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
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
                RateSpec::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::from((20_000_000_i64, Currency::USD)),
                RateSpec::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Clo,
            crate::structured_credit::StandardDealParams {
                id: "CLO-CONSERVE".into(),
                pool,
                tranches,
                closing_date: cc_test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 6);

        let market = MarketContext::new().insert(cc_discount_curve());
        // The per-period cash-conservation debug assertion fires inside
        // simulate_period; reaching `finalize` without a panic is the test.
        let results = simulate_with_source(
            &instrument,
            &market,
            cc_test_date(),
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
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
        use crate::structured_credit::types::AssetType;
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
            forward_curve_id: None,
            index_floor_bp: None,
            maturity,
            rating: None,
            industry: None,
            obligor_id: None,
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
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(original_balance, Currency::USD).expect("valid money fixture"),
            RateSpec::Fixed { rate: 0.05 },
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
            accrual_end: pay1,
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
            accrual_end: pay2,
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
        use crate::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: DayCount::Thirty360,
            id: InstrumentId::new("MTG1"),
            asset_type: AssetType::SingleFamilyMortgage { ltv: None },
            balance: Money::new(balance, Currency::USD).expect("valid money fixture"),
            rate,
            spread_bp: None,
            forward_curve_id: None,
            index_floor_bp: None,
            maturity,
            rating: None,
            industry: None,
            obligor_id: None,
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
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(balance, Currency::USD).expect("valid money fixture"),
            RateSpec::Fixed { rate: 0.05 },
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
            let pay = closing
                .add_months(i32::try_from(m).expect("month fits i32"))
                .expect("valid date shift");
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay,
                accrual_end: pay,
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

            let pay = closing
                .add_months(i32::try_from(m).expect("month"))
                .expect("valid date shift");
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay,
                accrual_end: pay,
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
                RateSpec::Fixed { rate: 0.05 },
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
            RateSpec::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Abs,
            crate::structured_credit::StandardDealParams {
                id: "ABS-RESERVE".into(),
                pool,
                tranches: TrancheStructure::new(vec![tranche]).expect("structure"),
                closing_date: cleanup_test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    /// Total cash distributed to every tranche over the life of a deal.
    fn total_distributed(instrument: &StructuredCredit) -> f64 {
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let results = instrument
            .run_simulation(&market, cleanup_test_date())
            .map(|run| run.tranches)
            .expect("simulation");
        results
            .values()
            .flat_map(|tc| tc.cashflows.iter())
            .map(|(_, m)| m.amount())
            .sum()
    }

    /// Build a deal with an excess-spread account and optional senior fees.
    fn excess_spread_deal(with_fees: bool) -> StructuredCredit {
        use crate::structured_credit::types::{ExcessSpreadSpec, WaterfallRules};
        let mut deal = reserve_account_deal(0.0);
        // Note coupon well below collateral, so genuine excess spread exists.
        deal.tranches.tranches[0].coupon = RateSpec::Fixed { rate: 0.02 };
        deal.waterfall_rules = Some(WaterfallRules {
            excess_spread: Some(ExcessSpreadSpec {
                target_balance: Money::from((10_000_000_i64, Currency::USD)),
                trap_loss_decimal: None,
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
        use crate::structured_credit::types::{ExcessSpreadSpec, WaterfallRules};

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
                    RateSpec::Fixed { rate: 0.02 },
                    maturity,
                )
                .expect("tranche")
            };
            let tranches = TrancheStructure::new(vec![
                mk("SR", TrancheSeniority::Senior, 20.0, 100.0, 8_000_000.0),
                mk("JR", junior_seniority, 0.0, 20.0, 2_000_000.0),
            ])
            .expect("structure");
            let mut deal = StructuredCredit::new_standard(
                crate::structured_credit::DealType::Abs,
                crate::structured_credit::StandardDealParams {
                    id: "ABS-SPREAD".into(),
                    pool,
                    tranches,
                    closing_date: cleanup_test_date(),
                    maturity,
                    discount_curve_id: "USD-OIS".into(),
                    calendar_id: None,
                },
            )
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse");
            deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
            deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
            deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
            deal.waterfall_rules = Some(WaterfallRules {
                excess_spread: Some(ExcessSpreadSpec {
                    target_balance: Money::from((10_000_000_i64, Currency::USD)),
                    trap_loss_decimal: None,
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
        let results = (excess_spread_deal(true))
            .run_simulation(&market, cleanup_test_date())
            .map(|run| run.tranches)
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
            deal.tranches.tranches[0].coupon = RateSpec::Fixed { rate: 0.09 };
            let market = MarketContext::new().insert(cleanup_discount_curve());
            let results = deal
                .run_simulation(&market, cleanup_test_date())
                .map(|run| run.tranches)
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
        triggers: Vec<crate::structured_credit::types::CoverageTestSpec>,
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
            RateSpec::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("senior");
        let equity = Tranche::new(
            "EQUITY",
            0.0,
            20.0,
            TrancheSeniority::Equity,
            Money::from((2_000_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("equity");
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Clo,
            crate::structured_credit::StandardDealParams {
                id: "CLO-OC".into(),
                pool,
                tranches: TrancheStructure::new(vec![senior, equity]).expect("structure"),
                closing_date: cleanup_test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
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
        triggers: Vec<crate::structured_credit::types::CoverageTestSpec>,
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
                RateSpec::Fixed { rate },
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
        let mut instrument = StructuredCredit::new_standard(
            crate::structured_credit::DealType::Clo,
            crate::structured_credit::StandardDealParams {
                id: "CLO-MEZZ".into(),
                pool,
                tranches,
                closing_date: cleanup_test_date(),
                maturity,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )
        .expect("valid structured-credit dates")
        .with_calendar_id("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.03);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument.coverage_triggers = triggers;
        instrument
    }

    /// Failing OC test traps subordinated interest and accelerates senior principal.
    #[test]
    fn failing_oc_test_diverts_subordinated_interest_to_senior_principal() {
        use crate::structured_credit::types::CoverageTestSpec;

        let untriggered = clo_with_mezz(vec![]);
        // Pool 10M against 7M senior = 143% OC at closing. A 175% requirement
        // breaches from the first period.
        let triggered = clo_with_mezz(vec![CoverageTestSpec::oc("CLASS_A", 1.75)]);

        // Assert timing: excess cure cash returns to CLASS_B, so lifetime totals
        // can converge while the trap still delays subordinated interest.
        let interest_wal = |deal: &StructuredCredit, tranche: &str| -> f64 {
            let market = MarketContext::new().insert(cleanup_discount_curve());
            let results = deal
                .run_simulation(&market, cleanup_test_date())
                .map(|run| run.tranches)
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
            if total > 0.0 { weighted / total } else { 0.0 }
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
            let results = deal
                .run_simulation(&market, cleanup_test_date())
                .map(|run| run.tranches)
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
            if total > 0.0 { weighted / total } else { 0.0 }
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
        use crate::structured_credit::pricing::waterfall::WaterfallContext;
        use crate::structured_credit::pricing::waterfall::execute_waterfall;
        use crate::structured_credit::types::CoverageTestSpec;

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
                accrual_end: pay,
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
        let ratio = ratio.expect("the OC test has a claim");
        assert!(
            !passing,
            "OC ratio {ratio:.4} against a 1.50 requirement must breach"
        );
        assert!(
            (ratio - 1.2875).abs() < 0.05,
            "OC ratio must be ~(10M pool + 0.3M cash)/8M senior = 1.2875, got {ratio:.4}"
        );
    }

    /// Deal with no triggers reports no coverage tests.
    #[test]
    fn deal_without_triggers_reports_no_coverage_tests() {
        use crate::structured_credit::pricing::waterfall::WaterfallContext;
        use crate::structured_credit::pricing::waterfall::execute_waterfall;

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
                accrual_end: Date::from_calendar_date(2024, Month::April, 1).expect("date"),
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
        use crate::structured_credit::types::CoverageTestSpec;

        use finstack_quant_valuations_core::instruments::Instrument;

        let mut deal = oc_trigger_deal(vec![]);
        deal.coverage_triggers = vec![CoverageTestSpec::oc("NOT_A_TRANCHE", 1.2)];
        let err = deal
            .validate_invariants()
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
        use crate::structured_credit::pricing::waterfall::{WaterfallContext, execute_waterfall};

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
                    accrual_end: Date::from_calendar_date(2024, Month::April, 1).expect("date"),
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
                .distributions()
                .expect("distributions")
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
            .distributions()
            .expect("distributions")
            .iter()
            .filter(|(r, _)| !matches!(r, RecipientType::ServiceProvider(_)))
            .map(|(_, m)| m.amount())
            .sum::<f64>();
        let to_notes_without = run(&without)
            .distributions()
            .expect("distributions")
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
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("senior tranche");
        let equity = Tranche::new(
            "E",
            80.0,
            100.0, // junior 20% = 2 M face
            TrancheSeniority::Equity,
            Money::from((2_000_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("equity tranche");
        let mut instrument = StructuredCredit {
            cleanup_call_decimal: Some(0.10),
            ..StructuredCredit::new_standard(
                crate::structured_credit::DealType::Abs,
                crate::structured_credit::StandardDealParams {
                    id: "ABS-BIG-RECOVERY-QUEUE".into(),
                    pool,
                    tranches: TrancheStructure::new(vec![senior, equity]).expect("structure"),
                    closing_date: start,
                    maturity,
                    discount_curve_id: "USD-OIS".into(),
                    calendar_id: None,
                },
            )
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse")
        };
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

        let results = simulate_with_source(
            &instrument,
            &market,
            start,
            Some(&mut DeterministicPoolFlowSource),
        )
        .map(|run| run.tranches)
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
        use crate::structured_credit::types::AssetType;
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
                asset_type: AssetType::HighYieldBond {},
                balance: Money::new(balance, Currency::USD).expect("valid money fixture"),
                rate,
                spread_bp: None,
                forward_curve_id: None,
                index_floor_bp: None,
                maturity,
                rating: None,
                industry: None,
                obligor_id: None,
                defaulted: false,
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
                RateSpec::Fixed { rate: 0.05 },
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
                accrual_end: pay1,
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
                    RateSpec::Fixed { rate: 0.05 },
                    maturity,
                )
                .expect("senior"),
                Tranche::new(
                    "EQ",
                    90.0,
                    100.0,
                    TrancheSeniority::Equity,
                    Money::from((10_000_000_i64, Currency::USD)),
                    RateSpec::Fixed { rate: 0.0 },
                    maturity,
                )
                .expect("equity"),
            ])
            .expect("structure");
            let mut instrument = StructuredCredit::new_standard(
                crate::structured_credit::DealType::Abs,
                crate::structured_credit::StandardDealParams {
                    id: "ABS-TAIL-RECOVERY".into(),
                    pool,
                    tranches,
                    closing_date: closing,
                    maturity,
                    discount_curve_id: "USD-OIS".into(),
                    calendar_id: None,
                },
            )
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse");
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
            simulate_with_source(
                &build_deal(lag),
                &market,
                closing,
                Some(&mut DeterministicPoolFlowSource),
            )
            .map(|run| run.tranches)
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
        let total_cash = |res: &FxHashMap<String, TrancheCashflows>| -> f64 {
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

    // ── Period waterfall detail and its reconciliation to tranche flows ──

    use super::super::state::tranche_credit_enhancement;
    use crate::structured_credit::types::CallAssumption;

    /// The records and the booked flows are the same `Money` values: float noise
    /// from summing them is the only allowed gap.
    const RECONCILE_TOLERANCE: f64 = 1e-6;

    fn sum_on(flows: &[(Date, Money)], date: Date) -> f64 {
        flows
            .iter()
            .filter(|(flow_date, _)| *flow_date == date)
            .map(|(_, amount)| amount.amount())
            .sum()
    }

    /// For every tranche and date: `payment_records` plus `non_waterfall_payments`
    /// equal the tranche's interest flows, its principal flows and its cashflows.
    fn assert_period_detail_reconciles(deal: &StructuredCredit, run: &SimulationRun) {
        let periods = &run.diagnostics.periods;
        for tranche in &deal.tranches.tranches {
            let id = tranche.id.as_str();
            let flows = &run.tranches[id];
            let key = if tranche.seniority == TrancheSeniority::Equity {
                RecipientType::Equity
            } else {
                RecipientType::Tranche(id.to_string())
            };
            let mut dates: Vec<Date> = flows
                .interest_flows
                .iter()
                .chain(&flows.principal_flows)
                .chain(&flows.cashflows)
                .map(|(date, _)| *date)
                .collect();
            dates.extend(periods.iter().map(|period| period.payment_date));
            dates.extend(
                periods
                    .iter()
                    .flat_map(|period| &period.non_waterfall_payments)
                    .map(|payment| payment.payment_date),
            );
            dates.sort();
            dates.dedup();
            for date in dates {
                let records: Vec<_> = periods
                    .iter()
                    .filter(|period| period.payment_date == date)
                    .flat_map(|period| &period.waterfall.payment_records)
                    .filter(|record| record.recipient == key)
                    .collect();
                let external: Vec<_> = periods
                    .iter()
                    .flat_map(|period| &period.non_waterfall_payments)
                    .filter(|payment| payment.payment_date == date && payment.tranche_id == id)
                    .collect();
                let paid = |principal: bool| -> f64 {
                    records
                        .iter()
                        .filter(|record| record.is_principal == principal)
                        .map(|record| record.paid_amount.amount())
                        .sum()
                };
                let external_interest: f64 = external.iter().map(|p| p.interest.amount()).sum();
                let external_principal: f64 = external.iter().map(|p| p.principal.amount()).sum();

                let interest_gap =
                    paid(false) + external_interest - sum_on(&flows.interest_flows, date);
                let principal_gap =
                    paid(true) + external_principal - sum_on(&flows.principal_flows, date);
                let cash_gap = paid(false) + paid(true) + external_interest + external_principal
                    - sum_on(&flows.cashflows, date);
                assert!(
                    interest_gap.abs() <= RECONCILE_TOLERANCE,
                    "interest payments do not reconcile with cashflows"
                );
                assert!(
                    principal_gap.abs() <= RECONCILE_TOLERANCE,
                    "principal payments do not reconcile with cashflows"
                );
                assert!(
                    cash_gap.abs() <= RECONCILE_TOLERANCE,
                    "total payments do not reconcile with cashflows"
                );
            }
        }
    }

    fn run_reconciled(deal: &StructuredCredit) -> SimulationRun {
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let run = deal
            .run_simulation(&market, deal.closing_date)
            .expect("simulation");
        assert_period_detail_reconciles(deal, &run);
        run
    }

    fn sources(run: &SimulationRun) -> Vec<NonWaterfallSource> {
        let mut sources: Vec<NonWaterfallSource> = Vec::new();
        for payment in run
            .diagnostics
            .periods
            .iter()
            .flat_map(|period| &period.non_waterfall_payments)
        {
            if !sources.contains(&payment.source) {
                sources.push(payment.source);
            }
        }
        sources
    }

    /// `distributions` cannot be JSON, so the period keeps a projection.
    #[test]
    fn waterfall_distribution_recipient_maps_do_not_serialize_to_json() {
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            RecipientType::Tranche("A".to_string()),
            Money::from((1_i64, Currency::USD)),
        );
        assert!(serde_json::to_string(&map).is_err());
    }

    /// The executor's own records reach the period: every tier's allocation and
    /// a paid coupon by name, and the interest/principal classification.
    #[test]
    fn period_waterfall_reports_who_was_paid() {
        let deal = clo_with_mezz(Vec::new());
        let run = run_reconciled(&deal);
        let first = &run.diagnostics.periods[0];
        let waterfall = &first.waterfall;
        assert!(waterfall.total_available.amount() > 0.0);
        assert!(!waterfall.tier_allocations.is_empty());
        let coupon = waterfall
            .payment_records
            .iter()
            .find(|record| {
                record.recipient == RecipientType::Tranche("CLASS_A".to_string())
                    && !record.is_principal
            })
            .expect("senior coupon");
        let booked = sum_on(&run.tranches["CLASS_A"].interest_flows, first.payment_date);
        assert!((coupon.paid_amount.amount() - booked).abs() <= RECONCILE_TOLERANCE);
        assert_eq!(
            coupon.shortfall.amount(),
            coupon.requested_amount.amount() - coupon.paid_amount.amount()
        );
        let parsed: PeriodDiagnostics =
            serde_json::from_str(&serde_json::to_string(first).expect("serialize"))
                .expect("deserialize");
        assert_eq!(&parsed, first);
    }

    /// A breached OC test's diversion is on the period, cash and all.
    #[test]
    fn period_waterfall_reports_a_diversion() {
        use crate::structured_credit::types::CoverageTestSpec;
        let deal = clo_with_mezz(vec![CoverageTestSpec::oc("CLASS_A", 2.0)]);
        let run = run_reconciled(&deal);
        let waterfall = &run.diagnostics.periods[0].waterfall;
        assert!(waterfall.had_diversions);
        assert!(waterfall.diverted_cash.amount() > 0.0);
        let diverted: f64 = waterfall
            .diverted_amounts
            .iter()
            .map(|record| record.amount.amount())
            .sum();
        assert!((diverted - waterfall.diverted_cash.amount()).abs() < 1e-6);
        assert!(
            waterfall
                .payment_records
                .iter()
                .any(|record| record.diverted)
        );
    }

    /// A clean-up call redeems the notes outside the waterfall.
    #[test]
    fn cleanup_call_redemption_is_recorded_on_the_last_period() {
        let run = run_reconciled(&cleanup_call_deal());
        assert_eq!(sources(&run), vec![NonWaterfallSource::Redemption]);
        let last = run.diagnostics.periods.last().expect("period");
        let redemption = &last.non_waterfall_payments[0];
        assert_eq!(redemption.tranche_id, "A");
        assert_eq!(redemption.payment_date, last.payment_date);
        assert!(redemption.principal.amount() > 0.0);
    }

    /// A deal call above par: the premium is interest, the surplus goes to the
    /// most junior class, and every class's record reconciles.
    #[test]
    fn deal_call_premium_and_surplus_reconcile() {
        let mut deal = clo_with_mezz(Vec::new());
        deal.call_assumption = Some(CallAssumption {
            date: Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            price_pct: 101.0,
            scope: CallScope::Deal,
            after_early_amortization_months: None,
        });
        let run = run_reconciled(&deal);
        assert_eq!(sources(&run), vec![NonWaterfallSource::Redemption]);
        let payments = &run
            .diagnostics
            .periods
            .last()
            .expect("period")
            .non_waterfall_payments;
        let class_a = payments
            .iter()
            .find(|payment| payment.tranche_id == "CLASS_A")
            .expect("senior redeemed");
        assert!(class_a.interest.amount() > 0.0, "the premium is interest");
        assert!(
            payments
                .iter()
                .any(|payment| payment.tranche_id == "EQUITY")
        );
    }

    /// A funded reserve is released to the notes at deal end.
    #[test]
    fn terminal_reserve_release_is_recorded_on_the_last_period() {
        let run = run_reconciled(&reserve_account_deal(1_000_000.0));
        assert!(sources(&run).contains(&NonWaterfallSource::ReserveAccountRelease));
    }

    /// The excess-spread account is released at deal end, and a loss trap
    /// sends it to principal.
    #[test]
    fn terminal_spread_release_is_recorded_on_the_last_period() {
        let run = run_reconciled(&excess_spread_deal(false));
        assert!(sources(&run).contains(&NonWaterfallSource::SpreadAccountRelease));
    }

    /// Reserve interest routed to a tranche is on the period that earned it.
    #[test]
    fn reserve_interest_routed_to_a_tranche_is_recorded_on_each_period() {
        let mut deal = reserve_account_deal(1_000_000.0);
        deal.pool.reserve_account_rate = 0.04;
        deal.pool.reserve_interest_destination =
            crate::structured_credit::ReserveInterestDestination::Tranche {
                tranche_id: "A".to_string(),
            };
        let run = run_reconciled(&deal);
        for period in &run.diagnostics.periods[..run.diagnostics.periods.len() - 1] {
            let [payment] = period.non_waterfall_payments.as_slice() else {
                panic!("{}: expected one payment", period.payment_date);
            };
            assert_eq!(payment.source, NonWaterfallSource::ReserveInterest);
            assert_eq!(payment.payment_date, period.payment_date);
            assert!(payment.interest.amount() > 0.0);
        }
    }

    /// Recoveries still pending at deal end settle after the last period and
    /// carry their own later date on its record.
    #[test]
    fn recoveries_settling_after_the_last_period_carry_their_own_date() {
        let mut deal = clo_with_mezz(Vec::new());
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.20);
        deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 18);
        let run = run_reconciled(&deal);
        let last = run.diagnostics.periods.last().expect("period");
        let later = last
            .non_waterfall_payments
            .iter()
            .filter(|payment| payment.source == NonWaterfallSource::RecoveryRelease)
            .find(|payment| payment.payment_date > last.payment_date);
        assert!(later.is_some(), "a recovery settles after the last period");
    }

    /// An incentive fee taken from a terminal release to equity is reported
    /// next to the net cash equity received.
    #[test]
    fn incentive_fee_on_a_terminal_release_is_reported_with_the_payment() {
        let mut deal = clo_with_mezz(Vec::new());
        deal.call_assumption = Some(CallAssumption {
            date: Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            price_pct: 100.0,
            scope: CallScope::Deal,
            after_early_amortization_months: None,
        });
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let mut fees = DealFees::clo_standard(Currency::USD);
        fees.incentive_fee = Some(IncentiveFeeSpec {
            hurdle_irr: 0.0,
            share: 0.5,
        });
        deal.fees = Some(fees);
        let run = run_reconciled(&deal);
        let last = run.diagnostics.periods.last().expect("period");
        let fee: f64 = last
            .non_waterfall_payments
            .iter()
            .map(|payment| payment.incentive_fee.amount())
            .sum();
        assert!(fee > 0.0 && fee <= last.fees_paid.amount());
    }

    /// With no payment period left, the settlement still has a record.
    #[test]
    fn deal_with_no_period_left_records_its_settlement() {
        let mut deal = reserve_account_deal(1_000_000.0);
        deal.pool.assets.clear();
        let run = run_reconciled(&deal);
        let [settlement] = run.diagnostics.periods.as_slice() else {
            panic!("one settlement record");
        };
        assert_eq!(
            sources(&run),
            vec![NonWaterfallSource::ReserveAccountRelease]
        );
        assert_eq!(settlement.waterfall.total_available.amount(), 0.0);
        assert_eq!(
            settlement.non_waterfall_payments[0].principal,
            Money::from((1_000_000_i64, Currency::USD))
        );
    }

    /// `CLASS_A` interest and sequential principal, then remaining cash to
    /// `CLASS_B`, optionally followed by a residual-holder tier.
    fn residual_to_class_b_deal(equity_tier: bool) -> StructuredCredit {
        use crate::structured_credit::types::{
            PaymentCalculation, PaymentType, Recipient, Waterfall, WaterfallTier,
        };
        let mut deal = clo_with_mezz(Vec::new());
        let mut waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("a_interest", "CLASS_A")),
            )
            .add_tier(
                WaterfallTier::new("principal", 2, PaymentType::Principal)
                    .add_recipient(Recipient::tranche_principal("a", "CLASS_A", None))
                    .add_recipient(Recipient::tranche_principal("b", "CLASS_B", None)),
            )
            .add_tier(
                WaterfallTier::new("residual", 3, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "b_residual",
                        RecipientType::Tranche("CLASS_B".to_string()),
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            );
        if equity_tier {
            waterfall = waterfall.add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            );
        }
        deal.waterfall = Some(waterfall.build().expect("waterfall"));
        deal
    }

    /// Residual cash paid to a debt class is a turbo: it reconciles as principal
    /// on every date, retires the balance and never exceeds it.
    #[test]
    fn residual_to_a_debt_class_pays_down_principal() {
        let deal = residual_to_class_b_deal(false);
        let run = run_reconciled(&deal);
        let first = &run.diagnostics.periods[0];
        let date = first.payment_date;
        let flows = &run.tranches["CLASS_B"];
        let [record] = first
            .waterfall
            .payment_records
            .iter()
            .filter(|record| record.recipient == RecipientType::Tranche("CLASS_B".to_string()))
            .filter(|record| record.paid_amount.amount() > 0.0)
            .collect::<Vec<_>>()[..]
        else {
            panic!("one payment to CLASS_B");
        };
        assert!(record.is_principal);
        let paid = record.paid_amount.amount();
        assert!(paid > 0.0);
        assert!((paid - sum_on(&flows.cashflows, date)).abs() <= RECONCILE_TOLERANCE);
        assert!((paid - sum_on(&flows.principal_flows, date)).abs() <= RECONCILE_TOLERANCE);
        assert_eq!(sum_on(&flows.interest_flows, date), 0.0);
        // The cap reads the running balance, so payments spread over several
        // periods stop at the original balance only if each one retired it.
        assert!(flows.principal_flows.len() > 1);
        assert_eq!(flows.total_interest.amount(), 0.0);
        assert!((flows.total_principal.amount() - 2_000_000.0).abs() <= RECONCILE_TOLERANCE);
        let cash: f64 = flows.cashflows.iter().map(|(_, m)| m.amount()).sum();
        assert!((cash - 2_000_000.0).abs() <= RECONCILE_TOLERANCE);
    }

    /// Cash paid in `period` to `recipient`, one entry per payment made.
    fn paid_to(period: &PeriodDiagnostics, recipient: &RecipientType) -> Vec<f64> {
        period
            .waterfall
            .payment_records
            .iter()
            .filter(|record| record.recipient == *recipient)
            .map(|record| record.paid_amount.amount())
            .filter(|paid| *paid > 0.0)
            .collect()
    }

    /// The turbo stops at the balance: within the period that retires the class,
    /// the cash above it reaches the next tier.
    #[test]
    fn residual_above_a_debt_balance_passes_to_the_next_tier() {
        let deal = residual_to_class_b_deal(true);
        let run = run_reconciled(&deal);
        let class_b = RecipientType::Tranche("CLASS_B".to_string());
        assert!(
            run.diagnostics.periods.iter().any(|period| {
                !paid_to(period, &class_b).is_empty()
                    && !paid_to(period, &RecipientType::Equity).is_empty()
            }),
            "no period pays both the class and the tier after it"
        );
        let total = run.tranches["CLASS_B"].total_principal.amount();
        assert!((total - 2_000_000.0).abs() <= RECONCILE_TOLERANCE);
    }

    /// A class also paying down through an earlier principal tier takes only
    /// what is left of its balance from the residual tier, so it is never overpaid.
    #[test]
    fn residual_to_a_debt_class_nets_principal_paid_earlier_in_the_period() {
        let mut deal = residual_to_class_b_deal(false);
        // `CLASS_B` first, so it takes principal before the residual tier runs.
        deal.waterfall.as_mut().expect("waterfall").tiers[1]
            .recipients
            .reverse();
        let run = run_reconciled(&deal);
        let class_b = RecipientType::Tranche("CLASS_B".to_string());
        let payments: Vec<Vec<f64>> = run
            .diagnostics
            .periods
            .iter()
            .map(|period| paid_to(period, &class_b))
            .collect();
        assert!(payments.iter().any(|paid| paid.len() == 2));
        let total: f64 = payments.iter().flatten().sum();
        assert!((total - 2_000_000.0).abs() <= RECONCILE_TOLERANCE);
    }

    /// `CLASS_A` coupon and sequential principal, plus one extra recipient for
    /// `CLASS_B` in a tier funded after them.
    fn deal_paying_class_b(
        recipient_id: &str,
        calculation: crate::structured_credit::types::PaymentCalculation,
    ) -> StructuredCredit {
        use crate::structured_credit::types::{PaymentType, Recipient, WaterfallTier};
        let mut deal = residual_to_class_b_deal(false);
        let waterfall = deal.waterfall.as_mut().expect("waterfall");
        waterfall.tiers[2] = WaterfallTier::new("residual", 3, PaymentType::Residual)
            .add_recipient(Recipient::new(
                recipient_id,
                RecipientType::Tranche("CLASS_B".to_string()),
                calculation,
            ));
        deal
    }

    /// A fixed amount used to reach `cashflows` alone: lifetime cash 3,000,000
    /// on a 2,000,000 note. The engine now refuses to price it, whichever path
    /// built the deal.
    #[test]
    fn simulation_rejects_a_fixed_amount_paid_to_a_debt_class() {
        use crate::structured_credit::types::PaymentCalculation;
        let deal = deal_paying_class_b(
            "b_extra",
            PaymentCalculation::FixedAmount {
                amount: Money::from((50_000_i64, Currency::USD)),
                rounding: None,
            },
        );
        let market = MarketContext::new().insert(cleanup_discount_curve());
        let message = deal
            .run_simulation(&market, deal.closing_date)
            .expect_err("rejected")
            .to_string();
        assert!(message.contains("fixed_amount"), "{message}");
        assert!(message.contains("CLASS_B"), "{message}");
    }

    /// A coupon rounded up pays cents above the claim the engine computes
    /// unrounded. That cash is still interest, so no gap opens.
    #[test]
    fn coupon_rounded_above_its_claim_is_booked_as_interest() {
        use crate::structured_credit::types::{
            PaymentCalculation, PaymentType, Recipient, RoundingConvention, WaterfallTier,
        };
        let mut deal = residual_to_class_b_deal(true);
        let waterfall = deal.waterfall.as_mut().expect("waterfall");
        waterfall.tiers[0] = WaterfallTier::new("interest", 1, PaymentType::Interest)
            .add_recipient(Recipient::new(
                "a_interest",
                RecipientType::Tranche("CLASS_A".to_string()),
                PaymentCalculation::TrancheInterest {
                    tranche_id: "CLASS_A".to_string(),
                    rounding: Some(RoundingConvention::Ceiling),
                },
            ));
        let run = run_reconciled(&deal);
        assert!(run.tranches["CLASS_A"].total_interest.amount() > 0.0);
    }

    /// Carryover the engine cannot settle against a balance is rejected: the
    /// tranche must accrue it and the recipient must carry the engine's id.
    #[test]
    fn simulation_rejects_carryover_with_no_balance_behind_it() {
        use crate::structured_credit::types::PaymentCalculation;
        let market = MarketContext::new().insert(cleanup_discount_curve());
        for recipient_id in ["b_extra", "net_wac_carryover_CLASS_B"] {
            let deal = deal_paying_class_b(
                recipient_id,
                PaymentCalculation::NetWacCarryover {
                    tranche_id: "CLASS_B".to_string(),
                    amount: Money::from((10_000_i64, Currency::USD)),
                },
            );
            let message = deal
                .run_simulation(&market, deal.closing_date)
                .expect_err("rejected")
                .to_string();
            assert!(message.contains("net_wac_carryover_CLASS_B"), "{message}");
        }
    }

    /// The carryover the validation admits: a capped class accruing it, repaid
    /// under the engine's recipient id, reconciles as interest.
    #[test]
    fn carryover_a_custom_waterfall_repays_reconciles_as_interest() {
        use crate::structured_credit::types::{
            AfcSpec, PaymentCalculation, PaymentType, Recipient, WaterfallRules, WaterfallTier,
        };
        let mut deal = deal_paying_class_b(
            "net_wac_carryover_CLASS_B",
            PaymentCalculation::NetWacCarryover {
                tranche_id: "CLASS_B".to_string(),
                amount: Money::from((0_i64, Currency::USD)),
            },
        );
        let waterfall = deal.waterfall.as_mut().expect("waterfall");
        waterfall.tiers[0] = WaterfallTier::new("interest", 1, PaymentType::Interest)
            .add_recipient(Recipient::tranche_interest("a_interest", "CLASS_A"))
            .add_recipient(Recipient::tranche_interest("b_interest", "CLASS_B"));
        deal.waterfall_rules = Some(WaterfallRules {
            afc: Some(AfcSpec {
                capped_tranches: vec!["CLASS_B".to_string()],
                net_wac_fee_bp: Some(100.0),
                carryover: true,
            }),
            ..WaterfallRules::default()
        });
        let run = run_reconciled(&deal);
        let class_b = RecipientType::Tranche("CLASS_B".to_string());
        let repaid = run
            .diagnostics
            .periods
            .iter()
            .flat_map(|period| &period.waterfall.payment_records)
            .filter(|record| {
                record.recipient == class_b && record.recipient_id == "net_wac_carryover_CLASS_B"
            })
            .map(|record| record.paid_amount.amount())
            .sum::<f64>();
        assert!(repaid > 0.0);
    }

    // ── First coverage breach and credit enhancement ─────────────────────

    fn breach<'a>(run: &'a SimulationRun, test_id: &str) -> &'a CoverageTestBreach {
        run.diagnostics
            .first_breaches
            .iter()
            .find(|breach| breach.test_id == test_id)
            .expect("test listed")
    }

    /// A pool eroding through an OC level: the first failing period is reported,
    /// not the first period and not a later one.
    #[test]
    fn first_breach_is_the_first_failing_period() {
        use crate::structured_credit::types::CoverageTestSpec;
        let mut deal = oc_trigger_deal(vec![CoverageTestSpec::oc("CLASS_A", 1.20)]);
        // No recovery cash pays the senior note down, so the ratio only erodes.
        deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
        let run = run_reconciled(&deal);
        let tests: Vec<(Date, bool)> = run
            .diagnostics
            .periods
            .iter()
            .flat_map(|period| {
                period
                    .coverage_tests
                    .iter()
                    .map(|test| (period.payment_date, test.passing))
            })
            .collect();
        let failing = tests
            .iter()
            .position(|(_, passing)| !passing)
            .expect("the test fails");
        assert!(failing > 0, "the test passes at first");
        assert_eq!(
            breach(&run, "OC_CLASS_A").first_breach,
            Some(tests[failing].0)
        );
    }

    /// A test that never fails is still listed, with no date.
    #[test]
    fn first_breach_is_none_when_the_test_always_passes() {
        use crate::structured_credit::types::CoverageTestSpec;
        let mut deal = oc_trigger_deal(vec![CoverageTestSpec::oc("CLASS_A", 1.01)]);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let run = run_reconciled(&deal);
        assert_eq!(
            run.diagnostics.first_breaches,
            vec![CoverageTestBreach {
                test_id: "OC_CLASS_A".to_string(),
                first_breach: None,
            }]
        );
    }

    /// One entry per test, sorted by id; none for a deal without tests.
    #[test]
    fn first_breaches_are_sorted_by_test_id() {
        use crate::structured_credit::types::CoverageTestSpec;
        let deal = clo_with_mezz(vec![
            CoverageTestSpec::oc("CLASS_B", 1.01),
            CoverageTestSpec::oc("CLASS_A", 1.01),
            CoverageTestSpec::ic("CLASS_A", 1.01),
        ]);
        let run = run_reconciled(&deal);
        let ids: Vec<&str> = run
            .diagnostics
            .first_breaches
            .iter()
            .map(|breach| breach.test_id.as_str())
            .collect();
        assert_eq!(ids, vec!["IC_CLASS_A", "OC_CLASS_A", "OC_CLASS_B"]);
        assert!(
            run_reconciled(&clo_with_mezz(Vec::new()))
                .diagnostics
                .first_breaches
                .is_empty()
        );
    }

    /// A tranche trigger that diverts nothing never reaches `coverage_tests`,
    /// and its first breach is still reported under its trigger id.
    #[test]
    fn tranche_trigger_without_a_diversion_reports_its_first_breach() {
        use crate::structured_credit::types::{CoverageTrigger, TriggerConsequence};
        let mut deal = clo_with_mezz(Vec::new());
        deal.tranches.tranches[0].oc_trigger = Some(CoverageTrigger::new(
            2.0,
            TriggerConsequence::StopReinvestment,
        ));
        let run = run_reconciled(&deal);
        let first = &run.diagnostics.periods[0];
        assert!(first.coverage_tests.is_empty());
        assert_eq!(
            breach(&run, "OC_CLASS_A_trigger").first_breach,
            Some(first.payment_date)
        );
    }

    /// A diverting trigger reports one entry under one id, whichever source saw it.
    #[test]
    fn diverting_tranche_trigger_has_one_entry() {
        use crate::structured_credit::types::{CoverageTrigger, TriggerConsequence};
        let mut deal = clo_with_mezz(Vec::new());
        deal.tranches.tranches[0].oc_trigger = Some(CoverageTrigger::new(
            2.0,
            TriggerConsequence::DivertCashFlow,
        ));
        let run = run_reconciled(&deal);
        let first = &run.diagnostics.periods[0];
        assert_eq!(first.coverage_tests[0].test_id, "OC_CLASS_A_trigger");
        assert_eq!(run.diagnostics.first_breaches.len(), 1);
        assert_eq!(
            breach(&run, "OC_CLASS_A_trigger").first_breach,
            Some(first.payment_date)
        );
    }

    /// 7M / 2M / 1M notes over 10M of collateral: 30%, 10%, 0%.
    #[test]
    fn credit_enhancement_matches_a_hand_calculation() {
        let mut deal = clo_with_mezz(Vec::new());
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        let run = run_reconciled(&deal);
        let first = &run.diagnostics.periods[0];
        assert_eq!(first.pool_balance.amount(), 10_000_000.0);
        let values: Vec<(&str, f64)> = first
            .credit_enhancement
            .iter()
            .map(|ce| (ce.tranche_id.as_str(), ce.credit_enhancement))
            .collect();
        assert_eq!(
            values,
            vec![("CLASS_A", 0.3), ("CLASS_B", 0.1), ("EQUITY", 0.0)]
        );
    }

    /// With prepayments and defaults: `(pool − notes at or ahead) ÷ pool` on the
    /// balances the row and the tranche flows report at that period's end.
    #[test]
    fn credit_enhancement_follows_end_of_period_balances() {
        let mut deal = clo_with_mezz(Vec::new());
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
        let run = run_reconciled(&deal);
        let period = &run.diagnostics.periods[3];
        let balance = |id: &str, original: f64| {
            let flows = &run.tranches[id];
            let retired: f64 = flows
                .principal_flows
                .iter()
                .chain(&flows.writedown_flows)
                .filter(|(date, _)| *date <= period.payment_date)
                .map(|(_, amount)| amount.amount())
                .sum();
            original - retired
        };
        let pool = period.pool_balance.amount();
        let class_a = balance("CLASS_A", 7_000_000.0);
        let class_b = balance("CLASS_B", 2_000_000.0);
        assert!(
            (period.credit_enhancement[0].credit_enhancement - (pool - class_a) / pool).abs()
                < 1e-9
        );
        assert!(
            (period.credit_enhancement[1].credit_enhancement - (pool - class_a - class_b) / pool)
                .abs()
                < 1e-9
        );
        assert!(period.credit_enhancement[0].credit_enhancement > 0.3);
    }

    /// The step-down trigger reads the recorded function's senior value.
    #[test]
    fn step_down_metric_is_the_senior_tranche_credit_enhancement() {
        let deal = clo_with_mezz(Vec::new());
        let mut state = SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state");
        state.principal_funding_account = Money::from((500_000_i64, Currency::USD));
        let senior = deal
            .tranches
            .tranches
            .iter()
            .find(|tranche| tranche.id.as_str() == "CLASS_A")
            .expect("senior");
        let value = tranche_credit_enhancement(&state, senior);
        assert_eq!(value, (10_500_000.0 - 7_000_000.0) / 10_500_000.0);
        assert_eq!(step_down_metrics(&state).credit_enhancement, value);
    }

    /// Two `Senior` classes and a mezzanine under a `MinCreditEnhancement`
    /// trigger of `level`. `TrancheStructure::new` ranks A1 ahead of A2.
    fn same_seniority_deal(level: f64) -> StructuredCredit {
        let mut deal = clo_with_mezz(Vec::new());
        let maturity = deal.maturity;
        let mk = |id: &str, seniority, attach: f64, detach: f64, bal: f64, rate: f64| {
            Tranche::new(
                id,
                attach,
                detach,
                seniority,
                Money::new(bal, Currency::USD).expect("valid money fixture"),
                RateSpec::Fixed { rate },
                maturity,
            )
            .expect("tranche")
        };
        deal.tranches = TrancheStructure::new(vec![
            mk(
                "A1",
                TrancheSeniority::Senior,
                60.0,
                100.0,
                4_000_000.0,
                0.05,
            ),
            mk(
                "A2",
                TrancheSeniority::Senior,
                30.0,
                60.0,
                3_000_000.0,
                0.05,
            ),
            mk(
                "B",
                TrancheSeniority::Mezzanine,
                10.0,
                30.0,
                2_000_000.0,
                0.08,
            ),
            mk("EQ", TrancheSeniority::Equity, 0.0, 10.0, 1_000_000.0, 0.0),
        ])
        .expect("structure");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
        deal.waterfall_rules = Some(crate::structured_credit::WaterfallRules {
            step_down: Some(crate::structured_credit::StepDownSpec {
                step_down_date: deal.closing_date,
                triggers: vec![
                    crate::structured_credit::StepDownTrigger::MinCreditEnhancement(level),
                ],
            }),
            ..Default::default()
        });
        deal
    }

    fn state_of(deal: &StructuredCredit) -> SimulationState<'_> {
        SimulationState::new(
            &deal.pool,
            &deal.tranches,
            deal.closing_date,
            deal.closing_date,
            0,
        )
        .expect("state")
    }

    /// The trigger's formula at `e9a972feb5^`, before it shared
    /// `tranche_credit_enhancement`: only the first tranche holding the lowest
    /// `payment_priority` is subtracted.
    fn formula_before_sharing(state: &SimulationState<'_>) -> f64 {
        let pool = state.pool_outstanding.amount() + state.principal_funding_account.amount();
        let senior = state
            .tranches
            .tranches
            .iter()
            .min_by_key(|t| t.payment_priority)
            .and_then(|t| state.tranche_balances.get(t.id.as_str()))
            .map_or(0.0, |m| m.amount());
        (pool - senior) / pool
    }

    /// `TrancheStructure::new` and deserialization give same-seniority classes
    /// distinct priorities, so the trigger subtracts A1 alone: the sharing of
    /// `tranche_credit_enhancement` moved no value for any deal built that way.
    #[test]
    fn same_seniority_classes_stay_ranked_so_the_trigger_value_is_the_first_class_alone() {
        let deal = same_seniority_deal(0.45);
        assert_eq!(
            deal.tranches
                .tranches
                .iter()
                .map(|t| t.payment_priority)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        let state = state_of(&deal);
        assert_eq!(step_down_metrics(&state).credit_enhancement, 0.60);
        assert_eq!(formula_before_sharing(&state), 0.60);
        // The step-down is in effect because the trigger reads 0.60 and not 0.30:
        // on the first payment date principal reaches the classes behind A1.
        let run = run_reconciled(&deal);
        let first_date = run.diagnostics.periods[0].payment_date;
        assert!(sum_on(&run.tranches["A1"].principal_flows, first_date) > 0.0);
        assert!(sum_on(&run.tranches["A2"].principal_flows, first_date) > 0.0);
    }

    /// Priorities tied by writing the public field (no constructor, wire form or
    /// binding does) are the one place the two formulas differ.
    #[test]
    fn tied_priorities_written_by_hand_subtract_every_tied_class() {
        let mut deal = same_seniority_deal(0.45);
        deal.tranches.tranches[1].payment_priority = 1;
        let state = state_of(&deal);
        assert_eq!(step_down_metrics(&state).credit_enhancement, 0.30);
        assert_eq!(formula_before_sharing(&state), 0.60);
    }

    /// A retired note leaves its OC and IC tests with no claim: the record states
    /// no ratio or cushion, and the test still passes.
    #[test]
    fn a_retired_note_has_no_ratio_or_cushion() {
        use crate::structured_credit::types::CoverageTestSpec;
        let mut deal = clo_with_mezz(vec![
            CoverageTestSpec::oc("CLASS_A", 1.2),
            CoverageTestSpec::ic("CLASS_A", 1.1),
            CoverageTestSpec::oc("CLASS_B", 1.1),
        ]);
        deal.tranches.tranches[0].current_balance = Money::from((0_i64, Currency::USD));
        let run = run_reconciled(&deal);
        let tests = &run.diagnostics.periods[0].coverage_tests;
        let retired: Vec<_> = tests
            .iter()
            .filter(|test| test.test_id.contains("CLASS_A"))
            .collect();
        assert_eq!(retired.len(), 2);
        for test in retired {
            assert_eq!(test.ratio, None, "{}", test.test_id);
            assert_eq!(test.cushion, None, "{}", test.test_id);
            assert!(test.passing, "{}", test.test_id);
        }
        let live = tests
            .iter()
            .find(|test| test.test_id.contains("CLASS_B"))
            .expect("CLASS_B test");
        assert_eq!(
            live.cushion.expect("claim outstanding"),
            live.ratio.expect("claim outstanding") - live.trigger_level
        );
        let json = serde_json::to_value(&run.diagnostics.periods[0].coverage_tests).expect("json");
        assert!(
            json.as_array()
                .expect("rows")
                .iter()
                .any(|row| row["ratio"].is_null())
        );
    }
}
