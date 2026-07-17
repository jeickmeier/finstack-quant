//! Inflation cap/floor instrument and pricing logic.
//!
//! Prices YoY inflation caps/floors using Black-76 (lognormal) or
//! Bachelier (normal) on the forward YoY inflation rate.
//!
//! # Inflation Rate Convention
//!
//! This module computes **period inflation rates** based on the schedule's accrual periods:
//!
//! ```text
//! forward_rate = (CPI_end / CPI_start - 1) / accrual_fraction
//! ```
//!
//! For annual frequency, this equals the true Year-over-Year (YoY) rate. For other
//! frequencies (semi-annual, quarterly), the rate is annualized over the shorter period.
//!
//! **Important**: If you need true YoY rates regardless of payment frequency (i.e.,
//! `CPI(T) / CPI(T - 1 year) - 1`), ensure the schedule uses annual frequency or
//! adjust the CPI observation dates accordingly.
//!
//! # Volatility Convention
//!
//! The volatility surface must match the pricing model convention:
//! - **Black-76 (lognormal)**: Vol surface should contain lognormal vols (percentage of rate)
//! - **Bachelier (normal)**: Vol surface should contain normal vols (absolute rate terms)
//!
//! # Observation Lag
//!
//! Inflation indices typically have an observation lag (e.g., 3 months for US CPI).
//! The lag is applied to both CPI lookups and the fixing date used for volatility.

use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::rates::cap_floor::pricing::{
    black as black_ir, normal as normal_ir, payoff::CapletFloorletInputs,
};
use crate::pricer::ModelKey;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use rust_decimal::Decimal;

/// Inflation option type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InflationCapFloorType {
    /// Cap (portfolio of caplets).
    Cap,
    /// Floor (portfolio of floorlets).
    Floor,
    /// Single-period caplet.
    Caplet,
    /// Single-period floorlet.
    Floorlet,
}

impl InflationCapFloorType {
    fn is_cap(self) -> bool {
        matches!(
            self,
            InflationCapFloorType::Cap | InflationCapFloorType::Caplet
        )
    }
}

impl std::fmt::Display for InflationCapFloorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InflationCapFloorType::Cap => write!(f, "cap"),
            InflationCapFloorType::Floor => write!(f, "floor"),
            InflationCapFloorType::Caplet => write!(f, "caplet"),
            InflationCapFloorType::Floorlet => write!(f, "floorlet"),
        }
    }
}

impl std::str::FromStr for InflationCapFloorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cap" => Ok(InflationCapFloorType::Cap),
            "floor" => Ok(InflationCapFloorType::Floor),
            "caplet" => Ok(InflationCapFloorType::Caplet),
            "floorlet" => Ok(InflationCapFloorType::Floorlet),
            other => Err(format!("Unknown inflation option type: {}", other)),
        }
    }
}

