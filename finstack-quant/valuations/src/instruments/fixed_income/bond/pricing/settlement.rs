//! Settlement and quote-date utilities for bond pricing and metrics.
//!
//! This module provides helpers for computing settlement dates and
//! quote-date-anchored values used by the quote engine and yield/spread metrics.
//!
//! # Conventions
//!
//! - **PV (present value)** is always anchored at `as_of` (valuation date).
//! - **Quote-derived metrics** (YTM, Z-spread, DM, OAS, duration) are computed
//!   relative to the **quote date** (= settlement date when `settlement_days`
//!   is set, otherwise `as_of`).
//! - Accrued interest for market quotes is computed at the quote date.

use finstack_quant_core::dates::{adjust, Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

use super::super::types::Bond;
use super::super::CashflowSpec;

/// Compute the settlement date from a trade/valuation date.
///
/// Advances by the configured number of business days on the bond's payment
/// calendar, then clamps the result to the issue date. A configured but
/// unknown calendar is an error; settlement never silently falls back to a
/// weekday-only calendar.
pub(crate) fn settlement_date(bond: &Bond, as_of: Date) -> Result<Date> {
    let Some(sd_u32) = bond.settlement_days() else {
        return Ok(as_of.max(bond.issue_date));
    };

    let sd = i32::try_from(sd_u32).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "bond settlement_days {sd_u32} exceeds the supported range"
        ))
    })?;
    let (calendar_id, business_day_convention) = match &bond.cashflow_spec {
        CashflowSpec::Fixed(spec) => (
            spec.schedule.calendar_id.as_str(),
            spec.schedule.business_day_convention,
        ),
        CashflowSpec::Floating(spec) => (
            spec.schedule.calendar_id.as_str(),
            spec.schedule.business_day_convention,
        ),
        CashflowSpec::StepUp(spec) => (
            spec.schedule.calendar_id.as_str(),
            spec.schedule.business_day_convention,
        ),
        CashflowSpec::Amortizing { base, .. } => match &**base {
            CashflowSpec::Fixed(spec) => (
                spec.schedule.calendar_id.as_str(),
                spec.schedule.business_day_convention,
            ),
            CashflowSpec::Floating(spec) => (
                spec.schedule.calendar_id.as_str(),
                spec.schedule.business_day_convention,
            ),
            CashflowSpec::StepUp(spec) => (
                spec.schedule.calendar_id.as_str(),
                spec.schedule.business_day_convention,
            ),
            CashflowSpec::Amortizing { .. } => {
                return Err(finstack_quant_core::Error::Validation(
                    "nested amortizing bond cashflow specifications are unsupported".to_string(),
                ));
            }
        },
    };

    let cal = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;
    let advanced = as_of.add_business_days(sd, cal)?;
    let adjusted = adjust(advanced, business_day_convention, cal)?;
    Ok(adjusted.max(bond.issue_date))
}

/// Quote-date context for yield/spread metric calculations.
///
/// Contains pre-computed values needed by metrics that interpret market quotes:
/// - `quote_date`: The date at which the quote is interpreted (settlement date)
/// - `accrued_at_quote_date`: Accrued interest in currency at the quote date
///
/// # Usage
///
/// Use this struct when computing YTM, Z-spread, DM, OAS, and other quote-derived
/// metrics to ensure consistent handling of settlement conventions.
#[derive(Debug, Clone, Copy)]
pub(crate) struct QuoteDateContext {
    /// The date at which the market quote is interpreted.
    /// Equals `settlement_date(bond, as_of)` when `settlement_days` is set,
    /// otherwise equals `as_of`.
    pub(crate) quote_date: Date,
    /// Accrued interest (in currency) computed at `quote_date`.
    pub(crate) accrued_at_quote_date: f64,
}

impl QuoteDateContext {
    /// Create a quote-date context for a bond at a given valuation date.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to compute context for
    /// * `curves` - Market context containing curves for floating coupon fixings
    /// * `as_of` - Valuation date (trade date)
    ///
    /// # Returns
    ///
    /// A `QuoteDateContext` with the quote date and accrued interest.
    pub(crate) fn new(bond: &Bond, curves: &MarketContext, as_of: Date) -> Result<Self> {
        let quote_date = settlement_date(bond, as_of)?;

        let schedule = bond.full_cashflow_schedule(curves)?;
        let accrued_at_quote_date = crate::cashflow::accrual::accrued_interest_amount(
            &schedule,
            quote_date,
            &bond.accrual_config(),
        )?;

        Ok(Self {
            quote_date,
            accrued_at_quote_date,
        })
    }

