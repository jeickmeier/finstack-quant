//! Private markets fund metrics: IRR, MOIC, DPI, TVPI, carry calculations, and theta.

use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};
use finstack_core::dates::{Date, DayCount};
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::money::Money;

/// LP Internal Rate of Return calculator.
pub struct LpIrrCalculator;

impl MetricCalculator for LpIrrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        // Get historical LP flows
        let mut flows: Vec<(Date, Money)> = ledger
            .lp_cashflows()
            .into_iter()
            .filter(|(d, _)| *d <= context.as_of)
            .collect();

        // Add Residual NAV as a terminal positive flow at as_of date
        // This aligns IRR with TVPI (mark-to-market performance)
        let nav = context.base_value;
        if nav.amount().abs() > 1e-6 {
            flows.push((context.as_of, nav));
        }

        if flows.len() < 2 {
            return Ok(0.0);
        }

        calculate_irr(&flows, pe.waterfall_spec.irr_basis)
    }
}

/// Total GP carry paid through the waterfall, in currency units.
///
/// A GP IRR is not well-defined here (the GP has no initial investment in
/// the carry stream), so this metric reports the total carry dollars instead
/// and is registered as `gp_carry_total` (review finding M11).
pub struct GpCarryTotalCalculator;

impl MetricCalculator for GpCarryTotalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
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

/// Multiple on Invested Capital (MOIC) for LP calculator.
///
/// Realized LP multiple: ledger `to_lp` distributions over contributions.
/// Gross fund events would overstate the LP multiple by the GP carry
/// (review finding M12).
pub struct MoicLpCalculator;

impl MetricCalculator for MoicLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let total_contributions: f64 = pe
            .events
            .iter()
            .filter(|e| {
                e.kind == crate::instruments::equity::pe_fund::FundEventKind::Contribution
                    && e.date <= context.as_of
            })
            .map(|e| e.amount.amount())
            .sum();

        let total_lp_distributions: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.date <= context.as_of)
            .map(|r| r.to_lp.amount())
            .sum();

        if total_contributions <= 1e-6 {
            return Ok(0.0);
        }

        Ok(total_lp_distributions / total_contributions)
    }
}

/// Distributions to Paid-In Capital (DPI) calculator.
pub struct DpiLpCalculator;

impl MetricCalculator for DpiLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let total_contributions: f64 = pe
            .events
            .iter()
            .filter(|e| {
                e.kind == crate::instruments::equity::pe_fund::FundEventKind::Contribution
                    && e.date <= context.as_of
            })
            .map(|e| e.amount.amount())
            .sum();

        let total_lp_distributions: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.date <= context.as_of)
            .map(|r| r.to_lp.amount())
            .sum();

        if total_contributions <= 1e-6 {
            return Ok(0.0);
        }

        Ok(total_lp_distributions / total_contributions)
    }
}

/// Total Value to Paid-In Capital (TVPI) calculator.
pub struct TvpiLpCalculator;

impl MetricCalculator for TvpiLpCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        let total_contributions: f64 = pe
            .events
            .iter()
            .filter(|e| {
                e.kind == crate::instruments::equity::pe_fund::FundEventKind::Contribution
                    && e.date <= context.as_of
            })
            .map(|e| e.amount.amount())
            .sum();

        // TVPI = (Realized Distributions + Residual NAV) / Contributions
        let realized_lp_distributions: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.date <= context.as_of)
            .map(|r| r.to_lp.amount())
            .sum();

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
pub struct CarryAccruedCalculator;

impl MetricCalculator for CarryAccruedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let pe: &PrivateMarketsFund = context.instrument_as()?;
        let ledger = pe.run_waterfall()?;

        // Return final GP carry cumulative amount
        Ok(ledger
            .rows
            .last()
            .map(|r| r.gp_carry_cum.amount())
            .unwrap_or(0.0))
    }
}

