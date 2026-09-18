//! Subordinated and incentive management fees in the standard template: the
//! subordinated fee ranks after every note coupon, the incentive fee takes
//! its share of the residual once equity has earned its hurdle IRR. The
//! 2026-09-15 audit found `DealFees.subordinated_mgmt_fee_bp` inert and no
//! incentive fee at all, so CLO equity was overstated.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, DealFees, DealType, FundingSource, IncentiveFeeSpec, PoolAsset,
    StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

const CLOSE: (i32, u8, u8) = (2024, 1, 1);

/// Five-class CLO on a flat 100M pool at 8% (no prepayments or defaults)
/// with the standard CLO fees, adjusted by `adjust`.
fn clo(adjust: impl FnOnce(&mut DealFees)) -> StructuredCredit {
    let close = d(CLOSE.0, CLOSE.1, CLOSE.2);
    let maturity = d(2032, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity,
            DayCount::Act360,
        ));
    }
    let tr = |id: &str, a: f64, b: f64, sen: TrancheSeniority, bal: f64, cpn: f64| {
        Tranche::new(
            id,
            a,
            b,
            sen,
            usd(bal),
            TrancheCoupon::Fixed { rate: cpn },
            maturity,
        )
        .expect("tranche")
    };
    let tranches = TrancheStructure::new(vec![
        tr("A", 0.0, 60.0, TrancheSeniority::Senior, 60_000_000.0, 0.05),
        tr(
            "B",
            60.0,
            75.0,
            TrancheSeniority::Mezzanine,
            15_000_000.0,
            0.07,
        ),
        tr(
            "C",
            75.0,
            85.0,
            TrancheSeniority::Mezzanine,
            10_000_000.0,
            0.09,
        ),
        tr(
            "D",
            85.0,
            90.0,
            TrancheSeniority::Subordinated,
            5_000_000.0,
            0.12,
        ),
        tr(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
        ),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-FEES", pool, tranches, close, maturity, "USD-OIS")
            .with_payment_calendar("nyse")
            .with_standard_fees();
    let mut fees = deal.fees.take().expect("standard fees");
    adjust(&mut fees);
    deal.fees = Some(fees);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

fn simulate(deal: &StructuredCredit) -> HashMap<String, TrancheCashflows> {
    let close = d(CLOSE.0, CLOSE.1, CLOSE.2);
    run_simulation(deal, &market(close), close).expect("simulation")
}

/// Equity residual per payment date.
fn residuals(results: &HashMap<String, TrancheCashflows>) -> Vec<(Date, f64)> {
    results["E"]
        .interest_flows
        .iter()
        .map(|(date, amount)| (*date, amount.amount()))
        .collect()
}

#[test]
fn clo_template_ranks_junior_and_incentive_fee_tiers() {
    let waterfall = clo(|_| {}).create_waterfall().expect("waterfall");
    let ids: Vec<&str> = waterfall.tiers.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "fees",
            "A_interest",
            "B_interest",
            "C_interest",
            "D_interest",
            "junior_fees",
            "principal",
            "incentive_fee",
            "equity"
        ]
    );
    let funding = |id: &str| {
        waterfall
            .tiers
            .iter()
            .find(|t| t.id == id)
            .expect("tier")
            .effective_funding()
    };
    assert_eq!(funding("fees"), FundingSource::InterestThenPrincipal);
    assert_eq!(
        funding("junior_fees"),
        FundingSource::Interest,
        "the subordinated fee never draws on principal"
    );
    assert_eq!(funding("incentive_fee"), FundingSource::Interest);
}

