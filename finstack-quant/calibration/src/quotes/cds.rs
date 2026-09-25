//! CDS market quote schema.

use super::ids::{Pillar, QuoteId};
use super::validate;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::market::conventions::ids::CdsConventionKey;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Standard fixed running coupons for ISDA-style upfront CDS quotes.
///
/// North American conventions (SNAC) use 100bp (IG) and 500bp (HY);
/// European/STEC conventions additionally trade 25bp and 1000bp fixed
/// coupons (ISDA Standard European Contract, Big Bang/Small Bang 2009).
const STANDARD_UPFRONT_RUNNING_COUPONS_BP: [f64; 4] = [25.0, 100.0, 500.0, 1000.0];

fn is_standard_upfront_running_coupon_bp(coupon_bp: f64) -> bool {
    // Running coupons are quoted in whole bp market standards, so an absolute
    // tolerance is intentional here.
    STANDARD_UPFRONT_RUNNING_COUPONS_BP
        .iter()
        .any(|standard| (coupon_bp - standard).abs() <= 1e-9)
}

/// Market quote for credit default swap (CDS) instruments.
///
/// CDS quotes can be specified in two formats:
/// 1. **Par spread**: The spread that makes the CDS have zero present value
/// 2. **Upfront + running**: A fixed upfront payment plus a running spread
///
/// Both formats include recovery rate assumptions and reference entity information.
///
/// # Examples
///
/// Par spread quote:
/// ```rust
/// use finstack_quant_calibration::quotes::cds::CdsQuote;
/// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::currency::Currency;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let quote = CdsQuote::CdsParSpread {
///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
///     entity: "ABC Corp".to_string(),
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
///     pillar: Pillar::Tenor("5Y".parse()?),
///     spread_bp: 150.0,
///     recovery_rate: 0.40,
/// };
/// # Ok(())
/// # }
/// ```
///
/// Upfront quote:
/// ```rust
/// use finstack_quant_calibration::quotes::cds::CdsQuote;
/// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
/// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
/// use finstack_quant_core::currency::Currency;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let quote = CdsQuote::CdsUpfront {
///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
///     entity: "ABC Corp".to_string(),
///     convention: CdsConventionKey {
///         currency: Currency::USD,
///         doc_clause: CdsDocClause::Cr14,
///     },
///     pillar: Pillar::Tenor("5Y".parse()?),
///     coupon_bp: 500.0,
///     upfront_pct: 0.02, // 2% upfront
///     recovery_rate: 0.40,
/// };
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CdsQuote {
    /// Credit Default Swap (par spread).
    CdsParSpread {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Reference entity name.
        entity: String,
        /// Convention key (currency + doc clause).
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: CdsConventionKey,
        /// Maturity pillar.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        pillar: Pillar,
        /// Par spread in basis points (e.g. 100.0).
        spread_bp: f64,
        /// Recovery rate assumption (e.g. 0.40).
        recovery_rate: f64,
    },
    /// Credit Default Swap (upfront + running).
    CdsUpfront {
        /// Unique identifier for the quote.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        id: QuoteId,
        /// Reference entity name.
        entity: String,
        /// Convention key.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        convention: CdsConventionKey,
        /// Maturity pillar.
        #[cfg_attr(feature = "ts_export", ts(type = "string"))]
        pillar: Pillar,
        /// Contractual running coupon in basis points (25.0, 100.0, 500.0 or 1000.0).
        coupon_bp: f64,
        /// Upfront payment percentage of notional (e.g. 0.01 for 1%).
        upfront_pct: f64,
        /// Recovery rate assumption.
        recovery_rate: f64,
    },
}

impl CdsQuote {
    /// Get the unique identifier of the quote.
    ///
    /// # Returns
    ///
    /// A reference to the quote's [`QuoteId`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::cds::CdsQuote;
    /// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
    /// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    /// use finstack_quant_core::currency::Currency;
    ///
    /// # fn example() -> finstack_quant_core::Result<()> {
    /// let quote = CdsQuote::CdsParSpread {
    ///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
    ///     entity: "ABC Corp".to_string(),
    ///     convention: CdsConventionKey {
    ///         currency: Currency::USD,
    ///         doc_clause: CdsDocClause::Cr14,
    ///     },
    ///     pillar: Pillar::Tenor("5Y".parse()?),
    ///     spread_bp: 150.0,
    ///     recovery_rate: 0.40,
    /// };
    ///
    /// assert_eq!(quote.id().as_str(), "CDS-ABC-CORP-5Y");
    /// # Ok(())
    /// # }
    /// ```
    pub fn id(&self) -> &QuoteId {
        match self {
            CdsQuote::CdsParSpread { id, .. } | CdsQuote::CdsUpfront { id, .. } => id,
        }
    }

