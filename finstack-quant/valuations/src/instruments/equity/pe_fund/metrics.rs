//! Private markets fund metrics: IRR, MOIC, DPI, TVPI, carry calculations, and theta.

use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;

/// Total LP contributions (paid-in capital) on or before `as_of`, in
/// currency units.
fn paid_in_to_date(pe: &PrivateMarketsFund, as_of: Date) -> f64 {
    pe.events
        .iter()
        .filter(|e| {
            e.kind == crate::instruments::equity::pe_fund::FundEventKind::Contribution
                && e.date <= as_of
        })
        .map(|e| e.amount.amount())
        .sum()
}

/// Total realized LP distributions from the allocation ledger on or before
/// `as_of`, in currency units.
fn realized_lp_to_date(
    ledger: &crate::instruments::equity::pe_fund::AllocationLedger,
    as_of: Date,
) -> f64 {
    ledger
        .rows
        .iter()
        .filter(|r| r.date <= as_of)
        .map(|r| r.to_lp.amount())
        .sum()
}

/// LP Internal Rate of Return calculator.
///
/// Mark-to-market IRR over realized LP cashflows up to the valuation date
/// plus the residual position value (`base_value`) as a terminal inflow,
/// aligning the IRR with TVPI.
///
/// # Errors
///
/// Propagates the XIRR solver's errors: fewer than two dated flows (an IRR
/// is undefined for a fund with no cashflow history), no sign change in the
/// flow stream, day-count failures, or no valid root.
struct LpIrrCalculator;

impl MetricCalculator for LpIrrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let mut flows: Vec<(Date, Money)> = ledger
            .lp_cashflows()?
            .into_iter()
            .filter(|(d, _)| *d <= context.as_of)
            .collect();

        // Add Residual NAV as a terminal positive flow at as_of date
        // This aligns IRR with TVPI (mark-to-market performance)
        let nav = context.base_value;
        if nav.amount().abs() > 1e-6 {
            flows.push((context.as_of, nav));
        }

        calculate_irr(&flows, pe.waterfall_spec.irr_basis)
    }
}

/// Total GP carry paid through the waterfall, in currency units.
///
/// A GP IRR is not well-defined here (the GP has no initial investment in
/// the carry stream), so this metric reports the total carry dollars instead
/// and is registered as `gp_carry_total` .
struct GpCarryTotalCalculator;

impl MetricCalculator for GpCarryTotalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let total_gp_carry: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.date <= context.as_of)
            .map(|r| r.to_gp.amount())
            .sum();
        Ok(total_gp_carry)
    }
}

/// Total value multiple on paid-in capital:
/// `(realized LP distributions to date + residual value) / paid-in capital`.
///
/// Shared implementation for the LP MOIC and TVPI metrics, which are the
/// same quantity by definition on a net LP basis.
fn total_value_to_paid_in(context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
    let pe: &PrivateMarketsFund = context.instrument_as()?;
    let ledger = pe.run_waterfall()?;

    let total_contributions = paid_in_to_date(pe, context.as_of);
    let realized_lp_distributions = realized_lp_to_date(&ledger, context.as_of);

    // Use the base_value (pricing result) as the NAV / Residual Value.
    // This correctly captures the NPV of future flows or the explicit valuation.
    let residual_nav = context.base_value.amount();

    if total_contributions <= 1e-6 {
        return Ok(0.0);
    }

    Ok((realized_lp_distributions + residual_nav) / total_contributions)
}

/// Multiple on Invested Capital (MOIC) for LP calculator.
///
/// Net LP MOIC: `(realized LP distributions + residual value) / paid-in
/// capital`, where realized distributions are the ledger's `to_lp` amounts
/// (net of GP carry) and the residual value is the pricing `base_value`. On
/// this net LP basis MOIC equals TVPI by definition; the metric is kept as a
/// separate id because desks quote both names.
struct MoicLpCalculator;

impl MetricCalculator for MoicLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        total_value_to_paid_in(context)
    }
}

/// Distributions to Paid-In Capital (DPI) calculator.
///
/// Realized-only multiple: ledger `to_lp` distributions over contributions,
/// both up to the valuation date. Gross fund events would overstate the LP
/// multiple by the GP carry.
struct DpiLpCalculator;

impl MetricCalculator for DpiLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let total_contributions = paid_in_to_date(pe, context.as_of);
        let total_lp_distributions = realized_lp_to_date(&ledger, context.as_of);

        if total_contributions <= 1e-6 {
            return Ok(0.0);
        }

        Ok(total_lp_distributions / total_contributions)
    }
}

/// Total Value to Paid-In Capital (TVPI) calculator.
///
/// `TVPI = (realized LP distributions + residual value) / paid-in capital`,
/// i.e. `DPI + RVPI`. The residual value is the pricing `base_value`.
struct TvpiLpCalculator;

