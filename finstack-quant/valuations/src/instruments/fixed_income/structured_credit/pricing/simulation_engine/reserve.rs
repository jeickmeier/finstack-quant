//! Reserve-account mechanics for collateral draws and reserve interest.
//!
//! The deal reserve account funds every collateral draw (revolver utilization
//! increases, delayed-draw term-loan draws, LEQ draws at default). Draws that
//! exceed the reserve are funded from the period's principal collections, and
//! anything still short is recorded as unfunded. Revolver repayments replenish
//! the reserve toward `reserve_target` before counting as principal. The
//! reserve earns `reserve_account_rate` on its opening balance each period,
//! routed per [`ReserveInterestDestination`].

use super::*;
use crate::instruments::fixed_income::structured_credit::types::ReserveInterestDestination;

/// Cash used to fund one period's collateral draws, by source.
#[derive(Debug, Clone, Copy)]
pub(super) struct DrawFunding {
    /// Drawn down from the reserve account.
    pub(super) from_reserve: Money,
    /// Taken from this period's principal collections.
    pub(super) from_principal: Money,
    /// Requested but not fundable this period.
    pub(super) unfunded: Money,
}

/// Fund the collateral draws requested this period.
///
/// Debits the reserve first, then the supplied principal collections, and
/// records the remainder as unfunded on the state. The caller applies the
/// funded amount (`from_reserve + from_principal`) to the drawing
/// instruments' balances and reports `from_principal` on the period flows so
/// the engine keeps it out of the waterfall.
///
/// # Arguments
///
/// * `state` - Simulation state whose reserve balance is debited.
/// * `requested` - Total draws requested by the collateral this period.
/// * `principal_collections` - Scheduled principal plus prepayments collected
///   this period, available to fund draws after the reserve is exhausted.
pub(super) fn fund_collateral_draws(
    state: &mut SimulationState,
    requested: Money,
    principal_collections: Money,
) -> Result<DrawFunding> {
    let ccy = state.base_currency;
    let requested_amt = requested.amount().max(0.0);
    let from_reserve = requested_amt.min(state.reserve_balance.amount().max(0.0));
    let remaining = requested_amt - from_reserve;
    let from_principal = remaining.min(principal_collections.amount().max(0.0));
    let unfunded = remaining - from_principal;

    let from_reserve = Money::new(from_reserve, ccy)?;
    let from_principal = Money::new(from_principal, ccy)?;
    let unfunded = Money::new(unfunded, ccy)?;

    state.reserve_balance = state.reserve_balance.checked_sub(from_reserve)?;
    state.draws_from_reserve = state.draws_from_reserve.checked_add(from_reserve)?;
    state.draws_from_principal = state.draws_from_principal.checked_add(from_principal)?;
    state.cumulative_unfunded_draws = state.cumulative_unfunded_draws.checked_add(unfunded)?;

    Ok(DrawFunding {
        from_reserve,
        from_principal,
        unfunded,
    })
}

/// Replenish the reserve from revolver repayments, up to `reserve_target`.
///
/// Returns the amount diverted, which the caller reports on the period flows
/// so the engine keeps it out of the waterfall. No-op without a target.
///
/// # Arguments
///
/// * `state` - Simulation state whose reserve balance is credited.
/// * `repayments` - Revolver principal repaid this period.
pub(super) fn replenish_reserve_from_repayments(
    state: &mut SimulationState,
    repayments: Money,
) -> Result<Money> {
    let ccy = state.base_currency;
    let Some(target) = state.pool.reserve_target else {
        return Ok(Money::from((0_i64, ccy)));
    };
    let room = (target.amount() - state.reserve_balance.amount()).max(0.0);
    let diverted = Money::new(repayments.amount().max(0.0).min(room), ccy)?;
    state.reserve_balance = state.reserve_balance.checked_add(diverted)?;
    state.reserve_replenished = state.reserve_replenished.checked_add(diverted)?;
    Ok(diverted)
}

