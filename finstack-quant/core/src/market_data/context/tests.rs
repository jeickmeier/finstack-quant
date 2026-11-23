use super::MarketContext;
use crate::InputError;
use crate::currency::Currency;
use crate::money::Money;
use crate::money::fx::{FxConversionPolicy, FxMatrix, SimpleFxProvider};
use std::sync::Arc;
use time::macros::date;

#[test]
fn market_context_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MarketContext>();
}

#[test]
fn fx_required_errors_when_matrix_absent() {
    let ctx = MarketContext::new();
    // `FxMatrix` does not impl Debug, so we can't use `unwrap_err` directly.
    let Err(err) = ctx.fx_required() else {
        panic!("expected fx_required to error on empty context");
    };
    assert!(
        matches!(
            err,
            crate::Error::Input(InputError::NotFound { ref id }) if id == "fx_matrix"
        ),
        "unexpected error: {err:?}",
    );
}

#[test]
fn convert_money_same_currency_is_identity() {
    let ctx = MarketContext::new();
    let amount = Money::from((1_000_i64, Currency::USD));
    let out = ctx
        .convert_money(
            amount,
            Currency::USD,
            date!(2025 - 01 - 15),
            FxConversionPolicy::CashflowDate,
        )
        .expect("same-ccy conversion should not consult FX")
        .amount;
    assert_eq!(out, amount);
}

#[test]
fn convert_money_uses_fx_matrix_rate() {
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
        .expect("valid rate");
    let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider), None));
    let eur = Money::from((1_000_i64, Currency::EUR));
    let usd = ctx
        .convert_money(
            eur,
            Currency::USD,
            date!(2025 - 01 - 15),
            FxConversionPolicy::CashflowDate,
        )
        .expect("EUR->USD should succeed")
        .amount;
    assert_eq!(usd.currency(), Currency::USD);
    assert!((usd.amount() - 1_100.0).abs() < 1e-9);
}

#[test]
fn convert_money_preserves_decimal_amount_precision() {
    use core::str::FromStr;

    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 2.0)])
        .expect("valid rate");
    let ctx = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(provider), None));
    let amount = rust_decimal::Decimal::from_str("10000000000000000.1").expect("valid decimal");
    let eur = Money::from_decimal(amount, Currency::EUR);

    let usd = ctx
        .convert_money(
            eur,
            Currency::USD,
            date!(2025 - 01 - 15),
            FxConversionPolicy::CashflowDate,
        )
        .expect("EUR->USD should succeed")
        .amount;
    assert_eq!(
        usd.amount_decimal(),
        rust_decimal::Decimal::from_str("20000000000000000.2").expect("valid decimal")
    );
}

#[test]
fn convert_money_missing_matrix_returns_not_found() {
    let ctx = MarketContext::new();
    let eur = Money::from((1_000_i64, Currency::EUR));
    let err = ctx
        .convert_money(
            eur,
            Currency::USD,
            date!(2025 - 01 - 15),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap_err();
    assert!(
        matches!(
            err,
            crate::Error::Input(InputError::NotFound { ref id }) if id == "fx_matrix"
        ),
        "unexpected error: {err:?}",
    );
}