impl MetricCalculator for TvpiLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        total_value_to_paid_in(context)
    }
}

/// GP carry accrued calculator.
///
/// Cumulative gross GP carry (`gp_carry_cum`, pre-holdback) as of the
/// valuation date: the last ledger row dated on or before `as_of`. Events
/// after the valuation date do not contribute.
struct CarryAccruedCalculator;

impl MetricCalculator for CarryAccruedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        Ok(ledger
            .rows
            .iter()
            .rfind(|r| r.date <= context.as_of)
            .map(|r| r.gp_carry_cum.amount())
            .unwrap_or(0.0))
    }
}

/// Calculate the LP/fund IRR for dated currency cashflows.
///
/// Thin adapter over the canonical
/// [`xirr_with_daycount`](finstack_quant_core::cashflow::xirr_with_daycount)
/// solver. It drops currency from the `Money` amounts — the IRR is a pure scalar
/// root of NPV — and measures cashflow times with `day_count` from the earliest
/// flow. Both the discount form `(1 + r)^{-t}` and multi-root handling
/// (deterministic selection of the root closest to `r = 0`) come from the core
/// solver, so PE-fund metrics inherit its compensated summation, scale
/// invariance, and unbounded high-rate handling. The waterfall IRR routine
/// (`WaterfallSpec::calculate_irr`) delegates here so both stay consistent.
///
/// # Arguments
///
/// * `flows` - Dated LP or fund cashflows. Negative amounts represent capital
///   contributions and positive amounts distributions; all amounts must share
///   one currency because currency is deliberately removed before solving.
/// * `day_count` - Convention used to convert each date from the earliest
///   cashflow into a fractional year for the XIRR discount equation.
///
/// # Errors
///
/// Propagates errors from
/// [`xirr_with_daycount`](finstack_quant_core::cashflow::xirr_with_daycount):
/// fewer than two cashflows, no sign change in the cashflow stream, a day-count
/// failure on any flow date, or no valid root.
pub(crate) fn calculate_irr(
    flows: &[(Date, Money)],
    day_count: DayCount,
) -> finstack_quant_core::Result<f64> {
    let dated: Vec<(Date, f64)> = flows.iter().map(|(d, m)| (*d, m.amount())).collect();
    finstack_quant_core::cashflow::xirr_with_daycount(&dated, day_count, None)
}

mod carry01;
mod hurdle01;
mod nav01;

