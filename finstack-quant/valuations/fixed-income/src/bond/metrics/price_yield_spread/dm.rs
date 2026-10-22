//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use super::quote_date_dirty_target;
use crate::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::bond::pricing::quote_conversions::{solve_kernel_spread, BondZSpreadPricingKernel};
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Discount-margin solver tolerance on the DM axis (decimal).
///
/// `1e-10` (~0.01 bp) matches the Z-spread and YTM solvers and keeps FRN price
/// errors below $0.01 per $1M face.
const DM_TOLERANCE: f64 = 1e-10;

/// Base half-width of the DM initial bracket, in basis points.
///
/// FRN margins are tighter than fixed-rate credit spreads (IG 20-100 bp,
/// HY 200-500 bp), so ±500 bp covers most of the universe.
const DM_BASE_BRACKET_BP: f64 = 500.0;

/// Cap on the maturity-scaled DM bracket half-width, in basis points.
const DM_MAX_BRACKET_BP: f64 = 1500.0;

/// Discount Margin (DM) for floating-rate bonds.
///
/// Definition: constant additive spread (decimal, e.g., 0.01 = 100bp) over the
/// bond's **discount curve** such that the PV of the bond's projected
/// cashflows — projected at the **contractual quoted margin** — equals the
/// observed dirty market price (Fabozzi; Bloomberg YAS convention). PV is
/// strictly decreasing in DM, so an FRN quoted below par solves to a DM above
/// its quoted margin, and vice versa. On a flat, consistent curve where the
/// discount curve equals the projection index curve, an FRN priced at par has
/// DM equal to its quoted margin.
///
/// Notes:
/// - Intended for bonds with **floating coupons**: plain FRNs and amortizing
///   floaters (`Amortizing` with a floating base). For fixed-rate bonds and
///   other non-floating `CashflowSpec` variants, this calculator returns an error,
///   since there is no forward index to spread over. In those cases, use **YTM**,
///   **Z-spread**, or asset-swap spreads instead.
/// - Requires quoted clean price, or falls back to the model PV forward-valued
///   to the quote/settlement date as target.
/// - Uses the FRN path: coupons are projected off the forward curve at reset
///   with margin and gearing from `FloatingCouponSpec` (unchanged by the DM),
///   then discounted with the DM added to the periodically-compounded zero
///   rate derived from the discount curve (the same mechanics as the
///   Z-spread; see `price_from_dm`). This is a **curve DM**: when the
///   discount curve differs from the projection index curve, the solved DM
///   includes that basis.
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first.
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations_fixed_income::bond::Bond;
/// use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Discount margin is computed automatically when requesting bond metrics for FRNs
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscountMarginCalculator;

impl MetricCalculator for DiscountMarginCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let target = quote_date_dirty_target(bond, context, &quote_ctx)?;

        // DM is only defined for bonds with floating coupons (plain FRNs and
        // amortizing floaters). For fixed-rate bonds, return an error; callers
        // should use YTM, Z-spread, or asset-swap spreads instead.
        if !bond.has_floating_coupons() {
            return Err(finstack_quant_core::Error::Validation(
                "discount margin is only defined for bonds with floating coupons".to_string(),
            ));
        }

        // Root-find DM such that PV(dm) - dirty = 0. The objective is
        // `price_from_dm`: the Z-spread kernel on a clone with price-driving
        // overrides cleared. The kernel (schedule projection, workout path,
        // discount factors) does not depend on the DM, so build it once.
        let mut model_bond = bond.clone();
        clear_price_driving_overrides(&mut model_bond);
        let kernel = BondZSpreadPricingKernel::new(&model_bond, &context.curves, context.as_of)?;
        solve_kernel_spread(
            bond,
            &kernel,
            target,
            quote_ctx.quote_date,
            DM_BASE_BRACKET_BP,
            DM_MAX_BRACKET_BP,
            DM_TOLERANCE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::metrics::MetricContext;
    use std::sync::Arc;
    use time::macros::date;

    /// Issue B regression: when pricing fails (e.g. missing forward curve), the DM
    /// solver must surface an error rather than silently returning a near-zero DM.
    ///
    /// With the pre-fix residual `1e12 * sign(dm)`, Brent found a fake sign-changing
    /// bracket straddling `dm = 0` and "converged" to a meaningless DM ≈ 0. A
    /// pricing error now aborts the solve, and a missing curve fails the
    /// one-time kernel build before the solver runs.
    ///
    /// This test drives the real `DiscountMarginCalculator::calculate` path: a valid FRN
    /// is constructed with only the discount curve present; the forward/projection curve
    /// is intentionally omitted so pricing returns a missing-curve error.
    #[test]
    fn dm_failure_residual_must_not_change_sign_across_zero() {
        let as_of = date!(2025 - 01 - 01);

        // Valid FRN that references the "USD-SOFR-3M" projection curve.
        let bond = crate::bond::Bond::floating(
            "DM-UNIT-MISSING-FWD",
            Money::from((1_000_000_i64, Currency::USD)),
            "USD-SOFR-3M",
            200,
            as_of,
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::Tenor::quarterly(),
            finstack_quant_core::dates::DayCount::Act360,
            "USD-OIS",
        )
        .expect("bond construction should succeed");

        // Market with only the discount curve — forward curve intentionally absent so
        // every price_from_dm call inside the objective fails with a missing-curve error.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("discount curve should build");
        let market = Arc::new(MarketContext::new().insert(disc));

        let mut mctx = MetricContext::new(
            Arc::new(bond),
            market,
            as_of,
            Money::from((1_000_000_i64, Currency::USD)),
            MetricContext::default_config(),
        );

        let calc = DiscountMarginCalculator;
        let result = calc.calculate(&mut mctx);

        // The missing forward curve fails the one-time kernel build, so the
        // missing-curve error is surfaced before the solver runs.
        assert!(
            result.is_err(),
            "DM solver must return Err when every price_from_dm call fails (missing forward \
             curve), not Ok({:?})",
            result.ok()
        );
    }
}
