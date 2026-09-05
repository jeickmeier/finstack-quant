//! Shared model fixtures for `statements_operations` and `statements_scale`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(dead_code)]

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, PaymentPriority, WaterfallSpec};
use finstack_quant_statements::prelude::*;
use finstack_quant_statements::types::FinancialStatementInstrument;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use time::Month;

/// Issue / as-of date shared by capital-structure fixtures.
pub fn issue_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

/// Bond / loan maturity far enough out for a 20-quarter horizon.
pub fn maturity_date() -> Date {
    Date::from_calendar_date(2035, Month::January, 15).unwrap()
}

/// Six-knot downward-sloping USD-OIS curve.
pub fn usd_ois_market(base: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.951),
            (3.0, 0.865),
            (5.0, 0.790),
            (10.0, 0.640),
            (30.0, 0.375),
        ])
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

/// Inclusive quarter range starting at `2025Q1`.
pub fn quarter_range(n_quarters: usize) -> String {
    let last = n_quarters.saturating_sub(1);
    format!("2025Q1..{}Q{}", 2025 + last / 4, (last % 4) + 1)
}

/// Inclusive month range starting at `2024M01`.
pub fn month_range(n_months: usize) -> String {
    let last = n_months.saturating_sub(1);
    format!("2024M01..{}M{:02}", 2024 + last / 12, (last % 12) + 1)
}

/// Quarterly value series starting at `2025Q1`.
pub fn quarterly_values(n: usize, base: f64, step: f64) -> Vec<(PeriodId, AmountOrScalar)> {
    (0..n)
        .map(|i| {
            (
                PeriodId::quarter(2025 + (i / 4) as i32, ((i % 4) + 1) as u8).expect("valid period fixture"),
                AmountOrScalar::scalar(base + i as f64 * step),
            )
        })
        .collect()
}

/// Monthly value series starting at `2024M01`.
pub fn monthly_values(n: usize, base: f64, step: f64) -> Vec<(PeriodId, AmountOrScalar)> {
    (0..n)
        .map(|i| {
            (
                PeriodId::month(2024 + (i / 12) as i32, ((i % 12) + 1) as u8).expect("valid period fixture"),
                AmountOrScalar::scalar(base + i as f64 * step),
            )
        })
        .collect()
}

/// Small P&L used by prepared-evaluation and check benches.
pub fn simple_pl_model() -> FinancialModelSpec {
    ModelBuilder::new("pl")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .forecast("revenue", ForecastSpec::growth(0.05))
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("opex", "revenue * 0.15")
        .unwrap()
        .compute("ebitda", "gross_profit - opex")
        .unwrap()
        .compute("margin", "ebitda / revenue")
        .unwrap()
        .build()
        .unwrap()
}

/// Articulating statements model for the check-suite hot path.
pub fn accounting_model() -> FinancialModelSpec {
    let cash = quarterly_values(4, 500_000.0, 25_000.0);
    let ar = quarterly_values(4, 200_000.0, 10_000.0);
    let debt = quarterly_values(4, 400_000.0, -10_000.0);
    let ni = quarterly_values(4, 40_000.0, 2_000.0);
    let dividends = quarterly_values(4, 10_000.0, 0.0);
    let re = quarterly_values(4, 300_000.0, 32_000.0);
    let cfo = quarterly_values(4, 30_000.0, 1_000.0);
    let cfi = quarterly_values(4, -5_000.0, 0.0);
    let cff = quarterly_values(4, 0.0, 0.0);

    ModelBuilder::new("accounting")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value("cash", &cash)
        .value("ar", &ar)
        .value("debt", &debt)
        .value("net_income", &ni)
        .value("dividends", &dividends)
        .value("retained_earnings", &re)
        .value("cfo", &cfo)
        .value("cfi", &cfi)
        .value("cff", &cff)
        .compute("assets", "cash + ar")
        .unwrap()
        .compute("equity", "assets - debt")
        .unwrap()
        .compute("total_cf", "cfo + cfi + cff")
        .unwrap()
        .build()
        .unwrap()
}

