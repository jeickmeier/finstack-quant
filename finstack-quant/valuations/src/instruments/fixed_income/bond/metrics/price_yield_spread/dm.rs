use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::CashflowSpec;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use std::cell::RefCell;

/// Configuration for the discount margin solver.
///
/// # Tolerance Design Rationale
///
/// The DM (discount margin) tolerance is specified on the **spread axis** (decimal).
/// The default `1e-10` (~0.01 bp) is chosen to ensure:
///
/// 1. **Price accuracy**: For typical FRNs, this yields price errors < $0.01 per $1M face.
///
/// 2. **Consistency**: Same precision as Z-spread and YTM solvers for coherent
///    cross-metric analysis.
///
/// ## FRN-Specific Considerations
///
/// Discount margin for FRNs is inherently more stable than fixed-rate yields because:
/// - Floating coupons reset to market rates, reducing duration
/// - Price sensitivity to DM is lower (typically 0.01-0.05% per bp for short-dated)
///
/// This allows slightly tighter brackets than fixed-rate bond Z-spreads.
///
/// ## Recommended Tolerances
///
/// | Use Case | Tolerance | DM Precision |
/// |----------|-----------|--------------|
/// | Regulatory | `1e-12` | < 0.0001 bp |
/// | Trading | `1e-10` | < 0.01 bp |
/// | Screening | `1e-8` | < 1 bp |
///
/// # Maturity-Aware Bracketing
///
/// FRN spreads are typically tighter than fixed-rate credit spreads:
/// - Investment grade FRNs: 20-100 bp
/// - High yield FRNs: 200-500 bp
/// - Distressed: 500+ bp
///
/// The bracket scales with maturity: `bracket = base × (1 + years/30)`
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::metrics::price_yield_spread::DiscountMarginSolverConfig;
///
/// // Default for standard FRNs
/// let default = DiscountMarginSolverConfig::default();
///
/// // Tighter for IG FRN trading
/// let ig_config = DiscountMarginSolverConfig {
///     tolerance: 1e-12,
///     base_bracket_bp: 300.0,
///     max_bracket_bp: 800.0,
/// };
///
/// // Wider for leveraged loan / HY FRN screening
/// let hy_config = DiscountMarginSolverConfig {
///     tolerance: 1e-8,
///     base_bracket_bp: 800.0,
///     max_bracket_bp: 2000.0,
/// };
/// ```
#[derive(Debug, Clone)]
pub(crate) struct DiscountMarginSolverConfig {
    /// Convergence tolerance for the DM root finder (on the DM axis, decimal).
    ///
    /// Default: `1e-10` (~0.01 bp precision). This is consistent with other
    /// spread solvers (Z-spread, OAS) and yields sub-penny price accuracy.
    pub tolerance: f64,

    /// Base half-width of the initial search bracket, in basis points.
    ///
    /// FRNs typically have tighter spreads than fixed-rate bonds:
    /// - IG: 20-100 bp
    /// - HY: 200-500 bp
    ///
    /// Default of ±500 bp covers most FRN universe without excessive searching.
    pub base_bracket_bp: f64,

    /// Maximum half-width of the initial search bracket (in bp) after maturity scaling.
    ///
    /// Caps the bracket for long-dated FRNs (rare, but possible in structured products).
    pub max_bracket_bp: f64,
}

impl Default for DiscountMarginSolverConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-10,
            // Short-dated FRNs: ±500 bp is ample, even in stressed markets
            base_bracket_bp: 500.0,
            // Allow widening for long-dated/distressed names without going extreme
            max_bracket_bp: 1500.0,
        }
    }
}

/// Discount Margin (DM) for floating-rate bonds.
///
/// Definition: constant additive spread (decimal, e.g., 0.01 = 100bp) over the
/// reference forward index such that the discounted PV of the bond's projected
/// cashflows equals the observed dirty market price.
///
/// Notes:
/// - Intended for **floating-rate notes (FRNs)**. For fixed-rate bonds and
///   other non-floating `CashflowSpec` variants, this calculator returns an error,
///   since there is no forward index to spread over. In those cases, use **YTM**,
///   **Z-spread**, or asset-swap spreads instead.
/// - Requires quoted clean price or falls back to base PV as target.
/// - Uses the FRN path: coupons are projected off the forward curve at reset
///   with margin and gearing from `FloatingCouponSpec`, then discounted with the
///   discount curve. The DM is added to the projected index rate.
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first.
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Discount margin is computed automatically when requesting bond metrics for FRNs
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct DiscountMarginCalculator {
    config: DiscountMarginSolverConfig,
}

impl DiscountMarginCalculator {
    /// Create a DM calculator with default production-grade solver settings.
    pub fn new() -> Self {
        Self::default()
    }

    fn pv_given_dm(
        bond: &Bond,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
        dm: f64,
    ) -> finstack_quant_core::Result<f64> {
        price_from_dm(bond, curves, as_of, dm)
    }

