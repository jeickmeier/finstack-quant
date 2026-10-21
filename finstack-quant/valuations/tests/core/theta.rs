//! Period cash collection over built-in money-market instruments.

use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::metrics::collect_period_cash;
use time::macros::date;

#[test]
fn opening_notional_draw_date_uses_deposit_effective_start() {
    let mut deposit = Deposit::example().expect("example deposit");
    // Saturday start rolls to Monday under ModifiedFollowing on the NYSE calendar.
    deposit.start_date = date!(2024 - 01 - 06);
    deposit.calendar_id = Some("nyse".into());
    let expected = deposit.effective_start_date().expect("effective start");
    assert_eq!(expected, date!(2024 - 01 - 08));
    assert_eq!(
        finstack_quant_valuations::instruments::Instrument::notional_draw_date(&deposit),
        Some(expected)
    );
}

#[test]
fn money_market_maturity_separates_principal_from_interest_income() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, Repo};
    use finstack_quant_valuations::instruments::Instrument;

    let principal = Money::from((1_000_000_i64, Currency::USD));
    let start = date!(2025 - 01 - 06);
    let maturity = date!(2025 - 04 - 07);
    let market = MarketContext::new();
    for rate in [0.0, 0.05] {
        let deposit = Deposit::builder()
            .id("DEPOSIT-INCOME".into())
            .notional(principal)
            .start_date(start)
            .maturity(maturity)
            .fixed_rate_opt(Some(rust_decimal::Decimal::try_from(rate).expect("rate")))
            .day_count(DayCount::Act360)
            .discount_curve_id("USD-OIS".into())
            .build()
            .expect("deposit");
        let repo = Repo::term(
            "REPO-INCOME",
            principal,
            CollateralSpec::new("BOND", 100.0, "BOND-PX"),
            rate,
            start,
            maturity,
            "USD-OIS",
        )
        .expect("repo");
        let expected_interest = principal.amount() * rate * (91.0 / 360.0);
        for instrument in [&deposit as &dyn Instrument, &repo as &dyn Instrument] {
            let cash = collect_period_cash(
                instrument,
                &market,
                date!(2025 - 04 - 06),
                date!(2025 - 04 - 08),
                Currency::USD,
            )
            .expect("maturity cash");
            assert!((cash.income.amount() - expected_interest).abs() < 1e-8);
            assert_eq!(
                cash.total.checked_sub(cash.income).expect("principal"),
                principal
            );
            assert!((cash.total.amount() - principal.amount() - expected_interest).abs() < 1e-8);
            if rate == 0.0 {
                assert_eq!(cash.income.amount(), 0.0);
            }
        }
    }
}
