use super::cap_floor::validate_cap_floor_quote;
use super::*;

/// Market quote for a European swaption used in HW1F calibration.
///
/// Represents an **ATM** European swaption with its market volatility. The
/// quote carries no strike: the calibrator always prices the swaption at the
/// forward swap rate implied by the supplied curve, so off-ATM quotes cannot
/// be represented by this type.
///
/// Deserialization rejects unknown fields and applies the same validation as
/// [`SwaptionQuote::try_new`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "SwaptionQuoteRaw")]
pub struct SwaptionQuote {
    /// Swaption expiry in years (T₀).
    pub expiry: f64,
    /// Underlying swap tenor in years (e.g. 5.0 for a 5Y swap).
    pub tenor: f64,
    /// Market-quoted volatility.
    pub volatility: f64,
    /// `true` for normal (Bachelier) vol, `false` for lognormal (Black-76) vol.
    pub is_normal_vol: bool,
}

/// Contractual fixed-leg schedule for one swaption calibration quote.
///
/// Payment times are year fractions from the discount curve's base date.
/// Accrual factors use the underlying swap's fixed-leg day count, and
/// `swap_start_time` and `maturity_time` are the adjusted swap start and
/// unlagged accrual-end times used in the par-rate numerator. This separates
/// settlement, payment lags, and business-day adjustments from accrual
/// fractions instead of approximating dates by cumulative accruals.
#[derive(Debug, Clone)]
pub struct SwaptionSchedule {
    /// Convention-adjusted underlying swap start time.
    pub swap_start_time: f64,
    /// Strictly increasing fixed-leg payment times.
    pub payment_times: Vec<f64>,
    /// Positive fixed-leg accrual factors aligned with `payment_times`.
    pub accruals: Vec<f64>,
    /// Underlying swap accrual-end time in ACT/365F model years from valuation date.
    pub maturity_time: f64,
}

/// Wire shape for [`SwaptionQuote`]: rejects unknown fields, then routes
/// through [`SwaptionQuote::try_new`] for value validation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SwaptionQuoteRaw {
    expiry: f64,
    tenor: f64,
    volatility: f64,
    is_normal_vol: bool,
}

impl TryFrom<SwaptionQuoteRaw> for SwaptionQuote {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: SwaptionQuoteRaw) -> Result<Self, Self::Error> {
        Self::try_new(raw.expiry, raw.tenor, raw.volatility, raw.is_normal_vol)
    }
}

/// Market quote for an interest-rate cap/floor used in HW1F calibration.
///
/// The quote represents a flat volatility for a full cap/floor from today to
/// `maturity`, with caplet/floorlet periods generated from the calibration
/// frequency. Normal vols are represented in decimal rate units: `0.0088`
/// means 88bp normal volatility.
///
/// Quotes are interpreted as standard market cap/floor quotes: the
/// spot-start caplet (whose rate fixes at `t = 0` and therefore carries no
/// optionality) is excluded from both the market and model legs, and each
/// caplet's option expiry is its fixing date (period start), not its payment
/// date.
///
/// Deserialization rejects unknown fields and applies the same validation as
/// [`CapFloorQuote::try_new`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "CapFloorQuoteRaw")]
pub struct CapFloorQuote {
    /// Cap/floor maturity in years.
    pub maturity: f64,
    /// Strike rate as a decimal.
    pub strike: f64,
    /// Market-quoted volatility.
    pub volatility: f64,
    /// `true` for cap, `false` for floor.
    pub is_cap: bool,
    /// `true` for normal (Bachelier) vol. Lognormal cap/floor HW1F
    /// calibration is intentionally not accepted yet.
    pub is_normal_vol: bool,
}

/// Wire shape for [`CapFloorQuote`]: rejects unknown fields, then routes
/// through [`CapFloorQuote::try_new`] for value validation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CapFloorQuoteRaw {
    maturity: f64,
    strike: f64,
    volatility: f64,
    is_cap: bool,
    is_normal_vol: bool,
}

