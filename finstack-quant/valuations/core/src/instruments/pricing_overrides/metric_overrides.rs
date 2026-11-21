//! Metric-time bump configuration, risk bases, and scenario valuation shocks.

use super::check_finite_fields;
use crate::instruments::BreakevenConfig;
use finstack_quant_core::money::Money;

/// Bump sizes for finite-difference sensitivity calculations.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct BumpConfig {
    /// Yield bump in basis points (1.0 = 1bp) for numerical yield duration and
    /// convexity: InflationLinkedBond `RealDuration` and structured-credit
    /// `DurationMod`/`Convexity`.
    ///
    /// `None` keeps each metric's default shock (1bp for duration, 10bp for
    /// structured-credit convexity).
    pub ytm_bump_bp: Option<f64>,
    /// Spot bump as a decimal fraction of spot (0.01 = 1%).
    ///
    /// Sizes every finite-difference spot greek (delta, gamma, vanna, charm,
    /// speed, color, FX delta). When set it also replaces the adaptive spot
    /// bump. `None` uses the `valuations.sensitivities.v1` value (default 1%).
    pub spot_bump_decimal: Option<f64>,
    /// Absolute volatility bump in decimal volatility (0.01 = 1 vol point).
    ///
    /// Sizes every finite-difference volatility greek (vega, vanna, volga, FX
    /// vega). `None` uses the `valuations.sensitivities.v1` value (default
    /// 1 vol point). Results stay reported per 1 vol point.
    pub vol_bump_decimal: Option<f64>,
    /// Parallel rate bump in basis points (1.0 = 1bp).
    ///
    /// Sizes DV01, rho, foreign rho and forward PV01. Results stay reported per
    /// 1bp: `(pv_bumped - pv) / rate_bump_bp`. `None` uses the
    /// `valuations.sensitivities.v1` value (default 1bp).
    pub rate_bump_bp: Option<f64>,
    /// Custom credit spread bump size override (in basis points, e.g., 1.0 for 1bp).
    ///
    /// Used by CS01 calculations that bump par spreads / hazard calibration quotes.
    pub credit_spread_bump_bp: Option<f64>,
    /// Enable adaptive bump sizes based on volatility and moneyness.
    ///
    /// When true, bump sizes are scaled based on:
    /// - Volatility level (higher vol → larger bumps)
    /// - Time to expiry (longer dated → larger bumps)
    /// - Moneyness (deep ITM/OTM → smaller bumps)
    ///
    /// Default: false (use fixed bump sizes)
    pub adaptive_bumps: bool,
}

impl BumpConfig {
    /// Return whether no finite-difference bump override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Validate bump sizes for non-negativity.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        check_finite_fields(&[
            (self.ytm_bump_bp, true),
            (self.spot_bump_decimal, true),
            (self.vol_bump_decimal, true),
            (self.rate_bump_bp, true),
            (self.credit_spread_bump_bp, true),
        ])
    }
}

/// Risk calculation basis for callable/putable bonds.
///
/// Selects whether analytical risk metrics (duration, convexity, DV01) evaluate
/// the maturity/workout cashflow path under quoted yields or reprice the embedded
/// option via the callable OAS/tree model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BondRiskBasis {
    /// Use maturity/workout cashflows under the quoted-yield convention.
    ///
    /// This matches Bloomberg YAS "Workout" risk fields and is the default for
    /// public bond risk metrics.
    #[default]
    BulletDiscountable,
    /// Use callable/putable option model repricing under the bond's OAS/tree configuration.
    CallableOas,
}

/// Day basis used to convert annual analytic option theta into a per-day amount.
///
/// Applied by the analytic-theta pricers (EquityOption, FxOption,
/// FxDigitalOption) through `metric_pricing_overrides.theta_day_basis`.
#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ThetaDayBasis {
    /// Calendar-day theta: annual theta divided by
    /// [`finstack_quant_models::closed_form::DEFAULT_THETA_DAYS_PER_YEAR`] (365).
    #[default]
    #[serde(rename = "calendar_365")]
    Calendar365,
    /// Trading-day theta: annual theta divided by
    /// [`crate::constants::TRADING_DAYS_PER_YEAR`] (252).
    #[serde(rename = "trading_252")]
    Trading252,
}

impl ThetaDayBasis {
    /// Days per year that divide the annual analytic theta.
    #[must_use]
    pub const fn days_per_year(self) -> f64 {
        match self {
            Self::Calendar365 => finstack_quant_models::closed_form::DEFAULT_THETA_DAYS_PER_YEAR,
            Self::Trading252 => crate::constants::TRADING_DAYS_PER_YEAR,
        }
    }
}

