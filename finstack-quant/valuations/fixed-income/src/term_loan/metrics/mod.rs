//! Metrics for term loan instruments.
//!
//! This module provides risk and return metrics for term loans including:
//! - **Yield metrics**: YTM, YTC, YTW, YT2Y/3Y/4Y
//! - **Spread metrics**: Discount Margin, All-In Rate
//! - **Risk metrics**: DV01, CS01, Theta (bucketed and parallel)
//!
//! # Available Metrics
//!
//! ## Yield and Return
//!
//! - **YTM** (Yield-to-Maturity): IRR to final maturity
//! - **YTC** (Yield-to-Call): IRR to first call date
//! - **YTW** (Yield-to-Worst): Minimum yield across all call dates and maturity
//! - **YT2Y/3Y/4Y**: IRR to fixed 2/3/4-year horizons
//! - **All-In Rate**: Effective borrower cost including fees
//! - **OID EIR Amortization**: Effective interest rate amortization schedule
//!
//! ## Spread Metrics
//!
//! - **Discount Margin**: Additive spread for floating-rate loans to match price
//!
//! ## Risk Metrics
//!
//! - **DV01**: Dollar value of 1bp parallel shift (combined parallel + key-rate)
//! - **BucketedDV01**: Key-rate risk by tenor bucket
//! - **CS01**: Credit spread sensitivity (parallel)
//! - **BucketedCS01**: Credit spread sensitivity by tenor
//! - **Theta**: Time decay (1-day price change)
//!
//! # Quick Example
//!
//! ```text
//! use finstack_quant_valuations_fixed_income::term_loan::TermLoan;
//! use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loan = TermLoan::example().unwrap();
//! let market = MarketContext::new();
//! let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
//!
//! // Compute base value
//! // let pv = loan.value(&market, as_of)?;
//!
//! // Request metrics
//! let metrics = vec![MetricId::Ytm, MetricId::Dv01, MetricId::custom("ytw")];
//! // let result = loan.price_with_metrics(&market, as_of, &metrics, finstack_quant_valuations_core::instruments::PricingOptions::default())?;
//! # Ok(())
//! # }
//! ```
//!
//! # Registration
//!
//! Metrics are registered via [`register_term_loan_metrics()`] in the global registry.
//!
//! # See Also
//!
//! - [`YtmCalculator`] for yield-to-maturity implementation
//! - [`DiscountMarginCalculator`] for floating-rate spread calculation
//! - [`AllInRateCalculator`] for borrower cost calculation

mod cs01;
mod discount_margin;
pub(crate) mod irr_helpers;
mod oas;
mod risk_view;
mod ytm;
mod ytn;
mod ytw;

pub(crate) use discount_margin::DiscountMarginCalculator;
pub(crate) use oas::OasCalculator;
pub(crate) use ytm::YtmCalculator;
pub(crate) use ytn::{Yt2yCalculator, Yt3yCalculator, Yt4yCalculator};
pub(crate) use ytw::YtwCalculator;

use self::irr_helpers::cached_full_schedule;
use self::irr_helpers::exercisable_call_candidates;
use self::irr_helpers::solve_irr_to_exercise;
use self::irr_helpers::target_price_from_quote_or_model;
use crate::term_loan::TermLoan;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all term loan metrics with the global registry.
///
/// Registers yield, spread, and risk metrics for term loan instruments.
/// Called automatically during registry initialization.
///
/// # Registered Metrics
///
/// - Standard: YTM, Discount Margin, DV01, BucketedDV01, CS01, BucketedCS01, Theta
/// - Custom: YTW (`"ytw"`), YTC (`"ytc"`), YT2Y/3Y/4Y, All-In Rate (`"all_in_rate"`),
///   OID EIR amortization (`"oid_eir_amortization"`)
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations_core::metrics::MetricRegistry;
/// use finstack_quant_valuations_fixed_income::term_loan::metrics::register_term_loan_metrics;
///
/// let mut registry = MetricRegistry::new();
/// register_term_loan_metrics(&mut registry);
/// ```
pub(crate) fn register_term_loan_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::TermLoan,
        metrics: [
            (Ytw, YtwCalculator),

            (Dv01, risk_view::TermLoanParallelDv01Calculator),
            (BucketedDv01, risk_view::TermLoanBucketedDv01Calculator),
            (Cs01, risk_view::TermLoanCs01Calculator),
            (BucketedCs01, risk_view::TermLoanBucketedCs01Calculator),
            (Ytm, YtmCalculator),
            (DiscountMargin, DiscountMarginCalculator),
            // Callable-tree metrics
            (Oas, OasCalculator),
            (EmbeddedOptionValue, EmbeddedOptionValueCalculator),
        ],
        custom: [
            ("all_in_rate", AllInRateCalculator),
            ("oid_eir_amortization", OidEirAmortizationCalculator),
            // Yield to first call (custom id: ytc)
            ("ytc", YtcCalculator),
            // Yields to fixed horizons
            ("yt2y", Yt2yCalculator),
            ("yt3y", Yt3yCalculator),
            ("yt4y", Yt4yCalculator),
        ]
    }

    // Loan-specific metrics

    Ok(())
}

