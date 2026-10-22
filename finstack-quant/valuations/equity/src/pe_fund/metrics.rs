//! Private markets fund metrics: IRR, DPI, TVPI, carry calculations, and theta.

use crate::pe_fund::waterfall::{FundEventKind, PeFundWaterfallSpec, PeFundWaterfallTranche};
use crate::pe_fund::PrivateMarketsFund;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Total LP contributions (paid-in capital) on or before `as_of`, in
/// currency units.
fn paid_in_to_date(pe: &PrivateMarketsFund, as_of: Date) -> f64 {
    pe.events
        .iter()
        .filter(|e| e.kind == crate::pe_fund::FundEventKind::Contribution && e.date <= as_of)
        .map(|e| e.amount.amount())
        .sum()
}

/// Total realized LP distributions from the allocation ledger on or before
/// `as_of`, in currency units.
fn realized_lp_to_date(ledger: &crate::pe_fund::AllocationLedger, as_of: Date) -> f64 {
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

        calculate_irr(&flows, pe.waterfall_spec.day_count)
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
/// [`xirr`](finstack_quant_core::cashflow::xirr)
/// solver. It drops currency from the `Money` amounts — the IRR is a pure scalar
/// root of NPV — and measures cashflow times with `day_count` from the earliest
/// flow. Both the discount form `(1 + r)^{-t}` and multi-root handling
/// (deterministic selection of the root closest to `r = 0`) come from the core
/// solver, so PE-fund metrics inherit its compensated summation, scale
/// invariance, and unbounded high-rate handling. The waterfall's LP
/// IRR-to-date uses this routine too, so both stay consistent.
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
/// [`xirr`](finstack_quant_core::cashflow::xirr):
/// fewer than two cashflows, no sign change in the cashflow stream, a day-count
/// failure on any flow date, or no valid root.
pub(crate) fn calculate_irr(
    flows: &[(Date, Money)],
    day_count: DayCount,
) -> finstack_quant_core::Result<f64> {
    let dated: Vec<(Date, f64)> = flows.iter().map(|(d, m)| (*d, m.amount())).collect();
    finstack_quant_core::cashflow::xirr(&dated, day_count, None)
}

/// Register all private markets fund metrics.
pub(crate) fn register_private_markets_fund_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::PrivateMarketsFund,
        metrics: [
            (LpIrr, LpIrrCalculator),
            (GpCarryTotal, GpCarryTotalCalculator),
            (DpiLp, DpiLpCalculator),
            (TvpiLp, TvpiLpCalculator),
            (CarryAccrued, CarryAccruedCalculator),
            // Private markets fund-specific risk metrics (custom metrics)
            (Nav01, Nav01Calculator),
            (Carry01, Carry01Calculator),
            (Hurdle01, Hurdle01Calculator),
        ]
    }
    Ok(())
}

// Carry01 calculator for PrivateMarketsFund.
//
// Computes Carry01 (GP carry sensitivity) using finite differences:
// the LP position's PV change for a 1bp move in the GP share.
//
// # Formula
// ```text
// Carry01 = (PV(GP_share + h) - PV(GP_share - h)) / 2
// ```
// Where the FD bump h is 1bp (0.0001). The central difference is divided
// by 2 (not 2h), so the result is the PV change *per 1bp move*, in
// currency units, consistent with the workspace `Dv01` convention. When a
// GP share sits at a boundary (0 or 1) so one direction cannot be bumped,
// a one-sided difference against the unbumped base value is used instead
// of silently halving the sensitivity with a clamped no-op bump.
//
// # Note
// GP carry is determined by the waterfall allocation (promote tiers, catch-up).
// This metric bumps the GP share in all promote tiers and catch-up tranches
// and measures the impact on LP valuation (lower carry = higher LP value).

/// Standard carry bump: 1bp (0.0001 = 0.01%)
const CARRY_BUMP: f64 = 0.0001;

/// Return a copy of the spec with GP shares in catch-up and promote tranches
/// shifted by `delta` (promote LP shares renormalized to sum to 1.0).
fn carry_bumped_spec(spec: &PeFundWaterfallSpec, delta: f64) -> PeFundWaterfallSpec {
    let mut bumped = spec.clone();
    for tranche in &mut bumped.tranches {
        match tranche {
            PeFundWaterfallTranche::CatchUp { gp_share } => {
                *gp_share += delta;
            }
            PeFundWaterfallTranche::PromoteTier {
                lp_share, gp_share, ..
            } => {
                *gp_share += delta;
                *lp_share = 1.0 - *gp_share;
            }
            _ => {
                // ReturnOfCapital, PreferredIrr don't affect carry directly
            }
        }
    }
    bumped
}

