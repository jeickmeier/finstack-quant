//! FX delta volatility surface, builder, and wire representation.

use super::super::VolQuoteType;
use super::conventions::{FxAtmConvention, FxButterflyConvention, FxDeltaConvention};
use super::delta_math::recover_fx_wing_vols;
use crate::{dates::VolatilityClock, error::InputError, money::fx::CurrencyPair, types::CurveId};

/// Construction parameters for [`FxDeltaVolSurface::from_params`].
#[derive(Clone, Debug, PartialEq)]
pub struct FxDeltaVolSurfaceParams {
    /// Unique non-empty surface identifier.
    pub id: CurveId,
    /// Currency pair (`base/quote`) whose forward is expressed in quote units per base unit.
    pub pair: CurrencyPair,
    /// Forward delta convention governing delta-to-strike inversion.
    pub delta_convention: FxDeltaConvention,
    /// At-the-money strike convention (`DeltaNeutralStraddle`).
    pub atm_convention: FxAtmConvention,
    /// Butterfly quote convention (`Smile`: wing average minus ATM).
    pub butterfly_convention: FxButterflyConvention,
    /// Strictly increasing positive expiry times in years.
    pub expiries: Vec<f64>,
    /// Positive ATM delta-neutral-straddle volatilities per expiry (decimal).
    pub atm_vols: Vec<f64>,
    /// 25-delta risk reversals per expiry (`sigma_call - sigma_put`, decimal).
    pub rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterflies per expiry (decimal).
    pub bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversals per expiry; must be paired with `bf_10d`.
    pub rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterflies per expiry; must be paired with `rr_10d`.
    pub bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates into numeric expiry times.
    pub expiry_clock: Option<VolatilityClock>,
}

/// Builder for constructing a validated [`FxDeltaVolSurface`] without positional argument sprawl.
#[derive(Clone, Debug)]
pub struct FxDeltaVolSurfaceBuilder {
    id: CurveId,
    pair: CurrencyPair,
    delta_convention: Option<FxDeltaConvention>,
    atm_convention: Option<FxAtmConvention>,
    butterfly_convention: Option<FxButterflyConvention>,
    expiries: Option<Vec<f64>>,
    atm_vols: Option<Vec<f64>>,
    rr_25d: Option<Vec<f64>>,
    bf_25d: Option<Vec<f64>>,
    rr_10d: Option<Vec<f64>>,
    bf_10d: Option<Vec<f64>>,
    expiry_clock: Option<VolatilityClock>,
}

impl FxDeltaVolSurfaceBuilder {
    /// Set the forward delta convention (`FxDeltaConvention::ForwardUnadjusted`).
    ///
    /// # Arguments
    ///
    /// * `convention` - Forward delta convention governing wing strikes.
    #[must_use]
    pub fn delta_convention(mut self, convention: FxDeltaConvention) -> Self {
        self.delta_convention = Some(convention);
        self
    }

    /// Set the at-the-money convention (`FxAtmConvention::DeltaNeutralStraddle`).
    ///
    /// # Arguments
    ///
    /// * `convention` - ATM strike definition.
    #[must_use]
    pub fn atm_convention(mut self, convention: FxAtmConvention) -> Self {
        self.atm_convention = Some(convention);
        self
    }

    /// Set the butterfly convention (`FxButterflyConvention::Smile`).
    ///
    /// # Arguments
    ///
    /// * `convention` - Arithmetic smile butterfly quote definition.
    #[must_use]
    pub fn butterfly_convention(mut self, convention: FxButterflyConvention) -> Self {
        self.butterfly_convention = Some(convention);
        self
    }

    /// Set the strictly increasing positive expiry times in years.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Option expiry times in years.
    #[must_use]
    pub fn expiries(mut self, expiries: &[f64]) -> Self {
        self.expiries = Some(expiries.to_vec());
        self
    }

    /// Set the ATM delta-neutral-straddle volatilities (decimal).
    ///
    /// # Arguments
    ///
    /// * `atm_vols` - Strictly positive decimal Black volatilities per expiry.
    #[must_use]
    pub fn atm_vols(mut self, atm_vols: &[f64]) -> Self {
        self.atm_vols = Some(atm_vols.to_vec());
        self
    }