    /// Compute dirty price in currency from a clean price quote (% of par).
    ///
    /// # Arguments
    ///
    /// * `clean_price_pct` - Clean price as percentage of par (e.g., 99.5)
    /// * `notional` - Bond notional in currency
    ///
    /// # Returns
    ///
    /// Dirty price in currency = (clean_pct × notional / 100) + accrued
    #[inline]
    pub(crate) fn dirty_from_clean_pct(&self, clean_price_pct: f64, notional: f64) -> f64 {
        clean_price_pct * notional / 100.0 + self.accrued_at_quote_date
    }

    /// Holder cashflows with coupon entitlement tested at the quote date.
    ///
    /// Wraps [`Bond::pricing_dated_cashflows_at`] with
    /// `entitlement_date = self.quote_date`, so quote-derived metrics drop a
    /// coupon whose ex-window contains the settlement date even when the
    /// trade/valuation date is still cum-coupon. When the bond has no
    /// ex-coupon rule or no settlement lag this is identical to
    /// [`Bond::pricing_dated_cashflows`].
    ///
    /// # Arguments
    ///
    /// * `bond` - Bond whose schedule and ex-coupon rule are used.
    /// * `curves` - Market context for schedule construction.
    /// * `as_of` - Valuation date; flows on or before it are excluded.
    pub(crate) fn entitled_flows(
        &self,
        bond: &Bond,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<(Date, finstack_quant_core::money::Money)>> {
        bond.pricing_dated_cashflows_at(curves, as_of, self.quote_date)
    }
}

/// Carry terms connecting an `as_of` holder value to a settlement quote.
struct QuoteCarry {
    settlement_df: f64,
    detached_pv_as_of: f64,
}

/// Compute the financing discount factor and any cashflows detached between
/// the valuation and quote-date entitlement sets.
fn quote_carry(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_date: Date,
) -> Result<QuoteCarry> {
    if quote_date <= as_of {
        return Ok(QuoteCarry {
            settlement_df: 1.0,
            detached_pv_as_of: 0.0,
        });
    }

    let curve = curves.get_discount(bond.discount_curve_id.as_str())?;
    let settlement_df = curve.df_between_dates(as_of, quote_date)?;

    // Aggregate the valuation-date holder flows, then subtract every flow
    // retained by a buyer settling on `quote_date`. The residual includes
    // payments before settlement and coupons detached by an ex-coupon rule.
    let schedule = bond.full_cashflow_schedule(curves)?;
    let mut detached_by_date = std::collections::BTreeMap::<Date, f64>::new();
    for (date, amount) in bond.pricing_dated_cashflows_from_schedule(&schedule, as_of, as_of)? {
        *detached_by_date.entry(date).or_default() += amount.amount();
    }
    for (date, amount) in
        bond.pricing_dated_cashflows_from_schedule(&schedule, as_of, quote_date)?
    {
        if date > quote_date {
            *detached_by_date.entry(date).or_default() -= amount.amount();
        }
    }

    let mut detached_pv_as_of = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for (date, amount) in detached_by_date {
        if amount != 0.0 {
            detached_pv_as_of.add(amount * curve.df_between_dates(as_of, date)?);
        }
    }

    Ok(QuoteCarry {
        settlement_df,
        detached_pv_as_of: detached_pv_as_of.total(),
    })
}

/// Forward-value a model NPV from `as_of` to the quote/settlement date.
///
/// The conversion removes cashflows owned by the `as_of` holder but not by
/// the settlement-date buyer, then divides the residual by
/// `DF(as_of → quote_date)`. This handles intervening payments and ex-coupon
/// entitlement changes rather than assuming the settlement window is empty.
pub(crate) fn model_dirty_at_quote_date(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_date: Date,
    pv_as_of: f64,
) -> Result<f64> {
    let carry = quote_carry(bond, curves, as_of, quote_date)?;
    Ok((pv_as_of - carry.detached_pv_as_of) / carry.settlement_df)
}

/// Carry a settlement-date dirty quote back to the valuation date.
///
/// The returned value is the `as_of` NPV required by
/// [`crate::instruments::Instrument::value`]:
/// the financed settlement amount plus any cashflows owned by the current
/// holder but detached before the settlement buyer's entitlement begins.
pub(crate) fn quote_dirty_at_as_of(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_date: Date,
    dirty_at_quote: f64,
) -> Result<f64> {
    let carry = quote_carry(bond, curves, as_of, quote_date)?;
    Ok(dirty_at_quote * carry.settlement_df + carry.detached_pv_as_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    fn t2_bond() -> Bond {
        let mut bond = Bond::fixed(
            "SETTLE-T2",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.settlement_convention = Some(super::super::super::types::BondSettlementConvention {
            settlement_days: 2,
            ..Default::default()
        });
        bond
    }

    /// The model PV must be carried to the quote date by exactly
    /// `1 / DF(as_of → quote_date)` on the bond's discount curve, and left
    /// unchanged when the quote date does not lie after `as_of`.
    #[test]
    fn model_dirty_forward_values_by_settlement_df() {
        let as_of = date!(2025 - 01 - 01);
        let bond = t2_bond();
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let quote_date = settlement_date(&bond, as_of).expect("settlement date");
        assert!(
            quote_date > as_of,
            "T+2 quote date must follow the trade date"
        );

        let pv_as_of = 987_654.321;
        let carried =
            model_dirty_at_quote_date(&bond, &market, as_of, quote_date, pv_as_of).expect("carry");
        let disc = market.get_discount("USD-OIS").expect("curve");
        let df = relative_df_discount_curve(disc.as_ref(), as_of, quote_date).expect("df");
        assert!((carried - pv_as_of / df).abs() < 1e-9);
        let restored = quote_dirty_at_as_of(&bond, &market, as_of, quote_date, carried)
            .expect("reverse carry");
        assert!((restored - pv_as_of).abs() < 1e-9);
        assert!(
            carried > pv_as_of,
            "positive rates must carry the PV upward"
        );

        let unchanged =
            model_dirty_at_quote_date(&bond, &market, as_of, as_of, pv_as_of).expect("no-op carry");
        assert_eq!(unchanged, pv_as_of);
    }

    #[test]
    fn settlement_never_precedes_issue_date() {
        let bond = t2_bond();
        let before_issue = date!(2024 - 12 - 20);
        assert_eq!(
            settlement_date(&bond, before_issue).expect("settlement"),
            bond.issue_date
        );
    }

    #[test]
    fn settlement_rejects_unknown_calendar() {
        let mut bond = t2_bond();
        if let CashflowSpec::Fixed(spec) = &mut bond.cashflow_spec {
            spec.schedule.calendar_id = "missing-calendar".to_string();
        }
        let err = settlement_date(&bond, date!(2025 - 01 - 02))
            .expect_err("unknown settlement calendar must fail");
        assert!(err.to_string().contains("missing-calendar"));
    }

    /// `entitled_flows` must test the ex-window at the quote/settlement date,
    /// not the trade date (gilt-style: T+1 settlement, 7-day ex-window).
    #[test]
    fn entitled_flows_use_quote_date_for_ex_window() {
        let coupon_date = date!(2025 - 07 - 01); // Tuesday
        let mut bond = Bond::fixed(
            "GILT-STYLE",
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.settlement_convention = Some(super::super::super::types::BondSettlementConvention {
            settlement_days: 1,
            ex_coupon_days: 7,
            ex_coupon_calendar_id: None,
        });
        let market = MarketContext::new();

        // Trade 8 days before the coupon: trade date is cum-coupon, but T+1
        // settlement lands inside the 7-day ex-window.
        let as_of = coupon_date - time::Duration::days(8);
        let quote_ctx = QuoteDateContext::new(&bond, &market, as_of).expect("quote ctx");
        assert!(
            quote_ctx.quote_date >= coupon_date - time::Duration::days(7),
            "settlement must be inside the ex-window for this scenario"
        );

        let entitled = quote_ctx
            .entitled_flows(&bond, &market, as_of)
            .expect("entitled flows");
        assert!(
            !entitled.iter().any(|(d, _)| *d == coupon_date),
            "ex-window settlement must drop the imminent coupon"
        );

        // The trade-date-anchored builder still keeps the coupon: the fix is
        // specific to the quote/settlement view.
        let trade_anchored = bond.pricing_dated_cashflows(&market, as_of).expect("flows");
        assert!(trade_anchored.iter().any(|(d, _)| *d == coupon_date));

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let dirty_at_quote = 100.0;
        let pv_as_of =
            quote_dirty_at_as_of(&bond, &market, as_of, quote_ctx.quote_date, dirty_at_quote)
                .expect("quote carry");
        let round_trip =
            model_dirty_at_quote_date(&bond, &market, as_of, quote_ctx.quote_date, pv_as_of)
                .expect("model carry");
        assert!((round_trip - dirty_at_quote).abs() < 1e-12);

        let disc = market.get_discount("USD-OIS").expect("curve");
        let financing_only = dirty_at_quote
            * disc
                .df_between_dates(as_of, quote_ctx.quote_date)
                .expect("df");
        assert!(
            pv_as_of > financing_only,
            "the cum-coupon holder value must include the coupon detached at settlement"
        );
    }
}