/// Standard accounting + data-quality suite used by check benches.
pub fn standard_check_suite() -> CheckSuite {
    CheckSuite::builder("bench-suite")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("assets")],
            liabilities_nodes: vec![NodeId::new("debt")],
            equity_nodes: vec![NodeId::new("equity")],
            tolerance: Some(1e-6),
        })
        .add_check(CashReconciliation {
            cash_balance_node: NodeId::new("cash"),
            total_cash_flow_node: NodeId::new("total_cf"),
            cfo_node: Some(NodeId::new("cfo")),
            cfi_node: Some(NodeId::new("cfi")),
            cff_node: Some(NodeId::new("cff")),
            tolerance: Some(1.0),
        })
        .add_check(RetainedEarningsReconciliation {
            retained_earnings_node: NodeId::new("retained_earnings"),
            net_income_node: NodeId::new("net_income"),
            dividends_node: Some(NodeId::new("dividends")),
            other_adjustments: vec![],
            tolerance: Some(1.0),
            dividends_sign_convention: Default::default(),
        })
        .add_check(NonFiniteCheck { nodes: vec![] })
        .add_check(MissingValueCheck {
            required_nodes: vec![NodeId::new("cash"), NodeId::new("assets")],
            scope: PeriodScope::AllPeriods,
        })
        .add_check(SignConventionCheck {
            positive_nodes: vec![NodeId::new("assets"), NodeId::new("cash")],
            negative_nodes: vec![],
        })
        .build()
}

/// `n_bonds` fixed-coupon bonds plus `cs.*` interest/principal/balance formulas.
pub fn bond_cs_model(n_bonds: usize, n_quarters: usize) -> FinancialModelSpec {
    let issue = issue_date();
    let maturity = maturity_date();
    let mut builder = ModelBuilder::new("cs-bonds")
        .periods(&quarter_range(n_quarters), Some("2025Q1"))
        .unwrap()
        .value(
            "ebitda",
            &quarterly_values(n_quarters, 5_000_000.0, 50_000.0),
        );

    for i in 0..n_bonds {
        let id = format!("BOND-{i:03}");
        builder = builder
            .add_bond(
                &id,
                Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .unwrap()
            .compute(format!("int_{i}"), format!("cs.interest_expense.{id}"))
            .unwrap()
            .compute(format!("prin_{i}"), format!("cs.principal_payment.{id}"))
            .unwrap();
    }

    builder
        .compute("interest_total", "cs.interest_expense.total")
        .unwrap()
        .compute("principal_total", "cs.principal_payment.total")
        .unwrap()
        .compute("debt_total", "cs.debt_balance.total")
        .unwrap()
        .compute("coverage", "ebitda / interest_total")
        .unwrap()
        .build()
        .unwrap()
}

fn term_loan(id: &str, notional: f64) -> FinancialStatementInstrument {
    FinancialStatementInstrument::TermLoan(
        TermLoan::builder()
            .id(id.into())
            .currency(Currency::USD)
            .notional_limit(Money::new(notional, Currency::USD).expect("valid money fixture"))
            .issue_date(issue_date())
            .maturity(maturity_date())
            .rate(RateSpec::Fixed { rate_bp: 500 })
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(Default::default())
            .attributes(Default::default())
            .build()
            .unwrap(),
    )
}

/// Multi-loan LBO-shaped waterfall with a 50% ECF sweep.
pub fn waterfall_model(n_loans: usize, n_quarters: usize) -> FinancialModelSpec {
    let cash = quarterly_values(n_quarters, 2_000_000_000.0, 0.0);
    let ebitda = quarterly_values(n_quarters, 8_000_000.0, 100_000.0);
    let taxes = quarterly_values(n_quarters, 1_600_000.0, 20_000.0);
    let capex = quarterly_values(n_quarters, 800_000.0, 0.0);

    let mut builder = ModelBuilder::new("cs-waterfall")
        .periods(&quarter_range(n_quarters), None)
        .unwrap()
        .value("cash", &cash)
        .value("ebitda", &ebitda)
        .value("taxes", &taxes)
        .value("capex", &capex);

    for i in 0..n_loans {
        let id = format!("TL-{i:03}");
        builder = builder.add_debt(&id, term_loan(&id, 10_000_000.0));
    }

    builder
        .waterfall(WaterfallSpec {
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".to_string(),
                taxes_node: Some("taxes".to_string()),
                capex_node: Some("capex".to_string()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: None,
            }),
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            pik_toggle: None,
            ..Default::default()
        })
        .compute("interest_total", "cs.interest_expense.total")
        .unwrap()
        .compute("principal_total", "cs.principal_payment.total")
        .unwrap()
        .compute("debt_total", "cs.debt_balance.total")
        .unwrap()
        .build()
        .unwrap()
}

