//! Contract tests for `CashflowProvider` implementations.
//!
//! These tests ensure all instruments correctly implement the trait contract.
//! Add new instruments here when they implement `CashflowProvider` to catch
//! drift and ensure consistent behavior across the codebase.
//!
//! # Contract Properties Verified
//!
//! 1. `cashflow_schedule` succeeds with minimal valid market context
//! 2. `dated_cashflows` is a pure flattening of `cashflow_schedule`
//! 3. Returned flows are sorted by date (non-decreasing)
//! 4. All flows have the same currency as the instrument's notional (if provided)
//! 5. All flows satisfy `date >= as_of` (future-only)
//! 6. No `CFKind::PIK` flows appear in the public schedule
//!
//! # Adding New Instruments
//!
//! To add contract tests for a new instrument:
//!
//! ```rust,ignore
//! #[test]
//! fn my_instrument_satisfies_contract() {
//!     let as_of = d(2025, 1, 1);
//!     let inst = MyInstrument::new(/* ... */);
//!     verify_provider_contract(&inst, &minimal_market(), as_of);
//! }
//! ```

use super::helpers::d;
use finstack_quant_cashflows::builder::schedule::CashflowRepresentation;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument as PublicInstrument;

// =============================================================================
// Contract Verification
// =============================================================================

/// Verifies `CashflowProvider` contract properties.
///
/// # Contract Properties
///
/// 1. `cashflow_schedule` returns `Ok` with valid market context
/// 2. `dated_cashflows` is a pure flattening of `cashflow_schedule`
/// 3. Flows are sorted by date (non-decreasing)
/// 4. All flows satisfy `date >= as_of` (future-only)
/// 5. No `CFKind::PIK` flows in the public schedule
/// 6. Currency consistency with notional (if provided)
fn verify_provider_contract<T: CashflowProvider>(
    provider: &T,
    market: &MarketContext,
    as_of: Date,
) {
    let type_name = std::any::type_name::<T>();

    let schedule = provider
        .cashflow_schedule(market, as_of)
        .unwrap_or_else(|e| {
            panic!(
                "[{}] cashflow_schedule failed with valid market context: {}",
                type_name, e
            )
        });
    let flows = provider.dated_cashflows(market, as_of).unwrap_or_else(|e| {
        panic!(
            "[{}] dated_cashflows failed with valid market context: {}",
            type_name, e
        )
    });
    let flattened_schedule_flows: Vec<_> = schedule
        .flows
        .iter()
        .map(|cf| (cf.date, cf.amount))
        .collect();
    assert_eq!(
        flows, flattened_schedule_flows,
        "[{}] dated_cashflows must be the flattened view of cashflow_schedule",
        type_name
    );

    // Contract: Flows must be sorted by date (non-decreasing)
    for window in flows.windows(2) {
        let (d1, _) = window[0];
        let (d2, _) = window[1];
        assert!(
            d1 <= d2,
            "[{}] Flows must be sorted by date: found {} after {}",
            type_name,
            d2,
            d1
        );
    }

    // Contract: All flows must be future-only (date >= as_of)
    for cf in &schedule.flows {
        assert!(
            cf.date >= as_of,
            "[{}] Flow on {} is before as_of {}; public schedule must be future-only",
            type_name,
            cf.date,
            as_of
        );
    }

    // Contract: No pure PIK flows in the public schedule
    use finstack_quant_core::cashflow::CFKind;
    for cf in &schedule.flows {
        assert!(
            cf.kind != CFKind::PIK,
            "[{}] PIK flow found on {}; pure PIK accretion must be omitted from public schedule",
            type_name,
            cf.date
        );
    }

    // Contract: Currency consistency (if notional provided)
    if let Some(notional) = provider.notional() {
        let expected_ccy = notional.currency();
        for (date, money) in &flows {
            assert_eq!(
                money.currency(),
                expected_ccy,
                "[{}] Flow on {} has currency {:?}, expected {:?} (from notional)",
                type_name,
                date,
                money.currency(),
                expected_ccy
            );
        }
    }
}

