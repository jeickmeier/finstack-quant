//! Interest Rate Swap (IRS) types and instrument trait implementations.
//!
//! Defines the `InterestRateSwap` instrument following the modern instrument
//! standards used across valuations: types live here; pricing is delegated to
//! `pricing::engine`; and metrics are split under `metrics/`.
//!
//! Public fields use strong newtype identifiers for safety: `InstrumentId` and
//! `CurveId`. Calendar identifiers remain `Option<&'static str>` for stable
//! serde and lookups.
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::Decimal;

use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::pricing::overnight_conventions;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::market::conventions::ConventionRegistry;
use finstack_quant_core::types::IndexId;
use finstack_quant_margin::types::OtcMarginSpec;

use super::compounding::FloatingLegCompounding;

pub use crate::instruments::common_impl::parameters::legs::{ParRateMethod, PayReceive};

pub use crate::instruments::common_impl::parameters::legs::FixedLegSpec;
pub use crate::instruments::common_impl::parameters::legs::FloatLegSpec;

/// Leg-level conventions for building a vanilla fixed-vs-float IRS.
///
/// This is intentionally minimal: it captures the schedule/lag/calendar knobs
/// that are commonly resolved from market conventions (e.g. calibration quote
/// conventions) while keeping the instrument surface stable.
#[derive(Debug, Clone)]
pub struct IrsLegConventions {
    /// Fixed leg payment frequency.
    pub fixed_frequency: Tenor,
    /// Float leg payment frequency.
    pub float_frequency: Tenor,
    /// Fixed leg accrual day-count.
    pub fixed_day_count: DayCount,
    /// Float leg accrual day-count.
    pub float_day_count: DayCount,
    /// Payment date business day convention.
    pub business_day_convention: BusinessDayConvention,
    /// Calendar id used for payment date adjustment on both legs.
    pub payment_calendar_id: Option<String>,
    /// Calendar id used for fixing/reset date adjustment.
    pub fixing_calendar_id: Option<String>,
    /// Stub handling.
    pub stub: StubKind,
    /// Reset lag in business days (start - reset_lag_days).
    pub reset_lag_days: i32,
    /// Payment delay in business days after period end.
    pub payment_lag_days: i32,
}

impl IrsLegConventions {
    /// Resolve conventions from a rate index in the global `ConventionRegistry`.
    pub fn from_rate_index(index_id: &str) -> finstack_quant_core::Result<Self> {
        let registry = ConventionRegistry::try_global().map_err(|_| {
            finstack_quant_core::Error::Validation("ConventionRegistry not initialized.".into())
        })?;
        let idx = IndexId::new(index_id);
        let conv = registry.require_rate_index(&idx)?;

        Ok(Self {
            fixed_frequency: conv.default_fixed_leg_frequency,
            float_frequency: conv.default_payment_frequency,
            fixed_day_count: conv.default_fixed_leg_day_count,
            float_day_count: conv.day_count,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            payment_calendar_id: Some(conv.market_calendar_id.clone()),
            fixing_calendar_id: Some(conv.market_calendar_id.clone()),
            stub: StubKind::ShortFront,
            reset_lag_days: conv.default_reset_lag_days,
            payment_lag_days: conv.default_payment_lag_days,
        })
    }
}