/// YoY inflation cap/floor instrument.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct InflationCapFloor {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Cap/floor type (cap, floor, caplet, floorlet).
    pub option_type: InflationCapFloorType,
    /// Notional amount in quote currency.
    pub notional: Money,
    /// Strike (annualized, decimal).
    pub strike: Decimal,
    /// Start date of the first inflation period.
    #[schemars(with = "String")]
    pub start_date: Date,
    /// End date of the final inflation period.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Payment frequency (ignored for caplet/floorlet).
    pub frequency: Tenor,
    /// Day count convention for accrual and option time.
    pub day_count: DayCount,
    /// Schedule stub convention.
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention for schedule and payments.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Optional holiday calendar identifier.
    #[builder(optional)]
    pub calendar_id: Option<String>,
    /// Inflation index/curve identifier (e.g., US-CPI-U).
    pub inflation_index_id: CurveId,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
    /// Volatility surface identifier.
    pub vol_surface_id: CurveId,
    /// Pricing overrides (implied volatility, surface extrapolation).
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Optional contract-level lag override.
    #[builder(optional)]
    pub lag_override: Option<InflationLag>,

    // --- YoY convexity / timing adjustment parameters ---
    //
    // A YoY inflation caplet pays `(CPI(Tᵢ)/CPI(Tᵢ₋₁) − 1 − K)⁺`. Under the
    // `Tᵢ`-payment measure `E[CPI(Tᵢ)/CPI(Tᵢ₋₁)] ≠ CPI_fwd(Tᵢ)/CPI_fwd(Tᵢ₋₁)`:
    // the ratio carries a YoY convexity/timing correction (Brigo-Mercurio Ch.
    // 16; Mercurio 2005). Feeding the raw deterministic forward ratio into
    // Black-76/Bachelier omits it. The leading-order Jarrow-Yildirim correction
    // `C ≈ σ_I·(σ_I − ρ·σ_n)·τ` needs the inflation/nominal-rate correlation
    // and the nominal short-rate volatility (the inflation vol `σ_I` comes from
    // the vol surface).
    /// Correlation between the inflation index and the nominal short rate,
    /// used in the YoY convexity/timing adjustment. `None` ⇒ treated as 0
    /// (the timing term vanishes; the pure inflation-vol Jensen convexity
    /// `σ_I²·τ` is still applied).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inflation_nominal_correlation: Option<f64>,
    /// Nominal short-rate volatility `σ_n` (annualized, absolute), used in the
    /// YoY timing term. `None` ⇒ the `ρ·σ_n` timing term is dropped.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_rate_volatility: Option<f64>,

    /// Attributes for scenario selection and tagging.
    pub attributes: Attributes,
}

