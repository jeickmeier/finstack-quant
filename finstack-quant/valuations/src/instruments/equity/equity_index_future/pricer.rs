//! Equity index future pricer engine.
//!
//! Provides deterministic PV for `EquityIndexFuture` instruments using
//! mark-to-market or cost-of-carry fair value pricing.

use crate::instruments::equity::equity_index_future::EquityIndexFuture;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

pub(crate) fn compute_pv(
    future: &EquityIndexFuture,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    Ok(Money::new(
        compute_pv_raw(future, market, as_of)?,
        future.notional.currency(),
    ))
}

pub(crate) fn compute_pv_raw(
    future: &EquityIndexFuture,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    future.validate()?;
    if future.expiry < as_of {
        return Ok(0.0);
    }
    if as_of > future.last_trading_date {
        let settlement = future.settlement_price.ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "EquityIndexFuture '{}' requires settlement_price after last_trading_date {}",
                future.id, future.last_trading_date
            ))
        })?;
        if !settlement.is_finite() || settlement <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityIndexFuture settlement_price must be finite and positive".to_string(),
            ));
        }
        // The settlement fixing is the final mark and is not a live quote.
        return price_quoted(future, settlement);
    }
    if let Some(quoted) = future.quoted_price {
        return price_quoted(future, quoted);
    }
    price_fair_value(future, market, as_of)
}

/// Resolve the entry price for an open position, erroring if absent.
///
/// `EquityIndexFuture::entry_price` is `Option<f64>` so that an unfilled
/// order can be represented in the data model. Once you ask the pricer for
/// PV, however, the entry price is mandatory: PV is mark-to-market minus
/// entry, so a missing entry would silently default to zero and book the
/// full quoted price as P&L. Delegates to the shared requirement on the
/// instrument (also used by `EquityIndexFuture::delta`).
fn require_entry_price(future: &EquityIndexFuture) -> finstack_quant_core::Result<f64> {
    future.require_entry_price()
}

fn entry_contracts(future: &EquityIndexFuture, entry_price: f64) -> f64 {
    future.num_contracts(entry_price)
}

pub(crate) fn price_quoted(
    future: &EquityIndexFuture,
    quoted_price: f64,
) -> finstack_quant_core::Result<f64> {
    let entry = require_entry_price(future)?;
    let price_diff = quoted_price - entry;
    let contracts = entry_contracts(future, entry);
    let pv = price_diff * future.contract_specs.multiplier * contracts * future.position_sign();
    Ok(pv)
}

pub(crate) fn price_fair_value(
    future: &EquityIndexFuture,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let fair_value = fair_forward(future, market, as_of)?;
    let entry = require_entry_price(future)?;
    let price_diff = fair_value - entry;
    let contracts = entry_contracts(future, entry);
    let pv = price_diff * future.contract_specs.multiplier * contracts * future.position_sign();
    Ok(pv)
}

pub(crate) fn resolve_dividend_yield(
    future: &EquityIndexFuture,
    context: &MarketContext,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    if let Some(ref div_id) = future.div_yield_id {
        let ms = context.get_price(div_id.as_str()).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "Dividend yield lookup failed for '{}': {}. If dividend yield is not needed, set div_yield_id to None.",
                div_id, e
            ))
        })?;
        match ms {
            MarketScalar::Unitless(v) => Ok(*v),
            MarketScalar::Price(m) => Err(finstack_quant_core::Error::Validation(format!(
                "Dividend yield '{}' should be a unitless scalar, got Price({})",
                div_id,
                m.currency()
            ))),
        }
    } else {
        Ok(0.0)
    }
}

pub(crate) fn fair_forward(
    future: &EquityIndexFuture,
    context: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let spot = match context.get_price(&future.spot_id)? {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let disc = context.get_discount(&future.discount_curve_id)?;
    let t = DayCount::Act365F
        .year_fraction(as_of, future.expiry, DayCountContext::default())?
        .max(0.0);
    // Date-based zero rate over [as_of, expiry]: avoids the axis bias of
    // `disc.zero(t)` when curve base != as_of or day counts differ.
    let r = if t > 0.0 {
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            future.expiry,
        )?;
        -df.ln() / t
    } else {
        0.0
    };

    if !future.discrete_dividends.is_empty() {
        let mut pv_dividends = 0.0;
        for (div_date, amount) in &future.discrete_dividends {
            if *div_date <= as_of
                || *div_date > future.expiry
                || !amount.is_finite()
                || *amount <= 0.0
            {
                continue;
            }
            pv_dividends += amount * disc.df_between_dates(as_of, *div_date)?;
        }
        let expiry_df = disc.df_between_dates(as_of, future.expiry)?;
        return Ok((spot - pv_dividends).max(1e-8) / expiry_df);
    }

    let q = resolve_dividend_yield(future, context)?;
    Ok(spot * ((r - q) * t).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Position;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_market() -> MarketContext {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Create flat 5% discount curve
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.9512), (2.0, 0.9048)]) // ~5% rate
            .build()
            .expect("should succeed");

        // Create market context with spot price
        MarketContext::new()
            .insert(discount_curve)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(4500.0))
    }

    fn create_test_future_without_quoted_price() -> EquityIndexFuture {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::equity_index_future::EquityFutureSpecs;

        EquityIndexFuture::builder()
            .id(InstrumentId::new("ES-FAIR"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(2_250_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::June, 20).expect("valid date"))
            .last_trading_date(Date::from_calendar_date(2025, Month::June, 19).expect("valid date"))
            .entry_price_opt(Some(4500.0))
            .position(Position::Long)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    }

    #[test]
    fn test_compute_pv_matches_instrument_value() {
        let future = create_test_future_without_quoted_price();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let via_pricer = compute_pv(&future, &market, as_of).expect("pricer pv");
        let via_instrument = future.value(&market, as_of).expect("instrument pv");

        assert_eq!(via_pricer, via_instrument);
    }

    /// Regression: previously, a missing `entry_price` silently defaulted to
    /// 0.0, booking the full quoted price as P&L. The pricer must now reject.
    #[test]
    fn pricer_errors_when_entry_price_missing_with_quoted_price() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::equity_index_future::EquityFutureSpecs;

        let future = EquityIndexFuture::builder()
            .id(InstrumentId::new("ES-NO-ENTRY"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(2_250_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::June, 20).expect("valid date"))
            .last_trading_date(Date::from_calendar_date(2025, Month::June, 19).expect("valid date"))
            .entry_price_opt(None)
            .quoted_price_opt(Some(4550.0))
            .position(Position::Long)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
            .expect("future should build (entry_price is optional in the data model)");

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let err = compute_pv(&future, &create_test_market(), as_of)
            .expect_err("PV with missing entry_price must fail");
        assert!(
            err.to_string().contains("no entry_price"),
            "error message should explain entry_price requirement: {}",
            err
        );
    }

    /// Same regression — fair-value path (no quoted_price) must also reject.
    #[test]
    fn pricer_errors_when_entry_price_missing_with_fair_value() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::equity_index_future::EquityFutureSpecs;

        let future = EquityIndexFuture::builder()
            .id(InstrumentId::new("ES-NO-ENTRY-FAIR"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(2_250_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::June, 20).expect("valid date"))
            .last_trading_date(Date::from_calendar_date(2025, Month::June, 19).expect("valid date"))
            .entry_price_opt(None)
            .quoted_price_opt(None)
            .position(Position::Long)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
            .expect("future should build");

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let err = compute_pv(&future, &create_test_market(), as_of)
            .expect_err("fair-value PV without entry must fail");
        assert!(err.to_string().contains("no entry_price"));
    }
}