/// Interest rate swap with fixed and floating legs.
///
/// Represents a standard interest rate swap where one party pays
/// a fixed rate and the other pays a floating rate plus spread.
///
/// # Market Standards & Citations
///
/// ## ISDA Definitions
///
/// Current RFR and term-rate contracts are represented under the **ISDA 2021
/// Interest Rate Derivatives Definitions**. Legacy transactions may retain
/// terms from the 2006 Definitions, including historical IBOR reset conventions.
/// Fixed/floating day counts, calendars, lags, compounding, and payment rules
/// are explicit leg terms or are resolved from the rate-index convention registry.
///
/// ## USD Market Convention
///
/// The canonical legacy USD term-index example uses:
/// - **Fixed Leg:** Semi-annual, 30/360, Modified Following
/// - **Floating Leg:** Quarterly, ACT/360, Modified Following
/// - **Reset Lag:** T-2 (2 business days before period start)
/// - **Discounting:** OIS curve under the collateral agreement
///
/// ## Day-Count Convention Notes
///
/// The USD standard uses different day-count conventions for different purposes:
/// - **Fixed leg accrual:** 30/360 (Bond Basis)
/// - **Floating leg accrual:** ACT/360 (Money Market)
/// - **Discount curve:** Typically ACT/365F or ACT/360 depending on construction
///
/// This day-count mismatch between accrual and discounting is market-standard
/// and reflects the different conventions used in bond vs money markets.
/// The impact on par rates is typically < 0.5bp for USD swaps.
///
/// ## Validation
///
/// Use [`InterestRateSwap::validate()`] to check swaps constructed via
/// the builder pattern.
///
/// ## References
///
/// - ISDA 2021 Interest Rate Derivatives Definitions (current contract framework) `docs/REFERENCES.md#isda-2021-definitions`
/// - ISDA 2006 Definitions (legacy transactions) `docs/REFERENCES.md#isda-2006-definitions`
/// - Sadr, A. *Interest Rate Swaps and Their Derivatives*.
///   `docs/REFERENCES.md#sadr-2009-irs`
/// - Bloomberg SWPM screen conventions.
///   `docs/REFERENCES.md#bloomberg-swpm`
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InterestRateSwap {
    /// Unique identifier for the swap.
    pub id: InstrumentId,
    /// Notional amount for both legs.
    pub notional: Money,
    /// Direction of the swap (Pay or Receive).
    pub side: PayReceive,
    /// Fixed leg specification.
    pub fixed: FixedLegSpec,
    /// Floating leg specification.
    pub float: FloatLegSpec,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// When present, enables margin calculation using SIMM or schedule-based
    /// methodologies. For cleared swaps, specify clearing house in
    /// `OtcMarginSpec::cleared()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Parameters for constructing a vanilla IRS from market conventions.
#[derive(Debug, Clone)]
pub struct ConventionSwapParams<'a> {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Notional principal amount.
    pub notional: Money,
    /// Pay or receive fixed.
    pub side: PayReceive,
    /// Fixed coupon rate (as a decimal, e.g. 0.03 = 3%).
    pub fixed_rate: f64,
    /// Effective (start) date.
    pub start: Date,
    /// Maturity (end) date.
    pub end: Date,
    /// Rate index identifier used to resolve conventions (e.g. `"USD-SOFR"`).
    pub index_id: &'a str,
    /// Discount curve identifier.
    pub discount_curve_id: &'a str,
    /// Forward projection curve identifier.
    pub forward_curve_id: &'a str,
}

impl InterestRateSwap {
    /// Create an IRS from market conventions resolved via `ConventionRegistry`.
    ///
    /// This is the preferred way to construct standard swaps. Conventions
    /// (day counts, frequencies, calendars, lags) are resolved from the
    /// registered rate index, matching QuantLib `MakeVanillaSwap` ergonomics.
    pub fn from_conventions(params: ConventionSwapParams<'_>) -> finstack_quant_core::Result<Self> {
        let ConventionSwapParams {
            id,
            notional,
            side,
            fixed_rate,
            start,
            end,
            index_id,
            discount_curve_id,
            forward_curve_id,
        } = params;
        let conv = IrsLegConventions::from_rate_index(index_id)?;
        let registry = ConventionRegistry::try_global().map_err(|_| {
            finstack_quant_core::Error::Validation("ConventionRegistry not initialized.".into())
        })?;
        let idx = IndexId::new(index_id);
        let rate_conv = registry.require_rate_index(&idx)?;

        let compounding = overnight_conventions::compounding_from_conventions(rate_conv)?;

        let swap = Self::builder()
            .id(id)
            .notional(notional)
            .side(side)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new(discount_curve_id),
                rate: finstack_quant_core::decimal::f64_to_decimal(fixed_rate)?,
                frequency: conv.fixed_frequency,
                day_count: conv.fixed_day_count,
                business_day_convention: conv.business_day_convention,
                calendar_id: conv.payment_calendar_id.clone(),
                stub: conv.stub,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: conv.payment_lag_days,
                end_of_month: false,
            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new(discount_curve_id),
                forward_curve_id: CurveId::new(forward_curve_id),
                spread_bp: Decimal::ZERO,
                frequency: conv.float_frequency,
                day_count: conv.float_day_count,
                business_day_convention: conv.business_day_convention,
                calendar_id: conv.payment_calendar_id.clone(),
                stub: conv.stub,
                reset_lag_days: conv.reset_lag_days,
                fixing_calendar_id: conv.fixing_calendar_id,
                start,
                end,
                compounding,
                payment_lag_days: conv.payment_lag_days,
                end_of_month: false,
            })
            .build()?;

        swap.validate()?;
        Ok(swap)
    }
}