impl InflationCapFloor {
    /// Create a canonical example USD 5Y inflation cap (US-CPI, 3% strike, $1M notional).
    ///
    /// Returns a 5-year YoY inflation cap with annual frequency, 3-month CPI lag,
    /// and lognormal vol convention.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        InflationCapFloor::builder()
            .id(InstrumentId::new("INFLCAP-USD-5Y"))
            .option_type(InflationCapFloorType::Cap)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike(Decimal::try_from(0.03).expect("valid decimal"))
            .start_date(
                Date::from_calendar_date(2024, Month::January, 15).expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2029, Month::January, 15).expect("Valid example date"),
            )
            .frequency(Tenor::annual())
            .day_count(DayCount::Act365F)
            .stub(StubKind::ShortFront)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("USD-INFL-VOL"))
            .lag_override(InflationLag::Months(3))
            .attributes(Attributes::new())
            .build()
            .expect("Example InflationCapFloor construction should not fail")
    }

    /// Validate structural invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_or(
            self.start_date < self.maturity,
            finstack_quant_core::InputError::InvalidDateRange,
        )?;
        validation::require_or(
            self.notional.amount() > 0.0,
            finstack_quant_core::InputError::NonPositiveValue,
        )?;
        validation::require_or(
            self.frequency.count() != 0,
            finstack_quant_core::InputError::Invalid,
        )?;
        Ok(())
    }

    pub(crate) fn strike_f64(&self) -> finstack_quant_core::Result<f64> {
        decimal_to_f64(self.strike, "InflationCapFloor strike")
    }

    /// Minimum reasonable CPI value for developed market indices.
    /// Used to catch data errors that could cause numerical instability.
    const MIN_REASONABLE_CPI: f64 = 50.0;

    fn effective_lag(&self, curves: &MarketContext) -> InflationLag {
        crate::instruments::common_impl::helpers::resolve_inflation_lag(
            self.lag_override,
            self.inflation_index_id.as_str(),
            curves,
        )
    }

    fn lagged_fixing_date(&self, curves: &MarketContext, date: Date) -> Date {
        crate::instruments::common_impl::helpers::apply_inflation_lag(
            date,
            self.effective_lag(curves),
        )
    }

    fn cpi_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        let lagged_date = self.lagged_fixing_date(curves, date);

        // Only consult realized fixings for observations whose (lagged) fixing
        // date is on or before the valuation date. Reading later entries from a
        // fixing series that extends past as_of would introduce look-ahead bias.
        if lagged_date <= as_of {
            let index = curves.get_inflation_index(self.inflation_index_id.as_str())?;
            let value = crate::instruments::common_impl::helpers::realized_inflation_index_value(
                index.as_ref(),
                date,
                lagged_date,
                self.effective_lag(curves),
            )?;
            return Self::validate_cpi_value(value, date);
        }

        // Fall back to curve projection with lag adjustment, honoring the
        // curve's anchor convention: epoch-anchored ("1970-01-01" default
        // anchor) curves are read at Act365F(as_of -> lagged) while date-based
        // (rebased) curves are read via `cpi_on_date` — the same anchor-aware
        // branch the inflation swaps use.
        let curve = curves.get_inflation_curve(self.inflation_index_id.as_str())?;
        let value = crate::instruments::rates::inflation_swap::InflationSwap::curve_cpi_value(
            curve.as_ref(),
            as_of,
            lagged_date,
        )?;
        Self::validate_cpi_value(value, date)
    }

    /// Validate that a CPI value is reasonable and won't cause numerical issues.
    fn validate_cpi_value(value: f64, date: Date) -> finstack_quant_core::Result<f64> {
        if value <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CPI value must be positive; got {:.4} on {}",
                value, date
            )));
        }
        if value < Self::MIN_REASONABLE_CPI {
            tracing::warn!(
                cpi = value,
                date = %date,
                min_reasonable = Self::MIN_REASONABLE_CPI,
                "CPI value is below minimum reasonable threshold for developed markets; \
                 this may indicate a data error"
            );
        }
        Ok(value)
    }

    fn schedule(&self) -> finstack_quant_core::Result<Vec<(Date, Date, Date)>> {
        if matches!(
            self.option_type,
            InflationCapFloorType::Caplet | InflationCapFloorType::Floorlet
        ) {
            let pay = crate::cashflow::builder::calendar::adjust_date(
                self.maturity,
                self.bdc,
                self.calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            )?;
            return Ok(vec![(self.start_date, self.maturity, pay)]);
        }

        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.start_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: self.stub,
                bdc: self.bdc,
                calendar_id: self
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        Ok(periods
            .into_iter()
            .map(|period| {
                (
                    period.accrual_start,
                    period.accrual_end,
                    period.payment_date,
                )
            })
            .collect())
    }

    /// Price using an explicit model key (Black-76 or Normal).
    ///
    /// # Model Selection
    ///
    /// - **Black-76**: Standard for positive inflation expectations. Requires `forward > 0` and `strike > 0`.
    /// - **Normal (Bachelier)**: Use when deflation is possible or strike is at/below zero.
    pub fn npv_with_model(
        &self,
        curves: &MarketContext,
        as_of: Date,
        model: ModelKey,
    ) -> finstack_quant_core::Result<Money> {
        let pv = self.npv_raw_with_model(curves, as_of, model)?;
        Ok(Money::new(pv, self.notional.currency()))
    }

    /// Raw (unrounded `f64`) present value, used by finite-difference metrics
    /// (gamma) where Money quantization noise would be amplified by tiny bump
    /// sizes.
    pub fn npv_raw_with_model(
        &self,
        curves: &MarketContext,
        as_of: Date,
        model: ModelKey,
    ) -> finstack_quant_core::Result<f64> {
        let strike = self.strike_f64()?;
        let disc = curves.get_discount(self.discount_curve_id.as_str())?;

        let mut total_pv = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }

            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;
            if accrual <= 0.0 {
                continue;
            }

            // CPI values are validated inside cpi_value()
            let cpi_start = self.cpi_value(curves, as_of, start)?;
            let cpi_end = self.cpi_value(curves, as_of, end)?;

            // Deterministic forward YoY ratio from the CPI curve. The YoY
            // *rate* over the period is `ratio − 1`, and the rate the option is
            // written on (annualized) is `(ratio − 1) / accrual`.
            let deterministic_ratio = cpi_end / cpi_start;
            let deterministic_rate = (deterministic_ratio - 1.0) / accrual;

            // Use consolidated lag method for fixing date
            let fixing_date = self.lagged_fixing_date(curves, end);

            // Time-to-fixing uses ACT/365F (standard option market convention)
            // regardless of the instrument's accrual day count. A failed
            // day-count calculation is propagated, not silently collapsed to
            // `t_fix = 0` — a spurious zero would force intrinsic-only pricing
            // and drop all option time value with no diagnostic.
            let t_fix = DayCount::Act365F.signed_year_fraction(
                as_of,
                fixing_date,
                DayCountContext::default(),
            )?;

            // Date-based DF from as_of to payment: correct when the curve base
            // date differs from as_of.
            let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                disc.as_ref(),
                as_of,
                pay,
            )?;

            // Volatility at the option strike (smile) prices the Black-76 /
            // Bachelier payoff.
            let sigma = if t_fix > 0.0 {
                crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
                    &self.instrument_pricing_overrides.market_quotes,
                    curves,
                    self.vol_surface_id.as_str(),
                    t_fix,
                    strike,
                )?
            } else {
                0.0
            };

            // YoY convexity / timing adjustment (Brigo-Mercurio Ch. 16;
            // Mercurio 2005). `E^{Tᵢ}[CPI(Tᵢ)/CPI(Tᵢ₋₁)] ≠ deterministic
            // ratio`: apply the leading-order Jarrow-Yildirim correction so the
            // forward rate fed to the option model is the payment-measure
            // expectation, not the raw deterministic ratio. The convexity uses
            // the ATM inflation vol σ(F) (a property of the YoY distribution),
            // not the strike vol.
            //
            // NOTE: because this forward is itself vol-dependent, the
            // Cap−Floor parity residual `Cap(K) − Floor(K) = DF·N·τ·(F − K)`
            // is also vol-dependent — that is the YoY convexity, NOT a
            // put-call-parity violation. Both legs share this same forward, so
            // the option time value cancels exactly; see the
            // `test_cap_floor_parity_strike_difference_is_vol_independent`
            // regression test, which confirms the strike-difference (where `F`
            // cancels) is vol-independent.
            let forward_rate = if t_fix > 0.0 {
                let atm_sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
                    &self.instrument_pricing_overrides.market_quotes,
                    curves,
                    self.vol_surface_id.as_str(),
                    t_fix,
                    deterministic_rate,
                )?;
                yoy_convexity_adjusted_rate(
                    deterministic_ratio,
                    accrual,
                    atm_sigma,
                    self.inflation_nominal_correlation,
                    self.nominal_rate_volatility,
                )
            } else {
                deterministic_rate
            };

            let inputs = CapletFloorletInputs {
                is_cap: self.option_type.is_cap(),
                notional: self.notional.amount(),
                strike,
                forward: forward_rate,
                discount_factor: df,
                volatility: sigma,
                time_to_fixing: t_fix,
                accrual_year_fraction: accrual,
                currency: self.notional.currency(),
            };
            let leg_pv = match model {
                ModelKey::Normal => normal_ir::price_caplet_floorlet(inputs)?,
                _ => {
                    // Black-76 requires a strictly positive forward and strike.
                    // For deflation scenarios (forward_rate ≤ 0) or non-positive
                    // strikes, transparently fall back to Bachelier (Normal)
                    // with the input lognormal vol converted to a normal vol
                    // via the standard mapping — matching how the regular cap/
                    // floor pricer handles the negative-rate regime. This makes
                    // inflation cap/floor pricing safe under deflation without
                    // requiring the user to manually switch models per quote.
                    let needs_normal_fallback =
                        t_fix > 0.0 && (forward_rate <= 0.0 || strike <= 0.0);
                    if needs_normal_fallback {
                        tracing::debug!(
                            forward = forward_rate,
                            strike,
                            t_fix,
                            "inflation cap/floor: forward/strike non-positive, \
                             falling back from Black-76 to Bachelier (normal)"
                        );
                        let normal_vol =
                            crate::instruments::rates::swaption::types::lognormal_to_normal_vol(
                                sigma,
                                forward_rate,
                                strike,
                                t_fix,
                                None,
                            );
                        normal_ir::price_caplet_floorlet(CapletFloorletInputs {
                            volatility: normal_vol,
                            ..inputs
                        })?
                    } else {
                        black_ir::price_caplet_floorlet(inputs)?
                    }
                }
            };

            total_pv += leg_pv.amount();
        }

        Ok(total_pv)
    }
}

