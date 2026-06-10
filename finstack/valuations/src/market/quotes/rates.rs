//! Interest rate market quote schema.

use super::ids::{Pillar, QuoteId};
use crate::market::conventions::ids::IrFutureContractId;
use finstack_core::dates::Date;
use finstack_core::types::{CurveId, IndexId};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Market quote for interest rate instruments.
///
/// This enum represents all supported interest rate quote types: deposits, forward rate agreements
/// (FRAs), interest rate futures, and interest rate swaps. Each variant includes the necessary
/// identifiers, pillars, and market values for instrument construction.
///
/// # Examples
///
/// Deposit quote:
/// ```rust
/// use finstack_valuations::market::quotes::rates::RateQuote;
/// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_core::types::IndexId;
///
/// # fn example() -> finstack_core::Result<()> {
/// let quote = RateQuote::Deposit {
///     id: QuoteId::new("USD-SOFR-DEP-1M"),
///     index: IndexId::new("USD-SOFR-1M"),
///     pillar: Pillar::Tenor("1M".parse()?),
///     rate: 0.0525,
/// };
/// # Ok(())
/// # }
/// ```
///
/// Swap quote:
/// ```rust
/// use finstack_valuations::market::quotes::rates::RateQuote;
/// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_core::types::IndexId;
///
/// # fn example() -> finstack_core::Result<()> {
/// let quote = RateQuote::Swap {
///     id: QuoteId::new("USD-OIS-SWAP-5Y"),
///     index: IndexId::new("USD-SOFR-OIS"),
///     pillar: Pillar::Tenor("5Y".parse()?),
///     rate: 0.0450,
///     spread_decimal: None,
/// };
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RateQuote {
    /// Money market deposit rate.
    Deposit {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Rate index identifier (e.g. "USD-SOFR-3M").
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        index: IndexId,
        /// Maturity pillar (e.g. Tenor("3M") or Date("2024-01-01")).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        pillar: Pillar,
        /// Rate value (decimal).
        rate: f64,
    },
    /// Forward Rate Agreement.
    Fra {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Rate index identifier.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        index: IndexId,
        /// Start date pillar.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        start: Pillar,
        /// End date pillar.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        end: Pillar,
        /// Rate value (decimal).
        rate: f64,
    },
    /// Interest Rate Future (price).
    Futures {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Future contract identifier (e.g. "CME:SR3").
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        contract: IrFutureContractId,
        /// Expiry date of the future.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        expiry: Date,
        /// Price of the future (e.g. 98.50).
        price: f64,
        /// Optional convexity adjustment (rate, decimal).
        ///
        /// This fixed value is currently required for calibration. If omitted,
        /// futures-quote calibration will fail closed rather than silently assume
        /// zero or attempt an unwired dynamic lookup.
        #[serde(default)]
        convexity_adjustment: Option<f64>,
        /// Optional volatility surface identifier for dynamic convexity adjustment.
        ///
        /// Reserved for future dynamic convexity support. Until that wiring exists,
        /// providing `vol_surface_id` without `convexity_adjustment` is treated as
        /// an invalid quote shape during calibration.
        #[serde(default)]
        #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
        vol_surface_id: Option<CurveId>,
    },
    /// Interest Rate Swap (par rate).
    Swap {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Rate index identifier (floating leg).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        index: IndexId,
        /// Maturity pillar of the swap.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        pillar: Pillar,
        /// Fixed rate (decimal) making the swap PV=0.
        rate: f64,
        /// Optional spread over the index in decimal format (e.g., 0.0010 for 10 basis points).
        ///
        /// This spread is added to the floating leg rate. The value is in decimal format
        /// and will be converted to basis points internally (multiplied by 10,000).
        #[serde(default)]
        spread_decimal: Option<f64>,
    },
}

impl RateQuote {
    /// Get the unique identifier of the quote.
    ///
    /// # Returns
    ///
    /// A reference to the quote's [`QuoteId`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::market::quotes::rates::RateQuote;
    /// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
    /// use finstack_core::types::IndexId;
    ///
    /// # fn example() -> finstack_core::Result<()> {
    /// let quote = RateQuote::Deposit {
    ///     id: QuoteId::new("USD-SOFR-DEP-1M"),
    ///     index: IndexId::new("USD-SOFR-1M"),
    ///     pillar: Pillar::Tenor("1M".parse()?),
    ///     rate: 0.0525,
    /// };
    ///
    /// assert_eq!(quote.id().as_str(), "USD-SOFR-DEP-1M");
    /// # Ok(())
    /// # }
    /// ```
    pub fn id(&self) -> &QuoteId {
        match self {
            RateQuote::Deposit { id, .. } => id,
            RateQuote::Fra { id, .. } => id,
            RateQuote::Futures { id, .. } => id,
            RateQuote::Swap { id, .. } => id,
        }
    }