#[test]
fn subordinated_fee_reduces_the_residual_by_its_accrual_each_period() {
    let with_fee = simulate(&clo(|fees| fees.incentive_fee = None));
    let without = simulate(&clo(|fees| {
        fees.subordinated_mgmt_fee_bp = 0.0;
        fees.incentive_fee = None;
    }));
    let mut prior = d(CLOSE.0, CLOSE.1, CLOSE.2);
    let mut periods = 0;
    // The fee accrues on the period's collateral balance, which is zero on
    // legal final once the bullets have matured, so compare life periods.
    let legal_final = d(2032, 1, 1);
    for ((date, paid), (other_date, unfeed)) in residuals(&with_fee)
        .iter()
        .zip(residuals(&without))
        .filter(|((date, _), _)| *date < legal_final)
    {
        assert_eq!(*date, other_date);
        // 35 bp per annum (the registry's subordinated fee) on the flat
        // 100M pool, ACT/360 over the period.
        let accrual = DayCount::Act360
            .year_fraction(prior, *date, DayCountContext::default())
            .expect("accrual");
        let expected_fee = 100_000_000.0 * 0.0035 * accrual;
        assert!(
            (unfeed - paid - expected_fee).abs() < 1.0,
            "on {date} the residual must fall by the accrued subordinated fee {expected_fee}, \
             got {unfeed} vs {paid}"
        );
        prior = *date;
        periods += 1;
    }
    assert!(
        periods >= 30,
        "quarterly residuals over the deal's life, got {periods}"
    );
    for note in ["A", "B", "C", "D"] {
        assert_eq!(
            with_fee[note].total_interest, without[note].total_interest,
            "{note} ranks ahead of the subordinated fee"
        );
    }
}

#[test]
fn incentive_fee_starts_once_equity_earns_the_hurdle() {
    let hurdle = 0.04;
    let with_incentive = simulate(&clo(|fees| {
        fees.subordinated_mgmt_fee_bp = 0.0;
        fees.incentive_fee = Some(IncentiveFeeSpec {
            hurdle_irr: hurdle,
            share_pct: 0.20,
        });
    }));
    let without = simulate(&clo(|fees| {
        fees.subordinated_mgmt_fee_bp = 0.0;
        fees.incentive_fee = None;
    }));
    let paid = residuals(&with_incentive);
    let unfeed = residuals(&without);
    assert_eq!(paid.len(), unfeed.len());
    let first_fee = paid
        .iter()
        .zip(unfeed.iter())
        .position(|((_, a), (_, b))| *a < b - 1.0)
        .expect("the residual stream earns the hurdle before legal final");
    assert!(
        first_fee > 4,
        "equity cannot have earned a 4% IRR within the first year, got period {first_fee}"
    );
    for ((date, a), (_, b)) in paid[..first_fee].iter().zip(&unfeed[..first_fee]) {
        assert!(
            (a - b).abs() < 1.0,
            "no incentive fee before the hurdle is met, got {a} vs {b} on {date}"
        );
    }
    // Crossing period: the cash that lifts the equity IRR exactly to the
    // hurdle passes untouched and the manager takes 20% of the excess only.
    let (date, a) = paid[first_fee];
    let (_, b) = unfeed[first_fee];
    let history_before =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityHistory {
            invested_on: d(CLOSE.0, CLOSE.1, CLOSE.2),
            invested: usd(10_000_000.0),
            distributions: unfeed[..first_fee]
                .iter()
                .map(|(date, amount)| (*date, usd(*amount)))
                .collect(),
        };
    let shortfall = history_before
        .hurdle_shortfall(date, usd(b), hurdle)
        .amount();
    assert!(
        shortfall > 1.0 && shortfall < b - 1.0,
        "the hurdle is crossed inside the period: shortfall {shortfall} of {b}"
    );
    assert!(
        (a - (b - 0.2 * (b - shortfall))).abs() < 1.0,
        "the manager shares only the excess over the hurdle: {a} vs {b} − 0.2 × ({b} − {shortfall}) on {date}"
    );
    let (next_date, next_a) = paid[first_fee + 1];
    let (_, next_b) = unfeed[first_fee + 1];
    assert!(
        (next_a - 0.8 * next_b).abs() < 1.0,
        "after the crossing the manager takes 20% of the whole residual: {next_a} vs 0.8 × {next_b} on {next_date}"
    );
    assert!(
        with_incentive["E"].total_interest.amount()
            < without["E"].total_interest.amount() - 100_000.0,
        "the incentive fee must change equity's cash: {} vs {}",
        with_incentive["E"].total_interest.amount(),
        without["E"].total_interest.amount()
    );
    // The fee only takes what is above the hurdle on the day it is tested, so
    // the equity IRR to date at the first fee date is at least the hurdle.
    let history =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityHistory {
            invested_on: d(CLOSE.0, CLOSE.1, CLOSE.2),
            invested: usd(10_000_000.0),
            distributions: unfeed[..first_fee]
                .iter()
                .map(|(date, amount)| (*date, usd(*amount)))
                .collect(),
        };
    let irr = history
        .irr_with(date, usd(b))
        .expect("irr at the first fee date");
    assert!(
        irr >= hurdle,
        "irr {irr} at {date} must reach the hurdle {hurdle}"
    );
    let before =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityHistory {
            distributions: unfeed[..first_fee - 1]
                .iter()
                .map(|(date, amount)| (*date, usd(*amount)))
                .collect(),
            ..history
        };
    let (prev_date, prev_residual) = unfeed[first_fee - 1];
    let prev_irr = before.irr_with(prev_date, usd(prev_residual));
    assert!(
        prev_irr.is_none_or(|irr| irr < hurdle),
        "the period before must be below the hurdle, got {prev_irr:?}"
    );
}