impl InflationCapFloorBuilder {
    /// Set the strike using a typed rate.
    pub fn strike_rate(mut self, rate: Rate) -> Self {
        self.strike = Decimal::try_from(rate.as_decimal()).ok();
        self
    }
}

/// Convexity / timing-adjusted forward YoY inflation rate.
///
/// A YoY inflation caplet pays `(CPI(Tᵢ)/CPI(Tᵢ₋₁) − 1 − K)⁺` at `Tᵢ`. Under
/// the nominal `Tᵢ`-forward measure the expected YoY ratio is **not** the
/// deterministic ratio of forward CPIs:
///
/// ```text
/// E^{n,Tᵢ}[CPI(Tᵢ)/CPI(Tᵢ₋₁)] = (CPI_fwd(Tᵢ)/CPI_fwd(Tᵢ₋₁)) · exp(C)
/// ```
///
/// `CPI(Tᵢ₋₁)` enters in the denominator (a Jensen convexity) and is observed
/// at `Tᵢ₋₁` while the payoff settles under the `Tᵢ`-forward measure (a timing
/// correction). In the Jarrow-Yildirim Gaussian model the leading-order
/// correction over a period of length `τ = Tᵢ − Tᵢ₋₁` is
///
/// ```text
/// C ≈ σ_I · (σ_I − ρ · σ_n) · τ
/// ```
///
/// where:
/// - `σ_I` — inflation (YoY) volatility (ATM, from the vol surface),
/// - `σ_n` — nominal short-rate volatility,
/// - `ρ` — correlation between the inflation index and the nominal rate.
///
/// The `σ_I²·τ` term is the Jensen convexity of the log-normal YoY ratio; the
/// `−ρ·σ_I·σ_n·τ` term is the measure/timing correction. When `ρ` or `σ_n`
/// are unavailable the timing term is dropped but the pure inflation-vol
/// convexity is still applied — feeding the unadjusted deterministic ratio
/// (`C = 0`) is never correct under stochastic inflation.
///
/// # Arguments
///
/// * `deterministic_ratio` — `CPI_fwd(Tᵢ)/CPI_fwd(Tᵢ₋₁)` from the inflation curve.
/// * `accrual` — accrual year fraction of the YoY period (`≈ τ`).
/// * `inflation_vol` — ATM inflation (YoY) volatility `σ_I`.
/// * `nominal_correlation` — `ρ`; `None` ⇒ timing term dropped.
/// * `nominal_rate_vol` — `σ_n`; `None` ⇒ timing term dropped.
///
/// # Returns
///
/// The convexity/timing-adjusted **annualized** YoY rate, ready for Black-76 /
/// Bachelier: `(deterministic_ratio · exp(C) − 1) / accrual`.
///
/// # References
///
/// - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models — Theory and
///   Practice* (2nd ed.), Ch. 16 (inflation-indexed derivatives, JY model).
/// - Mercurio, F. (2005). "Pricing Inflation-Indexed Derivatives."
///   *Quantitative Finance*, 5(3), 289-302.
pub fn yoy_convexity_adjusted_rate(
    deterministic_ratio: f64,
    accrual: f64,
    inflation_vol: f64,
    nominal_correlation: Option<f64>,
    nominal_rate_vol: Option<f64>,
) -> f64 {
    if accrual <= 0.0 || !deterministic_ratio.is_finite() {
        return (deterministic_ratio - 1.0) / accrual.max(f64::MIN_POSITIVE);
    }
    let sigma_i = inflation_vol.max(0.0);
    // Timing term coefficient ρ·σ_n; absent unless both parameters supplied.
    let rho_sigma_n = match (nominal_correlation, nominal_rate_vol) {
        (Some(rho), Some(sigma_n)) => rho.clamp(-1.0, 1.0) * sigma_n.max(0.0),
        _ => 0.0,
    };
    // Leading-order JY correction C = σ_I·(σ_I − ρ·σ_n)·τ.
    let correction = sigma_i * (sigma_i - rho_sigma_n) * accrual;
    let adjusted_ratio = deterministic_ratio * correction.exp();
    (adjusted_ratio - 1.0) / accrual
}

