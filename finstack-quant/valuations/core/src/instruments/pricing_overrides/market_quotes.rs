//! Market-quote overrides (prices, implied volatilities, spreads, premiums).

use super::check_finite_fields;

/// Overrides for market-quoted values (prices, vols, spreads, upfront payments).
///
/// # Price-driving fields
///
/// The following fields, when set, override the model PV returned by
/// [`Instrument::base_value`](crate::instruments::Instrument::base_value)
/// for bonds. At most one may be set at a time — [`Self::validate`] enforces this.
/// Precedence (applied top-to-bottom inside `Bond::base_value`):
///
/// 1. `quoted_dirty_price_currency` — currency units (bond native currency)
/// 2. `quoted_clean_price_pct` — percentage of par
/// 3. `quoted_ytm` — decimal YTM (e.g. `0.055` = 5.5%)
/// 4. `quoted_ytw` — decimal yield-to-worst
/// 5. `quoted_z_spread` — decimal Z-spread
/// 6. `quoted_oas` — decimal OAS
/// 7. `quoted_discount_margin` — decimal DM (FRNs)
/// 8. `quoted_i_spread` — decimal I-spread
/// 9. `quoted_asw_market` — decimal ASW (market convention)
/// 10. `quoted_japanese_simple_yield` — decimal Tokyo simple yield (単利)
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MarketQuoteOverrides {
    /// Quoted clean price in percent of par (e.g., `99.5` = 99.5% of par).
    ///
    /// Inflation-linked bonds quote this per 100 of real (unindexed) face.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_clean_price_pct: Option<f64>,

    /// Quoted dirty price in the bond's currency units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_dirty_price_currency: Option<f64>,

    /// Quoted yield-to-maturity in decimal (e.g., `0.055` = 5.5%).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_ytm: Option<f64>,

    /// Quoted yield-to-worst in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_ytw: Option<f64>,

    /// Quoted Z-spread in decimal (e.g., `0.0125` = 125bp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_z_spread: Option<f64>,

    /// Quoted OAS (option-adjusted spread) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_oas: Option<f64>,

    /// Quoted discount margin (for FRNs) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_discount_margin: Option<f64>,

    /// Quoted I-spread in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_i_spread: Option<f64>,

    /// Quoted asset-swap spread (market convention) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_asw_market: Option<f64>,

    /// Quoted Japanese simple yield (単利) in decimal.
    ///
    /// Seeds a JGB from the Tokyo quoted yield without touching Street
    /// [`Self::quoted_ytm`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_japanese_simple_yield: Option<f64>,

    /// Implied volatility (overrides vol surface). When set on surface-driven
    /// pricers, it is used as a flat σ across tenor and strike. Options on
    /// futures read it as their only volatility input: decimal lognormal for
    /// Black-76, futures-price points per √year for the normal model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implied_volatility: Option<f64>,

    /// Observed option premium: the total trade PV in the instrument currency
    /// (notional, contract multiplier and position included).
    ///
    /// This is the target every `ImpliedVol` calculator inverts; it does not
    /// drive PV and is not one of the mutually exclusive price-driving fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_premium: Option<f64>,

    /// CreditDefaultSwap clean par-spread quote in basis points.
    ///
    /// Used only by CreditDefaultSwap risk replay, where it replaces the
    /// matching contractual hazard-curve pillar. It does not drive PV, and no
    /// other instrument (CdsIndex included) reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cds_quote_bp: Option<f64>,
}

impl MarketQuoteOverrides {
    /// Return whether no market quote override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Return the observed option premium an `ImpliedVol` calculator inverts.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming
    /// `instrument_pricing_overrides.market_quotes.quoted_premium` when it is
    /// unset or not finite.
    #[doc(hidden)]
    pub fn required_quoted_premium(&self) -> finstack_quant_core::Result<f64> {
        match self.quoted_premium {
            Some(premium) if premium.is_finite() => Ok(premium),
            Some(premium) => Err(finstack_quant_core::Error::Validation(format!(
                "instrument_pricing_overrides.market_quotes.quoted_premium must be finite, \
                 got {premium}"
            ))),
            None => Err(finstack_quant_core::Error::Validation(
                "implied volatility requires the observed option premium in \
                 instrument_pricing_overrides.market_quotes.quoted_premium"
                    .to_string(),
            )),
        }
    }