    /// Create a new quote with the spread bumped.
    ///
    /// For par spread quotes, bumps `spread_bp`. For upfront quotes, bumps `coupon_bp`.
    /// The upfront percentage remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `bump_decimal` - The bump amount in decimal terms (e.g., `0.0001` for 1 basis point).
    ///   This is converted to basis points internally (multiplied by 10,000).
    ///
    /// # Returns
    ///
    /// A new `CdsQuote` with the bumped spread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_calibration::quotes::cds::CdsQuote;
    /// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
    /// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    /// use finstack_quant_core::currency::Currency;
    ///
    /// # fn example() -> finstack_quant_core::Result<()> {
    /// let quote = CdsQuote::CdsParSpread {
    ///     id: QuoteId::new("CDS-ABC-CORP-5Y"),
    ///     entity: "ABC Corp".to_string(),
    ///     convention: CdsConventionKey {
    ///         currency: Currency::USD,
    ///         doc_clause: CdsDocClause::Cr14,
    ///     },
    ///     pillar: Pillar::Tenor("5Y".parse()?),
    ///     spread_bp: 150.0,
    ///     recovery_rate: 0.40,
    /// };
    ///
    /// // Bump by 1 basis point (0.0001 decimal)
    /// let bumped = quote.bump_spread_decimal(0.0001);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bump_spread_decimal(&self, bump_decimal: f64) -> Self {
        let bump_bp = bump_decimal * 10_000.0;
        self.bump_spread_bp(bump_bp)
    }

    /// Bump by spread in basis points (e.g., `1.0` = 1bp).
    pub fn bump_spread_bp(&self, bump_bp: f64) -> Self {
        let mut quote = self.clone();
        match &mut quote {
            Self::CdsParSpread { spread_bp, .. } => *spread_bp += bump_bp,
            Self::CdsUpfront { coupon_bp, .. } => *coupon_bp += bump_bp,
        }
        quote
    }

    /// Validate spreads, recovery, upfront amount, and market-standard running coupons.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::CdsParSpread {
                spread_bp,
                recovery_rate,
                ..
            } => {
                validate::positive(*spread_bp, "spread_bp")?;
                validate::unit_interval(*recovery_rate, "recovery_rate")?;
            }
            Self::CdsUpfront {
                coupon_bp,
                upfront_pct,
                recovery_rate,
                ..
            } => {
                validate::positive(*coupon_bp, "coupon_bp")?;
                validate::finite(*upfront_pct, "upfront_pct")?;
                validate::unit_interval(*recovery_rate, "recovery_rate")?;
            }
        }
        self.validate_market_conventions()
    }

    /// Convention key carried by the quote.
    pub fn convention(&self) -> &CdsConventionKey {
        match self {
            Self::CdsParSpread { convention, .. } | Self::CdsUpfront { convention, .. } => {
                convention
            }
        }
    }

    /// Running coupon, in basis points, of the CDS built from this quote.
    ///
    /// Par-spread quotes return the par spread (the built CDS runs at par).
    /// Upfront quotes return the contractual running coupon.
    pub fn coupon_bp(&self) -> f64 {
        match self {
            CdsQuote::CdsParSpread { spread_bp, .. } => *spread_bp,
            CdsQuote::CdsUpfront { coupon_bp, .. } => *coupon_bp,
        }
    }

    /// Validate market-standard constraints that depend on the quote style.
    ///
    /// ISDA-style upfront quotes are only supported with standard running coupons.
    pub fn validate_market_conventions(&self) -> Result<()> {
        if let CdsQuote::CdsUpfront { coupon_bp, .. } = self {
            if !is_standard_upfront_running_coupon_bp(*coupon_bp) {
                return Err(Error::Validation(format!(
                    "CDS upfront quotes require a standard running coupon of 25bp, 100bp, 500bp or 1000bp; got {}bp",
                    coupon_bp
                )));
            }
        }

        Ok(())
    }
}