/// Whether every carry-bearing GP share in the spec can absorb `delta`
/// without leaving `[0, 1]`.
fn can_bump(spec: &PeFundWaterfallSpec, delta: f64) -> bool {
    spec.tranches.iter().all(|t| match t {
        PeFundWaterfallTranche::CatchUp { gp_share }
        | PeFundWaterfallTranche::PromoteTier { gp_share, .. } => {
            (0.0..=1.0).contains(&(gp_share + delta))
        }
        _ => true,
    })
}

/// Carry01 calculator for PrivateMarketsFund.
pub(crate) struct Carry01Calculator;

impl MetricCalculator for Carry01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let pv_for = |spec: PeFundWaterfallSpec| -> Result<f64> {
            let mut bumped_fund = fund.clone();
            bumped_fund.waterfall_spec = spec;
            Ok(bumped_fund.value(context.curves.as_ref(), as_of)?.amount())
        };

        let can_up = can_bump(&fund.waterfall_spec, CARRY_BUMP);
        let can_down = can_bump(&fund.waterfall_spec, -CARRY_BUMP);

        // Higher GP carry means less for LPs, so PV_up < PV_down typically.
        // Return the PV change for a +1bp GP share move.
        let carry01 = match (can_up, can_down) {
            (true, true) => {
                let pv_up = pv_for(carry_bumped_spec(&fund.waterfall_spec, CARRY_BUMP))?;
                let pv_down = pv_for(carry_bumped_spec(&fund.waterfall_spec, -CARRY_BUMP))?;
                (pv_up - pv_down) / 2.0
            }
            (true, false) => {
                let pv_up = pv_for(carry_bumped_spec(&fund.waterfall_spec, CARRY_BUMP))?;
                pv_up - context.base_value.amount()
            }
            (false, true) => {
                let pv_down = pv_for(carry_bumped_spec(&fund.waterfall_spec, -CARRY_BUMP))?;
                context.base_value.amount() - pv_down
            }
            // Shares pinned at opposite boundaries across tranches: no
            // consistent bump direction exists.
            (false, false) => 0.0,
        };

        Ok(carry01)
    }
}

// Hurdle01 calculator for PrivateMarketsFund.
//
// Computes Hurdle01 (hurdle rate sensitivity) using finite differences:
// the LP position's PV change for a 1bp move in the hurdle IRR rates.
//
// # Formula
// ```text
// Hurdle01 = (PV(hurdle_rate + h) - PV(hurdle_rate - h)) / 2
// ```
// Where the FD bump h is 1bp (0.0001). The central difference is divided
// by 2 (not 2h), so the result is the PV change *per 1bp move*, in
// currency units, consistent with the workspace `Dv01` convention. Bumps
// are not clamped at zero: the IRR solve accepts slightly negative hurdle
// targets, so a 0% hurdle still gets a genuine central difference.
//
// # Note
//
// Hurdle rates appear in:
// - `PeFundWaterfallTranche::PreferredIrr { hurdle_irr }` - preferred return hurdle
// - `PeFundWaterfallTranche::PromoteTier { hurdle_irr, ... }` - promote tier hurdle
//
// Higher hurdle rates increase the LP preferred return, potentially reducing GP carry
// and affecting LP valuation.

/// Standard hurdle bump: 1bp (0.0001)
const HURDLE_BUMP: f64 = 0.0001;

/// Return a copy of the spec with all hurdle IRR rates shifted by `delta`.
fn hurdle_bumped_spec(spec: &PeFundWaterfallSpec, delta: f64) -> PeFundWaterfallSpec {
    let mut bumped = spec.clone();
    for tranche in &mut bumped.tranches {
        match tranche {
            PeFundWaterfallTranche::PreferredIrr { hurdle_irr }
            | PeFundWaterfallTranche::PromoteTier { hurdle_irr, .. } => *hurdle_irr += delta,
            _ => {}
        }
    }
    bumped
}

/// Hurdle01 calculator for PrivateMarketsFund.
pub(crate) struct Hurdle01Calculator;