/// Metric-time overrides derived from an instrument's pricing metadata.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MetricPricingOverrides {
    /// Bump sizes for finite-difference sensitivities.
    #[serde(default, skip_serializing_if = "BumpConfig::is_empty")]
    pub bump_config: BumpConfig,
    /// Theta / carry horizon over which time decay is measured (for example
    /// 1D, 1W, 1M, 3M; wire form `{"count": 1, "unit": "weeks"}`). Day and
    /// week tenors roll a fixed number of days, month and year tenors roll
    /// calendar months (EOM-aware). `None` uses one day.
    pub theta_period: Option<finstack_quant_core::dates::Tenor>,
    /// Breakeven configuration: which parameter to solve for and solve mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakeven_config: Option<BreakevenConfig>,
    /// Basis used for bond duration, convexity, and DV01-style risk metrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_risk_basis: Option<BondRiskBasis>,
    /// Day basis for the per-day analytic option theta (`calendar_365` or
    /// `trading_252`). `None` uses calendar-day theta (annual theta / 365).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta_day_basis: Option<ThetaDayBasis>,
    /// Historical VaR / Expected Shortfall configuration override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_config: Option<crate::metrics::risk::var_calculator::VarConfig>,
}

impl MetricPricingOverrides {
    /// Return whether no metric-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Validate metric override fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.bump_config.validate()?;
        if let Some(var_config) = &self.var_config {
            var_config.validate()?;
        }
        Ok(())
    }

    /// Fill every unset field from `defaults`, keeping the fields already set.
    ///
    /// Used to apply a composite's metric overrides to its legs. `Option`
    /// fields take the default only when `None`; `bump_config.adaptive_bumps`
    /// has no unset state, so it is enabled when either side enables it.
    ///
    /// # Arguments
    ///
    /// * `defaults` - Lower-precedence overrides (for example the enclosing
    ///   composite's) whose values fill the fields this set leaves unset.
    #[doc(hidden)]
    pub fn apply_defaults(&mut self, defaults: &Self) {
        let Self {
            bump_config:
                BumpConfig {
                    ytm_bump_bp,
                    spot_bump_decimal,
                    vol_bump_decimal,
                    rate_bump_bp,
                    credit_spread_bump_bp,
                    adaptive_bumps,
                },
            theta_period,
            breakeven_config,
            bond_risk_basis,
            theta_day_basis,
            var_config,
        } = defaults;
        let bump = &mut self.bump_config;
        bump.ytm_bump_bp = bump.ytm_bump_bp.or(*ytm_bump_bp);
        bump.spot_bump_decimal = bump.spot_bump_decimal.or(*spot_bump_decimal);
        bump.vol_bump_decimal = bump.vol_bump_decimal.or(*vol_bump_decimal);
        bump.rate_bump_bp = bump.rate_bump_bp.or(*rate_bump_bp);
        bump.credit_spread_bump_bp = bump.credit_spread_bump_bp.or(*credit_spread_bump_bp);
        bump.adaptive_bumps |= *adaptive_bumps;
        self.theta_period = self.theta_period.or(*theta_period);
        if self.breakeven_config.is_none() {
            self.breakeven_config.clone_from(breakeven_config);
        }
        self.bond_risk_basis = self.bond_risk_basis.or(*bond_risk_basis);
        self.theta_day_basis = self.theta_day_basis.or(*theta_day_basis);
        if self.var_config.is_none() {
            self.var_config.clone_from(var_config);
        }
    }

    /// Days per year dividing annual analytic theta: the configured
    /// `theta_day_basis`, else calendar-day theta (365).
    pub fn theta_days_per_year(&self) -> f64 {
        self.theta_day_basis.unwrap_or_default().days_per_year()
    }

    /// Bond risk basis, defaulting to Bloomberg-style workout/bullet risk.
    pub fn bond_risk_basis_or_default(&self) -> BondRiskBasis {
        self.bond_risk_basis.unwrap_or_default()
    }

    /// Set custom spot bump size (decimal fraction of spot, e.g., 0.01 for 1%).
    ///
    /// # Arguments
    ///
    /// * `bump` - Relative spot bump size as a non-negative decimal.
    pub fn with_spot_bump_decimal(mut self, bump: f64) -> Self {
        self.bump_config.spot_bump_decimal = Some(bump);
        self
    }

    /// Set custom volatility bump size (as absolute vol, e.g., 0.01 for 1% vol).
    ///
    /// # Arguments
    ///
    /// * `bump` - Absolute decimal volatility bump size.
    pub fn with_vol_bump_decimal(mut self, bump: f64) -> Self {
        self.bump_config.vol_bump_decimal = Some(bump);
        self
    }

    /// Set custom rate bump size (in basis points, e.g., 1.0 for 1bp).
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Parallel rate shock size in basis points.
    pub fn with_rate_bump_bp(mut self, bump_bp: f64) -> Self {
        self.bump_config.rate_bump_bp = Some(bump_bp);
        self
    }

    /// Set custom credit spread bump size (in basis points, e.g., 1.0 for 1bp).
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Credit spread shock size in basis points.
    pub fn with_credit_spread_bump_bp(mut self, bump_bp: f64) -> Self {
        self.bump_config.credit_spread_bump_bp = Some(bump_bp);
        self
    }

    /// Set the yield bump for numerical yield duration and convexity.
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Yield shock in basis points (1.0 = 1bp); must be finite and
    ///   positive at metric time.
    pub fn with_ytm_bump_bp(mut self, bump_bp: f64) -> Self {
        self.bump_config.ytm_bump_bp = Some(bump_bp);
        self
    }

    /// Set the theta / carry horizon.
    ///
    /// # Arguments
    ///
    /// * `period` - Horizon over which theta and carry are measured, e.g.
    ///   `Tenor::weekly()`; month and year tenors roll calendar months.
    pub fn with_theta_period(mut self, period: finstack_quant_core::dates::Tenor) -> Self {
        self.theta_period = Some(period);
        self
    }

    /// Set breakeven configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Target parameter and solver mode for breakeven analytics.
    pub fn with_breakeven_config(mut self, config: BreakevenConfig) -> Self {
        self.breakeven_config = Some(config);
        self
    }

    /// Set bond risk basis for duration, convexity, and DV01-style metrics.
    ///
    /// # Arguments
    ///
    /// * `basis` - Workout bullet cashflows versus callable OAS repricing basis.
    pub fn with_bond_risk_basis(mut self, basis: BondRiskBasis) -> Self {
        self.bond_risk_basis = Some(basis);
        self
    }
}

