//! Option greeks implementation used by the traits subsystem.
//!
// Option risk metric providers

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

/// Supported option greek requests for the consolidated provider API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionGreekKind {
    /// Cash delta in instrument metric convention.
    Delta,
    /// Cash gamma in instrument metric convention.
    Gamma,
    /// Cash vega per 1 vol point.
    Vega,
    /// Theta per instrument day-count convention.
    Theta,
    /// Domestic rho per 1bp.
    Rho,
    /// Foreign/dividend rho per 1bp.
    ForeignRho,
    /// Vanna in instrument bump convention.
    Vanna,
    /// Volga in instrument bump convention.
    Volga,
}

/// Finite-difference bump sizes passed to every [`OptionGreeksProvider`] method.
///
/// The metric layer resolves them from the `valuations.sensitivities.v1`
/// `FinstackConfig` extension layered with
/// `metric_pricing_overrides.bump_config`, so a provider never reads bump
/// sizes from anywhere else. [`GreekBumps::default`] holds the library
/// defaults: a 1% relative spot bump, a 1 vol-point absolute volatility bump
/// and a 1bp parallel rate bump.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::GreekBumps;
///
/// let bumps = GreekBumps::default();
/// assert_eq!(bumps.spot_bump_decimal, 0.01);
/// assert_eq!(bumps.vol_bump_decimal, 0.01);
/// assert_eq!(bumps.rate_bump_bp, 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreekBumps {
    /// Relative spot bump as a decimal fraction of spot (0.01 = 1%).
    pub spot_bump_decimal: f64,
    /// Absolute volatility bump in decimal volatility (0.01 = 1 vol point).
    pub vol_bump_decimal: f64,
    /// Parallel rate bump in basis points (1.0 = 1bp).
    pub rate_bump_bp: f64,
}

impl Default for GreekBumps {
    fn default() -> Self {
        (&crate::metrics::sensitivities::config::SensitivitiesConfig::default()).into()
    }
}

/// Inputs needed to request a specific option greek.
///
/// `base_pv` is required only for [`OptionGreekKind::Volga`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionGreeksRequest {
    /// The greek being requested.
    pub greek: OptionGreekKind,
    /// Base PV required by some greeks such as volga.
    pub base_pv: Option<f64>,
    /// Finite-difference bump sizes resolved by the metric layer.
    pub bumps: GreekBumps,
}

impl OptionGreeksRequest {
    /// Return the requested base PV or an error when it is required but missing.
    pub fn require_base_pv(self) -> finstack_quant_core::Result<f64> {
        self.base_pv.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "OptionGreekKind::Volga requires base_pv in OptionGreeksRequest".to_string(),
            )
        })
    }
}

/// Sparse option greek payload returned by [`OptionGreeksProvider`].
///
/// Providers should populate the requested field when it is supported for the
/// instrument and leave unsupported greeks as `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OptionGreeks {
    /// Cash delta in instrument metric convention.
    pub delta: Option<f64>,
    /// Cash gamma in instrument metric convention.
    pub gamma: Option<f64>,
    /// Cash vega per 1 vol point.
    pub vega: Option<f64>,
    /// Theta per instrument day-count convention.
    pub theta: Option<f64>,
    /// Domestic rho per 1bp.
    pub rho_bp: Option<f64>,
    /// Foreign/dividend rho per 1bp.
    pub foreign_rho_bp: Option<f64>,
    /// Vanna in instrument bump convention.
    pub vanna: Option<f64>,
    /// Volga in instrument bump convention.
    pub volga: Option<f64>,
}

