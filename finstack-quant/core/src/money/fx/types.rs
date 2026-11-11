//! Public query, policy, and result types for foreign-exchange conversion.
//!
use crate::currency::Currency;
use crate::dates::Date;
use serde::{Deserialize, Serialize};

/// Provider-selection purpose of a point-date FX query (record
/// CORE-CONVENTIONS-013 #1/#2).
///
/// The policy tells a provider which source purpose the rate serves; it does
/// not calculate dates or choose spot versus forward. There is no default:
/// every query names its policy. Period averages are not point queries; an
/// owner that needs one computes the oriented aggregate itself.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FxConversionPolicy {
    /// The rate applicable on a cashflow's own date.
    CashflowDate,
    /// The rate applicable on a reporting period's closing date.
    PeriodEnd,
}

impl std::fmt::Display for FxConversionPolicy {
    /// Writes the serde label (`cashflow_date` or `period_end`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for FxConversionPolicy {
    type Err = crate::Error;

    /// Parses the serde label; the error names the accepted labels.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse(s)
    }
}

/// Simple FX rate query.
///
/// Contains only the essential parameters for currency conversion.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxQuery {
    /// Currency being converted from.
    pub from: Currency,
    /// Currency being converted to.
    pub to: Currency,
    /// Date on which the provider's rate must be applicable.
    pub on: Date,
    /// Required provider-selection purpose; there is no default.
    pub policy: FxConversionPolicy,
}

impl FxQuery {
    /// Create a query with an explicit provider rate-selection policy.
    ///
    /// # Arguments
    ///
    /// * `from` - Source currency whose units are being converted.
    /// * `to` - Target currency received after multiplying by the quoted rate.
    /// * `on` - Date on which the provider's quote must apply.
    /// * `policy` - Provider-selection purpose for the requested date, chosen
    ///   by the caller; there is no default.
    pub fn new(from: Currency, to: Currency, on: Date, policy: FxConversionPolicy) -> Self {
        Self {
            from,
            to,
            on,
            policy,
        }
    }
}

/// An ordered `base/quote` currency pair parsed from host text.
///
/// [`FromStr`](std::str::FromStr) accepts the slash form `"EUR/USD"` and the
/// six-letter compact form `"EURUSD"`. Each half parses with [`Currency`]'s
/// case-insensitive, non-trimming parser, so a bad half reports the code it
/// rejected. Orientation is kept: a quote on the pair means
/// `1 base = rate quote`.
///
/// # Examples
/// ```
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::fx::CurrencyPair;
///
/// let pair: CurrencyPair = "eur/usd".parse().expect("slash form");
/// assert_eq!((pair.base, pair.quote), (Currency::EUR, Currency::USD));
/// assert_eq!("GBPJPY".parse::<CurrencyPair>().expect("compact form").quote, Currency::JPY);
/// assert!("EURUS".parse::<CurrencyPair>().is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CurrencyPair {
    /// Base (from) currency: one unit of it is worth `rate` quote units.
    pub base: Currency,
    /// Quote (to) currency in which the rate is expressed.
    pub quote: Currency,
}

impl std::str::FromStr for CurrencyPair {
    type Err = crate::Error;

    /// Parse `"EUR/USD"` or `"EURUSD"`.
    ///
    /// The compact form is split after the third character only when the text
    /// is exactly six ASCII characters, so multi-byte input is rejected rather
    /// than sliced inside a character.
    fn from_str(s: &str) -> crate::Result<Self> {
        let (base, quote) = match s.split_once('/') {
            Some(halves) => halves,
            None if s.len() == 6 && s.is_ascii() => s.split_at(3),
            None => {
                return Err(crate::Error::Validation(format!(
                    "invalid FX pair {s:?}: expected \"EURUSD\" or \"EUR/USD\""
                )));
            }
        };
        Ok(Self {
            base: base.parse()?,
            quote: quote.parse()?,
        })
    }
}

/// Metadata describing the policy applied by the provider.
///
/// Attach [`FxPolicyMeta`] to valuation results so auditors can understand how
/// FX conversions were sourced.
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxPolicyMeta {
    /// Strategy the provider actually applied or was instructed to apply.
    pub strategy: FxConversionPolicy,
    /// Optional target currency stamped onto the resulting valuation.
    pub target_currency: Option<Currency>,
    /// Free-form provenance or audit notes supplied by the provider.
    pub notes: String,
}

/// Observed-quote capacity of [`FxConfig::new`] and an omitted
/// `cache_capacity` key: an operational bound, not a financial choice.
pub(crate) const DEFAULT_CACHE_CAPACITY: std::num::NonZeroUsize =
    std::num::NonZeroUsize::MIN.saturating_add(255);

fn default_cache_capacity() -> usize {
    DEFAULT_CACHE_CAPACITY.get()
}

/// Deserialize the pivot key as required: `null` selects no routing, while a
/// missing key is an error rather than an inferred choice.
fn required_pivot<'de, D>(deserializer: D) -> Result<Option<Currency>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<Currency> as serde::Deserialize>::deserialize(deserializer)
}