impl TryFrom<CapFloorQuoteRaw> for CapFloorQuote {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: CapFloorQuoteRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.maturity,
            raw.strike,
            raw.volatility,
            raw.is_cap,
            raw.is_normal_vol,
        )
    }
}

impl CapFloorQuote {
    /// Construct a validated cap/floor market quote.
    ///
    /// # Arguments
    ///
    /// * `maturity` - Cap/floor maturity in years from the curve base date.
    ///   Must be finite and positive.
    /// * `strike` - Strike as a decimal rate (for example `0.03` for 3%).
    /// * `volatility` - Quoted volatility. Normal vols use decimal rate units
    ///   (`0.0088` is 88 bp).
    /// * `is_cap` - `true` for a cap, `false` for a floor.
    /// * `is_normal_vol` - `true` for Bachelier vol. Lognormal quotes are rejected.
    pub fn try_new(
        maturity: f64,
        strike: f64,
        volatility: f64,
        is_cap: bool,
        is_normal_vol: bool,
    ) -> finstack_quant_core::Result<Self> {
        validate_cap_floor_quote(maturity, strike, volatility, is_normal_vol)?;
        Ok(Self {
            maturity,
            strike,
            volatility,
            is_cap,
            is_normal_vol,
        })
    }
}

/// Configuration for cap/floor HW1F calibration.
#[derive(Debug, Clone, Copy)]
pub struct CapFloorCalibrationConfig {
    /// Required positive maximum implied-quote error in quoted volatility units.
    /// Normal quotes use decimal rate volatility; Black quotes use relative volatility.
    /// This acceptance budget is independent of the numerical solver tolerance.
    pub fit_tolerance: f64,
    /// Payment frequency used to decompose full caps/floors into caplets.
    pub frequency: SwapFrequency,
    /// Optional source mean reversion. Required when calibrating from a
    /// single cap/floor quote because one quote cannot identify both κ and σ.
    pub fixed_kappa: Option<f64>,
    /// Optional initial guess when solving both κ and σ.
    pub initial_guess: Option<HullWhiteCalibrationParams>,
}

impl SwaptionQuote {
    /// Construct a validated swaption market quote.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Swaption expiry in years. Must be finite and positive.
    /// * `tenor` - Underlying swap tenor in years (for example `5.0` for 5Y).
    /// * `volatility` - Market-quoted volatility. Must be finite and positive.
    /// * `is_normal_vol` - `true` for Bachelier vol, `false` for Black-76.
    pub fn try_new(
        expiry: f64,
        tenor: f64,
        volatility: f64,
        is_normal_vol: bool,
    ) -> finstack_quant_core::Result<Self> {
        if !expiry.is_finite() || expiry <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption expiry must be positive, got {expiry}"
            )));
        }
        if !tenor.is_finite() || tenor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption tenor must be positive, got {tenor}"
            )));
        }
        if !volatility.is_finite() || volatility <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption volatility must be positive, got {volatility}"
            )));
        }
        Ok(Self {
            expiry,
            tenor,
            volatility,
            is_normal_vol,
        })
    }
}

/// Number of coupon payments per year for the underlying swap in HW1F calibration.
///
/// USD swaps are semi-annual (2), EUR swaps are annual (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SwapFrequency {
    /// 1 payment per year (EUR, GBP standard).
    Annual,
    /// 2 payments per year (USD standard).
    #[default]
    SemiAnnual,
    /// 4 payments per year.
    Quarterly,
}

impl SwapFrequency {
    pub(crate) fn periods_per_year(self) -> usize {
        match self {
            Self::Annual => 1,
            Self::SemiAnnual => 2,
            Self::Quarterly => 4,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Annual => "annual",
            Self::SemiAnnual => "semi_annual",
            Self::Quarterly => "quarterly",
        }
    }
}

impl std::fmt::Display for SwapFrequency {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
