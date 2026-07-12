//! Interest Rate Future types and implementation.
//!
//! # Convexity Adjustment
//!
//! Interest rate futures (e.g., Eurodollar, SOFR futures) are margined daily,
//! creating a convexity bias between futures rates and forward rates. This
//! adjustment accounts for the correlation between rates and present values.
//!
//! ## When to Apply
//!
//! - **Short-dated contracts (< 1Y)**: Convexity adjustment is typically negligible
//!   (< 1bp) and can often be ignored
//! - **Medium-dated contracts (1-5Y)**: Adjustment is material (1-10bp) and should
//!   be included for pricing and curve building
//! - **Long-dated contracts (> 5Y)**: Adjustment can be significant (10-50bp+) and
//!   is essential for accurate pricing
//!
//! ## Methods
//!
//! 1. **Fixed adjustment**: Set `convexity_adjustment` in [`FutureContractSpecs`] to
//!    a pre-computed value (e.g., from broker quotes or historical analysis)
//! 2. **Model-based**: Provide a `vol_surface_id` to compute the adjustment using
//!    the Hull-White zero-mean-reversion approximation. See
//!    [`InterestRateFuture::calculate_convexity_adjusted_rate`] for the exact
//!    formula and references; in short the adjustment depends on the underlying
//!    period endpoints `[T_start, T_end]`, not the fixing date.
//!
//! For STIR futures on SOFR, adjustments are typically sourced from broker screens
//! or implied from listed options.
use crate::cashflow::traits::CashflowProvider;
use crate::constants::ONE_BASIS_POINT;
// Params-based constructor removed; build via builder instead.
use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use time::macros::date;

/// Interest Rate Future instrument.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct InterestRateFuture {
    /// Unique identifier
    pub id: InstrumentId,
    /// Exposure size expressed in currency units. PV is scaled by
    /// `notional.amount() / contract_specs.face_value` to support
    /// multiples of the standard contract.
    pub notional: Money,
    /// Future expiry/delivery date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Underlying rate fixing date.
    ///
    /// Defaults to `expiry` when omitted.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub fixing_date: Option<Date>,
    /// Rate period start date.
    ///
    /// Defaults to 2 calendar days after fixing date when omitted.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub period_start: Option<Date>,
    /// Rate period end date.
    ///
    /// Defaults to `period_start + contract_specs.delivery_months` months when omitted.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub period_end: Option<Date>,
    /// Quoted future price (e.g., 99.25)
    pub quoted_price: f64,
    /// Day count convention
    pub day_count: DayCount,
    /// Position side (Long or Short)
    pub position: Position,
    /// Contract specifications
    pub contract_specs: FutureContractSpecs,
    /// Discount curve identifier
    pub discount_curve_id: CurveId,
    /// Forward curve identifier
    pub forward_curve_id: CurveId,
    /// Optional volatility surface identifier for convexity adjustment
    pub vol_surface_id: Option<CurveId>,
    /// Attributes
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Contract specifications for interest rate futures.
///
/// Encapsulates exchange-defined contract parameters and optional convexity
/// adjustment for pricing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct FutureContractSpecs {
    /// Face value of contract (e.g., $1,000,000 for Eurodollar/SOFR futures)
    pub face_value: f64,
    /// Tick size in price points (e.g., 0.0025 = 0.25bp for SOFR futures)
    pub tick_size: f64,
    /// Tick value in currency units (e.g., $6.25 for 3M SOFR)
    pub tick_value: f64,
    /// Number of delivery months (e.g., 3 for quarterly contracts)
    pub delivery_months: u8,
    /// Optional pre-computed convexity adjustment (in rate terms).
    ///
    /// # Usage
    ///
    /// - `Some(0.0)`: Explicitly disable model-based adjustment (strict mode)
    /// - `Some(x)`: Use fixed adjustment of `x` (e.g., from broker quote)
    /// - `None`: Compute adjustment from volatility surface (requires `vol_surface_id`)
    ///
    /// # Market Practice
    ///
    /// For calibration, use `Some(0.0)` and let the curve fitting process
    /// implicitly absorb the convexity. For pricing with a pre-built curve,
    /// either:
    /// - Use a fixed adjustment from broker/vendor data
    /// - Provide a volatility surface for model-based calculation
    pub convexity_adjustment: Option<f64>,
}