    /// Get the resolved value (rate or price) of the quote.
    ///
    /// # Returns
    ///
    /// For deposit, FRA, and swap quotes: the rate value (decimal).
    /// For futures quotes: the price value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::market::quotes::rates::RateQuote;
    /// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
    /// use finstack_core::types::IndexId;
    ///
    /// # fn example() -> finstack_core::Result<()> {
    /// let quote = RateQuote::Deposit {
    ///     id: QuoteId::new("USD-SOFR-DEP-1M"),
    ///     index: IndexId::new("USD-SOFR-1M"),
    ///     pillar: Pillar::Tenor("1M".parse()?),
    ///     rate: 0.0525,
    /// };
    ///
    /// assert_eq!(quote.value(), 0.0525);
    /// # Ok(())
    /// # }
    /// ```
    pub fn value(&self) -> f64 {
        match self {
            RateQuote::Deposit { rate, .. } => *rate,
            RateQuote::Fra { rate, .. } => *rate,
            RateQuote::Futures { price, .. } => *price,
            RateQuote::Swap { rate, .. } => *rate,
        }
    }

    /// Create a new quote with the underlying *rate* bumped by `rate_bump`.
    ///
    /// For rates (deposit, FRA, swap), `rate_bump` is added to the rate (in decimal
    /// terms, e.g., `0.0001` for 1 basis point). For futures, the price convention
    /// is `price = 100·(1 − rate)`, so a `+rate_bump` rate shock *subtracts*
    /// `100·rate_bump` from the price (e.g. +1bp rate → price −0.01).
    ///
    /// # Arguments
    ///
    /// * `rate_bump` - The decimal rate shock applied to the quote's underlying rate
    ///
    /// # Returns
    ///
    /// A new `RateQuote` with the bumped value.
    ///
    /// # Examples
    ///
    /// Bumping a deposit rate:
    /// ```rust
    /// use finstack_valuations::market::quotes::rates::RateQuote;
    /// use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
    /// use finstack_core::types::IndexId;
    ///
    /// # fn example() -> finstack_core::Result<()> {
    /// let quote = RateQuote::Deposit {
    ///     id: QuoteId::new("USD-SOFR-DEP-1M"),
    ///     index: IndexId::new("USD-SOFR-1M"),
    ///     pillar: Pillar::Tenor("1M".parse()?),
    ///     rate: 0.0525,
    /// };
    ///
    /// // Bump by 1 basis point (0.0001)
    /// let bumped = quote.bump_rate_decimal(0.0001);
    /// assert_eq!(bumped.value(), 0.0526);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bump_rate_decimal(&self, rate_bump: f64) -> Self {
        match self {
            RateQuote::Deposit {
                id,
                index,
                pillar,
                rate,
            } => RateQuote::Deposit {
                id: id.clone(),
                index: index.clone(),
                pillar: pillar.clone(),
                rate: rate + rate_bump,
            },
            RateQuote::Fra {
                id,
                index,
                start,
                end,
                rate,
            } => RateQuote::Fra {
                id: id.clone(),
                index: index.clone(),
                start: start.clone(),
                end: end.clone(),
                rate: rate + rate_bump,
            },
            RateQuote::Futures {
                id,
                contract,
                expiry,
                price,
                convexity_adjustment,
                vol_surface_id,
            } => RateQuote::Futures {
                id: id.clone(),
                contract: contract.clone(),
                expiry: *expiry,
                // price = 100·(1 − rate): a +rate bump lowers the price 100×.
                price: price - rate_bump * 100.0,
                convexity_adjustment: *convexity_adjustment,
                vol_surface_id: vol_surface_id.clone(),
            },
            RateQuote::Swap {
                id,
                index,
                pillar,
                rate,
                spread_decimal,
            } => RateQuote::Swap {
                id: id.clone(),
                index: index.clone(),
                pillar: pillar.clone(),
                rate: rate + rate_bump,
                spread_decimal: *spread_decimal,
            },
        }
    }

    /// Bump the quote by basis-point units (e.g., `1.0` = 1bp).
    pub fn bump_rate_bp(&self, bump_bp: f64) -> Self {
        self.bump_rate_decimal(bump_bp / 10_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that spread_decimal field works correctly with programmatic API
    #[test]
    fn test_swap_spread_decimal_programmatic_api() {
        let quote = RateQuote::Swap {
            id: QuoteId::new("TEST-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(0.0010), // 10bp in decimal
        };

        match quote {
            RateQuote::Swap { spread_decimal, .. } => {
                assert_eq!(spread_decimal, Some(0.0010));
            }
            _ => panic!("Expected Swap variant"),
        }
    }

    /// Test that spread_decimal serializes and deserializes correctly
    #[test]
    fn test_swap_spread_serde_new_field() {
        let json = r#"{
            "type": "swap",
            "id": "TEST-SWAP-5Y",
            "index": "USD-SOFR-OIS",
            "pillar": {"tenor": {"count": 5, "unit": "years"}},
            "rate": 0.0450,
            "spread_decimal": 0.0010
        }"#;

        let quote: RateQuote = serde_json::from_str(json).expect("Failed to deserialize");

        match quote {
            RateQuote::Swap { spread_decimal, .. } => {
                assert_eq!(spread_decimal, Some(0.0010));
            }
            _ => panic!("Expected Swap variant"),
        }
    }

    /// Old "spread" field should be rejected (use "spread_decimal")
    #[test]
    fn test_swap_spread_serde_rejects_legacy_field() {
        let json = r#"{
            "type": "swap",
            "id": "TEST-SWAP-5Y",
            "index": "USD-SOFR-OIS",
            "pillar": {"tenor": {"count": 5, "unit": "years"}},
            "rate": 0.0450,
            "spread": 0.0010
        }"#;

        let result: Result<RateQuote, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Legacy 'spread' field should be rejected");
    }

    /// Test that spread_decimal serializes using new field name
    #[test]
    fn test_swap_spread_serialization() {
        let quote = RateQuote::Swap {
            id: QuoteId::new("TEST-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(0.0010),
        };

        let json = serde_json::to_string(&quote).expect("Failed to serialize");
        println!("Serialized JSON: {}", json);

        // Should use new field name "spread_decimal" in output
        assert!(
            json.contains("spread_decimal"),
            "Serialized JSON should use 'spread_decimal' field name"
        );
        assert!(
            !json.contains("\"spread\":"),
            "Serialized JSON should not use old 'spread' field name (except in spread_decimal)"
        );

        // Test round-trip: deserialize and verify
        let roundtrip: RateQuote = serde_json::from_str(&json).expect("Failed to deserialize");
        match roundtrip {
            RateQuote::Swap { spread_decimal, .. } => {
                assert_eq!(spread_decimal, Some(0.0010));
            }
            _ => panic!("Expected Swap variant"),
        }
    }

    /// Test that None spread_decimal works correctly
    #[test]
    fn test_swap_no_spread() {
        let quote = RateQuote::Swap {
            id: QuoteId::new("TEST-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: None,
        };

        match quote {
            RateQuote::Swap { spread_decimal, .. } => {
                assert_eq!(spread_decimal, None);
            }
            _ => panic!("Expected Swap variant"),
        }

        // Test JSON without spread field
        let json = r#"{
            "type": "swap",
            "id": "TEST-SWAP-5Y",
            "index": "USD-SOFR-OIS",
            "pillar": {"tenor": {"count": 5, "unit": "years"}},
            "rate": 0.0450
        }"#;

        let quote: RateQuote =
            serde_json::from_str(json).expect("Failed to deserialize without spread");
        match quote {
            RateQuote::Swap { spread_decimal, .. } => {
                assert_eq!(spread_decimal, None);
            }
            _ => panic!("Expected Swap variant"),
        }
    }

    /// Test that bumping a swap preserves the spread_decimal
    #[test]
    fn test_swap_bump_preserves_spread() {
        let quote = RateQuote::Swap {
            id: QuoteId::new("TEST-SWAP-5Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(finstack_core::dates::Tenor::new(
                5,
                finstack_core::dates::TenorUnit::Years,
            )),
            rate: 0.0450,
            spread_decimal: Some(0.0010),
        };

        let bumped = quote.bump_rate_decimal(0.0001); // Bump by 1bp

        match bumped {
            RateQuote::Swap {
                rate,
                spread_decimal,
                ..
            } => {
                assert_eq!(rate, 0.0451); // rate bumped
                assert_eq!(spread_decimal, Some(0.0010)); // spread unchanged
            }
            _ => panic!("Expected Swap variant"),
        }
    }

    /// A +1bp *rate* bump must lower a futures price by 0.01
    /// (price = 100·(1 − rate)). Regression for the bug where the decimal
    /// rate bump was added to the price verbatim (wrong sign, 1/100 scale).
    #[test]
    fn test_futures_bump_moves_rate_not_price() {
        let quote = RateQuote::Futures {
            id: QuoteId::new("TEST-SR3-M6"),
            contract: IrFutureContractId::new("CME:SR3"),
            expiry: Date::from_calendar_date(2026, finstack_core::dates::Month::June, 17)
                .expect("valid date"),
            price: 96.00,
            convexity_adjustment: Some(0.0),
            vol_surface_id: None,
        };

        let bumped = quote.bump_rate_bp(1.0);
        assert!(
            (bumped.value() - 95.99).abs() < 1e-12,
            "+1bp rate bump should give price 95.99, got {}",
            bumped.value()
        );

        let bumped_down = quote.bump_rate_bp(-1.0);
        assert!(
            (bumped_down.value() - 96.01).abs() < 1e-12,
            "-1bp rate bump should give price 96.01, got {}",
            bumped_down.value()
        );
    }
}