    /// Compute an initial bracket half-width (in decimal) based on maturity.
    ///
    /// Short-dated FRNs use the base bracket (e.g., ±500 bp). Longer maturities
    /// widen the bracket smoothly up to `max_bracket_bp`, which improves
    /// robustness for high-yield/distressed names without over-bracketing
    /// short, high-grade bonds.
    fn initial_bracket_decimal(
        &self,
        bond: &Bond,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        if as_of >= bond.maturity {
            return Ok(self.config.base_bracket_bp / 10_000.0);
        }
        let dc = bond.cashflow_spec.day_count();
        let years = dc
            .year_fraction(
                as_of,
                bond.maturity,
                DayCountContext {
                    frequency: Some(bond.cashflow_spec.frequency()),
                    ..Default::default()
                },
            )?
            .max(0.0);

        // Scale bracket between 1x and 2x base over 0–30y, then clamp.
        let maturity_scale = 1.0 + (years / 30.0).min(1.0);
        let bracket_bp =
            (self.config.base_bracket_bp * maturity_scale).min(self.config.max_bracket_bp);

        Ok(bracket_bp / 10_000.0)
    }
}

impl MetricCalculator for DiscountMarginCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Determine dirty market price in currency at quote_date
        let dirty_ccy = if let Some(clean_px) = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
        {
            quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount())
        } else {
            context.base_value.amount()
        };

        // DM is only defined for floating-rate bonds. For fixed-rate bonds, return an error.
        // Callers should use YTM, Z-spread, or asset-swap spreads for fixed-rate instruments.
        if !matches!(&bond.cashflow_spec, CashflowSpec::Floating(_)) {
            return Err(finstack_quant_core::Error::from(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        // Root-find DM such that PV(dm) - dirty = 0
        // Note: price_from_dm uses as_of for cashflow projection timing.
        // The DM is still meaningful as it measures the spread that makes the
        // projected FRN cashflows equal the settlement dirty price.
        let pricing_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);
        let quote_date = quote_ctx.quote_date;

        let objective = |dm: f64| -> f64 {
            match Self::pv_given_dm(bond, &context.curves, quote_date, dm) {
                Ok(pv) => pv - dirty_ccy,
                Err(e) => {
                    // Capture the first pricing error and map to a large non-zero residual
                    let mut slot = pricing_error.borrow_mut();
                    if slot.is_none() {
                        *slot = Some(e);
                    }
                    drop(slot);
                    // Return a large *positive* residual that does NOT depend on the
                    // sign of `dm`. The DM objective is monotonically decreasing in
                    // `dm` (higher spread → lower PV), so a pricing failure in the
                    // deep-negative-DM regime means the true price diverges to
                    // +∞ and `price - target` is unambiguously large and positive.
                    //
                    // The previous `sign(dm)`-based residual flipped sign at dm = 0
                    // even when every pricing call failed, handing Brent a fake
                    // sign-changing bracket that "converged" to a meaningless DM ≈ 0.
                    // This is the same fix applied to the YTM solver (see ytm_solver.rs).
                    1e12
                }
            }
        };

        // Use a maturity-aware initial bracket with production-grade tolerance.
        let bracket = self.initial_bracket_decimal(bond, quote_date)?;
        let solver = BrentSolver::new()
            .tolerance(self.config.tolerance)
            .initial_bracket_size(Some(bracket));
        // Initial guess 0.0 (0 bp). DM returned in decimal (e.g., 0.01 = 100bp)
        let dm = solver.solve(objective, 0.0)?;

        // If any pricing error occurred during objective evaluation, surface it instead of
        // returning a potentially meaningless DM.
        if let Some(err) = pricing_error.into_inner() {
            return Err(err);
        }

        Ok(dm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricContext;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    /// Issue B regression: when every `pv_given_dm` call fails (e.g. missing forward
    /// curve), the DM solver must surface an error rather than silently returning a
    /// near-zero DM.
    ///
    /// With the pre-fix residual `1e12 * sign(dm)`, Brent found a fake sign-changing
    /// bracket straddling `dm = 0` and "converged" to a meaningless DM ≈ 0. The flat
    /// `+1e12` residual introduced by the fix gives Brent no sign-changing bracket, so
    /// `solver.solve(...)` returns `Err` and the captured pricing error is surfaced.
    ///
    /// This test drives the real `DiscountMarginCalculator::calculate` path: a valid FRN
    /// is constructed with only the discount curve present; the forward/projection curve
    /// is intentionally omitted so every internal `pv_given_dm` call returns a
    /// missing-curve error.
    #[test]
    fn dm_failure_residual_must_not_change_sign_across_zero() {
        let as_of = date!(2025 - 01 - 01);

        // Valid FRN that references the "USD-SOFR-3M" projection curve.
        let bond = crate::instruments::Bond::floating(
            "DM-UNIT-MISSING-FWD",
            Money::new(1_000_000.0, Currency::USD),
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
        // every pv_given_dm call inside the objective fails with a missing-curve error.
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
            Money::new(1_000_000.0, Currency::USD),
            MetricContext::default_config(),
        );

        let calc = DiscountMarginCalculator::default();
        let result = calc.calculate(&mut mctx);

        // With the flat +1e12 residual, the bracket search finds no sign change and
        // solver.solve() returns Err — the captured missing-curve error is surfaced.
        // Before the fix, the sign-flipping residual allowed Brent to "converge" to
        // dm ≈ 0, and the pricing error guard then also returned Err but only after
        // unnecessary fake convergence; removing either guard would yield Ok(~0.0).
        assert!(
            result.is_err(),
            "DM solver must return Err when every pv_given_dm call fails (missing forward \
             curve), not Ok({:?})",
            result.ok()
        );
    }
}