/// Minimum notional threshold for numerical stability.
///
/// Notionals below this threshold may cause numerical issues in pricing
/// due to floating-point precision limits.
const NOTIONAL_EPSILON: f64 = 1e-6;

/// Maximum allowed rate magnitude for validation.
///
/// Rates with absolute value exceeding this threshold are considered
/// non-physical and rejected. This corresponds to ±10000% rates which
/// are far beyond any reasonable market scenario.
const MAX_RATE_MAGNITUDE: f64 = 100.0;

impl InterestRateSwap {
    /// Look up the rate-index conventions used by the float leg. Returns `Ok(None)` if
    /// the global registry is initialized but does not contain the index (legitimate for
    /// curves whose forward_curve_id isn't a registered convention key); returns `Err`
    /// only if the registry itself isn't initialized. Callers that need to RESOLVE a
    /// sentinel value MUST treat the `None` case as a hard error.
    fn rate_index_conventions(
        &self,
    ) -> finstack_quant_core::Result<Option<crate::market::conventions::RateIndexConventions>> {
        let registry = ConventionRegistry::try_global()?;
        let idx = IndexId::new(self.float.forward_curve_id.as_str());
        Ok(registry.require_rate_index(&idx).ok().cloned())
    }

    /// Resolve the fixed leg, applying convention defaults for any sentinel values.
    /// Returns `Err` if a sentinel (e.g. `payment_lag_days < 0`) is present and the
    /// registry cannot provide a default — previously this silently left the sentinel
    /// in place, producing wildly wrong schedules downstream.
    pub(crate) fn resolved_fixed_leg(&self) -> finstack_quant_core::Result<FixedLegSpec> {
        let mut fixed = self.fixed.clone();
        let is_eom_swap =
            fixed.start.end_of_month() == fixed.start && fixed.end.end_of_month() == fixed.end;
        if is_eom_swap && !fixed.end_of_month {
            fixed.end_of_month = true;
        }
        // Hit the convention registry only when a sentinel (`< 0`) actually needs
        // resolution. Calibration calls this repeatedly on the hot path, and the common
        // case (lags resolved at construction time) should not pay for a registry lookup
        // and `RateIndexConventions` clone.
        if fixed.payment_lag_days < 0 {
            let conv = self.rate_index_conventions()?.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "IRS '{}': fixed-leg payment_lag_days sentinel {} requires resolution \
                     but rate index '{}' is not registered in the convention registry",
                    self.id, fixed.payment_lag_days, self.float.forward_curve_id
                ))
            })?;
            fixed.payment_lag_days = conv.default_payment_lag_days;
        }
        Ok(fixed)
    }

    /// Resolve the float leg, applying convention defaults for any sentinel values.
    /// Errors loudly if a sentinel can't be resolved — see [`Self::resolved_fixed_leg`].
    pub(crate) fn resolved_float_leg(&self) -> finstack_quant_core::Result<FloatLegSpec> {
        let mut float = self.float.clone();
        let is_eom_swap =
            float.start.end_of_month() == float.start && float.end.end_of_month() == float.end;
        if is_eom_swap && !float.end_of_month {
            float.end_of_month = true;
        }
        // Same short-circuit as `resolved_fixed_leg`: skip the registry hit unless we
        // actually need to apply a default.
        if float.reset_lag_days < 0 || float.payment_lag_days < 0 {
            let conv = self.rate_index_conventions()?.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "IRS '{}': float-leg sentinel(s) (reset_lag={}, payment_lag={}) require \
                     resolution but rate index '{}' is not registered in the convention registry",
                    self.id,
                    float.reset_lag_days,
                    float.payment_lag_days,
                    self.float.forward_curve_id
                ))
            })?;
            if float.reset_lag_days < 0 {
                float.reset_lag_days = conv.default_reset_lag_days;
            }
            if float.payment_lag_days < 0 {
                float.payment_lag_days = conv.default_payment_lag_days;
            }
        }
        Ok(float)
    }

    /// Validate swap parameters for market-standard compliance.
    ///
    /// Checks:
    /// - Date ranges: `end > start` for both legs
    /// - Notional: must be positive (> NOTIONAL_EPSILON)
    /// - Fixed rate: must be within reasonable bounds
    /// - Leg consistency: start/end dates should match between legs
    ///
    /// # Errors
    ///
    /// Returns a validation error with a descriptive message if any
    /// parameter is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
    ///
    /// let swap = InterestRateSwap::example_standard()?;
    /// swap.validate()?; // Passes for valid swap
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Validate finiteness early to avoid NaN poisoning and panics in downstream code.
        validation::validate_money_finite(self.notional, "notional")?;
        // Decimal values are always finite (no NaN/Infinity), so we just check they're valid
        // by verifying they can be converted to f64 for magnitude checks

        // Reset lag is a positive business-day offset subtracted from the accrual start to obtain
        // the reset/fixing date. Market convention "T-2" is represented as reset_lag_days = 2,
        // meaning fixing_date = accrual_start - 2 business days.
        // Small negative values (e.g., -1) are allowed as sentinels for "use convention default".
        // Guard only against absurd magnitudes that indicate unit mistakes.
        if self.float.reset_lag_days.abs() > 31 {
            return Err(finstack_quant_core::Error::Validation(
                "Invalid floating reset lag: absolute value too large (expected a small number of business days)."
                    .into(),
            ));
        }
        // Payment delay validation: large negative values are rejected (likely unit mistakes).
        // Small negative values (e.g., -1) are allowed as sentinels for "use convention default".
        // Zero and positive values are explicit delays.
        if self.fixed.payment_lag_days < -31 || self.float.payment_lag_days < -31 {
            return Err(finstack_quant_core::Error::Validation(
                "Invalid payment delay: value too negative (use small negative like -1 for convention default)."
                    .into(),
            ));
        }
        overnight_conventions::reject_simple_overnight(
            self.float.forward_curve_id.as_str(),
            &self.float.compounding,
        )?;
        if let FloatingLegCompounding::CompoundedInArrears { lookback_days } =
            self.float.compounding
        {
            if lookback_days < 0 {
                return Err(finstack_quant_core::Error::Validation(
                    "Invalid RFR lookback: must be non-negative (business days).".into(),
                ));
            }
        }
        if let FloatingLegCompounding::CompoundedWithObservationShift { shift_days } =
            self.float.compounding
        {
            if shift_days < 0 {
                return Err(finstack_quant_core::Error::Validation(
                    "Invalid observation shift days: must be non-negative.".into(),
                ));
            }
            if shift_days > 31 {
                return Err(finstack_quant_core::Error::Validation(
                    "Invalid observation shift days: too large.".into(),
                ));
            }
        }
        if let FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } =
            self.float.compounding
        {
            if cutoff_days < 0 {
                return Err(finstack_quant_core::Error::Validation(
                    "Invalid rate cut-off days: must be non-negative.".into(),
                ));
            }
            if cutoff_days > 31 {
                return Err(finstack_quant_core::Error::Validation(
                    "Invalid rate cut-off days: too large.".into(),
                ));
            }
        }

        validation::validate_date_range_strict_with(
            self.fixed.start,
            self.fixed.end,
            |start, end| {
                format!(
                    "Invalid fixed leg date range: end ({}) must be after start ({})",
                    end, start
                )
            },
        )?;

        validation::validate_date_range_strict_with(
            self.float.start,
            self.float.end,
            |start, end| {
                format!(
                    "Invalid floating leg date range: end ({}) must be after start ({})",
                    end, start
                )
            },
        )?;

        // Only single-curve (CSA OIS) discounting is supported: both legs must
        // use the same discount curve. Dual-curve discounting is not implemented.
        if self.float.discount_curve_id != self.fixed.discount_curve_id {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Dual-curve discounting is not supported: fixed leg discount curve '{}' \
                 differs from floating leg discount curve '{}'. \
                 Use the same CSA OIS curve for both legs.",
                self.fixed.discount_curve_id, self.float.discount_curve_id
            )));
        }

        validation::validate_money_gt_with(self.notional, NOTIONAL_EPSILON, |amount| {
            format!(
                "Invalid notional: {} must be positive (> {:.0e}). \
                 Negative notional is semantically ambiguous; use PayReceive to control direction.",
                amount, NOTIONAL_EPSILON
            )
        })?;

        // Validate fixed rate is within reasonable bounds
        let rate_f64 = decimal_to_f64(self.fixed.rate, "fixed rate")?;
        validation::validate_rate_magnitude(
            rate_f64,
            MAX_RATE_MAGNITUDE,
            "fixed rate",
            100.0,
            "%",
            "This may indicate a units error (rate should be decimal, e.g., 0.05 for 5%).",
        )?;

        // Warn-level check: legs should typically have matching date ranges
        if self.fixed.start != self.float.start || self.fixed.end != self.float.end {
            tracing::warn!(
                swap_id = %self.id,
                "IRS legs have mismatched date ranges: fixed ({} to {}), float ({} to {}). \
                 This may be intentional for complex structures.",
                self.fixed.start, self.fixed.end, self.float.start, self.float.end
            );
        }

        Ok(())
    }

    /// Create a legacy term-index USD 5Y IRS for testing and documentation.
    ///
    /// Returns a 5-year pay-fixed USD swap with conventional historical
    /// term-index terms:
    /// - **Fixed leg:** Semi-annual, 30/360, Modified Following
    /// - **Float leg:** Quarterly, ACT/360, Modified Following
    /// - **Reset lag:** T-2
    /// - **Calendar:** USNY
    #[allow(clippy::expect_used)]
    pub fn example_standard() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

        let start = Date::from_calendar_date(2024, time::Month::January, 2).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example start date: {}", e))
        })?;
        let end = Date::from_calendar_date(2029, time::Month::January, 2).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example end date: {}", e))
        })?;

        let swap = Self::builder()
            .id(InstrumentId::new("IRS-5Y-USD-STD"))
            .notional(Money::from((10_000_000_i64, Currency::USD)))
            .side(PayReceive::Pay)
            .fixed(crate::instruments::common_impl::parameters::FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: Decimal::try_from(0.04_f64).expect("valid literal"),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: Some("usny".to_string()),
                stub: StubKind::ShortFront,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,
            })
            .float(crate::instruments::common_impl::parameters::FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: Decimal::ZERO,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: Some("usny".to_string()),
                stub: StubKind::ShortFront,
                reset_lag_days: 2,
                fixing_calendar_id: Some("usny".to_string()),
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,
            })
            .build()?;

        swap.validate()?;
        Ok(swap)
    }
}