fn verify_public_instrument_cashflow_surface<T: PublicInstrument>(
    instrument: &T,
    market: &MarketContext,
    as_of: Date,
    expected_representation: CashflowRepresentation,
) {
    let schedule = instrument
        .cashflow_schedule(market, as_of)
        .expect("public instrument trait should expose cashflow_schedule");
    let flows = instrument
        .dated_cashflows(market, as_of)
        .expect("public instrument trait should expose dated_cashflows");
    assert_eq!(schedule.meta.representation, expected_representation);
    assert_eq!(flows.len(), schedule.flows.len());
}

fn verify_empty_schedule_surface<T: PublicInstrument>(
    instrument: &T,
    market: &MarketContext,
    as_of: Date,
    expected_representation: CashflowRepresentation,
) {
    let schedule = instrument
        .cashflow_schedule(market, as_of)
        .expect("public instrument trait should expose cashflow_schedule");
    let flows = instrument
        .dated_cashflows(market, as_of)
        .expect("public instrument trait should expose dated_cashflows");

    assert_eq!(schedule.meta.representation, expected_representation);
    assert!(
        schedule.flows.is_empty(),
        "schedule should be genuinely empty"
    );
    assert!(flows.is_empty(), "dated flow view should also be empty");
}

/// Creates a minimal market context for contract testing.
///
/// Contains flat discount and forward curves sufficient for most instruments.
fn minimal_market() -> MarketContext {
    let base = d(2025, 1, 1);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, 0.75)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid discount curve");

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([(0.0, 0.04), (10.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid forward curve");

    MarketContext::new().insert(disc).insert(fwd)
}

// =============================================================================
// Bond Contract Tests
// =============================================================================

mod bond_contract {
    use super::*;
    use finstack_quant_valuations::instruments::Bond;

    #[test]
    fn fixed_bond_satisfies_contract() {
        let as_of = d(2025, 1, 1);
        let issue = d(2025, 1, 15);
        let maturity = d(2030, 1, 15);

        let bond = Bond::fixed(
            "TEST-FIXED-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        verify_provider_contract(&bond, &minimal_market(), as_of);
        verify_public_instrument_cashflow_surface(
            &bond,
            &minimal_market(),
            as_of,
            CashflowRepresentation::Contractual,
        );
    }

    #[test]
    fn floating_bond_satisfies_contract() {
        use finstack_quant_core::dates::{DayCount, Tenor};

        let as_of = d(2025, 1, 1);
        let issue = d(2025, 1, 15);
        let maturity = d(2030, 1, 15);

        let bond = Bond::floating(
            "TEST-FLOAT-BOND",
            Money::new(1_000_000.0, Currency::USD),
            "USD-SOFR-3M",
            200, // 200 bps spread
            issue,
            maturity,
            Tenor::quarterly(),
            DayCount::Act360,
            "USD-OIS",
        )
        .unwrap();

        verify_provider_contract(&bond, &minimal_market(), as_of);
        verify_public_instrument_cashflow_surface(
            &bond,
            &minimal_market(),
            as_of,
            CashflowRepresentation::Projected,
        );
    }
}

// =============================================================================
// IRS Contract Tests
// =============================================================================

mod irs_contract {
    use super::*;
    use finstack_quant_valuations::instruments::rates::irs::PayReceive;

    #[test]
    fn usd_swap_pay_fixed_satisfies_contract() {
        let as_of = d(2025, 1, 1);
        let start = d(2025, 1, 15);
        let end = d(2030, 1, 15);

        let swap = crate::cashflows::finstack_quant_test_utils::usd_irs_swap(
            "TEST-IRS-PAY",
            Money::new(10_000_000.0, Currency::USD),
            0.04,
            start,
            end,
            PayReceive::Pay,
        )
        .expect("valid swap");

        verify_provider_contract(&swap, &minimal_market(), as_of);
    }

    #[test]
    fn usd_swap_receive_fixed_satisfies_contract() {
        let as_of = d(2025, 1, 1);
        let start = d(2025, 1, 15);
        let end = d(2030, 1, 15);

        let swap = crate::cashflows::finstack_quant_test_utils::usd_irs_swap(
            "TEST-IRS-REC",
            Money::new(10_000_000.0, Currency::USD),
            0.04,
            start,
            end,
            PayReceive::Receive,
        )
        .expect("valid swap");

        verify_provider_contract(&swap, &minimal_market(), as_of);
        verify_public_instrument_cashflow_surface(
            &swap,
            &minimal_market(),
            as_of,
            CashflowRepresentation::Projected,
        );
    }
}

// =============================================================================
// Repo Contract Tests
// =============================================================================

mod repo_contract {
    use super::*;
    use finstack_quant_valuations::instruments::rates::repo::Repo;

    #[test]
    fn repo_preserves_signed_flows_and_contractual_tag() {
        let as_of = d(2024, 1, 1);
        let repo = Repo::example();

        verify_provider_contract(&repo, &minimal_market(), as_of);

        let schedule =
            CashflowProvider::cashflow_schedule(&repo, &minimal_market(), as_of).expect("schedule");
        assert_eq!(
            schedule.meta.representation,
            CashflowRepresentation::Contractual
        );
        let has_negative = schedule.flows.iter().any(|cf| cf.amount.amount() < 0.0);
        assert!(
            has_negative,
            "Repo schedule should preserve the negative initial cash outflow"
        );
    }
}

mod empty_schedule_contract {
    use super::*;
    use finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture;
    use finstack_quant_valuations::instruments::equity::spot::Equity;
    use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;
    use finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture;
    use finstack_quant_valuations::instruments::equity::vol_index_option::VolatilityIndexOption;
    use finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap;
    use finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture;
    use finstack_quant_valuations::instruments::rates::ir_future_option::IrFutureOption;

    #[test]
    fn no_residual_products_emit_empty_no_residual_schedules() {
        let as_of = d(2025, 1, 1);
        let market = MarketContext::new();

        verify_empty_schedule_surface(
            &Equity::example(),
            &market,
            as_of,
            CashflowRepresentation::NoResidual,
        );
        verify_empty_schedule_surface(
            &InterestRateFuture::example().expect("ir future example"),
            &market,
            as_of,
            CashflowRepresentation::NoResidual,
        );
        verify_empty_schedule_surface(
            &EquityIndexFuture::example().expect("equity index future example"),
            &market,
            as_of,
            CashflowRepresentation::NoResidual,
        );
        verify_empty_schedule_surface(
            &VolatilityIndexFuture::example().expect("vol index future example"),
            &market,
            as_of,
            CashflowRepresentation::NoResidual,
        );
    }

    #[test]
    fn placeholder_products_emit_empty_placeholder_schedules() {
        let as_of = d(2025, 1, 1);
        let market = MarketContext::new();

        verify_empty_schedule_surface(
            &VarianceSwap::example().expect("variance swap example"),
            &market,
            as_of,
            CashflowRepresentation::Placeholder,
        );
        verify_empty_schedule_surface(
            &FxVarianceSwap::example(),
            &market,
            as_of,
            CashflowRepresentation::Placeholder,
        );
        verify_empty_schedule_surface(
            &IrFutureOption::example().expect("ir future option example"),
            &market,
            as_of,
            CashflowRepresentation::Placeholder,
        );
        verify_empty_schedule_surface(
            &VolatilityIndexOption::example().expect("vol index option example"),
            &market,
            as_of,
            CashflowRepresentation::Placeholder,
        );
    }
}

// =============================================================================
// Term Loan Contract Tests
// =============================================================================

mod term_loan_contract {
    use super::*;
    use finstack_quant_valuations::instruments::TermLoan;

    #[test]
    fn example_term_loan_satisfies_contract() {
        // TermLoan only exposes example() constructor; use builder for production
        let as_of = d(2025, 1, 1);
        let loan = TermLoan::example().unwrap();

        verify_provider_contract(&loan, &minimal_market(), as_of);
    }
}