impl FutureContractSpecs {
    /// Standard CME 3-month SOFR future (`CME:SR3`) contract specifications.
    ///
    /// These are exchange-defined, well-known constants and mirror the
    /// `CME:SR3` entry in the embedded IR-future conventions registry. They are
    /// hardcoded here so that [`FutureContractSpecs::default`] is infallible and
    /// can never panic — a `Default` impl must always succeed.
    pub const CME_SR3: FutureContractSpecs = FutureContractSpecs {
        face_value: 1_000_000.0,
        tick_size: 0.0025,
        tick_value: 6.25,
        delivery_months: 3,
        convexity_adjustment: None,
    };
}

impl Default for FutureContractSpecs {
    /// Returns the standard CME 3-month SOFR future ([`FutureContractSpecs::CME_SR3`]).
    ///
    /// This is infallible: it returns hardcoded exchange constants rather than
    /// looking them up in a registry, so it cannot panic.
    fn default() -> Self {
        Self::CME_SR3
    }
}

/// Position side for futures.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Position {
    /// Long position (buyer of futures contract)
    Long,
    /// Short position (seller of futures contract)
    Short,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Long => write!(f, "long"),
            Position::Short => write!(f, "short"),
        }
    }
}

impl std::str::FromStr for Position {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "long" => Ok(Position::Long),
            "short" => Ok(Position::Short),
            other => Err(format!("Unknown position: {}", other)),
        }
    }
}

impl InterestRateFuture {
    pub(crate) fn resolve_dates(&self) -> finstack_quant_core::Result<(Date, Date, Date)> {
        let fixing = self.fixing_date.unwrap_or(self.expiry);
        let period_start = self
            .period_start
            .unwrap_or(fixing + time::Duration::days(2));
        let period_end = if let Some(end) = self.period_end {
            end
        } else {
            period_start.add_months(self.contract_specs.delivery_months as i32)
        };
        if period_end < period_start {
            return Err(finstack_quant_core::error::InputError::InvalidDateRange.into());
        }
        Ok((fixing, period_start, period_end))
    }

    // Note: use the builder (FinancialBuilder) for construction.