    /// Set the 25-delta risk reversals (`sigma_call - sigma_put`, decimal).
    ///
    /// # Arguments
    ///
    /// * `rr_25d` - Finite 25-delta risk reversals per expiry.
    #[must_use]
    pub fn rr_25d(mut self, rr_25d: &[f64]) -> Self {
        self.rr_25d = Some(rr_25d.to_vec());
        self
    }

    /// Set the 25-delta arithmetic smile butterflies (`0.5 * (sigma_call + sigma_put) - sigma_ATM`, decimal).
    ///
    /// # Arguments
    ///
    /// * `bf_25d` - Finite 25-delta butterflies per expiry.
    #[must_use]
    pub fn bf_25d(mut self, bf_25d: &[f64]) -> Self {
        self.bf_25d = Some(bf_25d.to_vec());
        self
    }

    /// Set the optional 10-delta risk reversals (`sigma_call - sigma_put`, decimal).
    ///
    /// # Arguments
    ///
    /// * `rr_10d` - Finite 10-delta risk reversals per expiry; requires `bf_10d`.
    #[must_use]
    pub fn rr_10d(mut self, rr_10d: &[f64]) -> Self {
        self.rr_10d = Some(rr_10d.to_vec());
        self
    }

    /// Set the optional 10-delta arithmetic smile butterflies (decimal).
    ///
    /// # Arguments
    ///
    /// * `bf_10d` - Finite 10-delta butterflies per expiry; requires `rr_10d`.
    #[must_use]
    pub fn bf_10d(mut self, bf_10d: &[f64]) -> Self {
        self.bf_10d = Some(bf_10d.to_vec());
        self
    }

    /// Attach an optional [`VolatilityClock`] recording the expiry calibration basis.
    ///
    /// # Arguments
    ///
    /// * `clock` - Validated volatility clock for dated expiry lookup.
    #[must_use]
    pub fn expiry_clock(mut self, clock: VolatilityClock) -> Self {
        self.expiry_clock = Some(clock);
        self
    }

    /// Validate all required conventions, arrays, and implied wing volatilities and build [`FxDeltaVolSurface`].
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing or fails structural or wing validation.
    pub fn build(self) -> crate::Result<FxDeltaVolSurface> {
        let missing = |field: &str| {
            crate::Error::Validation(format!("FxDeltaVolSurface builder requires '{field}'"))
        };
        FxDeltaVolSurface::from_params(FxDeltaVolSurfaceParams {
            id: self.id,
            pair: self.pair,
            delta_convention: self
                .delta_convention
                .ok_or_else(|| missing("delta_convention"))?,
            atm_convention: self
                .atm_convention
                .ok_or_else(|| missing("atm_convention"))?,
            butterfly_convention: self
                .butterfly_convention
                .ok_or_else(|| missing("butterfly_convention"))?,
            expiries: self.expiries.ok_or_else(|| missing("expiries"))?,
            atm_vols: self.atm_vols.ok_or_else(|| missing("atm_vols"))?,
            rr_25d: self.rr_25d.ok_or_else(|| missing("rr_25d"))?,
            bf_25d: self.bf_25d.ok_or_else(|| missing("bf_25d"))?,
            rr_10d: self.rr_10d,
            bf_10d: self.bf_10d,
            expiry_clock: self.expiry_clock,
        })
    }
}

/// Delta-quoted FX volatility surface.
///
/// Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
/// 25-delta arithmetic smile butterfly, optional 10-delta wings) together with
/// explicit currency pair, quote profile conventions, and optional [`VolatilityClock`].
/// Models-layer functions perform delta conversion and volatility evaluation.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::surfaces::{
///     FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
/// };
/// use finstack_quant_core::money::fx::CurrencyPair;
///
/// let pair = CurrencyPair::try_new(Currency::EUR, Currency::USD)?;
/// let surface = FxDeltaVolSurface::builder("EURUSD-DELTA-VOL", pair)
///     .delta_convention(FxDeltaConvention::ForwardUnadjusted)
///     .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
///     .butterfly_convention(FxButterflyConvention::Smile)
///     .expiries(&[0.25, 0.5, 1.0])
///     .atm_vols(&[0.08, 0.085, 0.09])
///     .rr_25d(&[0.01, 0.012, 0.015])
///     .bf_25d(&[0.005, 0.006, 0.007])
///     .build()?;
///
/// assert_eq!(surface.expiries().len(), 3);
/// assert!((surface.atm_vols()[0] - 0.08).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "FxDeltaVolSurfaceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "FxDeltaVolSurfaceWire"))]
pub struct FxDeltaVolSurface {
    id: CurveId,
    /// Currency pair (`base/quote`) matching quoted FX forward orientation.
    pair: CurrencyPair,
    /// Delta convention governing wing delta-to-strike conversion.
    delta_convention: FxDeltaConvention,
    /// At-the-money strike convention.
    atm_convention: FxAtmConvention,
    /// Butterfly quote convention.
    butterfly_convention: FxButterflyConvention,
    /// Expiry times in years (strictly increasing, all positive).
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry (call vol - put vol).
    rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterfly per expiry (wing avg - ATM).
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates to `expiries`.
    expiry_clock: Option<VolatilityClock>,
}

