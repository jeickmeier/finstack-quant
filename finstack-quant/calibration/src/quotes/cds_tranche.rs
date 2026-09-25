//! CDS tranche market quote schema.

use super::ids::QuoteId;
use super::validate;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::market::conventions::ids::CdsConventionKey;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Market quote for a CDS index tranche.
///
/// CDS tranches represent slices of credit risk on a CDS index, defined by attachment and
/// detachment points. Quotes include upfront payments and running spreads for pricing.
///
/// # Upfront Convention
///
/// `upfront_pct` is expressed as a **decimal fraction** of tranche notional (consistent with
/// `CdsQuote`). For example, `-0.025` means a -2.5% upfront payment. Values with `abs() > 1.0`
/// are rejected by [`CdsTrancheQuote::validate`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
/// use finstack_quant_calibration::quotes::ids::QuoteId;
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::currency::Currency;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let quote = CdsTrancheQuote {
///     id: QuoteId::new("CDX-IG-3-7"),
///     index: "CDX.NA.IG".to_string(),
///     series: 46,
///     attachment: 0.03,
///     detachment: 0.07,
///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
///     upfront_pct: -0.025,
///     coupon_bp: 500.0,
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
/// };
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CdsTrancheQuote {
    /// Unique identifier.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub id: QuoteId,
    /// Index identifier (e.g. CDX.NA.HY).
    pub index: String,
    /// CDS index series number.
    pub series: u16,
    /// Attachment point (decimal, e.g. 0.03).
    pub attachment: f64,
    /// Detachment point (decimal, e.g. 0.07).
    pub detachment: f64,
    /// Maturity date.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: finstack_quant_core::dates::Date,
    /// Upfront payment as a decimal fraction of tranche notional (e.g., -0.025 for -2.5%).
    pub upfront_pct: f64,
    /// Contractual running coupon of the tranche, in basis points.
    pub coupon_bp: f64,
    /// Convention key (currency + doc clause).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub convention: CdsConventionKey,
}

impl CdsTrancheQuote {
    /// Get the unique identifier of the quote.
    ///
    /// # Returns
    ///
    /// A reference to the quote's [`QuoteId`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    /// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// # fn example() -> finstack_quant_core::Result<()> {
    /// let quote = CdsTrancheQuote {
    ///     id: QuoteId::new("CDX-IG-3-7"),
    ///     index: "CDX.NA.IG".to_string(),
    ///     series: 46,
    ///     attachment: 0.03,
    ///     detachment: 0.07,
    ///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
    ///     upfront_pct: -0.025,
    ///     coupon_bp: 500.0,
    ///     convention: CdsConventionKey {
    ///         currency: Currency::USD,
    ///         doc_clause: CdsDocClause::Cr14,
    ///     },
    /// };
    ///
    /// assert_eq!(quote.id().as_str(), "CDX-IG-3-7");
    /// # Ok(())
    /// # }
    /// ```
    pub fn id(&self) -> &QuoteId {
        &self.id
    }

    /// Validate attachment/detachment, running spread, and decimal upfront bounds.
    pub fn validate(&self) -> Result<()> {
        validate::unit_interval(self.attachment, "attachment")?;
        validate::unit_interval(self.detachment, "detachment")?;
        if self.attachment >= self.detachment {
            return Err(Error::Validation(format!(
                "attachment must be less than detachment; got attachment={}, detachment={}",
                self.attachment, self.detachment
            )));
        }
        validate::finite(self.upfront_pct, "upfront_pct")?;
        if self.upfront_pct.abs() > 1.0 {
            return Err(Error::Validation(format!(
                "upfront_pct must satisfy abs(upfront_pct) <= 1; got {}",
                self.upfront_pct
            )));
        }
        validate::positive(self.coupon_bp, "coupon_bp")
    }

    /// Create a new quote with the running spread bumped.
    ///
    /// The upfront percentage remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `bump_decimal` - The bump amount in decimal terms (e.g., `0.0001` for 1 basis point).
    ///   This is converted to basis points internally (multiplied by 10,000).
    ///
    /// # Returns
    ///
    /// A new `CdsTrancheQuote` with the bumped running spread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
    /// use finstack_quant_calibration::quotes::ids::QuoteId;
    /// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// # fn example() -> finstack_quant_core::Result<()> {
    /// let quote = CdsTrancheQuote {
    ///     id: QuoteId::new("CDX-IG-3-7"),
    ///     index: "CDX.NA.IG".to_string(),
    ///     series: 46,
    ///     attachment: 0.03,
    ///     detachment: 0.07,
    ///     maturity: Date::from_calendar_date(2029, time::Month::June, 20).unwrap(),
    ///     upfront_pct: -0.025,
    ///     coupon_bp: 500.0,
    ///     convention: CdsConventionKey {
    ///         currency: Currency::USD,
    ///         doc_clause: CdsDocClause::Cr14,
    ///     },
    /// };
    ///
    /// let bumped = quote.bump_spread_decimal(0.0001);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bump_spread_decimal(&self, bump_decimal: f64) -> Self {
        self.bump_spread_bp(bump_decimal * 10_000.0)
    }

    /// Bump by spread in basis points (e.g., `1.0` = 1bp).
    pub fn bump_spread_bp(&self, bump_bp: f64) -> Self {
        let mut quote = self.clone();
        quote.coupon_bp += bump_bp;
        quote
    }
}