    /// Create a canonical example 3M Eurodollar-style interest rate future.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        // SAFETY: All inputs are compile-time validated constants
        InterestRateFuture::builder()
            .id(InstrumentId::new("IRF-ED-3M-MAR25"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(date!(2025 - 03 - 17))
            .fixing_date_opt(Some(date!(2025 - 03 - 17)))
            .period_start_opt(Some(date!(2025 - 03 - 19)))
            .period_end_opt(Some(date!(2025 - 06 - 18)))
            .quoted_price(95.50)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .position(Position::Long)
            .contract_specs(FutureContractSpecs {
                convexity_adjustment: Some(0.0), // Strict mode requires explicit adjustment or vol surface
                ..FutureContractSpecs::default()
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-SOFR-3M"))
            .attributes(Attributes::new())
            .build()
    }

    /// Set contract specifications.
    pub fn with_contract_specs(mut self, specs: FutureContractSpecs) -> Self {
        self.contract_specs = specs;
        self
    }

    /// Get implied rate from quoted price.
    ///
    /// Interest rate futures quote as 100 minus the rate, i.e., a price of 97.50
    /// implies a 2.50% rate.
    pub fn implied_rate(&self) -> Rate {
        Rate::from_percent(100.0 - self.quoted_price)
    }

    /// Forward rate over the underlying futures accrual period.
    pub(crate) fn model_forward_rate(
        &self,
        context: &MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        let (_fixing_date, period_start, period_end) = self.resolve_dates()?;
        let fwd = context.get_forward(&self.forward_curve_id)?;
        crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            fwd.as_ref(),
            period_start,
            period_end,
        )
    }

    /// Convexity adjustment applied to the model forward rate.
    pub(crate) fn convexity_adjustment(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        use finstack_quant_core::dates::DayCountContext;
        if let Some(adjustment) = self.contract_specs.convexity_adjustment {
            return Ok(adjustment);
        }
        let (fixing_date, period_start, period_end) = self.resolve_dates()?;
        let fwd = context.get_forward(&self.forward_curve_id)?;
        let fwd_dc = fwd.day_count();
        let t_fixing = fwd_dc
            .year_fraction(as_of, fixing_date, DayCountContext::default())?
            .max(0.0);
        let t_start = fwd_dc
            .year_fraction(as_of, period_start, DayCountContext::default())?
            .max(0.0);
        let t_end = fwd_dc
            .year_fraction(as_of, period_end, DayCountContext::default())?
            .max(t_start);
        let forward_rate = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            fwd.as_ref(),
            period_start,
            period_end,
        )?;
        Ok(self.calculate_convexity_adjusted_rate(
            context,
            forward_rate,
            t_fixing,
            t_start,
            t_end,
        )? - forward_rate)
    }

    /// Calculates the present value of the interest rate future.
    ///
    /// PV = (model_price - contract_price) / tick_size × tick_value × contracts × position_sign
    ///
    /// Calculates the raw present value of the interest rate future (f64)
    ///
    /// # Day Count Conventions
    ///
    /// This method intentionally uses two different day count bases:
    /// - **Forward curve projection**: Uses the forward curve's own day count to compute
    ///   time-to-fixing and forward rate period. This ensures consistency with how the
    ///   curve was bootstrapped.
    /// - **Accrual calculation**: Uses the instrument's day count (`self.day_count`) for
    ///   the accrual period `tau`. This matches the contract's settlement convention.
    ///
    /// This is standard market practice: curves are interpolated in their native basis,
    /// while cashflow accruals use the instrument's contractual basis.
    ///
    /// # No Discounting
    ///
    /// Futures are marked-to-market daily with variation margin, so no discounting is
    /// applied. The PV represents the current mark-to-market gain/loss versus the
    /// quoted entry price.
    pub fn npv_raw(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        use finstack_quant_core::dates::DayCountContext;
        let (fixing_date, period_start, period_end) = self.resolve_dates()?;
        if as_of >= self.expiry {
            return Ok(0.0);
        }

        // Validate discount curve exists (required for curve dependencies, even though
        // futures don't discount due to daily margining)
        let _disc = context.get_discount(&self.discount_curve_id)?;
        let fwd = context.get_forward(&self.forward_curve_id)?;

        // Time to fixing and rate period for forward rate calculation use the forward
        // curve's day-count basis for consistency with curve construction.
        let fwd_dc = fwd.day_count();
        let t_fixing = fwd_dc
            .year_fraction(as_of, fixing_date, DayCountContext::default())?
            .max(0.0);
        let t_start_remaining = fwd_dc
            .year_fraction(as_of, period_start, DayCountContext::default())?
            .max(0.0);
        let t_end_remaining = fwd_dc
            .year_fraction(as_of, period_end, DayCountContext::default())?
            .max(t_start_remaining);
        // Forward rate over the period
        let forward_rate = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            fwd.as_ref(),
            period_start,
            period_end,
        )?;

        // Apply convexity adjustment policy
        let adjusted_rate = if let Some(ca) = self.contract_specs.convexity_adjustment {
            forward_rate + ca
        } else {
            self.calculate_convexity_adjusted_rate(
                context,
                forward_rate,
                t_fixing,
                t_start_remaining,
                t_end_remaining,
            )?
        };

        if !self.contract_specs.tick_size.is_finite()
            || self.contract_specs.tick_size <= 0.0
            || !self.contract_specs.tick_value.is_finite()
            || self.contract_specs.tick_value <= 0.0
            || !self.contract_specs.face_value.is_finite()
            || self.contract_specs.face_value <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IR Future {} requires positive finite face_value, tick_size, and tick_value",
                self.id
            )));
        }

        // Position sign: Long benefits when implied > model (rates down → price up)
        let sign = match self.position {
            Position::Long => 1.0,
            Position::Short => -1.0,
        };

        // Scale by contracts: notional may represent multiples of face value.
        // Zero face value means zero exposure (no contracts).
        let contracts_scale = if self.contract_specs.face_value > 0.0 {
            self.notional.amount() / self.contract_specs.face_value
        } else {
            0.0
        };

        let model_price = 100.0 * (1.0 - adjusted_rate);
        let price_delta = model_price - self.quoted_price;
        let pv_per_contract =
            price_delta / self.contract_specs.tick_size * self.contract_specs.tick_value;
        let pv_total = sign * contracts_scale * pv_per_contract;
        Ok(pv_total)
    }

    /// Derive contract tick value for the instrument accrual.
    ///
    /// tick_value ≈ Face × tau(period_start, period_end) × 1bp × (tick_size / 1bp)
    pub fn derived_tick_value(&self) -> finstack_quant_core::Result<f64> {
        let (_fixing_date, period_start, period_end) = self.resolve_dates()?;
        let tau = self
            .day_count
            .year_fraction(
                period_start,
                period_end,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        // tick_value = Face × tau × tick_size (tick_size is already in decimal form)
        Ok(self.contract_specs.face_value
            * tau
            * ONE_BASIS_POINT
            * (self.contract_specs.tick_size / ONE_BASIS_POINT))
    }

    /// Calculate convexity adjusted rate using volatility surface.
    ///
    /// Uses the Hull-White 1-factor model approximation in the zero-mean-reversion
    /// limit (a → 0):
    /// ```text
    /// Convexity Adjustment ≈ 0.5 × σ² × T_start × T_end
    /// ```
    ///
    /// This is Hull (10th ed., eq. 6.3) for the difference between futures-implied
    /// and forward rates of the underlying interest-rate period `[T_start, T_end]`.
    /// The convexity formula depends on the underlying period endpoints
    /// `(T_start, T_end)`, not the reset date `T_fixing`. The volatility used is
    /// sampled on the **same** time axis — at `T_start` — for consistency: a
    /// `T_fixing`-axis lookup mis-pairs the formula for SOFR-style
    /// backward-looking futures whose fixing date sits at the period end.
    /// Previous versions used `T_fixing × (T_fixing + τ)`, which silently
    /// embedded an error proportional to the reset lag (≈ 2 business days for
    /// IBOR, an entire accrual period for SOFR-style futures).
    ///
    /// The full HW formula is:
    /// ```text
    /// CA = σ² × B(0,T_start) × B(0,T_end), B(0,T) = (1 - exp(-aT)) / a
    /// ```
    /// which reduces to `0.5 σ² T_start T_end` as a → 0.
    ///
    /// # Arguments
    /// * `forward_rate` - The unadjusted forward rate from the curve
    /// * `t_fixing` - Time to fixing date in years (kept for the signature /
    ///   diagnostics; **not** used as the vol-surface expiry axis — see below)
    /// * `t_start` - Time to period start in years
    /// * `t_end` - Time to period end in years (must be >= t_start)
    ///
    /// # Vol-axis consistency (audit item 8)
    ///
    /// The convexity formula `0.5·σ²·T_start·T_end` is built from the rate
    /// variance accumulated over the underlying interest-rate period, paired
    /// `(T_start, T_end)`. The vol used must be sampled on the **same**
    /// time axis. Sampling the surface at `t_fixing` is inconsistent: for
    /// SOFR-style backward-looking futures the fixing date sits at the *period
    /// end* (`t_fixing ≈ T_end`), so the surface lookup and the
    /// `(T_start, T_end)` formula disagree by an entire accrual period. The
    /// convexity adjustment accrues until the rate locks in at the period
    /// start, so the consistent expiry-axis value is `T_start`.
    ///
    /// # Volatility units (normal / Bachelier contract)
    ///
    /// `σ` in `0.5·σ²·T_start·T_end` is an **absolute (normal) rate vol** in
    /// decimal rate units per √year — e.g. `0.012` for 120 bp/yr. Feeding a
    /// lognormal (Black) vol such as `0.20` inflates the adjustment by
    /// `(σ_LN/σ_N)² ≈ (σ_LN/(σ_N))²` — typically hundreds×. A sanity bound
    /// rejects vols above `MAX_NORMAL_RATE_VOL` (5% absolute, ≈500 bp/yr):
    /// genuine normal rate vols sit far below it while lognormal quotes sit
    /// far above it.
    fn calculate_convexity_adjusted_rate(
        &self,
        context: &MarketContext,
        forward_rate: f64,
        _t_fixing: f64,
        t_start: f64,
        t_end: f64,
    ) -> finstack_quant_core::Result<f64> {
        /// Upper sanity bound for an absolute (normal) rate vol in decimal
        /// units per √year. 500 bp/yr is far beyond any observed G10/EM normal
        /// rate vol; values above it are almost certainly lognormal quotes.
        const MAX_NORMAL_RATE_VOL: f64 = 0.05;

        // Hull-White zero-mean-reversion convexity adjustment: 0.5 σ² T_start T_end.
        let t1 = t_start.max(0.0);
        let t2 = t_end.max(t1);

        let vol_estimate = if let Some(vol_id) = &self.vol_surface_id {
            let surface = context.get_surface(vol_id)?;
            // Vol-axis consistency: sample at `T_start` (the time over which the
            // convexity variance accumulates), NOT at `t_fixing` — the latter
            // mis-pairs the `(T_start, T_end)` formula for SOFR-style futures
            // whose fixing date is at the period end.
            surface.value_checked(t1, forward_rate)?
        } else {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::NotFound {
                    id: format!(
                        "IR Future {}: Missing vol_surface_id or fixed convexity_adjustment",
                        self.id
                    ),
                },
            ));
        };

        if !(0.0..=MAX_NORMAL_RATE_VOL).contains(&vol_estimate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IR Future {}: convexity-adjustment vol {vol_estimate} from surface '{}' is \
                 outside the normal-vol sanity range [0, {MAX_NORMAL_RATE_VOL}]. The \
                 0.5·σ²·T₁·T₂ formula requires an absolute (normal/Bachelier) rate vol in \
                 decimal units (e.g. 0.012 = 120 bp/yr); a lognormal (Black) vol here \
                 inflates the adjustment by hundreds of times.",
                self.id,
                self.vol_surface_id.as_ref().map_or("", |v| v.as_str()),
            )));
        }

        let convexity = 0.5 * vol_estimate * vol_estimate * t1 * t2;
        Ok(forward_rate + convexity)
    }
}