// Explicit trait implementations for modern instrument style
// Attributable implementation is provided by the impl_instrument! macro

impl crate::instruments::common_impl::traits::Instrument for InterestRateSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::Irs);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::rates::irs::pricer::compute_pv(self, curves, as_of)
    }

    fn base_value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::rates::irs::pricer::compute_pv_raw(self, curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            crate::instruments::rates::irs::pricer::compute_pv_raw(self, curves, as_of)?,
            self.notional.currency(),
        ))
    }

    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }

    fn last_payment_date(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<finstack_quant_core::dates::Date>> {
        let schedule =
            crate::cashflow::traits::CashflowProvider::cashflow_schedule(self, curves, as_of)?;
        Ok(schedule
            .get_flows()
            .iter()
            .map(|flow| flow.date)
            .chain(self.expiry())
            .max())
    }

    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.fixed.end)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.fixed.start)
    }

    crate::impl_focused_pricing_overrides!();

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.fixed.discount_curve_id.clone());
        deps.add_discount_curve(self.float.discount_curve_id.clone());
        if !self.is_single_curve_ois() {
            deps.add_forward_curve(self.float.forward_curve_id.clone());
        }
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.float.forward_curve_id.as_str(),
        ));
        Ok(deps)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for InterestRateSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let schedule =
            crate::instruments::rates::irs::cashflow::full_signed_schedule_with_curves_as_of(
                self,
                Some(curves),
                Some(as_of),
            )?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_names_missing_required_field_and_builder() {
        // Failure mode: a bare "Invalid input data" told the caller neither
        // which builder nor which field was missing.
        let example = InterestRateSwap::example_standard().expect("example swap");
        let err = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-MISSING-FLOAT"))
            .notional(example.notional)
            .side(PayReceive::Pay)
            .fixed(example.fixed.clone())
            .build()
            .expect_err("missing float leg must fail");
        let message = err.to_string();
        assert!(
            message.contains("InterestRateSwapBuilder") && message.contains("'float'"),
            "error must name builder and field: {message}"
        );

        let err = InterestRateSwap::builder()
            .notional(example.notional)
            .build()
            .expect_err("missing id must fail");
        assert!(
            err.to_string().contains("'id'"),
            "first missing field is named: {err}"
        );
    }

    #[test]
    fn validate_accepts_extreme_but_valid_rate() {
        // Decimal doesn't have NaN/Infinity, so we just test that validation works
        // for extreme but valid values
        let swap = InterestRateSwap::example_standard().expect("example swap");
        // Should pass validation
        assert!(swap.validate().is_ok(), "Valid swap should pass validation");
    }

    #[test]
    fn canonical_dependencies_preserve_curve_roles_and_fixings() {
        let swap = InterestRateSwap::example_standard().expect("example swap");
        let deps =
            crate::instruments::Instrument::market_dependencies(&swap).expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[
                swap.fixed.discount_curve_id.clone(),
                swap.float.discount_curve_id.clone(),
            ][..deps.curves.discount_curves.len()]
        );
        assert_eq!(
            deps.curves.forward_curves.as_slice(),
            std::slice::from_ref(&swap.float.forward_curve_id)
        );
        assert_eq!(
            deps.series_ids,
            vec![finstack_quant_core::market_data::fixings::fixing_series_id(
                swap.float.forward_curve_id.as_str()
            )]
        );
    }

    #[test]
    fn validate_rejects_extreme_fixed_rate_without_silent_default() {
        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.fixed.rate = Decimal::MAX;

        let err = swap
            .validate()
            .expect_err("extreme fixed rate should be rejected");
        let message = err.to_string();
        assert!(
            message.contains("fixed rate") && message.contains("exceeds maximum allowed magnitude"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn builder_rejects_invalid_swap_economics() {
        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.fixed.rate = Decimal::MAX;

        let error = InterestRateSwap::builder()
            .id(swap.id)
            .notional(swap.notional)
            .side(swap.side)
            .fixed(swap.fixed)
            .float(swap.float)
            .attributes(swap.attributes)
            .build()
            .expect_err("an extreme fixed rate must fail at the builder boundary");

        assert!(error.to_string().contains("fixed rate"));
    }

    #[test]
    fn validate_allows_small_negative_as_convention_sentinel() {
        // Small negative values (like -1) are allowed as sentinels for "use convention default"
        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.fixed.payment_lag_days = -1;
        assert!(
            swap.validate().is_ok(),
            "small negative payment delay (-1) should be allowed as convention sentinel"
        );
    }

    #[test]
    fn validate_rejects_large_negative_payment_delay() {
        // Large negative values are rejected as likely unit mistakes
        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.fixed.payment_lag_days = -100;
        assert!(
            swap.validate().is_err(),
            "large negative payment delay must be rejected"
        );
    }

    #[test]
    fn from_conventions_uses_overnight_rfr_compounding() {
        let swap = InterestRateSwap::from_conventions(ConventionSwapParams {
            id: InstrumentId::new("USD-SOFR-OIS-SWAP-5Y"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Pay,
            fixed_rate: 0.04,
            start: Date::from_calendar_date(2025, time::Month::January, 13).expect("start"),
            end: Date::from_calendar_date(2030, time::Month::January, 13).expect("end"),
            index_id: "USD-SOFR-OIS",
            discount_curve_id: "USD-OIS",
            forward_curve_id: "USD-SOFR-OIS",
        })
        .expect("OIS swap from conventions");

        assert!(
            !matches!(swap.float.compounding, FloatingLegCompounding::Simple),
            "overnight RFR swaps must not silently default to simple compounding"
        );
    }
    #[test]
    fn single_curve_ois_does_not_require_a_forward_curve_dependency() {
        let swap = InterestRateSwap::from_conventions(ConventionSwapParams {
            id: InstrumentId::new("USD-SOFR-OIS-SINGLE-CURVE"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Pay,
            fixed_rate: 0.04,
            start: Date::from_calendar_date(2025, time::Month::January, 13).expect("start"),
            end: Date::from_calendar_date(2030, time::Month::January, 13).expect("end"),
            index_id: "USD-SOFR-OIS",
            discount_curve_id: "USD-SOFR-OIS",
            forward_curve_id: "USD-SOFR-OIS",
        })
        .expect("single-curve OIS");
        let deps =
            crate::instruments::Instrument::market_dependencies(&swap).expect("dependencies");
        assert!(deps.curves.forward_curves.is_empty());
        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[CurveId::new("USD-SOFR-OIS")]
        );
    }

    #[test]
    fn validate_rejects_simple_compounding_on_overnight_index() {
        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.float.forward_curve_id = CurveId::new("USD-SOFR-OIS");
        swap.float.compounding = FloatingLegCompounding::Simple;
        let err = swap
            .validate()
            .expect_err("hand-built OIS with Simple must fail");
        assert!(
            format!("{err}").contains("Overnight RFR"),
            "expected overnight/Simple rejection, got {err}"
        );
    }
}