impl MetricCalculator for Hurdle01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let mut fund_up = fund.clone();
        fund_up.waterfall_spec = hurdle_bumped_spec(&fund.waterfall_spec, HURDLE_BUMP);
        let pv_up = fund_up.value(context.curves.as_ref(), as_of)?.amount();

        let mut fund_down = fund.clone();
        fund_down.waterfall_spec = hurdle_bumped_spec(&fund.waterfall_spec, -HURDLE_BUMP);
        let pv_down = fund_down.value(context.curves.as_ref(), as_of)?.amount();

        // Scenarios are ±1bp; return the PV change for one basis point.
        // Higher hurdle typically benefits LPs (more preferred return before GP carry)
        Ok((pv_up - pv_down) / 2.0)
    }
}

// NAV01 calculator for PrivateMarketsFund.
//
// Computes NAV01 (residual NAV sensitivity) using finite differences:
// the LP position's PV change for a 1% proportional move in fund value.
//
// # Formula
// ```text
// NAV01 = (PV(value scaled * (1 + h)) - PV(value scaled * (1 - h))) / 2
// ```
// Where the FD bump h is 1% (0.01). The central difference is divided by 2
// (not 2h), so the result is the PV change *per 1% move*, in currency
// units, consistent with the per-1bp `Dv01` convention.
//
// # Note
// Both value inputs are scaled by ±1%: every distribution/proceeds event
// (which drives the waterfall and future LP cashflows) and the fund's
// stated `unrealized_nav` (the residual mark that dominates holder-view PV
// for an unrealized fund). Contributions are unchanged.

/// Standard NAV bump: 1% (0.01)
const NAV_BUMP_PCT: f64 = 0.01;

/// Return a copy of the fund with all distribution/proceeds events and the
/// stated `unrealized_nav` scaled by `1 + bump`.
fn scaled_fund(
    fund: &PrivateMarketsFund,
    bump: f64,
) -> finstack_quant_core::Result<PrivateMarketsFund> {
    let mut scaled = fund.clone();
    for event in &mut scaled.events {
        if matches!(
            event.kind,
            FundEventKind::Distribution | FundEventKind::Proceeds
        ) {
            event.amount = Money::new(
                event.amount.amount() * (1.0 + bump),
                event.amount.currency(),
            )?;
        }
    }
    if let Some(nav) = scaled.unrealized_nav {
        scaled.unrealized_nav = Some(Money::new(nav.amount() * (1.0 + bump), nav.currency())?);
    }
    Ok(scaled)
}

/// NAV01 calculator for PrivateMarketsFund.
pub(crate) struct Nav01Calculator;

impl MetricCalculator for Nav01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let pv_up = scaled_fund(fund, NAV_BUMP_PCT)?
            .value(context.curves.as_ref(), as_of)?
            .amount();
        let pv_down = scaled_fund(fund, -NAV_BUMP_PCT)?
            .value(context.curves.as_ref(), as_of)?
            .amount();

        // Scenarios are ±1%; return the PV change for a 1% NAV move.
        Ok((pv_up - pv_down) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe_fund::{FundEvent, PeFundWaterfallSpec};
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
    /// standalone routine with the waterfall's LP IRR-to-date, which calls it.
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
    fn test_tvpi_calculation() {
        // 100% LP promote tier so the full distribution reaches the LP and
        // the ledger-basis TVPI  equals the naive 2x.
        let spec = PeFundWaterfallSpec::builder()
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

        let pe =
            PrivateMarketsFund::new("TEST", test_currency(), spec, events).expect("valid fund");

        let curves = finstack_quant_core::market_data::context::MarketContext::new();
        // Fully realized fund: holder-view residual value is ~0, so the net
        // LP TVPI is realized distributions / paid-in = 2x.
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
        assert!((tvpi - 2.0).abs() < 1e-6); // 2x multiple

        // TVPI also includes residual value:
        // (2.0M realized + 0.5M residual) / 1.0M paid-in = 2.5x.
        context.set_base_value(Money::from((500_000_i64, test_currency())));
        let tvpi_with_residual = TvpiLpCalculator
            .calculate(&mut context)
            .expect("should succeed");
        assert!((tvpi_with_residual - 2.5).abs() < 1e-6);
    }

    /// Holder-view PV : a fully realized fund has
    /// base_value ≈ 0, so TVPI collapses to DPI and LpIrr equals the
    /// realized IRR.
    #[test]
    fn fully_realized_fund_tvpi_equals_dpi_and_lp_irr_is_realized() {
        let spec = PeFundWaterfallSpec::builder()
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

        let pe =
            PrivateMarketsFund::new("TEST", test_currency(), spec, events).expect("valid fund");

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