impl crate::instruments::common_impl::traits::Instrument for InflationCapFloor {
    impl_instrument_base!(crate::pricer::InstrumentType::InflationCapFloor);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_inflation_curve(self.inflation_index_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike_f64()?),
            ),
        );
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.npv_with_model(curves, as_of, crate::pricer::ModelKey::Black76)
    }

    fn base_value_raw(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw_with_model(curves, as_of, crate::pricer::ModelKey::Black76)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.start_date)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    InflationCapFloor,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod yoy_convexity_tests {
    use super::yoy_convexity_adjusted_rate;

    /// Item 3: with stochastic inflation, the convexity-adjusted YoY rate must
    /// differ from the deterministic ratio. The Jensen term `σ_I²·τ` raises the
    /// forward; feeding the raw deterministic ratio (zero convexity) is wrong.
    #[test]
    fn convexity_raises_forward_above_deterministic() {
        // 1-year YoY period, deterministic ratio 1.025 (2.5% YoY), 1.5% inflation vol.
        let ratio = 1.025_f64;
        let accrual = 1.0_f64;
        let sigma_i = 0.015_f64;

        let deterministic = (ratio - 1.0) / accrual;
        // No correlation/nominal vol -> pure Jensen convexity σ_I²·τ.
        let adjusted = yoy_convexity_adjusted_rate(ratio, accrual, sigma_i, None, None);

        assert!(
            adjusted > deterministic,
            "YoY convexity must raise the forward above the deterministic ratio: \
             adjusted={adjusted}, deterministic={deterministic}"
        );
        // Leading-order check: adjusted ratio ≈ ratio·exp(σ_I²·τ).
        let expected_ratio = ratio * (sigma_i * sigma_i * accrual).exp();
        let expected_rate = (expected_ratio - 1.0) / accrual;
        assert!(
            (adjusted - expected_rate).abs() < 1e-12,
            "adjusted rate must equal the JY leading-order form"
        );
    }

    /// The timing term `−ρ·σ_I·σ_n·τ` reduces the convexity for positive
    /// inflation/nominal correlation. With `ρ > 0` and a nominal vol supplied,
    /// the adjusted forward is below the Jensen-only (ρ = 0) value.
    #[test]
    fn positive_correlation_reduces_convexity_via_timing_term() {
        let ratio = 1.025_f64;
        let accrual = 1.0_f64;
        let sigma_i = 0.015_f64;

        let jensen_only = yoy_convexity_adjusted_rate(ratio, accrual, sigma_i, None, None);
        let with_timing =
            yoy_convexity_adjusted_rate(ratio, accrual, sigma_i, Some(0.40), Some(0.010));

        assert!(
            with_timing < jensen_only,
            "positive inflation/nominal correlation must reduce the YoY convexity \
             via the timing term: with_timing={with_timing}, jensen_only={jensen_only}"
        );
        // Both must still exceed the deterministic rate for these moderate params
        // (σ_I − ρ·σ_n = 0.015 − 0.4·0.01 = 0.011 > 0).
        let deterministic = (ratio - 1.0) / accrual;
        assert!(with_timing > deterministic);
    }

    /// Degenerate guard: a non-positive accrual must not panic.
    #[test]
    fn zero_accrual_is_handled() {
        let r = yoy_convexity_adjusted_rate(1.02, 0.0, 0.015, None, None);
        assert!(
            r.is_finite() || r.is_infinite(),
            "must not panic on zero accrual"
        );
    }
}
