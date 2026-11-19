//! Independent qualification of the capital contracts.
//!
//! Each case states an agreement in plain terms, computes what it implies
//! with arithmetic written here (no capital-structure code), and reconciles
//! the evaluator to it period by period. The cases span dated principal and
//! arrears (STATEMENTS-CAPITAL-004), seniority and conservation (006), an
//! agreement-style excess-cash-flow sweep (007), a PIK election (008) and
//! native and consolidated reporting across dated FX (009).

use finstack_quant_cashflows::builder::{AmortizationSpec, CouponType, RateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, PeriodId, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{
    EcfSweepSpec, EcfTerm, FacilitySelection, PaymentClassSpec, PaymentPriority, PikElectionSpec,
    PikToggleSpec, WaterfallSpec,
};
use finstack_quant_statements::evaluator::{Evaluator, ObservationCutoff, StatementResult};
use finstack_quant_statements::types::{
    AmountOrScalar, CapitalFxPolicy, FinancialStatementInstrument,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use rust_decimal::Decimal;
use time::macros::date;

const QUARTERS: usize = 8;

/// A term loan issued on 1 January 2025 that pays quarterly on the first day
/// of each quarter on 30/360, so every coupon is a quarter of the annual
/// rate on the balance held through the quarter.
fn loan(
    id: &str,
    currency: Currency,
    face: f64,
    rate: f64,
    amortization: AmortizationSpec,
) -> TermLoan {
    let mut loan = TermLoan::example().expect("term loan");
    loan.id = id.into();
    loan.currency = currency;
    loan.notional_limit = Money::new(face, currency).expect("face");
    loan.issue_date = date!(2025 - 01 - 01);
    loan.maturity = date!(2026 - 10 - 01);
    loan.rate = RateSpec::Fixed { rate };
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Thirty360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.amortization = amortization;
    loan.coupon_type = CouponType::Cash;
    loan
}

fn periods() -> Vec<PeriodId> {
    (2025..=2026)
        .flat_map(|year| {
            (1..=4).map(move |quarter| PeriodId::quarter(year, quarter).expect("quarter"))
        })
        .collect()
}

fn series(values: [f64; QUARTERS]) -> Vec<(PeriodId, AmountOrScalar)> {
    periods()
        .into_iter()
        .zip(values)
        .map(|(period, value)| (period, AmountOrScalar::scalar(value)))
        .collect()
}

fn read(result: &StatementResult, component: &str, scope: &str, period: &PeriodId) -> f64 {
    result
        .cs_cashflows
        .as_ref()
        .expect("capital cash flows")
        .get_component(component, scope, period)
        .unwrap_or_else(|error| panic!("{component} {scope} {period}: {error}"))
}

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: expected {expected:.6}, got {actual:.6}"
    );
}

/// One instrument's row of the hand ledger for one quarter.
#[derive(Debug, Default, Clone, Copy)]
struct Row {
    interest_paid: f64,
    interest_pik: f64,
    interest_arrears: f64,
    scheduled_due: f64,
    scheduled_paid: f64,
    principal_arrears: f64,
    sweep_paid: f64,
    closing: f64,
}

/// The agreement, in terms a credit agreement would use:
///
/// - Term Loan A: 100 at 8%, 5 of amortization on each payment date, senior.
/// - Term Loan B: 60 at 12%, bullet, junior. In a quarter in which liquidity
///   is below 20 the borrower elects to capitalize half of the next interest
///   period's coupon, for at least one interest period.
/// - Both mature on 1 October 2026.
/// - Each quarter the cash pool pays interest, then scheduled principal,
///   senior before junior, with unpaid amounts carried as arrears; then 50%
///   of excess cash flow, to the cent, prepays the loans at par, senior
///   first; the rest is distributed.
/// - Excess cash flow is EBITDA less cash taxes, less capital expenditure,
///   less cash interest and scheduled principal actually paid in the
///   quarter, plus asset-sale proceeds.
struct Agreement {
    pool: [f64; QUARTERS],
    ebitda: [f64; QUARTERS],
    asset_sale: [f64; QUARTERS],
    liquidity: [f64; QUARTERS],
}

const TAXES: f64 = 2.0;
const CAPEX: f64 = 3.0;
const INSTALLMENT: f64 = 5.0;
const LIQUIDITY_FLOOR: f64 = 20.0;