/// Monte Carlo forecast model (small node set, multi-year horizon).
pub fn mc_forecast_model() -> FinancialModelSpec {
    ModelBuilder::new("mc-model")
        .periods("2024Q4..2026Q4", Some("2024Q4"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2024, 4).expect("valid period fixture"), AmountOrScalar::scalar(100.0))],
        )
        .forecast("revenue", ForecastSpec::normal(0.05, 0.02, 42))
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap()
}

/// Single driver plus `n_rolling` rolling-mean formulas over `n_periods` quarters.
pub fn rolling_model(n_rolling: usize, n_periods: usize) -> FinancialModelSpec {
    let revenue_values: Vec<(PeriodId, AmountOrScalar)> = (0..n_periods)
        .map(|i| {
            (
                PeriodId::quarter(2020 + (i / 4) as i32, ((i % 4) + 1) as u8).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0 + i as f64),
            )
        })
        .collect();

    let period_range = format!(
        "2020Q1..{}Q{}",
        2020 + (n_periods - 1) / 4,
        ((n_periods - 1) % 4) + 1
    );

    let mut builder = ModelBuilder::new("rolling")
        .periods(&period_range, None)
        .unwrap()
        .value("revenue", &revenue_values);

    for i in 0..n_rolling {
        let window = (i % 6) + 2;
        builder = builder
            .compute(
                format!("rolling_{i}"),
                format!("rolling_mean(revenue, {window})"),
            )
            .unwrap();
    }

    builder.build().unwrap()
}

/// LBO-shaped operating model: a handful of drivers plus many derived nodes.
pub fn large_lbo_model(n_nodes: usize, n_months: usize) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("lbo")
        .periods(&month_range(n_months), None)
        .unwrap()
        .value("revenue", &monthly_values(n_months, 1_000_000.0, 1_000.0))
        .compute("cogs", "revenue * 0.55")
        .unwrap()
        .compute("opex", "revenue * 0.20")
        .unwrap()
        .compute("ebitda", "revenue - cogs - opex")
        .unwrap();

    for i in 0..n_nodes.saturating_sub(4) {
        let formula = match i % 5 {
            0 => format!("ebitda * {}", 0.01 + 0.001 * i as f64),
            1 => format!("revenue / {}", 1.0 + 0.001 * i as f64),
            2 => format!("rolling_mean(ebitda, 3) + {i}"),
            3 => format!("lag(ebitda, 1) * {}", 0.5 + 0.001 * i as f64),
            _ => format!("ebitda - cogs * {}", 0.001 * i as f64),
        };
        builder = builder.compute(format!("derived_{i}"), formula).unwrap();
    }

    builder.build().unwrap()
}

/// 24-month series plus the formula families missing from the original suite.
pub fn formula_family_model() -> FinancialModelSpec {
    ModelBuilder::new("formula-families")
        .periods("2024M01..2025M12", None)
        .unwrap()
        .value("revenue", &monthly_values(24, 100_000.0, 1_000.0))
        .compute("ewm", "ewm_mean(revenue, 0.3)")
        .unwrap()
        .compute("ewm_vol", "ewm_std(revenue, 0.3)")
        .unwrap()
        .compute("rev_rank", "rank(revenue)")
        .unwrap()
        .compute("rev_q50", "quantile(revenue, 0.5)")
        .unwrap()
        .compute("rev_ytd", "ytd(revenue)")
        .unwrap()
        .compute("rev_ttm", "ttm(revenue)")
        .unwrap()
        .compute("guarded", "if(revenue > 0, revenue, 0)")
        .unwrap()
        .compute("safe_div", "coalesce(revenue / 0, 0)")
        .unwrap()
        .compute("lagged_roll", "lag(rolling_mean(revenue, 12), 1)")
        .unwrap()
        .build()
        .unwrap()
}
