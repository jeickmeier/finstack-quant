use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::CashflowSpec;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;
use rust_decimal::prelude::ToPrimitive;

/// Calculates yield to maturity (YTM) for bonds.
///
/// YTM is defined here as the internal rate of return that equates the present
/// value of **all projected future cashflows** to the current dirty market
/// price (quoted clean price plus accrued interest at the **quote date**).
///
/// # Quote-Date Convention
///
/// YTM is computed relative to the **quote date** (settlement date when
/// `settlement_days` is set, otherwise `as_of`):
/// - Accrued interest is computed at the quote date
/// - Cashflows before the quote date are excluded
/// - Time to each cashflow is measured from the quote date
///
/// This matches market convention where bond quotes are settlement-date quotes.
///
/// # Applicability
///
/// - **Primary use**: plain-vanilla **fixed-rate bullet bonds**, where YTM has
///   the usual market interpretation (coupon-like yield for comparison).
/// - **Other cashflow specs**: for floating-rate, amortizing, or custom
///   cashflow structures, this calculator still solves a well-defined IRR off
///   the full discounted cashflow schedule. The resulting YTM is a
///   **cashflow-implied yield**, but it is **not** the market-standard quote
///   for FRNs (where **discount margin** is preferred) and may have less direct
///   interpretation for exotic structures.
///
/// Implementation detail: the `coupon_rate` field in `YtmPricingSpec` is used
/// only as a **solver hint / initial guess**. For non-fixed `CashflowSpec`
/// variants this is set to `0.0`, but the solved YTM is fully determined by
/// the explicit projected cashflows and the target price, not by this hint.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // YTM is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(crate) struct YtmCalculator;

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Extract fields we need from the bond
        let bond: &Bond = context.instrument_as()?;
        let maybe_clean_px = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let notional = bond.notional;
        let dc = bond.cashflow_spec.day_count();
        let discount_curve_id = bond.discount_curve_id.to_owned();
        let coupon = match &bond.cashflow_spec {
            // Rate overflow is extremely unlikely for interest rates,
            // but use 0.0 as initial guess hint (solver will find correct YTM)
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            _ => 0.0,
        };
        let freq = bond.cashflow_spec.frequency();

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Determine dirty price in currency at the quote date.
        //
        // Preferred path: use quoted clean price (market quote) plus accrued
        // interest at the quote date to build the dirty market price.
        // When no quoted clean price is available, fall back to the model PV
        // adjusted for time value between as_of and quote_date.
        let dirty: Money = if let Some(clean_px) = maybe_clean_px {
            // Compute dirty price at quote_date: clean% × notional + accrued_at_quote
            let dirty_amt = quote_ctx.dirty_from_clean_pct(clean_px, notional.amount());
            Money::new(dirty_amt, notional.currency())
        } else {
            // Fallback: forward-value the model PV (computed at `as_of`) to the
            // quote/settlement date so the solved YTM discounts cashflows from
            // the same origin (`quote_date`) at which the dirty price is
            // expressed. `base_value` is PV at `as_of`; dividing by
            // `DF(as_of → quote_date)` removes the settlement-period (typically
            // T+2) carry that would otherwise bias the YTM and the
            // duration/convexity derived from it. This assumes no cashflow falls
            // strictly between `as_of` and `quote_date`; for a standard T+1/T+2
            // settlement lag that window contains no coupon, but a coupon inside
            // it would be carried forward yet excluded by the solver, leaving a
            // small residual bias.
            let pv_as_of = context.base_value.amount();
            let pv_at_quote = if quote_ctx.quote_date > context.as_of {
                let curve = context.curves.get_discount(discount_curve_id.as_str())?;
                let df = relative_df_discount_curve(
                    curve.as_ref(),
                    context.as_of,
                    quote_ctx.quote_date,
                )?;
                if df > 0.0 {
                    pv_as_of / df
                } else {
                    pv_as_of
                }
            } else {
                pv_as_of
            };
            Money::new(pv_at_quote, notional.currency())
        };

        // Build and cache flows and hints if not already present
        if context.cashflows.is_none() {
            let bond: &Bond = context.instrument_as()?;
            let flows = bond.pricing_dated_cashflows(&context.curves, context.as_of)?;
            context.cashflows = Some(flows);
            context.discount_curve_id = Some(discount_curve_id);
            context.day_count = Some(dc);
        }
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "cashflows".to_string(),
            })
        })?;

        // Solve for YTM using shared solver with Street compounding (default)
        // Time origin is the quote_date (settlement date) to match market convention
        let ytm = crate::instruments::fixed_income::bond::pricing::ytm_solver::solve_ytm(
            flows,
            quote_ctx.quote_date,
            dirty,
            crate::instruments::fixed_income::bond::pricing::ytm_solver::YtmPricingSpec {
                day_count: dc,
                notional,
                coupon_rate: coupon,
                compounding:
                    crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
                frequency: freq,
            },
        )?;

        Ok(ytm)
    }
}
