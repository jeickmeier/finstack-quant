//! Market inputs implementation used by the convertible subsystem.
//!
use super::ConvertibleBond;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::{Error, Result};

/// Resolve the continuous dividend yield (decimal) from `div_yield_id`.
///
/// `None` means a zero dividend yield. A configured id must resolve to a
/// unitless scalar; a missing scalar is an error, never a silent zero.
pub(super) fn resolve_dividend_yield(ctx: &MarketContext, bond: &ConvertibleBond) -> Result<f64> {
    let Some(div_yield_id) = &bond.div_yield_id else {
        return Ok(0.0);
    };
    match ctx.get_price(div_yield_id)? {
        MarketScalar::Unitless(value) => Ok(*value),
        MarketScalar::Price(_) => Err(Error::Validation(format!(
            "ConvertibleBond '{}' div_yield_id '{}' must be a unitless scalar",
            bond.id, div_yield_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dividend_yield_is_zero_without_an_id_and_required_with_one() {
        let mut bond = ConvertibleBond::example().expect("example");
        // A `{spot_id}-DIVYIELD` scalar is never picked up implicitly.
        let market = MarketContext::new().insert_price(
            "TECH-DIVYIELD",
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(0.03),
        );
        bond.div_yield_id = None;
        assert_eq!(resolve_dividend_yield(&market, &bond).expect("zero"), 0.0);

        bond.div_yield_id = Some("TECH-DIVYIELD".into());
        assert_eq!(
            resolve_dividend_yield(&market, &bond).expect("typed id"),
            0.03
        );

        bond.div_yield_id = Some("MISSING-DIVYIELD".into());
        assert!(resolve_dividend_yield(&market, &bond).is_err());
    }
}
