//! Bond market quote schema.

use super::ids::QuoteId;
use crate::market::conventions::ids::BondConventionId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Market quote for bond instruments.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BondQuote {
    /// Fixed-rate bullet bond quoted in clean price (% of par).
    FixedRateBulletCleanPrice {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Settlement / pricing currency of the bond.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        currency: Currency,
        /// Bond issue date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        issue_date: Date,
        /// Bond maturity date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Annual coupon rate in decimal form.
        coupon_rate: f64,
        /// Bond convention identifier.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: BondConventionId,
        /// Quoted clean price as percent of par.
        clean_price_pct: f64,
    },
    /// Fixed-rate bullet bond quoted in Z-spread over a discount curve (decimal).
    FixedRateBulletZSpread {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Settlement / pricing currency of the bond.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        currency: Currency,
        /// Bond issue date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        issue_date: Date,
        /// Bond maturity date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Annual coupon rate in decimal form.
        coupon_rate: f64,
        /// Bond convention identifier.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: BondConventionId,
        /// Z-spread in decimal form (e.g. 0.01 = 100bp).
        z_spread: f64,
    },
    /// Fixed-rate bullet bond quoted in OAS (decimal).
    FixedRateBulletOas {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Settlement / pricing currency of the bond.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        currency: Currency,
        /// Bond issue date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        issue_date: Date,
        /// Bond maturity date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Annual coupon rate in decimal form.
        coupon_rate: f64,
        /// Bond convention identifier.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: BondConventionId,
        /// Option-adjusted spread in decimal form (e.g. 0.005 = 50bp).
        oas: f64,
    },
    /// Fixed-rate bullet bond quoted in yield-to-maturity (decimal).
    FixedRateBulletYtm {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Settlement / pricing currency of the bond.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        currency: Currency,
        /// Bond issue date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        issue_date: Date,
        /// Bond maturity date.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        #[schemars(with = "String")]
        maturity: Date,
        /// Annual coupon rate in decimal form.
        coupon_rate: f64,
        /// Bond convention identifier.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: BondConventionId,
        /// Yield to maturity in decimal form.
        ytm: f64,
    },
}

impl BondQuote {
    /// Get the unique identifier of the quote.
    pub fn id(&self) -> &QuoteId {
        match self {
            BondQuote::FixedRateBulletCleanPrice { id, .. }
            | BondQuote::FixedRateBulletZSpread { id, .. }
            | BondQuote::FixedRateBulletOas { id, .. }
            | BondQuote::FixedRateBulletYtm { id, .. } => id,
        }
    }

    /// Get the primary market value of the quote.
    pub fn value(&self) -> f64 {
        match self {
            BondQuote::FixedRateBulletCleanPrice {
                clean_price_pct, ..
            } => *clean_price_pct,
            BondQuote::FixedRateBulletZSpread { z_spread, .. } => *z_spread,
            BondQuote::FixedRateBulletOas { oas, .. } => *oas,
            BondQuote::FixedRateBulletYtm { ytm, .. } => *ytm,
        }
    }

    /// Bump the primary quoted value by a decimal amount.
    ///
    /// - For clean-price quotes, bumps `clean_price_pct`.
    /// - For YTM quotes, bumps `ytm`.
    /// - For Z-spread quotes, bumps `z_spread`.
    /// - For OAS quotes, bumps `oas`.
    pub fn bump_value_decimal(&self, bump: f64) -> Self {
        match self {
            BondQuote::FixedRateBulletCleanPrice {
                id,
                currency,
                issue_date,
                maturity,
                coupon_rate,
                convention,
                clean_price_pct,
            } => BondQuote::FixedRateBulletCleanPrice {
                id: id.clone(),
                currency: *currency,
                issue_date: *issue_date,
                maturity: *maturity,
                coupon_rate: *coupon_rate,
                convention: convention.clone(),
                clean_price_pct: clean_price_pct + bump,
            },
            BondQuote::FixedRateBulletZSpread {
                id,
                currency,
                issue_date,
                maturity,
                coupon_rate,
                convention,
                z_spread,
            } => BondQuote::FixedRateBulletZSpread {
                id: id.clone(),
                currency: *currency,
                issue_date: *issue_date,
                maturity: *maturity,
                coupon_rate: *coupon_rate,
                convention: convention.clone(),
                z_spread: z_spread + bump,
            },
            BondQuote::FixedRateBulletOas {
                id,
                currency,
                issue_date,
                maturity,
                coupon_rate,
                convention,
                oas,
            } => BondQuote::FixedRateBulletOas {
                id: id.clone(),
                currency: *currency,
                issue_date: *issue_date,
                maturity: *maturity,
                coupon_rate: *coupon_rate,
                convention: convention.clone(),
                oas: oas + bump,
            },
            BondQuote::FixedRateBulletYtm {
                id,
                currency,
                issue_date,
                maturity,
                coupon_rate,
                convention,
                ytm,
            } => BondQuote::FixedRateBulletYtm {
                id: id.clone(),
                currency: *currency,
                issue_date: *issue_date,
                maturity: *maturity,
                coupon_rate: *coupon_rate,
                convention: convention.clone(),
                ytm: ytm + bump,
            },
        }
    }