impl crate::instruments::common_impl::traits::Instrument for InterestRateFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::InterestRateFuture);

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        MarketDependencies::from_curve_dependencies(self)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let pv = self.npv_raw(curves, as_of)?;
        Ok(finstack_quant_core::money::Money::new(
            pv,
            self.notional.currency(),
        ))
    }

    fn value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        self.period_start
            .or_else(|| self.fixing_date.map(|d| d + time::Duration::days(2)))
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl CashflowProvider for InterestRateFuture {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                ..Default::default()
            },
        ))
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for InterestRateFuture {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use time::macros::date;

    #[test]
    fn ir_future_defaults_dates_from_expiry_and_contract_specs() {
        let irf = InterestRateFuture::builder()
            .id(InstrumentId::new("IRF-DEFAULT-DATES"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(date!(2025 - 03 - 17))
            .quoted_price(95.50)
            .day_count(DayCount::Act360)
            .position(Position::Long)
            .contract_specs(FutureContractSpecs::default())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-SOFR-3M"))
            .attributes(Attributes::new())
            .build()
            .expect("build");

        assert_eq!(irf.fixing_date, None);
        assert_eq!(irf.period_start, None);
        assert_eq!(irf.period_end, None);
        let (_fixing, period_start, period_end) = irf.resolve_dates().expect("resolve dates");
        assert_eq!(period_start, date!(2025 - 03 - 19));
        assert_eq!(period_end, date!(2025 - 06 - 19));
    }

    #[test]
    fn ir_future_respects_explicit_date_overrides() {
        let irf = InterestRateFuture::builder()
            .id(InstrumentId::new("IRF-EXPLICIT-DATES"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(date!(2025 - 03 - 17))
            .fixing_date_opt(Some(date!(2025 - 03 - 18)))
            .period_start_opt(Some(date!(2025 - 03 - 20)))
            .period_end_opt(Some(date!(2025 - 06 - 20)))
            .quoted_price(95.50)
            .day_count(DayCount::Act360)
            .position(Position::Long)
            .contract_specs(FutureContractSpecs::default())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-SOFR-3M"))
            .attributes(Attributes::new())
            .build()
            .expect("build");

        let (_fixing, period_start, period_end) = irf.resolve_dates().expect("resolve dates");
        assert_eq!(period_start, date!(2025 - 03 - 20));
        assert_eq!(period_end, date!(2025 - 06 - 20));
    }

    #[test]
    fn future_contract_specs_default_is_infallible_and_matches_registry() {
        // `Default` must never panic; it returns hardcoded CME:SR3 constants.
        let specs = FutureContractSpecs::default();
        assert_eq!(specs.face_value, 1_000_000.0);
        assert_eq!(specs.tick_size, 0.0025);
        assert_eq!(specs.tick_value, 6.25);
        assert_eq!(specs.delivery_months, 3);
        assert_eq!(specs.convexity_adjustment, None);

        // Hardcoded constants must stay in sync with the embedded registry.
        let conventions = crate::market::conventions::ConventionRegistry::try_global()
            .and_then(|registry| {
                registry.require_ir_future(
                    &crate::market::conventions::ids::IrFutureContractId::new("CME:SR3"),
                )
            })
            .expect("embedded registry should contain CME:SR3");
        assert_eq!(specs.face_value, conventions.face_value);
        assert_eq!(specs.tick_size, conventions.tick_size);
        assert_eq!(specs.tick_value, conventions.tick_value);
        assert_eq!(specs.delivery_months, conventions.delivery_months);
        assert_eq!(specs.convexity_adjustment, conventions.convexity_adjustment);
    }

    #[test]
    fn convexity_adjustment_rolls_with_valuation_date_not_curve_base() {
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        use finstack_quant_core::prelude::VolSurface;

        let curve_base = date!(2025 - 01 - 02);
        let later_as_of = date!(2025 - 02 - 03);
        let mut future = InterestRateFuture::example().expect("example future");
        future.contract_specs.convexity_adjustment = None;
        future.vol_surface_id = Some(CurveId::new("USD-SR3-NORMAL-VOL"));

        let normal_vol = 0.01;
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(curve_base)
                    .knots(vec![(0.0, 1.0), (2.0, (-0.03_f64 * 2.0).exp())])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                ForwardCurve::builder("USD-SOFR-3M", 0.25)
                    .base_date(curve_base)
                    .knots(vec![(0.0, 0.04), (2.0, 0.04)])
                    .build()
                    .expect("forward curve"),
            )
            .insert_surface(
                VolSurface::builder("USD-SR3-NORMAL-VOL")
                    .expiries(&[0.1, 0.25, 0.5, 1.0])
                    .strikes(&[0.0, 0.04, 0.10])
                    .row(&[normal_vol; 3])
                    .row(&[normal_vol; 3])
                    .row(&[normal_vol; 3])
                    .row(&[normal_vol; 3])
                    .build()
                    .expect("vol surface"),
            );

        let base_adjustment = future
            .convexity_adjustment(&market, curve_base)
            .expect("base adjustment");
        let later_adjustment = future
            .convexity_adjustment(&market, later_as_of)
            .expect("rolled adjustment");

        assert!(later_adjustment < base_adjustment);
        let (_, period_start, period_end) = future.resolve_dates().expect("dates");
        let t1 = future
            .day_count
            .year_fraction(
                later_as_of,
                period_start,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("t1")
            .max(0.0);
        let t2 = future
            .day_count
            .year_fraction(
                later_as_of,
                period_end,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("t2")
            .max(t1);
        assert!((later_adjustment - 0.5 * normal_vol * normal_vol * t1 * t2).abs() < 1e-12);
    }

    #[test]
    fn ir_future_rejects_pre_curve_or_straddling_projection_period() {
        let future = InterestRateFuture::builder()
            .id(InstrumentId::new("IRF-PRE-BASE"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(date!(2025 - 01 - 15))
            .fixing_date_opt(Some(date!(2025 - 01 - 15)))
            .period_start_opt(Some(date!(2025 - 01 - 17)))
            .period_end_opt(Some(date!(2025 - 04 - 17)))
            .quoted_price(95.0)
            .day_count(DayCount::Act360)
            .position(Position::Long)
            .contract_specs(FutureContractSpecs {
                convexity_adjustment: Some(0.0),
                ..FutureContractSpecs::default()
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-SOFR-3M"))
            .attributes(Attributes::new())
            .build()
            .expect("future");
        let market = MarketContext::new().insert(
            ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(date!(2025 - 02 - 01))
                .knots([(0.0, 0.04), (2.0, 0.04)])
                .build()
                .expect("forward curve"),
        );

        let error = future
            .model_forward_rate(&market)
            .expect_err("pre-base futures period must not be clamped");
        assert!(error.to_string().contains("historical fixing"));
    }
}