/// Principal proceeds reaching equity share the incentive fee too: at
/// maturity the 100M pool pays down through the principal tier, the notes
/// take 90M and the manager (whose hurdle equity earned years earlier) takes
/// 20% of the 10M reaching equity.
#[test]
fn incentive_fee_shares_principal_proceeds_above_the_hurdle() {
    let deal = clo(|fees| {
        fees.subordinated_mgmt_fee_bp = 0.0;
        fees.incentive_fee = Some(IncentiveFeeSpec {
            hurdle_irr: 0.04,
            share_pct: 0.20,
        });
    });
    let run = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
        &deal,
        &market(d(CLOSE.0, CLOSE.1, CLOSE.2)),
        d(CLOSE.0, CLOSE.1, CLOSE.2),
    )
    .expect("simulation");
    let equity_principal = run.tranches["E"].total_principal.amount();
    assert!(
        (equity_principal - 8_000_000.0).abs() < 1.0,
        "equity keeps 80% of its 10M principal: {equity_principal}"
    );
    let notes_principal: f64 = ["A", "B", "C", "D"]
        .iter()
        .map(|id| run.tranches[*id].total_principal.amount())
        .sum();
    assert!((notes_principal - 90_000_000.0).abs() < 1.0);
    let last = run.diagnostics.periods.last().expect("periods");
    assert!(
        last.fees_paid.amount() >= 2_000_000.0 - 1.0,
        "the manager's 2M share of principal is booked as a fee in the final period: {}",
        last.fees_paid.amount()
    );

    // During a revolving period principal is recycled, not shared.
    let mut revolving = clo(|fees| {
        fees.subordinated_mgmt_fee_bp = 0.0;
        fees.incentive_fee = Some(IncentiveFeeSpec {
            hurdle_irr: 0.0,
            share_pct: 0.20,
        });
    });
    revolving.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    revolving.pool.reinvestment_period = Some(
        finstack_quant_valuations::instruments::fixed_income::structured_credit::ReinvestmentPeriod {
            end_date: d(2028, 1, 1),
            is_active: true,
            criteria: Default::default(),
            amortizing_tranches: Vec::new(),
            assumptions: None,
        },
    );
    let run = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
        &revolving,
        &market(d(CLOSE.0, CLOSE.1, CLOSE.2)),
        d(CLOSE.0, CLOSE.1, CLOSE.2),
    )
    .expect("simulation");
    for period in run
        .diagnostics
        .periods
        .iter()
        .filter(|period| period.payment_date <= d(2028, 1, 1))
    {
        assert!(
            period.reinvested_par.amount() > 0.0,
            "principal is recycled while revolving on {}",
            period.payment_date
        );
    }
}