/// All-in rate calculator for term loans.
///
/// Returns the cash-cost all-in rate: (cash interest + fees) / time-weighted outstanding.
/// PIK interest is excluded from the numerator (cash cost only) but affects outstanding
/// (through the outstanding path derived from the full schedule).
///
/// Cash flows are read directly from the generated `CashFlowSchedule` to ensure
/// perfect consistency with the cashflow generator. No fee or rate logic is
/// duplicated.
///
/// Computes the effective annualized borrower **cash cost** including cash interest
/// and periodic fees, divided by time-weighted outstanding principal.
///
/// This metric reads cash flows directly from the full cashflow schedule,
/// ensuring perfect consistency with the cashflow generator. PIK interest is
/// excluded from the numerator (cash cost only) but affects outstanding.
///
/// # Scope: running cash cost, not the fee-inclusive effective rate
///
/// Only flows strictly **after** the valuation date enter the numerator (they
/// must align with the post-`as_of` time-weighted outstanding denominator). A
/// one-time origination / upfront fee is dated at the issue date, so for a
/// freshly issued loan (`issue_date == as_of`) it is **excluded** here — this
/// metric reports the ongoing cash running cost, which for a plain bullet loan
/// equals the coupon.
///
/// The fee-inclusive "all-in" borrower cost — the IFRS-9 effective interest
/// rate that amortizes upfront fees and OID over the loan's life — is a
/// separate metric, [`OidEirAmortizationCalculator`]
/// (`"oid_eir_rate"`), which solves an XIRR over the full flow set (including
/// the upfront fee when `OidEirSpec::include_fees`). Use that metric when the
/// borrower's effective rate inclusive of origination economics is required.
pub(crate) struct AllInRateCalculator;

impl MetricCalculator for AllInRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Snapshot scalar fields off the loan before borrowing the cached schedule.
        let (day_count, maturity) = {
            let loan: &TermLoan = context.instrument_as()?;
            (loan.day_count, loan.maturity)
        };
        let as_of = context.as_of;

        // Use the cached full cashflow schedule — the single source of truth for
        // all flows. Cached across other yield/spread calculators on the same
        // loan to avoid repeated rebuilds in multi-metric requests.
        let schedule = cached_full_schedule(context)?;

        crate::loan_quotes::all_in_rate_from_schedule(&schedule, as_of, day_count, maturity)
    }
}

/// Embedded option value calculator for term loans (callability only).
///
/// Prices the callable and straight loans through the same selected model at
/// one OAS:
/// 1) With call schedule -> P_callable
/// 2) Without call schedule -> P_straight
///
/// The metric is the holder/lender value:
/// V_embedded = P_callable - P_straight
///
/// This is negative when callability reduces lender value (borrower owns the call),
/// matching the bond `EmbeddedOptionValue` convention.
pub(crate) struct EmbeddedOptionValueCalculator;

impl MetricCalculator for EmbeddedOptionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;

        let has_calls = loan
            .call_schedule
            .as_ref()
            .is_some_and(|cs| !cs.calls.is_empty());
        if !has_calls {
            return Ok(0.0);
        }
        oas::require_tree_model(loan, context)?;

        risk_view::with_term_loan_risk_view(context, |ctx| {
            let priced: TermLoan = ctx.instrument_as::<TermLoan>()?.clone();
            let price_callable =
                ctx.reprice_instrument_raw(&priced, ctx.curves.as_ref(), ctx.as_of)?;
            let mut straight = priced;
            straight.call_schedule = None;
            let price_straight =
                ctx.reprice_instrument_raw(&straight, ctx.curves.as_ref(), ctx.as_of)?;
            Ok(price_callable - price_straight)
        })
    }
}

/// Computes EIR amortization series and returns total amortization.
pub(crate) struct OidEirAmortizationCalculator;

impl MetricCalculator for OidEirAmortizationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;
        let market = &context.curves;

        let schedule = crate::term_loan::cashflows::build_oid_eir_schedule(loan, market)?;

        context
            .computed
            .insert(MetricId::custom("oid_eir_rate"), schedule.effective_rate);

        if !schedule.periods.is_empty() {
            let amort_series = schedule
                .periods
                .iter()
                .map(|p| (p.date.to_string(), p.oid_amortization.amount()));
            context.store_bucketed_series(MetricId::custom("oid_eir_amortization"), amort_series);

            let carrying_series = schedule
                .periods
                .iter()
                .map(|p| (p.date.to_string(), p.closing_balance.amount()));
            context
                .store_bucketed_series(MetricId::custom("oid_eir_carrying_value"), carrying_series);
        }

        let total = schedule
            .periods
            .iter()
            .map(|p| p.oid_amortization.amount())
            .sum();
        Ok(total)
    }
}

/// Yield-to-call calculator for callable term loans.
///
/// Solves for IRR to the earliest exercisable call candidate — the first
/// coupon date after settlement when a standing provision is already
/// effective, or the first future-dated provision otherwise. Redemption
/// uses the contractual clean call price or make-whole value plus accrued cash.
///
/// Computes the IRR to the earliest valid call date, using the full cashflow
/// schedule with kind-aware filtering plus the call redemption based on
/// pre-exercise outstanding principal.
pub(crate) struct YtcCalculator;

impl MetricCalculator for YtcCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Use the cached internal schedule (rebuilt only if absent).
        let schedule = cached_full_schedule(context)?;

        let loan: &TermLoan = context.instrument_as()?;

        // Earliest exercisable candidate (standing or future-dated provision).
        let candidates = exercisable_call_candidates(loan, &schedule, as_of, &context.curves)?;
        let Some((call_date, redemption)) = candidates.into_iter().next() else {
            // No exercisable calls → fallback to YTM.
            return crate::term_loan::metrics::ytm::YtmCalculator.calculate(context);
        };

        let target_price = {
            let loan: &TermLoan = context.instrument_as()?;
            target_price_from_quote_or_model(
                loan,
                &schedule,
                &context.curves,
                as_of,
                context.base_value,
            )?
        };

        // Re-fetch the loan reference (cache write path released the borrow).
        let loan: &TermLoan = context.instrument_as()?;
        solve_irr_to_exercise(loan, &schedule, as_of, target_price, call_date, redemption)
    }
}
