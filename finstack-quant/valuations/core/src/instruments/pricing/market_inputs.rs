//! Market-scalar input lookups shared across instrument pricers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::{context::MarketContext, scalars::MarketScalar};

/// Read a scalar price amount without discarding an attached currency.
///
/// # Arguments
///
/// * `scalar` - Market scalar quote read from `MarketContext::get_price`.
/// * `expected_currency` - Settlement or instrument currency required when
///   `scalar` is currency-denominated.
pub fn scalar_price_amount(
    scalar: &MarketScalar,
    expected_currency: Currency,
) -> finstack_quant_core::Result<f64> {
    if let Some(value) = scalar.get_unitless() {
        return Ok(value);
    }
    if let Some(price) = scalar.get_price() {
        if price.currency() == expected_currency {
            return Ok(price.amount());
        }
        return Err(finstack_quant_core::Error::CurrencyMismatch {
            expected: expected_currency,
            actual: price.currency(),
        });
    }
    Err(finstack_quant_core::Error::Validation(
        "Unsupported market scalar variant".into(),
    ))
}

/// Resolve an optional dividend-yield scalar from the market context.
///
/// Returns `0.0` only when no dividend yield ID is configured. If an ID is
/// configured, missing or wrongly-typed market data is treated as a validation
/// error rather than silently assuming zero carry.
///
/// # Arguments
///
/// * `market` - Market data context queried when `div_yield_id` is `Some`.
/// * `div_yield_id` - Optional identifier of the unitless dividend-yield scalar.
pub fn resolve_optional_dividend_yield(
    market: &MarketContext,
    div_yield_id: Option<&finstack_quant_core::types::PriceId>,
) -> finstack_quant_core::Result<f64> {
    let Some(div_id) = div_yield_id else {
        return Ok(0.0);
    };

    let scalar = market.get_price(div_id.as_str()).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "Failed to fetch dividend yield '{}': {}",
            div_id, e
        ))
    })?;

    if let Some(v) = scalar.get_unitless() {
        if !v.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Dividend yield '{}' must be finite, got {v}",
                div_id
            )));
        }
        Ok(v)
    } else if let Some(m) = scalar.get_price() {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar, got Price({})",
            div_id,
            m.currency()
        )))
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar",
            div_id
        )))
    }
}

/// Read a required unitless market scalar, erroring when it is missing or typed.
///
/// The `model` argument is purely diagnostic and appears in the error
/// message (e.g. `"Heston"`, `"rough Bergomi"`).
///
/// # Arguments
///
/// * `market` - Market context queried for `key`.
/// * `key` - Identifier of the required unitless market scalar.
/// * `model` - Model name surfaced in missing-input and type-mismatch diagnostics.
pub fn get_unitless_scalar_strict(
    market: &MarketContext,
    key: &str,
    model: &str,
) -> finstack_quant_core::Result<f64> {
    match market.get_price(key) {
        Ok(scalar) => {
            let v = scalar.get_unitless().ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "{model} parameter '{key}' must be a unitless market scalar, got {scalar:?}"
                ))
            })?;
            if !v.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{model} parameter '{key}' must be finite, got {v}"
                )));
            }
            Ok(v)
        }
        Err(_) => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: format!("{key} (required by {model})"),
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::Money;

    #[test]
    fn configured_dividend_yield_must_exist() {
        let market = MarketContext::new();
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_quant_core::types::PriceId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(
            err.as_deref()
                .is_some_and(|msg| msg.contains("Failed to fetch dividend yield"))
        );
    }

    #[test]
    fn configured_dividend_yield_must_be_unitless() {
        let market = MarketContext::new().insert_price(
            "DIV",
            finstack_quant_core::market_data::scalars::MarketScalar::price(Money::from((
                1_i64,
                Currency::USD,
            ))),
        );
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_quant_core::types::PriceId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(
            err.as_deref()
                .is_some_and(|msg| msg.contains("should be a unitless scalar"))
        );
    }
}