impl Agreement {
    fn example() -> Self {
        Self {
            // The third quarter cannot cover the senior coupon and
            // installment; the last quarter funds the maturity.
            pool: [50.0, 50.0, 3.0, 80.0, 50.0, 50.0, 50.0, 500.0],
            ebitda: [0.0, 0.0, 0.0, 60.0, 0.0, 30.0, 0.0, 0.0],
            asset_sale: [0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 0.0, 0.0],
            liquidity: [100.0, 10.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
        }
    }

    /// The ledger the agreement implies, by hand: `[A, B]` rows and the
    /// residual distributed, for each quarter.
    fn ledger(&self) -> Vec<([Row; 2], f64)> {
        let quarterly_rate = [0.08 / 4.0, 0.12 / 4.0];
        let mut balance = [100.0_f64, 60.0];
        let mut interest_arrears = [0.0_f64; 2];
        let mut principal_arrears = [0.0_f64; 2];
        // Balance each loan held through the previous quarter, and whether
        // Term Loan B elected PIK for the interest period that quarter was.
        let mut held = [0.0_f64; 2];
        let mut elected = [false; QUARTERS + 2];
        let mut out = Vec::new();
        for quarter in 0..QUARTERS {
            let mut rows = [Row::default(); 2];
            let mut cash = self.pool[quarter];
            let mut interest_due = [0.0_f64; 2];
            let mut scheduled_due = [0.0_f64; 2];
            // The first payment date is the first day of the second quarter.
            if quarter > 0 {
                for loan in 0..2 {
                    let coupon = quarterly_rate[loan] * held[loan];
                    if loan == 1 && elected[quarter - 1] {
                        // Half is capitalized on the payment date.
                        rows[loan].interest_pik = coupon / 2.0;
                        balance[loan] += coupon / 2.0;
                        interest_due[loan] = coupon / 2.0;
                    } else {
                        interest_due[loan] = coupon;
                    }
                }
                let maturity = quarter == QUARTERS - 1;
                scheduled_due[0] = if maturity {
                    balance[0]
                } else {
                    INSTALLMENT.min(balance[0])
                };
                scheduled_due[1] = if maturity { balance[1] } else { 0.0 };
                // Principal falls on its effective date, paid or not.
                for loan in 0..2 {
                    balance[loan] -= scheduled_due[loan];
                    rows[loan].scheduled_due = scheduled_due[loan];
                }
            }
            held = balance;
            // Interest, senior first; then scheduled principal, senior first.
            for loan in 0..2 {
                let claim = interest_due[loan] + interest_arrears[loan];
                let paid = claim.min(cash);
                cash -= paid;
                interest_arrears[loan] = claim - paid;
                rows[loan].interest_paid = paid;
                rows[loan].interest_arrears = interest_arrears[loan];
            }
            for loan in 0..2 {
                let claim = scheduled_due[loan] + principal_arrears[loan];
                let paid = claim.min(cash);
                cash -= paid;
                principal_arrears[loan] = claim - paid;
                rows[loan].scheduled_paid = paid;
                rows[loan].principal_arrears = principal_arrears[loan];
            }
            // The sweep, at par and senior first, limited by the request,
            // the cash left and the principal outstanding.
            let paid_service: f64 = rows
                .iter()
                .map(|row| row.interest_paid + row.scheduled_paid)
                .sum();
            let excess =
                self.ebitda[quarter] - TAXES - CAPEX - paid_service + self.asset_sale[quarter];
            // A prepayment is requested in whole cents.
            let request = (0.5 * excess.max(0.0) * 100.0).round() / 100.0;
            let mut budget = request.min(cash);
            for loan in 0..2 {
                let paid = budget.min(balance[loan]);
                budget -= paid;
                cash -= paid;
                balance[loan] -= paid;
                rows[loan].sweep_paid = paid;
                rows[loan].closing = balance[loan];
            }
            // The election covers the next interest period.
            elected[quarter + 1] = self.liquidity[quarter] < LIQUIDITY_FLOOR;
            out.push((rows, cash));
        }
        out
    }

    fn evaluate(&self) -> StatementResult {
        let mut builder = ModelBuilder::new("independent-ledger");
        builder.periods("2025Q1..2026Q4", None).expect("periods");
        builder.value("pool", &series(self.pool));
        builder.value("ebitda", &series(self.ebitda));
        builder.value("asset_sale", &series(self.asset_sale));
        builder.value("liquidity", &series(self.liquidity));
        builder.value("taxes", &series([TAXES; QUARTERS]));
        builder.value("capex", &series([CAPEX; QUARTERS]));
        builder.add_debt(
            "TL-A",
            FinancialStatementInstrument::TermLoan(loan(
                "TL-A",
                Currency::USD,
                100.0,
                0.08,
                AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.05 },
            )),
        );
        builder.add_debt(
            "TL-B",
            FinancialStatementInstrument::TermLoan(loan(
                "TL-B",
                Currency::USD,
                60.0,
                0.12,
                AmortizationSpec::None,
            )),
        );
        let input = |expression: &str| EcfTerm::Input {
            expression: expression.to_string(),
        };
        builder.waterfall(WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "pool".into(),
            payment_classes: vec![
                PaymentClassSpec {
                    id: "senior".into(),
                    rank: 0,
                    instrument_ids: vec!["TL-A".into()],
                },
                PaymentClassSpec {
                    id: "junior".into(),
                    rank: 1,
                    instrument_ids: vec!["TL-B".into()],
                },
            ],
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    input("taxes"),
                    input("capex"),
                    EcfTerm::PaidCashInterest,
                    EcfTerm::PaidScheduledPrincipal,
                ],
                credits: vec![input("asset_sale")],
                sweep_percentage: 0.5,
                facilities: FacilitySelection::AllEligible,
                payment_lag_days: None,
            }),
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: LIQUIDITY_FLOOR,
                elections: vec![PikElectionSpec {
                    instrument_id: "TL-B".into(),
                    pik_fraction: Decimal::new(5, 1),
                    min_coupon_periods: 1,
                    notice_days: 0,
                }],
            }),
            ..Default::default()
        });
        let model = builder.build().expect("model");
        Evaluator::new()
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                date!(2025 - 01 - 01),
                ObservationCutoff::FullSnapshot,
            )
            .expect("evaluation")
    }
}

