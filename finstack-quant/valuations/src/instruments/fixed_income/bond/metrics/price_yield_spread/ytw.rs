//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;

/// Calculates yield-to-worst (YTW) for bonds with call/put schedules.
///
/// YTW is defined here as the minimum yield-to-maturity across all admissible
/// exercise paths (calls, puts, and final maturity), where each candidate
/// yield is solved as an IRR that equates the present value of the **truncated
/// projected cashflows** to the current dirty market price at the **quote date**
/// (settlement date).
///
/// # Quote-Date Convention
///
/// Like YTM, YTW is computed relative to the **quote date** (settlement date when
/// `settlement_days` is set, otherwise `as_of`):
/// - Accrued interest is computed at the quote date
/// - Cashflows before the quote date are excluded
/// - Time to each cashflow is measured from the quote date
///
/// # Applicability
///
/// - **Primary use**: callable / putable **fixed-rate bonds**, where YTW is
///   the standard market risk measure ("worst case" yield to any exercise).
/// - **Other cashflow specs**: for floating-rate, amortizing, or custom
///   structures, this calculator still computes a well-defined "worst-case
///   cashflow-implied yield" across exercise dates, but this is **not** the
///   standard FRN quoting convention and should be interpreted as an internal
///   risk/analytics measure rather than a market quote. For FRNs, **discount
///   margin** is typically the preferred spread metric.
///
/// As with `YtmCalculator`, the `coupon_rate` field passed into the YTM solver
/// is only an **initial guess**; for non-fixed `CashflowSpec` variants it is
/// set to `0.0`, but the solved YTW values are driven entirely by the explicit
/// projected cashflows and the target price along each exercise path.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct YtwCalculator;

impl MetricCalculator for YtwCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Build and cache flows and hints if not already present
        let flows = if let Some(ref flows) = context.cashflows {
            flows
        } else {
            let (discount_curve_id, day_count, built) = {
                let bond: &Bond = context.instrument_as()?;
                (
                    bond.discount_curve_id.to_owned(),
                    bond.cashflow_spec.day_count(),
                    // Coupon entitlement at the quote/settlement date,
                    // consistent with the accrued used for the dirty target.
                    quote_ctx.entitled_flows(bond, &context.curves, context.as_of)?,
                )
            };
            context.cashflows = Some(built);
            context.discount_curve_id = Some(discount_curve_id);
            context.day_count = Some(day_count);
            context.cashflows.as_ref().ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "cashflows".to_string(),
                })
            })?
        };

        // Construct current dirty market price from quoted clean price + accrued at quote_date.
        let bond: &Bond = context.instrument_as()?;
        let clean_px = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "bond.instrument_pricing_overrides.market_quotes.quoted_clean_price"
                        .to_string(),
                })
            })?;

        // Dirty price in currency at quote_date: quoted clean is % of par.
        let dirty_amt = quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount());
        let dirty_now = Money::new(dirty_amt, bond.notional.currency())?;

        // Lower return-floor protection into its pricing-effective call
        // schedule before scanning workouts. Contractual cashflows are
        // unchanged, while floor-only bonds gain their admissible early exits.
        let effective_bond = bond.effective_for_pricing(&context.curves, context.as_of)?;
        let schedule = effective_bond.full_cashflow_schedule(&context.curves)?;

        // Delegate candidate scanning and YTM solving to shared helper.
        // Use quote_date as the time origin to match market convention.
        let (best_ytm, _best_flows) =
            crate::instruments::fixed_income::bond::pricing::quote_conversions::solve_ytw_from_flows(
                &effective_bond,
                &context.curves,
                flows,
                quote_ctx.quote_date,
                dirty_now,
                &schedule,
            )?;

        Ok(best_ytm)
    }
}

impl YtwCalculator {}