/// Helper function to calculate IRR using robust root finding.
pub fn calculate_irr(flows: &[(Date, Money)], day_count: DayCount) -> finstack_core::Result<f64> {
    if flows.len() < 2 {
        return Err(finstack_core::InputError::TooFewPoints.into());
    }

    let base_date = flows[0].0;

    let npv_function = |rate: f64| -> f64 {
        let mut npv = 0.0;
        for (date, amount) in flows {
            let t = day_count
                .year_fraction(
                    base_date,
                    *date,
                    finstack_core::dates::DayCountContext::default(),
                )
                .unwrap_or(0.0);
            // Discount factor `(1 + r)^{-t}`. This expression is well-defined
            // and continuous at `r = 0` — it evaluates to exactly `1.0^{-t} =
            // 1.0` — so no special-case is needed. Using the same closed form
            // at every rate keeps this routine consistent with the waterfall's
            // IRR routine (`WaterfallSpec::calculate_irr`), which also discounts
            // with `(1 + r)^{-t}` and treats `r = 0` as the `1.0` limit.
            let df = (1.0 + rate).powf(-t);
            npv += amount.amount() * df;
        }
        npv
    };

    // Use BrentSolver with reasonable bounds for PE returns
    let solver = BrentSolver::new()
        .tolerance(1e-12)
        .initial_bracket_size(Some(1.0)); // Start with reasonable IRR range

    solver
        .solve(npv_function, 0.15) // Start with 15% initial guess for PE returns
        .map_err(|_| finstack_core::InputError::Invalid.into())
}

mod carry01;
mod hurdle01;
mod nav01;

/// Register all private markets fund metrics.
pub(crate) fn register_private_markets_fund_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Private markets fund-specific risk metrics (custom metrics)
    registry.register_metric(
        MetricId::Nav01,
        Arc::new(nav01::Nav01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    );
    registry.register_metric(
        MetricId::Carry01,
        Arc::new(carry01::Carry01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    );
    registry.register_metric(
        MetricId::Hurdle01,
        Arc::new(hurdle01::Hurdle01Calculator),
        &[InstrumentType::PrivateMarketsFund],
    );

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::equity::pe_fund::{FundEvent, WaterfallSpec};
    use time::Month;

    fn test_currency() -> finstack_core::currency::Currency {
        finstack_core::currency::Currency::USD
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
                Money::new(-1000000.0, test_currency()),
            ), // Contribution
            (
                test_date(2025, 1, 1),
                Money::new(2000000.0, test_currency()),
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
            (start, Money::new(-contribution, test_currency())),
            (end, Money::new(distribution, test_currency())),
        ];

        let irr = calculate_irr(&flows, DayCount::Act365F).expect("IRR should solve");

        // Analytically exact IRR for a single contribution/distribution pair:
        //   C = D · (1 + r)^{-t}   =>   r = (D / C)^{1/t} − 1.
        let t = DayCount::Act365F
            .year_fraction(start, end, finstack_core::dates::DayCountContext::default())
            .expect("year fraction");
        let exact_irr = (distribution / contribution).powf(1.0 / t) - 1.0;

        assert!(
            (irr - exact_irr).abs() < 1e-9,
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
        // the ledger-basis MOIC (review finding M12) equals the naive 2x.
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 1.0, 0.0)
            .build()
            .expect("should succeed");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::new(2000000.0, test_currency()),
            ),
        ];

        let pe = PrivateMarketsFund::new("TEST", test_currency(), spec, events);

        let curves = finstack_core::market_data::context::MarketContext::new();
        let base_value = Money::new(2000000.0, test_currency());
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
    }

    /// Holder-view PV (review finding M13): a fully realized fund has
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
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::new(2000000.0, test_currency()),
            ),
        ];

        let pe = PrivateMarketsFund::new("TEST", test_currency(), spec, events);

        let curves = finstack_core::market_data::context::MarketContext::new();
        // base_value ≈ 0 for a fully realized fund under holder-view PV.
        let base_value = Money::new(0.0, test_currency());
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