/// A two-tranche agreement over eight quarters, with a cash shortfall, a
/// PIK election, an agreement-defined sweep and a maturity, reconciled
/// quarter by quarter to a ledger computed by hand.
#[test]
fn the_evaluator_reconciles_to_an_independent_debt_ledger() {
    let agreement = Agreement::example();
    let ledger = agreement.ledger();
    let result = agreement.evaluate();
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");

    for (period, (rows, distributed)) in periods().iter().zip(&ledger) {
        for (id, row) in ["TL-A", "TL-B"].iter().zip(rows) {
            let get = |component: &str| read(&result, component, id, period);
            let label = |what: &str| format!("{id} {period} {what}");
            close(
                get("interest_expense_cash"),
                row.interest_paid,
                &label("cash interest"),
            );
            close(get("interest_expense_pik"), row.interest_pik, &label("PIK"));
            close(
                get("interest_arrears"),
                row.interest_arrears,
                &label("interest arrears"),
            );
            close(
                get("scheduled_principal_due"),
                row.scheduled_due,
                &label("principal due"),
            );
            close(
                get("scheduled_principal"),
                row.scheduled_paid,
                &label("scheduled paid"),
            );
            close(
                get("principal_arrears"),
                row.principal_arrears,
                &label("principal arrears"),
            );
            close(get("sweep_prepayment"), row.sweep_paid, &label("sweep"));
            close(get("debt_balance"), row.closing, &label("closing balance"));
        }
        close(
            cashflows.equity_distribution[period].amount(),
            *distributed,
            &format!("{period} distribution"),
        );
        // Every dollar of the pool is accounted for.
        let uses: f64 = rows
            .iter()
            .map(|row| row.interest_paid + row.scheduled_paid + row.sweep_paid)
            .sum::<f64>()
            + distributed;
        close(
            uses,
            cashflows.available_cash[period].amount(),
            &format!("{period} uses"),
        );
    }

    // The ledger itself shows each feature the agreement describes, so the
    // reconciliation above is not vacuous.
    let third = &ledger[2].0;
    assert!(third[0].interest_arrears > 0.0 || third[0].principal_arrears > 0.0);
    assert!(
        third[1].interest_arrears > 0.0,
        "the junior coupon is unpaid"
    );
    assert!(
        ledger[3].0[1].interest_pik > 0.0,
        "the elected coupon capitalizes"
    );
    assert!(
        ledger[4].0[1].interest_pik == 0.0,
        "the election lasts one period"
    );
    assert!(ledger[3].0[0].sweep_paid > 0.0 && ledger[5].0[0].sweep_paid > 0.0);
    let last = &ledger[QUARTERS - 1].0;
    assert!(
        last.iter()
            .all(|row| row.closing == 0.0 && row.principal_arrears == 0.0)
    );
    // Each loan repays what it borrowed plus what it capitalized.
    for (loan, face) in [100.0, 60.0].into_iter().enumerate() {
        let repaid: f64 = ledger
            .iter()
            .map(|(rows, _)| rows[loan].scheduled_paid + rows[loan].sweep_paid)
            .sum();
        let capitalized: f64 = ledger.iter().map(|(rows, _)| rows[loan].interest_pik).sum();
        close(repaid, face + capitalized, "principal repaid");
    }
}