/// Raw deserialization state of [`FxDeltaVolSurface`].
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FxDeltaVolSurfaceWire {
    /// Surface identifier.
    id: CurveId,
    /// Currency pair (`base/quote`) matching quoted FX forward orientation.
    pair: CurrencyPair,
    /// Delta convention governing wing delta-to-strike conversion.
    delta_convention: FxDeltaConvention,
    /// At-the-money strike convention.
    atm_convention: FxAtmConvention,
    /// Butterfly quote convention.
    butterfly_convention: FxButterflyConvention,
    /// Expiry times in years.
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry.
    rr_25d: Vec<f64>,
    /// 25-delta arithmetic smile butterfly per expiry.
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta arithmetic smile butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
    /// Optional volatility clock translating calendar expiry dates to `expiries`.
    #[serde(default)]
    expiry_clock: Option<VolatilityClock>,
}

impl TryFrom<FxDeltaVolSurfaceWire> for FxDeltaVolSurface {
    type Error = crate::Error;

    fn try_from(raw: FxDeltaVolSurfaceWire) -> crate::Result<Self> {
        Self::from_params(FxDeltaVolSurfaceParams {
            id: raw.id,
            pair: raw.pair,
            delta_convention: raw.delta_convention,
            atm_convention: raw.atm_convention,
            butterfly_convention: raw.butterfly_convention,
            expiries: raw.expiries,
            atm_vols: raw.atm_vols,
            rr_25d: raw.rr_25d,
            bf_25d: raw.bf_25d,
            rr_10d: raw.rr_10d,
            bf_10d: raw.bf_10d,
            expiry_clock: raw.expiry_clock,
        })
    }
}