/// Consolidated option greek provider.
///
/// Implementations return a sparse [`OptionGreeks`] payload keyed by the
/// requested [`OptionGreekKind`]. Callers should interpret `None` as "not
/// supported for this instrument" rather than as a zero-valued greek.
pub trait OptionGreeksProvider: Send + Sync {
    /// Return cash delta per instrument conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_delta(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return cash gamma per instrument conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_gamma(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return cash vega per instrument conventions (1 vol point).
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_vega(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return theta per instrument conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_theta(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return domestic rho per instrument conventions (per 1bp).
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_rho_bp(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return foreign/dividend rho per instrument conventions (per 1bp).
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_foreign_rho_bp(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return vanna per instrument conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_vanna(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return volga per instrument conventions.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data (curves, surfaces, spot scalars) used for base and
    ///   bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `base_pv` - Unbumped present value in instrument currency, reused as the
    ///   centre of the second difference.
    /// * `bumps` - Resolved finite-difference bump sizes; providers must use these
    ///   rather than hard-coded constants.
    fn option_volga(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        _base_pv: f64,
        _bumps: GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Return the requested greek in a sparse [`OptionGreeks`] payload.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data used for base and bumped repricing.
    /// * `as_of` - Valuation date.
    /// * `request` - Requested greek, optional base PV (required for volga) and
    ///   the resolved finite-difference bump sizes.
    fn option_greeks(
        &self,
        market: &MarketContext,
        as_of: Date,
        request: &OptionGreeksRequest,
    ) -> finstack_quant_core::Result<OptionGreeks> {
        let mut greeks = OptionGreeks::default();
        let bumps = request.bumps;
        match request.greek {
            OptionGreekKind::Delta => greeks.delta = self.option_delta(market, as_of, bumps)?,
            OptionGreekKind::Gamma => greeks.gamma = self.option_gamma(market, as_of, bumps)?,
            OptionGreekKind::Vega => greeks.vega = self.option_vega(market, as_of, bumps)?,
            OptionGreekKind::Theta => greeks.theta = self.option_theta(market, as_of, bumps)?,
            OptionGreekKind::Rho => greeks.rho_bp = self.option_rho_bp(market, as_of, bumps)?,
            OptionGreekKind::ForeignRho => {
                greeks.foreign_rho_bp = self.option_foreign_rho_bp(market, as_of, bumps)?;
            }
            OptionGreekKind::Vanna => greeks.vanna = self.option_vanna(market, as_of, bumps)?,
            OptionGreekKind::Volga => {
                greeks.volga =
                    self.option_volga(market, as_of, request.require_base_pv()?, bumps)?;
            }
        }
        Ok(greeks)
    }
}

/// Implement standard equity-exotic trait boilerplate for instruments with
/// `day_count` fields.
///
/// For types with custom `HasExpiry`, use the internal `@mc_daycount` arm
/// directly and implement `HasExpiry` manually.
#[macro_export]
macro_rules! impl_equity_exotic_traits {
    ($ty:ty) => {
        $crate::impl_equity_exotic_traits!(@inner $ty);
    };

    (@inner $ty:ty) => {
        $crate::impl_equity_exotic_traits!(@mc_daycount $ty);


        impl $crate::metrics::HasExpiry for $ty {
            fn expiry(&self) -> finstack_quant_core::dates::Date {
                self.expiry
            }
        }
    };

    (@mc_daycount $ty:ty) => {

        impl $crate::metrics::HasDayCount for $ty {
            fn day_count(&self) -> finstack_quant_core::dates::DayCount {
                self.day_count
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DeltaOnlyProvider;

    impl OptionGreeksProvider for DeltaOnlyProvider {
        fn option_delta(
            &self,
            _market: &MarketContext,
            _as_of: Date,
            _bumps: GreekBumps,
        ) -> finstack_quant_core::Result<Option<f64>> {
            Ok(Some(42.0))
        }
    }

    #[test]
    fn provider_trait_dispatches_individual_greek_methods() {
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2026, time::Month::January, 1).expect("valid date");
        let greeks = DeltaOnlyProvider
            .option_greeks(
                &market,
                as_of,
                &OptionGreeksRequest {
                    greek: OptionGreekKind::Delta,
                    base_pv: None,
                    bumps: GreekBumps::default(),
                },
            )
            .expect("delta should compute");

        assert_eq!(greeks.delta, Some(42.0));
        assert_eq!(greeks.gamma, None);
    }
}