/// A euro loan and a dollar loan reported in dollars while the period-close
/// rate moves: each native total stays in its own currency, and the
/// consolidated balance is the dollar balance plus the euro balance at that
/// quarter's own close.
#[test]
fn native_and_consolidated_balances_reconcile_across_dated_fx() {
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, SimpleFxProvider};
    use std::sync::Arc;

    let mut builder = ModelBuilder::new("independent-fx");
    builder.periods("2025Q1..2025Q4", None).expect("periods");
    let quarters: Vec<PeriodId> = (1..=4)
        .map(|quarter| PeriodId::quarter(2025, quarter).expect("quarter"))
        .collect();
    builder.value(
        "pool",
        &quarters
            .iter()
            .map(|period| (*period, AmountOrScalar::scalar(1_000.0)))
            .collect::<Vec<_>>(),
    );
    builder.add_debt(
        "EUR-TL",
        FinancialStatementInstrument::TermLoan(loan(
            "EUR-TL",
            Currency::EUR,
            200.0,
            0.04,
            AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.10 },
        )),
    );
    builder.add_debt(
        "USD-TL",
        FinancialStatementInstrument::TermLoan(loan(
            "USD-TL",
            Currency::USD,
            300.0,
            0.08,
            AmortizationSpec::None,
        )),
    );
    builder.reporting_currency(Currency::USD);
    builder.fx_policy(CapitalFxPolicy::PeriodEnd);
    let model = builder.build().expect("model");

    // A different rate at each quarter's close.
    let closes = [
        (date!(2025 - 03 - 31), 1.10),
        (date!(2025 - 06 - 30), 1.20),
        (date!(2025 - 09 - 30), 1.05),
        (date!(2025 - 12 - 31), 1.15),
    ];
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 9.99)])
        .expect("fallback quote");
    let matrix = FxMatrix::new(Arc::new(provider), None);
    for (on, rate) in closes {
        matrix
            .set_quote_on(
                Currency::EUR,
                Currency::USD,
                on,
                FxConversionPolicy::PeriodEnd,
                rate,
            )
            .expect("dated quote");
    }
    let result = Evaluator::new()
        .evaluate_with_market(
            &model,
            &MarketContext::new().insert_fx(Arc::new(matrix)),
            date!(2025 - 01 - 01),
            ObservationCutoff::FullSnapshot,
        )
        .expect("evaluation");
    let cashflows = result.cs_cashflows.as_ref().expect("capital cash flows");

    // By hand: the euro loan repays 20 on each payment date from 1 April.
    let euro_balance = [200.0, 180.0, 160.0, 140.0];
    let euro_interest = [0.0, 2.0, 1.8, 1.6];
    for (index, period) in quarters.iter().enumerate() {
        let (on, rate) = closes[index];
        close(
            read(&result, "debt_balance", "total.EUR", period),
            euro_balance[index],
            "EUR balance",
        );
        close(
            read(&result, "debt_balance", "total.USD", period),
            300.0,
            "USD balance",
        );
        close(
            read(&result, "debt_balance", "total", period),
            300.0 + euro_balance[index] * rate,
            "consolidated balance",
        );
        let dollar_interest = if index == 0 { 0.0 } else { 6.0 };
        close(
            read(&result, "interest_expense_cash", "total", period),
            dollar_interest + euro_interest[index] * rate,
            "consolidated interest",
        );
        let record = &cashflows.fx_conversions[period][0];
        assert_eq!(
            (record.from, record.to, record.date),
            (Currency::EUR, Currency::USD, on)
        );
        close(record.rate, rate, "recorded rate");
    }
}