/// Register all private markets fund metrics.
pub(crate) fn register_private_markets_fund_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Private markets fund-specific risk metrics (custom metrics)
    registry.replace_metric(
        MetricId::Nav01,
        Arc::new(nav01::Nav01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    )?;
    registry.replace_metric(
        MetricId::Carry01,
        Arc::new(carry01::Carry01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    )?;
    registry.replace_metric(
        MetricId::Hurdle01,
        Arc::new(hurdle01::Hurdle01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    )?;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::PrivateMarketsFund,
        metrics: [
            (LpIrr, LpIrrCalculator),
            (GpCarryTotal, GpCarryTotalCalculator),
            (MoicLp, MoicLpCalculator),
            (DpiLp, DpiLpCalculator),
            (TvpiLp, TvpiLpCalculator),
            (CarryAccrued, CarryAccruedCalculator),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::equity::pe_fund::{FundEvent, WaterfallSpec};
    use time::Month;

    fn test_currency() -> finstack_quant_core::currency::Currency {
        finstack_quant_core::currency::Currency::USD
    }

    fn test_date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid date"), day)
            .expect("should succeed")
    }

    #[test]
    fn test_irr_calculation() {
        // Simple 2x return over 5 years should be ~15% IRR
        let flows = vec![
            (
                test_date(2020, 1, 1),
                Money::from((-1000000_i64, test_currency())),
            ), // Contribution
            (
                test_date(2025, 1, 1),
                Money::from((2000000_i64, test_currency())),
            ), // Distribution
        ];

        let irr = calculate_irr(&flows, DayCount::Act365F).expect("should succeed");

        // 2x over 5 years = (2.0)^(1/5) - 1 ≈ 0.1487 or ~14.87%
        assert!(
            (irr - 0.1487).abs() < 0.01,
            "Expected ~14.87% IRR, got {:.4}%",
            irr * 100.0
        );
    }

    /// `calculate_irr` must discount cashflows with the exact closed form
    /// `(1 + r)^{-t}` — no linearized `1 − r·t` near zero. This reconciles the
    /// standalone routine with the waterfall's IRR routine
    /// (`WaterfallSpec::calculate_irr`), which also uses `(1 + r)^{-t}`.
    ///
    /// With a near-zero IRR (a marginal gain over a multi-year horizon) the old
    /// `1 − r·t` linearization is a different discount function from
    /// `(1 + r)^{-t}`, so the recovered IRR drifts off the analytically exact
    /// `(D/C)^{1/t} − 1`. The exact closed form recovers it tightly.
    #[test]
    fn irr_uses_exact_closed_form_discounting_near_zero_rate() {
        // 0.05% total gain over (almost) 6 years => IRR ≈ 0.0083%/yr, deep in
        // the near-zero region where the linearization and the closed form
        // diverge.
        let contribution = 1_000_000.0;
        let distribution = 1_000_500.0;
        let start = test_date(2020, 1, 1);
        let end = test_date(2026, 1, 1);
        let flows = vec![
            (
                start,
                Money::new(-contribution, test_currency()).expect("valid money fixture"),
            ),
            (
                end,
                Money::new(distribution, test_currency()).expect("valid money fixture"),
            ),
        ];

        let irr = calculate_irr(&flows, DayCount::Act365F).expect("IRR should solve");

        // Analytically exact IRR for a single contribution/distribution pair:
        //   C = D · (1 + r)^{-t}   =>   r = (D / C)^{1/t} − 1.
        let t = DayCount::Act365F
            .year_fraction(
                start,
                end,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        let exact_irr = (distribution / contribution).powf(1.0 / t) - 1.0;

        // Bound is core::xirr's documented solver tolerance (1e-8), not the
        // 1e-12 of the former bespoke pe_fund solver. At 1e-7 this still
        // distinguishes the exact `(1 + r)^{-t}` discount form from a
        // linearized `1 − r·t` (which would diverge by ~r·t² ≫ 1e-7 here),
        // so the test's intent — exact closed-form discounting — is preserved.
        assert!(
            (irr - exact_irr).abs() < 1e-7,
            "near-zero IRR must match the exact (D/C)^(1/t) − 1 closed form: \
             got {irr}, expected {exact_irr}"
        );
        // Confirm we are genuinely in the near-zero regime the fix targets.
        assert!(
            irr.abs() < 1e-3,
            "test flows must produce a near-zero IRR; got {irr}"
        );
    }

    #[test]
    fn test_moic_calculation() {
        // 100% LP promote tier so the full distribution reaches the LP and
        // the ledger-basis MOIC  equals the naive 2x.
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 1.0, 0.0)
            .build()
            .expect("should succeed");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::from((1000000_i64, test_currency())),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::from((2000000_i64, test_currency())),
            ),
        ];

        let pe = PrivateMarketsFund::new("TEST", test_currency(), spec, events);

        let curves = finstack_quant_core::market_data::context::MarketContext::new();
        // Fully realized fund: holder-view residual value is ~0, so the net
        // LP MOIC is realized distributions / paid-in = 2x.
        let base_value = Money::from((0_i64, test_currency()));
        let mut context = MetricContext::new(
            std::sync::Arc::new(pe),
            std::sync::Arc::new(curves),
            test_date(2025, 1, 1),
            base_value,
            MetricContext::default_config(),
        );

        let moic = MoicLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        assert!((moic - 2.0).abs() < 1e-6); // 2x multiple

        // With residual value, MOIC includes it (net LP MOIC == TVPI):
        // (2.0M realized + 0.5M residual) / 1.0M paid-in = 2.5x.
        context.set_base_value(Money::from((500_000_i64, test_currency())));
        let moic_with_residual = MoicLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        assert!((moic_with_residual - 2.5).abs() < 1e-6);
        let tvpi = TvpiLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        assert!((moic_with_residual - tvpi).abs() < 1e-12);
    }

    /// Holder-view PV : a fully realized fund has
    /// base_value ≈ 0, so TVPI collapses to DPI and LpIrr equals the
    /// realized IRR.
    #[test]
    fn fully_realized_fund_tvpi_equals_dpi_and_lp_irr_is_realized() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 1.0, 0.0)
            .build()
            .expect("should succeed");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::from((1000000_i64, test_currency())),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::from((2000000_i64, test_currency())),
            ),
        ];

        let pe = PrivateMarketsFund::new("TEST", test_currency(), spec, events);

        let curves = finstack_quant_core::market_data::context::MarketContext::new();
        // base_value ≈ 0 for a fully realized fund under holder-view PV.
        let base_value = Money::from((0_i64, test_currency()));
        let mut context = MetricContext::new(
            std::sync::Arc::new(pe),
            std::sync::Arc::new(curves),
            test_date(2025, 1, 1),
            base_value,
            MetricContext::default_config(),
        );

        let tvpi = TvpiLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        let dpi = DpiLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        assert!(
            (tvpi - dpi).abs() < 1e-9,
            "TVPI ({tvpi}) should equal DPI ({dpi}) when residual NAV is 0"
        );

        let lp_irr = LpIrrCalculator
            .calculate(&mut context)
            .expect("should succeed");
        // Realized IRR: 2x over 5 years ≈ 14.87%.
        assert!(
            (lp_irr - 0.1487).abs() < 0.01,
            "LpIrr should equal the realized IRR, got {lp_irr}"
        );
    }
}
