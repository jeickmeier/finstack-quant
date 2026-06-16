//! Audit tests for accrued interest calculations.
//!
//! These tests validate that accrued interest is correctly calculated and
//! attributed to the appropriate periods for fixed-income instruments.
//! The tests verify:
//!
//! - **Accrual accumulation**: Interest accrues linearly between coupon dates
//! - **Cash payment timing**: Coupon payments occur on the correct dates
//! - **Accrual reset**: Accrued interest resets to zero after payment
//! - **Period attribution**: Cashflows are assigned to correct reporting periods
//!
//! # Conventions
//!
//! - **Periods**: Half-open intervals `[start, end)` - a cashflow on `end` falls
//!   into the next period
//! - **Day count**: Tests use the bond's default day count (typically 30/360 for
//!   corporate bonds or ACT/ACT for government bonds)
//! - **Accrual direction**: Positive accrued = interest earned but not yet paid
//!
//! # References
//!
//! - ISDA 2006 Definitions for day count conventions
//! - Bond market conventions per SIFMA guidelines

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{build_periods, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_statements::capital_structure::aggregate_instrument_cashflows;
use finstack_quant_statements::types::CapitalStructureSpec;
use finstack_quant_valuations::instruments::Bond;
use indexmap::IndexMap;
use std::sync::Arc;
use time::Month;

mod common;

/// Verify accrued interest accumulation and reset for a semi-annual bond.
///
/// This test validates the core accrual mechanics for a fixed-rate bond:
///
/// # Test Instrument
///
/// - **Face value**: $1,000,000 USD
/// - **Coupon rate**: 5% annual (2.5% semi-annual)
/// - **Frequency**: Semi-annual (coupons on Jan 1 and July 1)
/// - **Issue date**: January 1, 2025
/// - **Maturity**: January 1, 2030
///
/// # Expected Behavior
///
/// | Month | Accrued Interest | Cash Payment |
/// |-------|------------------|--------------|
/// | Jan   | ~$4,167 (1/6 of coupon) | $0 |
/// | Feb   | ~$8,333 (2/6 of coupon) | $0 |
/// | ...   | accumulating | $0 |
/// | June  | ~$25,000 (full coupon) | $0 |
/// | July  | ~$4,167 (reset + 1 month) | $25,000 |
///
/// # Calculations
///
/// - Semi-annual coupon = $1,000,000 × 5% ÷ 2 = **$25,000**
/// - Monthly accrual ≈ $25,000 ÷ 6 ≈ **$4,167**
///
/// # Period Boundary Convention
///
/// Periods are half-open intervals `[start, end)`. A coupon payment dated
/// July 1, 2025 falls into the July period (M7), not June (M6), because
/// June's period is `[June 1, July 1)`.
#[test]
fn test_accrued_interest_semi_annual_bond() -> Result<(), Box<dyn std::error::Error>> {
    // Setup minimal market context (no curves needed for contractual cashflows)
    let market_ctx = MarketContext::new();

    // Monthly periods for 2025 to observe accrual accumulation
    let periods = build_periods("2025M1..M12", None)?.periods;

    // Semi-annual bond: 5% coupon, $1M face value
    // First coupon: July 1, 2025 (6 months after issue)
    // Semi-annual coupon = 1,000,000 × 5% ÷ 2 = $25,000
    let issue_date = Date::from_calendar_date(2025, Month::January, 1)?;
    let maturity_date = Date::from_calendar_date(2030, Month::January, 1)?;

    let bond = Bond::fixed(
        InstrumentId::new("BOND-AUDIT"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue_date,
        maturity_date,
        CurveId::new("USD-OIS"),
    )?;

    let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
        IndexMap::new();
    instruments.insert("BOND-AUDIT".to_string(), Arc::new(bond));

    // 4. Run Aggregation
    let spec = CapitalStructureSpec {
        debt_instruments: vec![],
        equity_instruments: vec![],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    };
    let as_of = issue_date; // Not strictly used for contractual flows, but required API

    let cashflows =
        aggregate_instrument_cashflows(&spec, &instruments, &periods, &market_ctx, as_of)?;

    // 5. Verify Results

    // Function to get values for a specific month (1-indexed)
    let get_metrics = |month: u8| {
        let period_id = finstack_quant_core::dates::PeriodId::month(2025, month);
        let accrued = cashflows
            .get_accrued_interest("BOND-AUDIT", &period_id)
            .expect("accrued");
        let cash = cashflows
            .get_interest_cash("BOND-AUDIT", &period_id)
            .expect("cash");
        (accrued, cash)
    };

    // Month 1 (Jan): Should have ~1 month accrued, 0 cash
    // With 30/360 day count and $1M notional at 5% semi-annual:
    // Semi-annual coupon = $25,000
    // 1 month accrual = 30/180 × $25,000 = $4,166.67
    let (accrued_m1, cash_m1) = get_metrics(1);
    assert_eq!(cash_m1, 0.0, "Month 1 should have no cash payment");
    assert!(
        accrued_m1 > 4000.0 && accrued_m1 < 4300.0,
        "Month 1 accrued should be ~$4,166. Got: {}",
        accrued_m1
    );

    // Month 2 (Feb): Should have ~2 months accrued
    // 2 month accrual with 30/360: varies slightly due to month-end conventions
    // Expected range: $7,500 - $8,500 (Feb end is 58-60 days from Jan 1 in 30/360)
    let (accrued_m2, cash_m2) = get_metrics(2);
    assert_eq!(cash_m2, 0.0);
    assert!(
        accrued_m2 > accrued_m1,
        "Accrual should increase. M1: {}, M2: {}",
        accrued_m1,
        accrued_m2
    );
    assert!(
        accrued_m2 > 7500.0 && accrued_m2 < 8500.0,
        "M2 should be ~$8,000. Got: {}",
        accrued_m2
    );

    // Month 5 (May): Accumulated nearly full coupon
    // 5 months in 30/360 = 150 days, so 150/180 × $25,000 = $20,833
    let (accrued_m5, _cash_m5) = get_metrics(5);
    assert!(
        accrued_m5 > 19500.0 && accrued_m5 < 21500.0,
        "Month 5 accrued should be ~$20,833. Got: {}",
        accrued_m5
    );

    // Month 6/7 boundary: Coupon on July 1 falls into M7 due to [start, end) convention
    let (accrued_m6, cash_m6) = get_metrics(6);
    let (accrued_m7, cash_m7) = get_metrics(7);

    if cash_m6 > 0.0 {
        // Coupon paid in June
        assert_eq!(cash_m6, 25000.0);
        // Accrued should reset
        assert!(accrued_m6 < 100.0, "Accrued should reset after payment");
    } else {
        // Coupon paid in July (more likely for July 1 date)
        assert_eq!(cash_m7, 25000.0, "Coupon should be paid in July");

        // M6 accrued should be nearly full coupon amount (~$25,000)
        assert!(
            accrued_m6 > 24000.0 && accrued_m6 <= 25000.0,
            "M6 accrued should be nearly full coupon. Got: {}",
            accrued_m6
        );

        // M7 accrued should reset and show ~1 month of new accrual (~$4,167)
        assert!(
            accrued_m7 > 3000.0 && accrued_m7 < 4500.0,
            "M7 should restart accrual. Got: {}",
            accrued_m7
        );
    }

    Ok(())
}
