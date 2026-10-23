//! Tests for market data scalar types.
//!
//! This module covers:
//! - DividendSchedule construction and filtering
//! - ScalarTimeSeries (tested in serde.rs)

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::dividends::DividendSchedule;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_quant_core::money::Money;
use time::Month;

#[test]
fn inflation_lag_display_round_trips_with_from_str() {
    for lag in [
        InflationLag::None,
        InflationLag::Months(3),
        InflationLag::Days(90),
    ] {
        let parsed: InflationLag = lag.to_string().parse().expect("display form parses");
        assert_eq!(parsed, lag);
    }
    assert_eq!(InflationLag::Months(3).to_string(), "3M");
    assert_eq!(InflationLag::None.to_string(), "none");
}

#[test]
fn series_interpolation_parse_error_names_input() {
    let err = "cubic"
        .parse::<SeriesInterpolation>()
        .expect_err("cubic is not supported");
    assert!(err.to_string().contains("cubic"), "{err}");
}

fn jan(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).unwrap()
}

// DividendSchedule Tests

#[test]
fn dividend_schedule_builds_and_filters() {
    let schedule = DividendSchedule::builder("AAPL-DIVS")
        .underlying("AAPL")
        .currency(Currency::USD)
        .cash(
            jan(20),
            Money::new(1.30, Currency::USD).expect("valid money fixture"),
        )
        .cash(
            jan(5),
            Money::new(1.25, Currency::USD).expect("valid money fixture"),
        )
        .cash(
            jan(12),
            Money::new(1.28, Currency::USD).expect("valid money fixture"),
        )
        .build()
        .expect("valid dividend schedule");

    assert!(schedule
        .get_events()
        .windows(2)
        .all(|pair| pair[0].date <= pair[1].date));
}

#[test]
fn dividend_schedule_validate_rejects_negative_cash() {
    let schedule = DividendSchedule::builder("NEG")
        .cash(
            jan(5),
            Money::new(-1.0, Currency::USD).expect("valid money fixture"),
        )
        .build();
    assert!(schedule.is_err());
}

#[test]
fn scalar_time_series_rejects_pre_history_lookups() {
    let series =
        ScalarTimeSeries::new("TEST-SERIES", vec![(jan(10), 10.0), (jan(20), 20.0)], None).unwrap();

    let result = series.value_on(jan(5));
    assert!(
        result.is_err(),
        "queries before the first observation should error instead of leaking the first value"
    );
}

#[test]
fn scalar_time_series_rejects_non_finite_observations() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = ScalarTimeSeries::new("INVALID", vec![(jan(10), value)], None)
            .expect_err("non-finite observations must be rejected");
        assert!(error.to_string().contains("finite"));
    }
}

#[test]
fn inflation_index_rejects_pre_history_lookups() {
    let index = InflationIndex::new(
        "US-CPI",
        vec![(jan(10), 100.0), (jan(20), 101.0)],
        Currency::USD,
    )
    .unwrap();

    let result = index.value_on(jan(5));
    assert!(
        result.is_err(),
        "inflation index lookups before history starts should error"
    );
}

// Moved from phase2_strictness.rs: finite-value deserialization.
mod finite_deserialization {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use serde::de::value::Error as ValueError;
    use serde::de::{self, DeserializeSeed, EnumAccess, IntoDeserializer, VariantAccess, Visitor};
    use serde::Deserialize;

    struct UnitlessScalarDeserializer(f64);

    impl<'de> serde::Deserializer<'de> for UnitlessScalarDeserializer {
        type Error = ValueError;

        fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom("expected MarketScalar enum"))
        }

        fn deserialize_enum<V>(
            self,
            _name: &'static str,
            _variants: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_enum(UnitlessScalarEnumAccess(self.0))
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
            map struct identifier ignored_any
        }
    }

    struct UnitlessScalarEnumAccess(f64);

    impl<'de> EnumAccess<'de> for UnitlessScalarEnumAccess {
        type Error = ValueError;
        type Variant = UnitlessScalarVariantAccess;

        fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
        where
            V: DeserializeSeed<'de>,
        {
            let variant = seed.deserialize("unitless".into_deserializer())?;
            Ok((variant, UnitlessScalarVariantAccess(self.0)))
        }
    }

    struct UnitlessScalarVariantAccess(f64);

    impl<'de> VariantAccess<'de> for UnitlessScalarVariantAccess {
        type Error = ValueError;

        fn unit_variant(self) -> Result<(), Self::Error> {
            Err(de::Error::custom("unitless requires a numeric value"))
        }

        fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            seed.deserialize(self.0.into_deserializer())
        }

        fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom("unitless is a newtype variant"))
        }

        fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom("unitless is a newtype variant"))
        }
    }

    #[test]
    fn market_scalar_shadow_conversion_rejects_every_non_finite_value() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = MarketScalar::deserialize(UnitlessScalarDeserializer(value))
                .expect_err("the serde shadow conversion must reject non-finite values");
            assert!(error.to_string().contains("finite"), "{error}");
        }
    }
}
