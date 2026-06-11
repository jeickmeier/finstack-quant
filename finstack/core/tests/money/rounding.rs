use finstack_core::config::{CurrencyScalePolicy, FinstackConfig, RoundingMode};
use finstack_core::currency::Currency;
use finstack_core::money::Money;

#[test]
fn money_display_respects_output_scale() {
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = RoundingMode::AwayFromZero;
    // Keep ingest high so display rounding is the observable effect
    cfg.rounding.ingest_scale = CurrencyScalePolicy {
        overrides: Default::default(),
    };
    cfg.rounding.output_scale = CurrencyScalePolicy {
        overrides: Default::default(),
    };
    let m = Money::new_with_config(1.23456, Currency::USD, &cfg);
    // default USD decimals is 2 by ISO; override output to 3 for the test
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = RoundingMode::AwayFromZero;
    cfg.rounding.ingest_scale = CurrencyScalePolicy {
        overrides: Default::default(),
    };
    cfg.rounding.output_scale = CurrencyScalePolicy {
        overrides: std::collections::BTreeMap::from([(Currency::USD, 3)]),
    };
    let s = m.format_with_config(&cfg);
    assert_eq!(s, "USD 1.235");
}

#[test]
fn money_display_matches_format_bankers_rounding() {
    // Regression: `Display` used a raw `{:.prec$}` on the Decimal, which
    // truncates (10.006 -> "10.00") while `format` banker's-rounds ("10.01").
    // Display must agree with `format(currency_decimals, true)` exactly.
    for amount in [10.005, 10.006, 99.9] {
        let usd = Money::new(amount, Currency::USD);
        assert_eq!(
            format!("{usd}"),
            usd.format(usize::from(Currency::USD.decimals()), true),
            "Display and format() must agree for USD {amount}"
        );
    }
    // Concrete rounded values (not truncated).
    assert_eq!(
        format!("{}", Money::new(10.006, Currency::USD)),
        "USD 10.01"
    );
    // Updated per the 2026-06-09 core quant review (user decision): f64
    // ingestion now uses shortest round-trip `Decimal::from_f64`, so `10.005`
    // ingests as exactly 10.005 — a true half tie — and banker's rounding
    // takes it to the even digit, 10.00. (Under the old `from_f64_retain`
    // it ingested as 10.0050000000000007..., which rounded up to 10.01.)
    assert_eq!(
        format!("{}", Money::new(10.005, Currency::USD)),
        "USD 10.00"
    );

    // 0-decimal currency: 99.9 JPY must banker's-round up to 100, not truncate to 99.
    let jpy = Money::new(99.9, Currency::JPY);
    assert_eq!(format!("{jpy}"), "JPY 100");
    assert_eq!(
        format!("{jpy}"),
        jpy.format(usize::from(Currency::JPY.decimals()), true)
    );
}

#[test]
fn money_f64_ingestion_uses_shortest_round_trip() {
    // 2026-06-09 core quant review (user decision): f64 ingestion uses
    // `Decimal::from_f64` (shortest round-trip) instead of
    // `from_f64_retain`, so the classic IEEE artifact disappears:
    // 0.1 + 0.2 == 0.3 exactly in the Decimal store.
    let sum = Money::new(0.1, Currency::USD)
        .checked_add(Money::new(0.2, Currency::USD))
        .unwrap();
    assert_eq!(sum, Money::new(0.3, Currency::USD));

    // Wire format carries the shortest decimal, not 28 noise digits.
    let json = serde_json::to_string(&Money::new(0.1, Currency::USD)).unwrap();
    assert!(
        json.contains("\"0.1\""),
        "Money(0.1) must serialize its amount as \"0.1\", got {json}"
    );
}

#[test]
fn money_integer_tuple_conversion_is_exact_above_2_pow_53() {
    // 2026-06-09 core quant review (minor): `From<(i64|u64, Currency)>` must
    // route through `Decimal::from` (exact), not `as f64`, which cannot
    // represent 2^53 + 1 and would silently round it to 2^53.
    let v: i64 = (1_i64 << 53) + 1; // 9_007_199_254_740_993
    let m = Money::from((v, Currency::USD));
    let exact =
        Money::from_decimal(rust_decimal::Decimal::from(v), Currency::USD).expect("in range");
    assert_eq!(m, exact);

    let u: u64 = (1_u64 << 53) + 1;
    let m = Money::from((u, Currency::USD));
    let exact =
        Money::from_decimal(rust_decimal::Decimal::from(u), Currency::USD).expect("in range");
    assert_eq!(m, exact);
}