/// Interest earned by the reserve over `[period_start, pay_date]` on its
/// opening balance: simple ACT/360 at `reserve_account_rate`.
///
/// # Arguments
///
/// * `state` - Simulation state supplying the opening reserve balance and rate.
/// * `period_start` - Accrual start of the legal period.
/// * `pay_date` - Payment date of the legal period.
pub(super) fn reserve_interest_amount(
    state: &SimulationState,
    period_start: Date,
    pay_date: Date,
) -> Result<Money> {
    let rate = state.pool.reserve_account_rate;
    let balance = state.reserve_balance.amount();
    if rate <= 0.0 || balance <= 0.0 || pay_date <= period_start {
        return Ok(Money::from((0_i64, state.base_currency)));
    }
    let accrual =
        DayCount::Act360.year_fraction(period_start, pay_date, DayCountContext::default())?;
    Money::new(balance * rate * accrual, state.base_currency)
}

/// Reserve interest routed for one period.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReserveInterest {
    /// Portion that enters the waterfall as interest proceeds.
    pub(super) to_waterfall: Money,
}

/// Route the period's reserve interest per the pool's destination.
///
/// `Waterfall` returns the amount as interest proceeds; `Tranche` pays it
/// directly to the named tranche (recorded on its interest flows, outside the
/// waterfall and the interest-coverage numerator); `Retain` capitalizes it
/// into the reserve.
///
/// # Arguments
///
/// * `state` - Simulation state receiving the routed cash.
/// * `amount` - Interest computed by [`reserve_interest_amount`].
/// * `pay_date` - Date the interest is credited or paid.
pub(super) fn route_reserve_interest(
    state: &mut SimulationState,
    amount: Money,
    pay_date: Date,
) -> Result<ReserveInterest> {
    let zero = Money::from((0_i64, state.base_currency));
    if amount.amount() <= 0.0 {
        return Ok(ReserveInterest { to_waterfall: zero });
    }
    state.reserve_interest_paid.push((pay_date, amount));
    match &state.pool.reserve_interest_destination {
        ReserveInterestDestination::Waterfall => Ok(ReserveInterest {
            to_waterfall: amount,
        }),
        ReserveInterestDestination::Tranche { tranche_id } => {
            let res = state.results.get_mut(tranche_id.as_str()).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "reserve_interest_destination names unknown tranche '{tranche_id}'"
                ))
            })?;
            res.cashflows.push((pay_date, amount));
            res.interest_flows.push((pay_date, amount));
            res.total_interest = res.total_interest.checked_add(amount)?;
            Ok(ReserveInterest { to_waterfall: zero })
        }
        ReserveInterestDestination::Retain => {
            state.reserve_balance = state.reserve_balance.checked_add(amount)?;
            Ok(ReserveInterest { to_waterfall: zero })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, PoolAsset, Tranche, TrancheCoupon, TrancheStructure,
    };
    use finstack_quant_core::types::InstrumentId;
    use time::macros::date;

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("money")
    }

    fn pool(reserve: f64, rate: f64, target: Option<f64>) -> AssetPool {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            usd(10_000_000.0),
            0.06,
            date!(2030 - 01 - 01),
            DayCount::Thirty360,
        ));
        pool.reserve_account = usd(reserve);
        pool.reserve_account_rate = rate;
        pool.reserve_target = target.map(usd);
        pool
    }

    fn tranches() -> TrancheStructure {
        let senior = Tranche::new(
            "SENIOR",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(9_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            date!(2030 - 01 - 01),
        )
        .expect("senior");
        let equity = Tranche::new(
            "EQUITY",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(1_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            date!(2030 - 01 - 01),
        )
        .expect("equity");
        TrancheStructure::new(vec![senior, equity]).expect("tranches")
    }

    #[test]
    fn draws_fund_from_reserve_then_principal_then_unfunded() {
        let pool = pool(300_000.0, 0.0, None);
        let tranches = tranches();
        let mut state = SimulationState::new(
            &pool,
            &tranches,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 01),
            6,
        )
        .expect("state");

        let funding =
            fund_collateral_draws(&mut state, usd(1_000_000.0), usd(500_000.0)).expect("funding");
        assert_eq!(funding.from_reserve, usd(300_000.0));
        assert_eq!(funding.from_principal, usd(500_000.0));
        assert_eq!(funding.unfunded, usd(200_000.0));
        assert_eq!(state.reserve_balance, usd(0.0));
        assert_eq!(state.draws_from_reserve, usd(300_000.0));
        assert_eq!(state.draws_from_principal, usd(500_000.0));
        assert_eq!(state.cumulative_unfunded_draws, usd(200_000.0));

        // A draw fully covered by the reserve leaves principal untouched.
        state.reserve_balance = usd(2_000_000.0);
        let funding =
            fund_collateral_draws(&mut state, usd(1_000_000.0), usd(500_000.0)).expect("funding");
        assert_eq!(funding.from_reserve, usd(1_000_000.0));
        assert_eq!(funding.from_principal, usd(0.0));
        assert_eq!(funding.unfunded, usd(0.0));
        assert_eq!(state.reserve_balance, usd(1_000_000.0));
    }

    #[test]
    fn repayments_replenish_the_reserve_up_to_target() {
        let pool = pool(0.0, 0.0, Some(300_000.0));
        let tranches = tranches();
        let mut state = SimulationState::new(
            &pool,
            &tranches,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 01),
            6,
        )
        .expect("state");
        let diverted =
            replenish_reserve_from_repayments(&mut state, usd(450_000.0)).expect("replenish");
        assert_eq!(diverted, usd(300_000.0));
        assert_eq!(state.reserve_balance, usd(300_000.0));
        // At target nothing more is diverted.
        let diverted =
            replenish_reserve_from_repayments(&mut state, usd(450_000.0)).expect("replenish");
        assert_eq!(diverted, usd(0.0));

        // Without a target repayments are ordinary principal.
        let pool = pool_without_target();
        let mut state = SimulationState::new(
            &pool,
            &tranches,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 01),
            6,
        )
        .expect("state");
        assert_eq!(
            replenish_reserve_from_repayments(&mut state, usd(450_000.0)).expect("replenish"),
            usd(0.0)
        );
    }

    fn pool_without_target() -> AssetPool {
        pool(0.0, 0.0, None)
    }

    /// Deterministic pool flows plus one collateral draw in `draw_period`,
    /// funded through the shared helper and applied to the first asset.
    struct DrawingSource {
        draw_period: usize,
        draw: f64,
        period: usize,
    }

    impl super::super::PoolFlowSource for DrawingSource {
        fn calculate_pool_flows(
            &mut self,
            request: super::super::PoolFlowRequest<'_, '_>,
        ) -> Result<PoolFlows> {
            let idx = self.period;
            self.period += 1;
            let super::super::PoolFlowRequest {
                state,
                instrument,
                pay_date,
                prev_date,
                seasoning_months,
                months_per_period,
                context,
            } = request;
            let mut flows = super::super::DeterministicPoolFlowSource.calculate_pool_flows(
                super::super::PoolFlowRequest {
                    state: &mut *state,
                    instrument,
                    pay_date,
                    prev_date,
                    seasoning_months,
                    months_per_period,
                    context,
                },
            )?;
            if idx == self.draw_period {
                let collections = flows.scheduled_principal.checked_add(flows.prepayment)?;
                let funding = fund_collateral_draws(
                    state,
                    Money::new(self.draw, state.base_currency)?,
                    collections,
                )?;
                let funded = funding.from_reserve.checked_add(funding.from_principal)?;
                state.pool_state.balances[0] += funded.amount();
                flows.draw_from_principal = funding.from_principal;
                assert_eq!(funding.unfunded, usd(0.0));
            }
            Ok(flows)
        }
    }

    /// The example deal with an equity tranche so residual cash (repaid draws,
    /// reserve interest) has a recipient, and no prepayment or default noise.
    fn quiet_example(reserve: f64, rate: f64) -> StructuredCredit {
        let mut deal = StructuredCredit::example();
        deal.tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                10.0,
                100.0,
                TrancheSeniority::Senior,
                usd(90_000_000.0),
                TrancheCoupon::Fixed { rate: 0.06 },
                deal.maturity,
            )
            .expect("senior"),
            Tranche::new(
                "EQ",
                0.0,
                10.0,
                TrancheSeniority::Equity,
                usd(10_000_000.0),
                TrancheCoupon::Fixed { rate: 0.0 },
                deal.maturity,
            )
            .expect("equity"),
        ])
        .expect("tranches");
        deal.credit_model.prepayment_spec =
            crate::instruments::fixed_income::structured_credit::PrepaymentModelSpec::constant_cpr(
                0.0,
            );
        deal.credit_model.default_spec =
            crate::instruments::fixed_income::structured_credit::DefaultModelSpec::constant_cdr(
                0.0,
            );
        deal.pool.reserve_account = usd(reserve);
        deal.pool.reserve_account_rate = rate;
        deal
    }

    fn totals(run: &super::super::SimulationRun) -> (f64, f64) {
        let interest: f64 = run
            .tranches
            .values()
            .map(|r| r.total_interest.amount())
            .sum();
        let principal: f64 = run
            .tranches
            .values()
            .map(|r| r.total_principal.amount())
            .sum();
        (interest, principal)
    }

    #[test]
    fn collateral_draw_is_funded_from_the_reserve_and_conserves_value() {
        let deal = quiet_example(5_000_000.0, 0.0);
        let market = MarketContext::new();
        let baseline = super::super::simulate_with_source(
            &deal,
            &market,
            deal.closing_date,
            &mut super::super::DeterministicPoolFlowSource,
        )
        .expect("baseline");
        let drawn = super::super::simulate_with_source(
            &deal,
            &market,
            deal.closing_date,
            &mut DrawingSource {
                draw_period: 1,
                draw: 1_000_000.0,
                period: 0,
            },
        )
        .expect("drawn run passes cash conservation");

        let d = &drawn.diagnostics;
        assert_eq!(d.draws_from_reserve, usd(1_000_000.0));
        assert_eq!(d.draws_from_principal, usd(0.0));
        assert_eq!(d.unfunded_draws, usd(0.0));

        // The reserve path is the baseline path shifted down by the draw from
        // the draw period onward.
        let base_path = &baseline.diagnostics.reserve_balance_path;
        assert_eq!(base_path.len(), d.reserve_balance_path.len());
        for (k, ((date_b, bal_b), (date_d, bal_d))) in
            base_path.iter().zip(&d.reserve_balance_path).enumerate()
        {
            assert_eq!(date_b, date_d);
            let expected = if k >= 1 {
                bal_b.amount() - 1_000_000.0
            } else {
                bal_b.amount()
            };
            assert!(
                (bal_d.amount() - expected).abs() < 1e-6,
                "period {k}: reserve {} vs expected {expected}",
                bal_d.amount()
            );
        }

        // Value conserved: the drawn 1M is repaid by the collateral and the
        // reserve released at deal end is 1M smaller, so the only change in
        // total tranche cash is the extra coupon earned on the larger balance
        // (positive, and well below the 1M principal itself). The waterfall
        // may classify the repaid draw as residual rather than principal, so
        // the split is not asserted.
        let (int_b, prin_b) = totals(&baseline);
        let (int_d, prin_d) = totals(&drawn);
        let extra = (int_d + prin_d) - (int_b + prin_b);
        assert!(
            extra > 1.0 && extra < 1_000_000.0,
            "total tranche cash moved by {extra}; expected only the coupon on the drawn 1M"
        );
    }

    #[test]
    fn deal_level_reserve_interest_reaches_its_destination() {
        let market = MarketContext::new();
        let silent = quiet_example(5_000_000.0, 0.0);
        let base = super::super::simulate_with_source(
            &silent,
            &market,
            silent.closing_date,
            &mut super::super::DeterministicPoolFlowSource,
        )
        .expect("base");
        let (int_base, prin_base) = totals(&base);

        // Waterfall: interest proceeds rise.
        let wf = quiet_example(5_000_000.0, 0.04);
        let run_wf = super::super::simulate_with_source(
            &wf,
            &market,
            wf.closing_date,
            &mut super::super::DeterministicPoolFlowSource,
        )
        .expect("waterfall destination");
        let earned: f64 = run_wf
            .diagnostics
            .reserve_interest_paid
            .iter()
            .map(|(_, m)| m.amount())
            .sum();
        assert!(earned > 0.0);
        let (int_wf, prin_wf) = totals(&run_wf);
        assert!(
            ((int_wf + prin_wf) - (int_base + prin_base) - earned).abs() < 1.0,
            "waterfall destination must distribute exactly the reserve earnings {earned}"
        );

        // Named tranche: that tranche alone receives exactly the earnings.
        let target = wf
            .tranches
            .tranches
            .iter()
            .find(|t| t.seniority == TrancheSeniority::Equity)
            .or_else(|| wf.tranches.tranches.last())
            .expect("a tranche")
            .id
            .clone();
        let mut direct = quiet_example(5_000_000.0, 0.04);
        direct.pool.reserve_interest_destination = ReserveInterestDestination::Tranche {
            tranche_id: target.clone(),
        };
        let run_direct = super::super::simulate_with_source(
            &direct,
            &market,
            direct.closing_date,
            &mut super::super::DeterministicPoolFlowSource,
        )
        .expect("tranche destination");
        let earned_direct: f64 = run_direct
            .diagnostics
            .reserve_interest_paid
            .iter()
            .map(|(_, m)| m.amount())
            .sum();
        let got = run_direct.tranches[target.as_str()].total_interest.amount()
            - base.tranches[target.as_str()].total_interest.amount();
        assert!(
            (got - earned_direct).abs() < 1e-6,
            "tranche {target} received {got}, reserve earned {earned_direct}"
        );
        for (id, res) in &run_direct.tranches {
            if id != target.as_str() {
                assert!(
                    (res.total_interest.amount() - base.tranches[id].total_interest.amount()).abs()
                        < 1e-6,
                    "tranche {id} interest must be unchanged"
                );
            }
        }

        // Retain: the reserve grows and is released as principal at deal end.
        let mut retain = quiet_example(5_000_000.0, 0.04);
        retain.pool.reserve_interest_destination = ReserveInterestDestination::Retain;
        let run_retain = super::super::simulate_with_source(
            &retain,
            &market,
            retain.closing_date,
            &mut super::super::DeterministicPoolFlowSource,
        )
        .expect("retain destination");
        let earned_retain: f64 = run_retain
            .diagnostics
            .reserve_interest_paid
            .iter()
            .map(|(_, m)| m.amount())
            .sum();
        assert!(earned_retain > earned, "retained interest compounds");
        let (int_retain, prin_retain) = totals(&run_retain);
        assert!((int_retain - int_base).abs() < 1e-6);
        assert!((prin_retain - prin_base - earned_retain).abs() < 1.0);
    }

    #[test]
    fn reserve_interest_is_act360_simple_and_routed_per_destination() {
        let tranches = tranches();
        let start = date!(2025 - 01 - 01);
        let pay = date!(2025 - 04 - 01); // 90 days
        let expected = 1_000_000.0 * 0.04 * 90.0 / 360.0;

        // Waterfall destination.
        let pool_wf = pool(1_000_000.0, 0.04, None);
        let mut state = SimulationState::new(&pool_wf, &tranches, start, start, 6).expect("state");
        let amount = reserve_interest_amount(&state, start, pay).expect("amount");
        assert!((amount.amount() - expected).abs() < 1e-9);
        let routed = route_reserve_interest(&mut state, amount, pay).expect("route");
        assert_eq!(routed.to_waterfall, amount);
        assert_eq!(state.reserve_balance, usd(1_000_000.0));
        assert_eq!(state.reserve_interest_paid, vec![(pay, amount)]);

        // Direct to the equity tranche.
        let mut pool_eq = pool(1_000_000.0, 0.04, None);
        pool_eq.reserve_interest_destination = ReserveInterestDestination::Tranche {
            tranche_id: InstrumentId::new("EQUITY"),
        };
        let mut state = SimulationState::new(&pool_eq, &tranches, start, start, 6).expect("state");
        let routed = route_reserve_interest(&mut state, amount, pay).expect("route");
        assert_eq!(routed.to_waterfall, usd(0.0));
        let equity = &state.results["EQUITY"];
        assert_eq!(equity.total_interest, amount);
        assert_eq!(equity.interest_flows, vec![(pay, amount)]);
        assert_eq!(state.results["SENIOR"].total_interest, usd(0.0));

        // Retained in the reserve.
        let mut pool_ret = pool(1_000_000.0, 0.04, None);
        pool_ret.reserve_interest_destination = ReserveInterestDestination::Retain;
        let mut state = SimulationState::new(&pool_ret, &tranches, start, start, 6).expect("state");
        let routed = route_reserve_interest(&mut state, amount, pay).expect("route");
        assert_eq!(routed.to_waterfall, usd(0.0));
        assert!((state.reserve_balance.amount() - (1_000_000.0 + expected)).abs() < 1e-9);

        // Zero rate or empty reserve earns nothing.
        let pool_zero = pool(1_000_000.0, 0.0, None);
        let state = SimulationState::new(&pool_zero, &tranches, start, start, 6).expect("state");
        assert_eq!(
            reserve_interest_amount(&state, start, pay).expect("amount"),
            usd(0.0)
        );
    }
}