impl FxDeltaVolSurface {
    /// Start building an [`FxDeltaVolSurface`] for `id` and `pair`.
    ///
    /// # Arguments
    ///
    /// * `id` - Non-empty unique surface identifier used for lookup in a market context.
    /// * `pair` - Ordered FX pair (`base/quote`) matching the forward rate's quote-per-base units.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>, pair: CurrencyPair) -> FxDeltaVolSurfaceBuilder {
        FxDeltaVolSurfaceBuilder {
            id: id.into(),
            pair,
            delta_convention: None,
            atm_convention: None,
            butterfly_convention: None,
            expiries: None,
            atm_vols: None,
            rr_25d: None,
            bf_25d: None,
            rr_10d: None,
            bf_10d: None,
            expiry_clock: None,
        }
    }

    /// Construct and validate an [`FxDeltaVolSurface`] from explicit [`FxDeltaVolSurfaceParams`].
    ///
    /// # Arguments
    ///
    /// * `params` - Complete FX delta quote profile, expiry/quote vectors, and optional clock.
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is empty, `pair` has identical base and quote currencies,
    /// `rr_10d`/`bf_10d` are unpaired, any quote vector is empty or mismatched in length,
    /// `expiries` are non-positive/non-finite/non-increasing, or any ATM/wing volatility
    /// recovered via [`recover_fx_wing_vols`] is non-positive or non-finite.
    pub fn from_params(params: FxDeltaVolSurfaceParams) -> crate::Result<Self> {
        if params.id.as_str().trim().is_empty() {
            return Err(crate::Error::Validation(
                "FX delta vol surface id cannot be empty".to_string(),
            ));
        }
        let pair = CurrencyPair::try_new(params.pair.base, params.pair.quote)?;
        Self::validate(
            &params.expiries,
            &params.atm_vols,
            &params.rr_25d,
            &params.bf_25d,
            params.rr_10d.as_deref(),
            params.bf_10d.as_deref(),
        )?;
        Ok(Self {
            id: params.id,
            pair,
            delta_convention: params.delta_convention,
            atm_convention: params.atm_convention,
            butterfly_convention: params.butterfly_convention,
            expiries: params.expiries,
            atm_vols: params.atm_vols,
            rr_25d: params.rr_25d,
            bf_25d: params.bf_25d,
            rr_10d: params.rr_10d,
            bf_10d: params.bf_10d,
            expiry_clock: params.expiry_clock,
        })
    }

    /// Surface identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Currency pair (`base/quote`) whose forwards are quoted in `quote` per `base`.
    #[inline]
    pub const fn pair(&self) -> CurrencyPair {
        self.pair
    }

    /// Expiry times (years).
    #[inline]
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Return the ATM delta-neutral-straddle volatility nodes.
    pub fn atm_vols(&self) -> &[f64] {
        &self.atm_vols
    }

    /// Return the 25-delta risk-reversal quote nodes.
    pub fn rr_25d(&self) -> &[f64] {
        &self.rr_25d
    }

    /// Return the 25-delta butterfly quote nodes.
    pub fn bf_25d(&self) -> &[f64] {
        &self.bf_25d
    }

    /// Return the optional 10-delta risk-reversal quote nodes.
    pub fn rr_10d(&self) -> Option<&[f64]> {
        self.rr_10d.as_deref()
    }

    /// Return the optional 10-delta butterfly quote nodes.
    pub fn bf_10d(&self) -> Option<&[f64]> {
        self.bf_10d.as_deref()
    }

    /// Quoting convention of the stored implied volatilities (`black_lognormal`).
    #[inline]
    pub fn quote_type(&self) -> VolQuoteType {
        VolQuoteType::BlackLognormal
    }

    /// Delta convention governing delta-to-strike inversion (`forward_unadjusted`).
    #[inline]
    pub const fn delta_convention(&self) -> FxDeltaConvention {
        self.delta_convention
    }

    /// At-the-money strike convention (`delta_neutral_straddle`).
    #[inline]
    pub const fn atm_convention(&self) -> FxAtmConvention {
        self.atm_convention
    }

    /// Butterfly quote convention (`smile`: wing average minus ATM).
    #[inline]
    pub const fn butterfly_convention(&self) -> FxButterflyConvention {
        self.butterfly_convention
    }

    /// Optional volatility clock recording the date-to-expiry basis.
    #[inline]
    pub const fn expiry_clock(&self) -> Option<&VolatilityClock> {
        self.expiry_clock.as_ref()
    }

    fn validate(
        expiries: &[f64],
        atm_vols: &[f64],
        rr_25d: &[f64],
        bf_25d: &[f64],
        rr_10d: Option<&[f64]>,
        bf_10d: Option<&[f64]>,
    ) -> crate::Result<()> {
        if rr_10d.is_some() != bf_10d.is_some() {
            return Err(crate::Error::Validation(
                "rr_10d and bf_10d must both be provided or both omitted".into(),
            ));
        }
        if expiries.is_empty() || atm_vols.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let n = expiries.len();
        if atm_vols.len() != n || rr_25d.len() != n || bf_25d.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(rr) = rr_10d
            && rr.len() != n
        {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(bf) = bf_10d
            && bf.len() != n
        {
            return Err(InputError::DimensionMismatch.into());
        }

        for &t in expiries {
            if !t.is_finite() || t <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }
        for w in expiries.windows(2) {
            if w[1] <= w[0] {
                return Err(InputError::NonMonotonicKnots.into());
            }
        }

        for &v in atm_vols {
            if !v.is_finite() || v <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }

        for ((&atm, &rr), &bf) in atm_vols.iter().zip(rr_25d.iter()).zip(bf_25d.iter()) {
            recover_fx_wing_vols(atm, rr, bf)?;
        }
        if let (Some(rr10), Some(bf10)) = (rr_10d, bf_10d) {
            for ((&atm, &rr), &bf) in atm_vols.iter().zip(rr10.iter()).zip(bf10.iter()) {
                recover_fx_wing_vols(atm, rr, bf)?;
            }
        }

        Ok(())
    }
}