    /// Bump the primary quoted value by basis points (e.g., `1.0` = 1bp).
    pub fn bump_value_bp(&self, bump_bp: f64) -> Self {
        self.bump_value_decimal(bump_bp / 10_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn sample_clean_price() -> BondQuote {
        BondQuote::FixedRateBulletCleanPrice {
            id: QuoteId::new("UST-5Y"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2029 - 01 - 01),
            coupon_rate: 0.04,
            convention: BondConventionId::new("USD-UST"),
            clean_price_pct: 99.25,
        }
    }

    #[test]
    fn id_and_value_accessors_match_active_variant() {
        let clean = sample_clean_price();
        let z = BondQuote::FixedRateBulletZSpread {
            id: QuoteId::new("CORP-Z"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            z_spread: 0.012,
        };
        let oas = BondQuote::FixedRateBulletOas {
            id: QuoteId::new("CORP-OAS"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            oas: 0.0095,
        };
        let ytm = BondQuote::FixedRateBulletYtm {
            id: QuoteId::new("CORP-YTM"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            ytm: 0.0475,
        };

        assert_eq!(clean.id().as_str(), "UST-5Y");
        assert_eq!(clean.value(), 99.25);
        assert_eq!(z.value(), 0.012);
        assert_eq!(oas.value(), 0.0095);
        assert_eq!(ytm.value(), 0.0475);
    }

    #[test]
    fn decimal_and_bp_bumps_only_change_primary_quote_field() {
        let bumped_price = sample_clean_price().bump_value_decimal(0.5);
        let bumped_ytm = BondQuote::FixedRateBulletYtm {
            id: QuoteId::new("CORP-YTM"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            ytm: 0.0475,
        }
        .bump_value_bp(5.0);

        assert!(matches!(
            bumped_price,
            BondQuote::FixedRateBulletCleanPrice {
                clean_price_pct,
                coupon_rate,
                ..
            } if (clean_price_pct - 99.75).abs() < 1e-12 && (coupon_rate - 0.04).abs() < 1e-12
        ));
        assert!(matches!(
            bumped_ytm,
            BondQuote::FixedRateBulletYtm {
                ytm,
                coupon_rate,
                ..
            } if (ytm - 0.048).abs() < 1e-12 && (coupon_rate - 0.055).abs() < 1e-12
        ));
    }

    #[test]
    fn z_spread_and_oas_bp_bumps_scale_in_decimal_units() {
        let z = BondQuote::FixedRateBulletZSpread {
            id: QuoteId::new("CORP-Z"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            z_spread: 0.012,
        }
        .bump_value_bp(10.0);
        let oas = BondQuote::FixedRateBulletOas {
            id: QuoteId::new("CORP-OAS"),
            currency: Currency::USD,
            issue_date: date!(2024 - 01 - 01),
            maturity: date!(2030 - 01 - 01),
            coupon_rate: 0.055,
            convention: BondConventionId::new("USD-UST"),
            oas: 0.0095,
        }
        .bump_value_bp(-5.0);

        assert!(matches!(
            z,
            BondQuote::FixedRateBulletZSpread { z_spread, .. }
                if (z_spread - 0.013).abs() < 1e-12
        ));
        assert!(matches!(
            oas,
            BondQuote::FixedRateBulletOas { oas, .. }
                if (oas - 0.009).abs() < 1e-12
        ));
    }

    #[test]
    fn serde_roundtrip_preserves_each_bond_quote_variant() {
        let quotes = vec![
            sample_clean_price(),
            BondQuote::FixedRateBulletZSpread {
                id: QuoteId::new("CORP-Z"),
                currency: Currency::USD,
                issue_date: date!(2024 - 01 - 01),
                maturity: date!(2030 - 01 - 01),
                coupon_rate: 0.055,
                convention: BondConventionId::new("USD-UST"),
                z_spread: 0.012,
            },
            BondQuote::FixedRateBulletOas {
                id: QuoteId::new("CORP-OAS"),
                currency: Currency::USD,
                issue_date: date!(2024 - 01 - 01),
                maturity: date!(2030 - 01 - 01),
                coupon_rate: 0.055,
                convention: BondConventionId::new("USD-UST"),
                oas: 0.0095,
            },
            BondQuote::FixedRateBulletYtm {
                id: QuoteId::new("CORP-YTM"),
                currency: Currency::USD,
                issue_date: date!(2024 - 01 - 01),
                maturity: date!(2030 - 01 - 01),
                coupon_rate: 0.055,
                convention: BondConventionId::new("USD-UST"),
                ytm: 0.0475,
            },
        ];

        for quote in quotes {
            let encoded = serde_json::to_string(&quote);
            assert!(encoded.is_ok(), "quote should serialize");
            if let Ok(json) = encoded {
                let decoded = serde_json::from_str::<BondQuote>(&json);
                assert!(decoded.is_ok(), "quote should deserialize");
                if let Ok(roundtrip) = decoded {
                    assert_eq!(roundtrip.id(), quote.id());
                    assert!((roundtrip.value() - quote.value()).abs() < 1e-12);
                }
            }
        }
    }
}