/// Configuration for [`FxMatrix`](crate::money::fx::FxMatrix) behaviour
/// (record CORE-CONVENTIONS-013 #5, resolution O9).
///
/// The routing choice is explicit: there is no default pivot and no default
/// configuration. `cache_capacity` may be omitted on the wire and then takes
/// the operational default.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::fx::FxConfig;
///
/// assert_eq!(FxConfig::new(Some(Currency::USD)).pivot_currency, Some(Currency::USD));
/// assert!(serde_json::from_str::<FxConfig>("{}").is_err());
/// let direct: FxConfig = serde_json::from_str(r#"{"pivot_currency": null}"#)?;
/// assert_eq!(direct.pivot_currency, None);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "json-schema",
    schemars(extend("required" = ["pivot_currency"]))
)]
#[serde(deny_unknown_fields)]
pub struct FxConfig {
    /// Single pivot for `from -> pivot -> to` cross routing, or `None` for
    /// direct quotes only. Required on the wire (`null` disables routing).
    ///
    /// Only this one pivot is tried: no alternative pivot, multi-hop search or
    /// market-order inference. A pair whose market convention routes through
    /// another currency needs its direct quote seeded explicitly.
    #[serde(deserialize_with = "required_pivot")]
    pub pivot_currency: Option<Currency>,
    /// Maximum number of provider-observed quotes retained in the LRU cache.
    #[serde(default = "default_cache_capacity")]
    pub cache_capacity: usize,
}

impl FxConfig {
    /// Configuration with an explicit routing choice and the default cache
    /// capacity (256 observed quotes).
    ///
    /// # Arguments
    ///
    /// * `pivot_currency` - The single routing pivot, or `None` for direct
    ///   quotes only.
    pub const fn new(pivot_currency: Option<Currency>) -> Self {
        Self {
            pivot_currency,
            cache_capacity: DEFAULT_CACHE_CAPACITY.get(),
        }
    }
}

/// Result of an FX rate lookup with simple triangulation info.
///
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxRateResult {
    /// Rate such that source amount multiplied by this value is target amount.
    pub rate: f64,
    /// Whether this rate was constructed through the configured pivot currency.
    pub triangulated: bool,
}

/// Serializable state of an FxMatrix.
/// Contains the configuration and cached quotes that can be persisted and restored.
/// Serialization fails with an ordinary serializer error if any captured
/// explicit or provider rate is non-finite or non-positive, including rates
/// that overflow after a mutable underlying provider changes under a shock.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxMatrixState {
    /// Matrix configuration, including pivot and cache capacity.
    pub config: FxConfig,
    /// Pair-global quotes as source, target, rate tuples.
    pub quotes: Vec<(Currency, Currency, f64)>,
    /// Captured provider quotes, below explicit global and date/policy-pinned
    /// quotes in lookup priority. Market-context restoration uses these to
    /// rebuild a quote-only provider; arbitrary live provider behavior is not
    /// serialized. Required even when the provider has no snapshot quotes.
    pub provider_quotes: Vec<(Currency, Currency, f64)>,
    /// Captured date/policy-scoped provider quotes. These override captured
    /// pair-global provider quotes for their scope while remaining below
    /// explicit matrix quotes in either direction. Required even when empty.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(Currency, Currency, String, FxConversionPolicy, f64)>")
    )]
    pub provider_pinned_quotes: Vec<(Currency, Currency, Date, FxConversionPolicy, f64)>,
    /// Pinned, date/policy-scoped quotes as `(from, to, on, policy, rate)`.
    ///
    /// Required: a snapshot that omits it would silently re-derive those
    /// dates from the provider instead of restoring the pinned fixings.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(Currency, Currency, String, FxConversionPolicy, f64)>")
    )]
    pub pinned_quotes: Vec<(Currency, Currency, Date, FxConversionPolicy, f64)>,
}

impl Serialize for FxMatrixState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error, SerializeStruct};

        for &(from, to, rate) in self.quotes.iter().chain(&self.provider_quotes) {
            super::provider::validate_fx_rate(from, to, rate).map_err(S::Error::custom)?;
        }
        for &(from, to, _, _, rate) in self
            .pinned_quotes
            .iter()
            .chain(&self.provider_pinned_quotes)
        {
            super::provider::validate_fx_rate(from, to, rate).map_err(S::Error::custom)?;
        }
        let mut state = serializer.serialize_struct("FxMatrixState", 5)?;
        state.serialize_field("config", &self.config)?;
        state.serialize_field("quotes", &self.quotes)?;
        state.serialize_field("provider_quotes", &self.provider_quotes)?;
        state.serialize_field("provider_pinned_quotes", &self.provider_pinned_quotes)?;
        state.serialize_field("pinned_quotes", &self.pinned_quotes)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::FxConversionPolicy;

    #[test]
    fn fx_conversion_policy_display_and_fromstr_use_the_serde_labels() {
        for (policy, label) in [
            (FxConversionPolicy::CashflowDate, "cashflow_date"),
            (FxConversionPolicy::PeriodEnd, "period_end"),
        ] {
            assert_eq!(policy.to_string(), label);
            assert_eq!(label.parse::<FxConversionPolicy>().expect(label), policy);
        }
        let err = "spot"
            .parse::<FxConversionPolicy>()
            .expect_err("unknown label");
        assert!(err.to_string().contains("cashflow_date"), "{err}");
    }
}