/// Scenario-only valuation adjustments.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ScenarioPricingOverrides {
    /// Scenario price shock as a decimal fraction (`-0.05` = a -5% price
    /// shock).
    ///
    /// When set, valuation helpers apply it as a multiplier:
    /// `price * (1 + scenario_price_shock_decimal)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_price_shock_decimal: Option<f64>,

    /// Scenario spread shock in basis points (e.g., `150.0` for +150 bp widening).
    ///
    /// Applied as an additional flat Z-spread during valuation by pricers that
    /// support spread-based revaluation. Currently consumed by `Bond::base_value`
    /// for bonds without embedded options, without an assigned credit curve, and
    /// without a price-pinning quote override other than `quoted_z_spread`
    /// (where the shock is additive on the quoted spread). See
    /// [`Instrument::scenario_spread_shock_supported`](crate::instruments::Instrument::scenario_spread_shock_supported).
    ///
    /// Setting this on an unsupported configuration produces a validation error
    /// at pricing time rather than a silent no-op. For hazard-priced (credit
    /// curve) bonds, shock the hazard curve instead (e.g. a par-CDS curve bump).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_spread_shock_bp: Option<f64>,
}

impl ScenarioPricingOverrides {
    /// Return whether no scenario-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Apply a scenario price shock as a decimal fraction.
    ///
    /// # Arguments
    ///
    /// * `shock` - Relative price shock as a decimal fraction (`-0.05` = -5%).
    pub fn with_scenario_price_shock_decimal(mut self, shock: f64) -> Self {
        self.scenario_price_shock_decimal = Some(shock);
        self
    }

    /// Apply a scenario spread shock in basis points.
    ///
    /// # Arguments
    ///
    /// * `shock_bp` - Flat Z-spread shock in basis points (e.g. `150.0`).
    pub fn with_scenario_spread_shock_bp(mut self, shock_bp: f64) -> Self {
        self.scenario_spread_shock_bp = Some(shock_bp);
        self
    }

    /// Validate scenario shocks for finiteness.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        check_finite_fields(&[
            (self.scenario_price_shock_decimal, false),
            (self.scenario_spread_shock_bp, false),
        ])
    }

    /// Apply the configured price shock to a present value.
    ///
    /// # Arguments
    ///
    /// * `value` - Unshocked present value in the instrument's settlement currency.
    pub fn apply_to_value(&self, value: Money) -> finstack_quant_core::Result<Money> {
        let Some(shock) = self.scenario_price_shock_decimal else {
            return Ok(value);
        };
        Money::from_model_value(value.amount() * (1.0 + shock), value.currency())
    }
}
