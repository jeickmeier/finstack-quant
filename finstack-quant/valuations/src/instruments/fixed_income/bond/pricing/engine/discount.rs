use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::kahan_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use super::super::super::types::Bond;

/// Bond pricing engine providing core valuation methods.
///
/// Uses `Bond::pricing_dated_cashflows` (internal helper) for discount flows:
/// coupons, amortization, and positive notional (redemption). Negative
/// notionals and PIK are excluded as they are not discounted receipt flows.
///
/// # Pricing Formula
///
/// The present value is computed by discounting all future holder-view cashflows:
/// ```text
/// PV = Σ CF_i · DF(as_of → t_i)
/// ```
/// where:
/// - `CF_i` are holder-view cashflows (coupons, amortization, redemption)
/// - `DF(as_of → t_i)` is the discount factor from valuation date to cashflow date
///
/// # Settlement Convention
///
/// Settlement days (`bond.settlement_days`) affect how market **quotes** are
/// interpreted (e.g., accrued interest at settlement date), but the instrument
/// PV is always anchored at `as_of`. The quote engine handles settlement-date
/// accrued interest separately when computing quote-derived metrics (YTM, Z-spread, etc.).
///
/// # Examples
///
/// Bond pricing is performed via the [`Instrument`] trait or the pricer registry:
///
/// ```ignore
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use time::macros::date;
///
/// let bond = Bond::example().unwrap();
/// let market = MarketContext::new();
/// let as_of = date!(2024-01-15);
///
/// // Use Instrument trait for public API
/// let pv = bond.value(&market, as_of)?;
/// ```
///
/// [`Instrument`]: crate::instruments::common_impl::traits::Instrument
pub struct BondEngine;

impl BondEngine {
    /// Price a bond using discount curve present value calculation.
    ///
    /// Computes the present value by discounting all future holder-view cashflows
    /// from the valuation date (`as_of`) using the bond's discount curve.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to price
    /// * `context` - Market context containing the discount curve
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value of the bond in the bond's currency, discounted from `as_of`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found in market context
    /// - Bond has no future cashflows
    /// - Cashflow schedule building fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::discount::BondEngine;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::dates::Date;
    ///
    /// # let bond = Bond::example().unwrap();
    /// # let market = MarketContext::new();
    /// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
    /// let pv = BondEngine::price(&bond, &market, as_of)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub(crate) fn price(bond: &Bond, context: &MarketContext, as_of: Date) -> Result<Money> {
        Self::price_with_explanation(bond, context, as_of, ExplainOpts::disabled())
            .map(|(pv, _)| pv)
    }

    /// Price a bond with optional explanation trace.
    ///
    /// Returns the present value and an optional trace containing
    /// cashflow-level PV breakdown when explanation is enabled.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to price
    /// * `context` - Market context containing the discount curve
    /// * `as_of` - Valuation date
    /// * `explain` - Explanation options controlling trace generation
    ///
    /// # Returns
    ///
    /// Tuple of `(Money, Option<ExplanationTrace>)`:
    /// - Present value of the bond
    /// - Optional explanation trace with cashflow-level breakdown (if enabled)
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found in market context
    /// - Bond has no future cashflows
    /// - Cashflow schedule building fails
    /// - Calendar adjustment fails (if settlement days and calendar are specified)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::discount::BondEngine;
    /// use finstack_quant_core::explain::ExplainOpts;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::dates::Date;
    ///
    /// # let bond = Bond::example().unwrap();
    /// # let market = MarketContext::new();
    /// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
    /// let (pv, trace) = BondEngine::price_with_explanation(
    ///     &bond,
    ///     &market,
    ///     as_of,
    ///     ExplainOpts::enabled(),
    /// )?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn price_with_explanation(
        bond: &Bond,
        context: &MarketContext,
        as_of: Date,
        explain: ExplainOpts,
    ) -> Result<(Money, Option<ExplanationTrace>)> {
        let flows = bond.pricing_dated_cashflows(context, as_of)?;
        let disc = context.get_discount(bond.discount_curve_id.as_str())?;
        if flows.is_empty() {
            return Ok((
                Money::new(0.0, bond.notional.currency()),
                if explain.enabled {
                    Some(ExplanationTrace::new("pricing"))
                } else {
                    None
                },
            ));
        }
        let ccy = flows[0].1.currency();

        // Initialize explanation trace if requested
        let mut trace = if explain.enabled {
            Some(ExplanationTrace::new("pricing"))
        } else {
            None
        };

        // PV is anchored at as_of (valuation date), not settlement.
        // Settlement days affect quote interpretation (accrued at settle), but PV
        // is the instrument's theoretical value at as_of.
        // Collect PV values for Kahan summation (O(1) error growth vs O(n) for naive sum).
        // This is particularly important for long-dated bonds (50Y+ monthly-pay).
        let mut pv_values: Vec<f64> = Vec::with_capacity(flows.len());

        for (d, amt) in &flows {
            // Include same-day cashflows with DF(as_of, as_of)=1.0 for consistency with
            // shared schedule-based pricing helpers used by other fixed-income instruments.
            if *d < as_of {
                continue;
            }
            let df = disc.df_between_dates(as_of, *d)?;
            let pv_cf = *amt * df;
            pv_values.push(pv_cf.amount());

            // Add trace entry if explanation is enabled
            if let Some(ref mut t) = trace {
                t.push(
                    TraceEntry::CashflowPV {
                        date: d.to_string(),
                        cashflow_amount: amt.amount(),
                        cashflow_currency: amt.currency().to_string(),
                        discount_factor: df,
                        pv_amount: pv_cf.amount(),
                        pv_currency: pv_cf.currency().to_string(),
                        curve_id: bond.discount_curve_id.to_string(),
                    },
                    explain.max_entries,
                );
            }
        }

        // Use Kahan compensated summation from finstack-quant-core for numerical stability
        let total = Money::new(kahan_sum(pv_values), ccy);
        Ok((total, trace))
    }
}
