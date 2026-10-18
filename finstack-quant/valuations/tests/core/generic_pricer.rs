//! The generic instrument pricer over built-in instruments.

use finstack_quant_valuations::instruments::pricing::GenericInstrumentPricer;
use finstack_quant_valuations::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError,
};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use time::macros::date;

#[test]
fn test_generic_pricer_keys() {
    // Test the new discounting() convenience method
    let bond_pricer =
        GenericInstrumentPricer::<finstack_quant_valuations::instruments::Bond>::discounting(
            InstrumentType::Bond,
        );
    assert_eq!(
        bond_pricer.key(),
        PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
    );

    let deposit_pricer =
        GenericInstrumentPricer::<finstack_quant_valuations::instruments::Deposit>::discounting(
            InstrumentType::Deposit,
        );
    assert_eq!(
        deposit_pricer.key(),
        PricerKey::new(InstrumentType::Deposit, ModelKey::Discounting)
    );
}

#[test]
fn test_generic_instrument_pricer_with_model_key() {
    // Test that GenericInstrumentPricer works with any model key
    let pricer = GenericInstrumentPricer::<finstack_quant_valuations::instruments::Bond>::new(
        InstrumentType::Bond,
        ModelKey::HazardRate,
    );
    assert_eq!(
        pricer.key(),
        PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
    );
}

#[test]
fn generic_pricer_attaches_instrument_and_model_context_to_failures() {
    let bond = finstack_quant_valuations::instruments::Bond::fixed(
        "GENERIC-CTX",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let pricer =
        GenericInstrumentPricer::<finstack_quant_valuations::instruments::Bond>::discounting(
            InstrumentType::Bond,
        );

    let err = pricer
        .price_dyn(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
        .expect_err("missing discount curve must fail");

    match err {
        PricingError::MissingMarketData {
            missing_id,
            context,
        } => {
            assert_eq!(missing_id, "USD-OIS");
            assert_eq!(context.instrument_id.as_deref(), Some("GENERIC-CTX"));
            assert_eq!(context.instrument_type, Some(InstrumentType::Bond));
            assert_eq!(context.model, Some(ModelKey::Discounting));
        }
        other => panic!("expected missing market data, got {other:?}"),
    }
    // The kind survives the lossy conversion into the core error: a host
    // binding maps `NotFound` to `KeyError`, not `RuntimeError`.
    let core: finstack_quant_core::Error = pricer
        .price_dyn(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
        .expect_err("missing discount curve must fail")
        .into();
    assert_eq!(core.kind(), finstack_quant_core::error::ErrorKind::NotFound);
}

#[test]
fn generic_pricer_raw_path_attaches_context_to_failures() {
    let bond = finstack_quant_valuations::instruments::Bond::fixed(
        "GENERIC-RAW-CTX",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let pricer =
        GenericInstrumentPricer::<finstack_quant_valuations::instruments::Bond>::discounting(
            InstrumentType::Bond,
        );

    let err = pricer
        .price_raw_dyn(&bond, &MarketContext::new(), date!(2025 - 01 - 01))
        .expect_err("missing discount curve must fail");

    match err {
        PricingError::MissingMarketData { context, .. } => {
            assert_eq!(context.instrument_id.as_deref(), Some("GENERIC-RAW-CTX"));
            assert_eq!(context.instrument_type, Some(InstrumentType::Bond));
            assert_eq!(context.model, Some(ModelKey::Discounting));
        }
        other => panic!("expected missing market data, got {other:?}"),
    }
}