    /// The price-driving fields and their values, by wire name, in precedence order.
    fn price_drivers(&self) -> [(&'static str, Option<f64>); 10] {
        [
            ("quoted_clean_price_pct", self.quoted_clean_price_pct),
            (
                "quoted_dirty_price_currency",
                self.quoted_dirty_price_currency,
            ),
            ("quoted_ytm", self.quoted_ytm),
            ("quoted_ytw", self.quoted_ytw),
            ("quoted_z_spread", self.quoted_z_spread),
            ("quoted_oas", self.quoted_oas),
            ("quoted_discount_margin", self.quoted_discount_margin),
            ("quoted_i_spread", self.quoted_i_spread),
            ("quoted_asw_market", self.quoted_asw_market),
            (
                "quoted_japanese_simple_yield",
                self.quoted_japanese_simple_yield,
            ),
        ]
    }

    /// Wire names of the mutually exclusive price-driving fields.
    ///
    /// [`Self::validate`] rejects more than one of these being set, so a host
    /// editor can clear the others when one is entered.
    ///
    /// # Returns
    ///
    /// The field names as they appear on the wire, in precedence order.
    pub fn price_driver_fields() -> Vec<&'static str> {
        Self::default()
            .price_drivers()
            .iter()
            .map(|(name, _)| *name)
            .collect()
    }

    /// Return the number of price-driving fields that are currently set.
    pub(super) fn price_driver_count(&self) -> usize {
        self.price_drivers()
            .iter()
            .filter(|(_, value)| value.is_some())
            .count()
    }

    /// Whether any price-driving quote other than `quoted_z_spread` is set.
    ///
    /// Used by scenario spread-shock routing: a shock composes additively with
    /// a quoted Z-spread, but is ambiguous against price-pinning quotes
    /// (clean/dirty price, YTM/YTW, OAS, DM, I-spread, ASW).
    #[doc(hidden)]
    pub fn has_non_z_price_driver(&self) -> bool {
        self.price_driver_count() > usize::from(self.quoted_z_spread.is_some())
    }

    /// Whether any market quote field should drive bond quote-date economics.
    ///
    /// Bond market quotes are interpreted at the quote date (settlement date
    /// when a settlement convention is present), so accrued interest and
    /// clean/dirty price relationships must use the same date anchor whenever
    /// one of these fields is set.
    #[doc(hidden)]
    pub fn has_price_driver(&self) -> bool {
        self.price_driver_count() > 0
    }

    /// Clear every quote field that can replace model value.
    #[doc(hidden)]
    pub fn clear_price_drivers(&mut self) {
        self.quoted_clean_price_pct = None;
        self.quoted_dirty_price_currency = None;
        self.quoted_ytm = None;
        self.quoted_ytw = None;
        self.quoted_z_spread = None;
        self.quoted_oas = None;
        self.quoted_discount_margin = None;
        self.quoted_i_spread = None;
        self.quoted_asw_market = None;
        self.quoted_japanese_simple_yield = None;
    }

    /// Validate market quote values for finiteness, non-negativity, and
    /// mutual exclusivity among price-driving fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        check_finite_fields(&[
            (self.quoted_clean_price_pct, false),
            (self.quoted_dirty_price_currency, false),
            (self.quoted_ytm, false),
            (self.quoted_ytw, false),
            (self.quoted_z_spread, false),
            (self.quoted_oas, false),
            (self.quoted_discount_margin, false),
            (self.quoted_i_spread, false),
            (self.quoted_asw_market, false),
            (self.quoted_japanese_simple_yield, false),
            (self.implied_volatility, true),
            (self.quoted_premium, false),
            (self.cds_quote_bp, true),
        ])?;

        if self
            .quoted_dirty_price_currency
            .is_some_and(|price| price <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "quoted_dirty_price_currency must be positive".to_string(),
            ));
        }

        if self.price_driver_count() > 1 {
            return Err(finstack_quant_core::Error::Validation(
                "at most one price-driving override may be set".to_string(),
            ));
        }

        Ok(())
    }
}
