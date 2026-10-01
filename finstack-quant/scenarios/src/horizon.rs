//! Horizon total return analysis.
//!
//! Composes [`ScenarioSpec`] application with P&L attribution to answer:
//! "If I hold this instrument under these market assumptions, what is my
//! decomposed total return?"
//!
//! The caller supplies a [`ScenarioSpec`] that may include a
//! [`OperationSpec::TimeRollForward`] (holding period) alongside any market
//! shocks (spread widening, rate shifts, vol changes, etc.).  The engine
//! applies the spec to construct a T₁ market state, then delegates to the
//! existing attribution framework to decompose the P&L.
//!
//! # Quick Start
//!
//! ```no_run
//! use finstack_quant_scenarios::horizon::HorizonAnalysis;
//! use finstack_quant_scenarios::{OperationSpec, ScenarioSpec, TimeRollMode};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_valuations::instruments::Instrument;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let instrument: Arc<dyn Instrument> = todo!("your instrument");
//! let market = MarketContext::new();
//! let as_of = date!(2025-01-15);
//!
//! // Hold for 3 months, spreads widen 25bp
//! let scenario = ScenarioSpec {
//!     id: "hold_3m_spread_25".into(),
//!     name: None,
//!     description: None,
//!     operations: vec![
//!         OperationSpec::TimeRollForward {
//!             period: "3M".into(),
//!             apply_shocks: true,
//!             roll_mode: TimeRollMode::BusinessDays,
//!         },
//!         OperationSpec::CurveParallelBp {
//!             curve_kind: finstack_quant_scenarios::CurveKind::ParCDS,
//!             curve_id: "AAPL-CDS".into(),
//!             discount_curve_id: None,
//!             bp: 25.0,
//!         },
//!     ],
//!     priority: 0,
//!     resolution_mode: Default::default(),
//!     hazard_bump_mode: Default::default(),
//! };
//!
//! let analyzer = HorizonAnalysis::default();
//! let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;
//!
//! println!("Total return: {:.2}%", result.total_return() * 100.0);
//! println!("Carry: {}", result.attribution.carry);
//! println!("Credit P&L: {}", result.attribution.credit_curves_pnl);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, default_attribution_metrics, AttributionRequest,
    ExecutionPolicy,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::{calendars_by_ids, Date, HolidayCalendar};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CalendarId;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::recalibration::RecalibrationProvider;

use crate::engine::ApplicationReport;
use crate::{ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec};

/// Re-export attribution types appearing in horizon-analysis public APIs so
/// direct scenarios consumers do not need a separate attribution dependency.
pub use finstack_quant_attribution::{AttributionFactor, AttributionMethod, PnlAttribution};

/// Parse an [`AttributionMethod`] from its binding string form.
///
/// This is the single shared parser behind the Python
/// `compute_horizon_return(method=...)` keyword and the WASM
/// `computeHorizonReturn` `method` parameter. It cannot be replaced by a
/// bare serde deserialization: `AttributionMethod` is externally tagged, so
/// serde only accepts the plain strings `"parallel"` / `"metrics_based"` —
/// the tuple variants require `{"waterfall": [...]}` / `{"taylor": {...}}`
/// payloads, whereas the binding contract maps the bare strings
/// `"waterfall"` and `"taylor"` to their canonical defaults
/// ([`finstack_quant_attribution::default_waterfall_order`] and
/// [`finstack_quant_attribution::TaylorAttributionConfig::default`]).
///
/// # Arguments
///
/// * `method` - One of `"parallel"`, `"waterfall"`, `"metrics_based"`, or
///   `"taylor"`.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for any other string, naming the
/// accepted values.
pub fn attribution_method_from_str(method: &str) -> crate::Result<AttributionMethod> {
    match method {
        "parallel" => Ok(AttributionMethod::Parallel),
        "waterfall" => Ok(AttributionMethod::Waterfall(
            finstack_quant_attribution::default_waterfall_order(),
        )),
        "metrics_based" => Ok(AttributionMethod::MetricsBased),
        "taylor" => Ok(AttributionMethod::Taylor(
            finstack_quant_attribution::TaylorAttributionConfig::default(),
        )),
        other => Err(crate::Error::Validation(format!(
            "Unknown attribution method '{other}'. Expected: parallel, waterfall, metrics_based, taylor"
        ))),
    }
}

fn horizon_unsupported_instrument_operation(scenario: &ScenarioSpec) -> Option<&'static str> {
    scenario.operations.iter().find_map(|op| match op {
        OperationSpec::InstrumentPricePctByType { .. } => Some("InstrumentPricePctByType"),
        OperationSpec::InstrumentPricePctByAttr { .. } => Some("InstrumentPricePctByAttr"),
        OperationSpec::InstrumentSpreadBpByType { .. } => Some("InstrumentSpreadBpByType"),
        OperationSpec::InstrumentSpreadBpByAttr { .. } => Some("InstrumentSpreadBpByAttr"),
        OperationSpec::AssetCorrelationPts { .. } => Some("AssetCorrelationPts"),
        OperationSpec::PrepayDefaultCorrelationPts { .. } => Some("PrepayDefaultCorrelationPts"),
        _ => None,
    })
}

/// Horizon total return analyzer.
///
/// Composes scenario application with P&L attribution.  Construct with an
/// [`AttributionMethod`] and [`FinstackConfig`], then call [`compute`] to
/// project an instrument forward under a [`ScenarioSpec`] and decompose the
/// resulting P&L.
///
/// [`compute`]: HorizonAnalysis::compute
#[derive(Debug, Clone)]
pub struct HorizonAnalysis {
    /// Attribution methodology for decomposing the horizon P&L.
    pub attribution_method: AttributionMethod,
    /// Finstack configuration (rounding, tolerances).
    pub config: FinstackConfig,
    /// Scenario engine instance.
    pub engine: ScenarioEngine,
    /// Holiday calendar used to business-day adjust
    /// [`OperationSpec::TimeRollForward`] targets under
    /// [`crate::TimeRollMode::BusinessDays`].
    ///
    /// `None` falls back to [`finstack_quant_core::dates::WEEKENDS_ONLY`], so
    /// business-day rolls always avoid weekends even when no calendar is named.
    /// Supply an identifier (e.g. `"nyse"`, `"target"`) for holiday awareness.
    pub calendar_id: Option<CalendarId>,
}

impl Default for HorizonAnalysis {
    fn default() -> Self {
        Self {
            attribution_method: AttributionMethod::Parallel,
            config: FinstackConfig::default(),
            engine: ScenarioEngine::new(),
            calendar_id: None,
        }
    }
}

impl HorizonAnalysis {
    /// Create a new analyzer with the given attribution method and config.
    ///
    /// The config is also threaded into the internal [`ScenarioEngine`] so the
    /// rounding policy stamped into the scenario report reflects the active
    /// configuration. No holiday calendar is set; use
    /// [`with_calendar_id`](Self::with_calendar_id) to name one.
    pub fn new(attribution_method: AttributionMethod, config: FinstackConfig) -> Self {
        let engine = ScenarioEngine::with_config(config.clone());
        Self {
            attribution_method,
            config,
            engine,
            calendar_id: None,
        }
    }

    /// Set the holiday calendar used for business-day roll adjustment.
    ///
    /// The identifier is resolved against core's built-in calendar registry
    /// during [`compute`](Self::compute); unknown identifiers surface there as
    /// an error listing near matches rather than being silently ignored.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_scenarios::HorizonAnalysis;
    ///
    /// let analyzer = HorizonAnalysis::default().with_calendar_id("nyse");
    /// assert!(analyzer.calendar_id.is_some());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registry identifier of the holiday calendar used for adjustments
    #[must_use]
    pub fn with_calendar_id(mut self, calendar_id: impl Into<CalendarId>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Attach a quote-recalibration provider to the internal scenario engine.
    ///
    /// The same provider is threaded into the [`PricingOptions`] used by the
    /// metrics-based attribution path, so quote-replay operations and the
    /// pricing they feed share one cache for the whole horizon computation.
    ///
    /// # Arguments
    ///
    /// * `provider` - Shared recalibration service used by quote-replay
    ///   scenario operations and by instrument pricing during attribution.
    #[must_use]
    pub fn with_recalibration_provider(mut self, provider: Arc<dyn RecalibrationProvider>) -> Self {
        self.engine = self.engine.with_recalibration_provider(provider);
        self
    }

    /// Pricing options carrying this analyzer's configuration and, when one
    /// was attached, the engine's recalibration provider.
    fn pricing_options(&self) -> PricingOptions {
        let mut options = PricingOptions::default().with_config(&self.config);
        if let Some(provider) = self.engine.recalibration_provider() {
            options = options.with_recalibration_provider(Arc::clone(provider));
        }
        options
    }

    /// Resolve [`Self::calendar_id`] against core's built-in calendar registry.
    ///
    /// # Errors
    ///
    /// Propagates core's calendar-not-found error (which includes suggestions)
    /// when the identifier does not name a built-in calendar.
    fn resolve_calendar(&self) -> crate::Result<Option<&'static dyn HolidayCalendar>> {
        let Some(id) = self.calendar_id.as_ref() else {
            return Ok(None);
        };
        let resolved = calendars_by_ids(std::slice::from_ref(id))?;
        Ok(resolved.into_iter().next())
    }
}

/// Result of a horizon total return computation.
///
/// Wraps a [`PnlAttribution`] with scenario context and convenience
/// accessors for total return percentage and annualized return.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HorizonResult {
    /// Full factor-decomposed P&L from the attribution framework.
    pub attribution: PnlAttribution,
    /// Initial instrument value at (market_t0, as_of_t0).
    pub initial_value: Money,
    /// Final instrument value at (market_t1, as_of_t1).
    pub terminal_value: Money,
    /// Number of calendar days in the horizon (`None` if no time-roll in spec).
    pub horizon_days: Option<i64>,
    /// Report from scenario engine application.
    pub scenario_report: ApplicationReport,
}

/// Borrowed JSON view of a horizon result, including Rust-derived returns.
///
/// Undefined or non-finite returns are represented as `null`. The underlying
/// result fields are flattened into the same object without cloning attribution
/// details or scenario reports.
#[derive(Debug, serde::Serialize)]
pub struct HorizonResultJson<'a> {
    /// Original horizon fields flattened into the serialized result object.
    #[serde(flatten)]
    pub result: &'a HorizonResult,
    /// Total P&L divided by positive initial value, as a decimal fraction.
    pub total_return: Option<f64>,
    /// Compounded annual return, or `None` when annualization is undefined.
    pub annualized_return: Option<f64>,
    /// Each canonical factor's P&L divided by positive initial value.
    pub factor_contributions: indexmap::IndexMap<AttributionFactor, Option<f64>>,
}

impl HorizonAnalysis {
    /// Compute horizon total return under a scenario.
    ///
    /// Applies the [`ScenarioSpec`] to the provided market context (cloned
    /// internally) and runs P&L attribution between the original and
    /// scenario-modified states.
    ///
    /// The spec may include a [`crate::spec::OperationSpec::TimeRollForward`]
    /// to define the holding period.  If no time-roll is present, the
    /// analysis is a pure mark-to-scenario (carry will be zero, `horizon_days`
    /// will be `None`).
    ///
    /// Horizon analysis supports market-state scenarios. It rejects operations
    /// that mutate the instrument collection itself, including instrument
    /// price/spread shocks and structured-credit correlation shocks, because
    /// attribution prices the same instrument instance at both `t0` and `t1`.
    /// Apply those instrument-scoped changes before calling this method, or use
    /// a market-only horizon scenario.
    ///
    /// Metrics-based attribution calculates the canonical registry's applicable
    /// subset of default attribution metrics at the opening snapshot. Metrics
    /// unsupported by the instrument type are omitted; selected metric
    /// calculation failures propagate. Terminal pricing does not request metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if scenario application or attribution fails (e.g.
    /// missing market data for a curve referenced in the spec), if the
    /// scenario contains an instrument-scoped operation unsupported by horizon
    /// attribution, or if [`Self::calendar_id`] does not name a built-in
    /// calendar.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument whose cash flows, dependencies, or value are evaluated.
    /// * `market_t0` - Opening market snapshot used as the attribution or scenario baseline.
    /// * `as_of_t0` - Opening ISO-8601 valuation date for the attribution interval.
    /// * `scenario` - Scenario definition applied to the baseline model or market state.
    pub fn compute(
        &self,
        instrument: &Arc<dyn Instrument>,
        market_t0: &MarketContext,
        as_of_t0: Date,
        scenario: &ScenarioSpec,
    ) -> crate::Result<HorizonResult> {
        scenario.validate()?;
        if let Some(op_name) = horizon_unsupported_instrument_operation(scenario) {
            return Err(crate::Error::Validation(format!(
                "{op_name} is not supported by HorizonAnalysis because attribution uses one \
                 instrument instance for both t0 and t1 pricing. Apply instrument-scoped \
                 shocks before calling horizon analysis or use a market-only horizon scenario."
            )));
        }

        let calendar = self.resolve_calendar()?;
        let mut market_t1 = market_t0.clone();
        if let Some((period, mode)) = scenario.operations.iter().find_map(|op| match op {
            OperationSpec::TimeRollForward {
                period, roll_mode, ..
            } => Some((period, *roll_mode)),
            _ => None,
        }) {
            let (horizon_date, _) =
                crate::adapters::time_roll::resolve_roll_dates(as_of_t0, period, mode, calendar)?;
            if horizon_date > as_of_t0 {
                let schedule = instrument.cashflow_schedule(market_t0, as_of_t0)?;
                market_t1 = finstack_quant_cashflows::fixings::materialize_fixings(
                    market_t0,
                    [&schedule],
                    as_of_t0,
                    horizon_date,
                )?;
            }
        }
        let mut ctx = ExecutionContext {
            market: &mut market_t1,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar,
            as_of: as_of_t0,
        };

        let scenario_report = self.engine.apply(scenario, &mut ctx)?;
        let as_of_t1 = ctx.as_of;

        let diff_days = (as_of_t1 - as_of_t0).whole_days();
        let horizon_days = if diff_days > 0 { Some(diff_days) } else { None };

        let (initial_value, terminal_value, attribution) =
            self.run_attribution(instrument, market_t0, &market_t1, as_of_t0, as_of_t1)?;

        Ok(HorizonResult {
            attribution,
            initial_value,
            terminal_value,
            horizon_days,
            scenario_report,
        })
    }

    /// Dispatch P&L attribution for `self.attribution_method`.
    fn run_attribution(
        &self,
        instrument: &Arc<dyn Instrument>,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
        as_of_t0: Date,
        as_of_t1: Date,
    ) -> crate::Result<(Money, Money, PnlAttribution)> {
        if matches!(self.attribution_method, AttributionMethod::MetricsBased) {
            let window_days = (as_of_t1 - as_of_t0).whole_days();
            let metrics_instrument = if window_days > 0 {
                let mut producer = instrument.clone_box();
                if let Some(overrides) = producer.get_metric_pricing_overrides_mut() {
                    let days = u32::try_from(window_days).map_err(|_| {
                        crate::Error::Validation(format!(
                            "horizon of {window_days} days exceeds the theta horizon range"
                        ))
                    })?;
                    overrides.theta_period = Some(finstack_quant_core::dates::Tenor::new(
                        days,
                        finstack_quant_core::dates::TenorUnit::Days,
                    )?);
                }
                Arc::from(producer)
            } else {
                Arc::clone(instrument)
            };
            let metrics = finstack_quant_valuations::metrics::standard_registry()
                .applicable_subset(&default_attribution_metrics(), instrument.key());
            let val_t0 = metrics_instrument.price_with_metrics(
                market_t0,
                as_of_t0,
                &metrics,
                self.pricing_options(),
            )?;
            let val_t1 =
                instrument.price_with_metrics(market_t1, as_of_t1, &[], self.pricing_options())?;
            let attribution = attribute_pnl_metrics_based(
                &metrics_instrument,
                market_t0,
                market_t1,
                &val_t0,
                &val_t1,
                as_of_t0,
                as_of_t1,
            )?;
            return Ok((val_t0.value, val_t1.value, attribution));
        }

        let initial_value = instrument.value(market_t0, as_of_t0)?;
        let terminal_value = instrument.value(market_t1, as_of_t1)?;
        let request = AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            strict_validation: false,
            prepared_endpoints: Some((initial_value, terminal_value)),
            ..AttributionRequest::new(
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                &self.config,
            )
        };
        let attribution = attribute_pnl(&self.attribution_method, &request)?;
        Ok((initial_value, terminal_value, attribution))
    }
}

impl HorizonResult {
    /// Build the canonical serializable view with computed return fields.
    ///
    /// The view includes every underlying horizon field, decimal total and
    /// annualized returns, and all factor contributions. Undefined and
    /// non-finite derived values become JSON `null`; no financial formulas are
    /// delegated to host bindings.
    #[must_use]
    pub fn to_json(&self) -> HorizonResultJson<'_> {
        let total_return = self.total_return();
        HorizonResultJson {
            result: self,
            total_return: total_return.is_finite().then_some(total_return),
            annualized_return: self.annualized_return(),
            factor_contributions: [
                AttributionFactor::Carry,
                AttributionFactor::RatesCurves,
                AttributionFactor::CreditCurves,
                AttributionFactor::InflationCurves,
                AttributionFactor::Correlations,
                AttributionFactor::Fx,
                AttributionFactor::Volatility,
                AttributionFactor::MarketScalars,
                AttributionFactor::ModelParameters,
            ]
            .into_iter()
            .map(|factor| {
                let contribution = self.factor_contribution(&factor);
                (factor, contribution.is_finite().then_some(contribution))
            })
            .collect(),
        }
    }

    /// Total return as a decimal fraction (e.g. `0.05` = 5%).
    ///
    /// Computed as `total_pnl / initial_value`. Returns:
    /// - [`f64::NAN`] when `initial_value` is zero (return requires a positive capital base),
    /// - [`f64::NAN`] when `initial_value` and `total_pnl` are denominated in
    ///   different currencies. Multi-currency results require an explicit
    ///   base-currency conversion policy that this helper does not apply, and
    /// - [`f64::NAN`] when `initial_value` is negative. A return on a negative
    ///   mark (payer swaps, short protection, any liability position) is not
    ///   defined by this ratio: dividing by a negative denominator inverts the
    ///   sign, so a profitable horizon would report as a loss. Compute a return
    ///   on notional or on an explicit capital base instead.
    pub fn total_return(&self) -> f64 {
        if self.initial_value.currency() != self.attribution.total_pnl.currency() {
            return f64::NAN;
        }
        let iv = self.initial_value.amount();
        if iv <= 0.0 {
            return f64::NAN;
        }
        self.attribution.total_pnl.amount() / iv
    }

    /// Annualized total return.
    ///
    /// Uses `(1 + total_return)^(365 / horizon_days) - 1`.
    ///
    /// Returns `None` when there is no time-roll in the scenario, when total
    /// return is not finite (e.g. multi-currency result), or when the
    /// compounded result would not be finite. A return below `-100%` has a
    /// negative accumulation factor and cannot be compounded as a capital
    /// return, so it also returns `None`. Exactly `-100%` remains `Some(-1.0)`.
    pub fn annualized_return(&self) -> Option<f64> {
        let days = self.horizon_days? as f64;
        if days <= 0.0 {
            return None;
        }
        let tr = self.total_return();
        if !tr.is_finite() || tr < -1.0 {
            return None;
        }
        if tr <= -1.0 {
            return Some(-1.0);
        }
        let out = (1.0 + tr).powf(365.0 / days) - 1.0;
        out.is_finite().then_some(out)
    }

    /// A single factor's P&L as a fraction of initial value.
    ///
    /// Returns [`f64::NAN`] if initial value is zero or the factor
    /// P&L currency does not match the initial value currency or if initial
    /// value is negative (see [`total_return`](Self::total_return) for
    /// why a negative denominator is rejected rather than divided through).
    ///
    /// # Arguments
    /// * `factor` - Attribution component whose P&L is divided by positive initial value.
    pub fn factor_contribution(&self, factor: &AttributionFactor) -> f64 {
        let factor_money = match factor {
            AttributionFactor::Carry => &self.attribution.carry,
            AttributionFactor::RatesCurves => &self.attribution.rates_curves_pnl,
            AttributionFactor::CreditCurves => &self.attribution.credit_curves_pnl,
            AttributionFactor::InflationCurves => &self.attribution.inflation_curves_pnl,
            AttributionFactor::Correlations => &self.attribution.correlations_pnl,
            AttributionFactor::Fx => &self.attribution.fx_pnl,
            AttributionFactor::Volatility => &self.attribution.vol_pnl,
            AttributionFactor::ModelParameters => &self.attribution.model_params_pnl,
            AttributionFactor::MarketScalars => &self.attribution.market_scalars_pnl,
        };
        if self.initial_value.currency() != factor_money.currency() {
            return f64::NAN;
        }
        let iv = self.initial_value.amount();
        if iv <= 0.0 {
            return f64::NAN;
        }
        factor_money.amount() / iv
    }
}

impl std::fmt::Display for HorizonResult {
    /// Multi-line human-readable summary: total and annualized return,
    /// horizon length, initial/terminal values, and the carry / rates /
    /// credit / residual legs of the attribution.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Horizon Total Return: {:.4}%",
            self.total_return() * 100.0
        )?;
        if let Some(ann) = self.annualized_return() {
            writeln!(f, "Annualized: {:.4}%", ann * 100.0)?;
        }
        if let Some(days) = self.horizon_days {
            writeln!(f, "Horizon: {days} days")?;
        }
        writeln!(f, "Initial Value: {}", self.initial_value)?;
        writeln!(f, "Terminal Value: {}", self.terminal_value)?;
        writeln!(f, "Total P&L: {}", self.attribution.total_pnl)?;
        writeln!(f, "  Carry: {}", self.attribution.carry)?;
        writeln!(f, "  Rates: {}", self.attribution.rates_curves_pnl)?;
        writeln!(f, "  Credit: {}", self.attribution.credit_curves_pnl)?;
        writeln!(f, "  Residual: {}", self.attribution.residual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations::instruments::{Attributes, Bond};
    use time::macros::date;

    /// Build a simple 2-year fixed-rate bond for testing.
    fn test_bond(base_date: Date) -> crate::Result<Arc<dyn Instrument>> {
        let bond = Bond::builder()
            .id("TEST-BOND".into())
            .notional(Money::from((100_i64, Currency::USD)))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(730))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()?;
        Ok(Arc::new(bond))
    }

    /// Build a market with a flat discount curve.
    fn test_market(base_date: Date) -> crate::Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.90)])
            .build()?;
        Ok(MarketContext::new().insert(curve))
    }

    #[test]
    fn no_op_scenario_returns_zero_pnl() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "no_op".into(),
            name: None,
            description: None,
            operations: vec![],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(
            result.attribution.total_pnl.amount().abs() < 1e-10,
            "no-op scenario should produce zero P&L, got {}",
            result.attribution.total_pnl.amount()
        );
        assert!(result.horizon_days.is_none());
        assert!(result.annualized_return().is_none());
        assert!((result.total_return()).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn time_roll_only_has_horizon_days_and_carry() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_1m".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: crate::TimeRollMode::BusinessDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(matches!(result.horizon_days, Some(days) if days > 0));
        assert!(result.annualized_return().is_some());

        // Carry should be non-zero since the bond accrues over the holding period.
        let carry = result.attribution.carry.amount();
        assert!(
            carry.abs() > 1e-6,
            "time-roll only: carry should be non-zero, got {carry}"
        );

        // Factor decomposition should be coherent
        let a = &result.attribution;
        let sum_of_factors = a.carry.amount()
            + a.rates_curves_pnl.amount()
            + a.credit_curves_pnl.amount()
            + a.inflation_curves_pnl.amount()
            + a.correlations_pnl.amount()
            + a.fx_pnl.amount()
            + a.vol_pnl.amount()
            + a.cross_factor_pnl.amount()
            + a.model_params_pnl.amount()
            + a.market_scalars_pnl.amount()
            + a.residual.amount();
        assert!(
            (a.total_pnl.amount() - sum_of_factors).abs() < 1e-8,
            "factors + residual ({sum_of_factors}) should equal total ({})",
            a.total_pnl.amount()
        );
        Ok(())
    }

    #[test]
    fn combined_time_roll_and_shock() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_and_shock".into(),
            name: None,
            description: None,
            operations: vec![
                crate::OperationSpec::TimeRollForward {
                    period: "1M".into(),
                    apply_shocks: true,
                    roll_mode: crate::TimeRollMode::BusinessDays,
                },
                crate::OperationSpec::CurveParallelBp {
                    curve_kind: crate::CurveKind::Discount,
                    curve_id: "USD-OIS".into(),
                    discount_curve_id: None,
                    bp: 50.0,
                },
            ],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        // Horizon present
        assert!(result.horizon_days.is_some());

        // Both carry and rates should be non-zero
        assert!(
            result.attribution.carry.amount().abs() > 1e-6,
            "combined: carry should be non-zero"
        );
        assert!(
            result.attribution.rates_curves_pnl.amount().abs() > 1e-6,
            "combined: rates P&L should be non-zero"
        );

        // Factor decomposition should be coherent: sum of factors + residual = total
        let a = &result.attribution;
        let sum_of_factors = a.carry.amount()
            + a.rates_curves_pnl.amount()
            + a.credit_curves_pnl.amount()
            + a.inflation_curves_pnl.amount()
            + a.correlations_pnl.amount()
            + a.fx_pnl.amount()
            + a.vol_pnl.amount()
            + a.cross_factor_pnl.amount()
            + a.model_params_pnl.amount()
            + a.market_scalars_pnl.amount()
            + a.residual.amount();
        assert!(
            (a.total_pnl.amount() - sum_of_factors).abs() < 1e-8,
            "factors + residual ({sum_of_factors}) should equal total ({})",
            a.total_pnl.amount()
        );
        Ok(())
    }

    #[test]
    fn shock_only_has_no_horizon_and_zero_carry() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "rate_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::CurveParallelBp {
                curve_kind: crate::CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 50.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(result.horizon_days.is_none());
        assert!(result.annualized_return().is_none());

        assert!(
            result.attribution.carry.amount().abs() < 1e-10,
            "shock-only: carry should be zero, got {}",
            result.attribution.carry.amount()
        );

        assert!(
            result.attribution.rates_curves_pnl.amount().abs() > 1e-6,
            "shock-only: rates P&L should be non-zero"
        );
        Ok(())
    }

    #[test]
    fn instrument_scoped_shocks_are_rejected_in_horizon_analysis() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "instrument_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![finstack_quant_valuations::pricer::InstrumentType::Bond],
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let err = analyzer
            .compute(&instrument, &market, as_of, &scenario)
            .expect_err("instrument-scoped horizon shocks should fail loudly");

        assert!(
            err.to_string().contains("InstrumentPricePctByType")
                && err.to_string().contains("not supported by HorizonAnalysis"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn total_return_matches_pnl_over_initial() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "rate_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::CurveParallelBp {
                curve_kind: crate::CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 50.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        let expected_pct = result.attribution.total_pnl.amount() / result.initial_value.amount();
        assert!(
            (result.total_return() - expected_pct).abs() < 1e-12,
            "total_return() should match manual calculation"
        );
        Ok(())
    }

    fn empty_report() -> ApplicationReport {
        ApplicationReport {
            operations_applied: 0,
            user_operations: 0,
            expanded_operations: 0,
            changes: Default::default(),
            warnings: vec![],
            meta: None,
            time_roll: None,
        }
    }

    fn synthetic_result(
        initial_currency: Currency,
        initial_amount: f64,
        pnl_currency: Currency,
        pnl_amount: f64,
        days: i64,
    ) -> HorizonResult {
        let attribution = PnlAttribution::new(
            Money::new(pnl_amount, pnl_currency).expect("valid money fixture"),
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 15) + time::Duration::days(days),
            AttributionMethod::Parallel,
        );
        HorizonResult {
            attribution,
            initial_value: Money::new(initial_amount, initial_currency)
                .expect("valid money fixture"),
            terminal_value: Money::new(initial_amount + pnl_amount, initial_currency)
                .expect("valid money fixture"),
            horizon_days: Some(days),
            scenario_report: empty_report(),
        }
    }

    /// Currency mismatch between initial value and total P&L must surface as
    /// NaN rather than a silently wrong ratio.
    #[test]
    fn total_return_currency_mismatch_returns_nan() {
        let result = synthetic_result(Currency::USD, 100.0, Currency::EUR, 10.0, 30);

        assert!(result.total_return().is_nan());
        assert!(result.annualized_return().is_none());
        assert!(result
            .factor_contribution(&AttributionFactor::Carry)
            .is_nan());
    }

    /// A negative initial value (payer swap, short protection, any liability
    /// mark) must not produce a sign-inverted return. Dividing a gain by a
    /// negative denominator previously reported a profitable horizon as a
    /// loss, and could drive `annualized_return` to a spurious `-100%`.
    #[test]
    fn negative_initial_value_returns_nan_rather_than_inverted_sign() {
        // +$5 of P&L against a -$100 mark.
        let gain_on_liability = synthetic_result(Currency::USD, -100.0, Currency::USD, 5.0, 30);

        assert!(
            gain_on_liability.total_return().is_nan(),
            "return on a negative mark is undefined, must not report -5%"
        );
        assert!(
            gain_on_liability.annualized_return().is_none(),
            "non-finite total return must not annualize"
        );
        assert!(gain_on_liability
            .factor_contribution(&AttributionFactor::Carry)
            .is_nan());
    }

    #[test]
    fn zero_initial_value_has_undefined_return_even_with_pnl() {
        for pnl in [0.0, 5.0, -5.0] {
            let result = synthetic_result(Currency::USD, 0.0, Currency::USD, pnl, 30);
            assert!(result.total_return().is_nan());
            assert!(result
                .factor_contribution(&AttributionFactor::Carry)
                .is_nan());
            assert!(result.annualized_return().is_none());
        }
    }

    /// An unknown calendar identifier must fail loudly at compute time rather
    /// than silently degrading to an unadjusted roll.
    #[test]
    fn unknown_calendar_id_is_rejected() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_1m".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: crate::TimeRollMode::BusinessDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default().with_calendar_id("not_a_real_calendar");
        let err = analyzer
            .compute(&instrument, &market, as_of, &scenario)
            .expect_err("unknown calendar id should fail loudly");
        assert!(
            err.to_string().contains("not_a_real_calendar"),
            "error should name the bad calendar: {err}"
        );
        Ok(())
    }

    /// A named holiday calendar must actually reach the time-roll adapter: a
    /// roll targeting a market holiday has to land past it.
    #[test]
    fn named_calendar_adjusts_horizon_past_a_holiday() -> crate::Result<()> {
        // Thu 2025-06-19 (Juneteenth) is an NYSE holiday, and is the
        // unadjusted 1M target from Mon 2025-05-19.
        let as_of = date!(2025 - 05 - 19);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_1m".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: crate::TimeRollMode::BusinessDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        };

        // Weekends-only fallback: 2025-06-19 is a Thursday, so no adjustment.
        let weekends_only = HorizonAnalysis::default();
        let base = weekends_only.compute(&instrument, &market, as_of, &scenario)?;
        assert_eq!(base.horizon_days, Some(31));

        // NYSE: Juneteenth is a holiday, so ModifiedFollowing carries to Fri 06-20.
        let nyse = HorizonAnalysis::default().with_calendar_id("nyse");
        let adjusted = nyse.compute(&instrument, &market, as_of, &scenario)?;
        assert_eq!(
            adjusted.horizon_days,
            Some(32),
            "NYSE calendar should push the horizon past Juneteenth"
        );
        Ok(())
    }

    #[test]
    fn attribution_method_from_str_parses_all_binding_forms() {
        assert!(matches!(
            attribution_method_from_str("parallel"),
            Ok(AttributionMethod::Parallel)
        ));
        assert!(matches!(
            attribution_method_from_str("metrics_based"),
            Ok(AttributionMethod::MetricsBased)
        ));
        match attribution_method_from_str("waterfall") {
            Ok(AttributionMethod::Waterfall(order)) => {
                assert_eq!(order, finstack_quant_attribution::default_waterfall_order());
            }
            other => panic!("expected default waterfall order, got {other:?}"),
        }
        assert!(matches!(
            attribution_method_from_str("taylor"),
            Ok(AttributionMethod::Taylor(_))
        ));
        let err = attribution_method_from_str("brinson").expect_err("unknown method");
        assert!(
            err.to_string()
                .contains("Unknown attribution method 'brinson'"),
            "unexpected error: {err}"
        );
    }

    /// Exactly a total loss annualizes to `-100%`; losses beyond initial
    /// capital have no compounded annualized return.
    #[test]
    fn annualized_return_total_loss_returns_minus_one() {
        let total_loss = synthetic_result(Currency::USD, 100.0, Currency::USD, -100.0, 30);
        assert_eq!(total_loss.total_return(), -1.0);
        assert_eq!(total_loss.annualized_return(), Some(-1.0));

        // A loss beyond initial capital has a negative accumulation factor
        // and cannot be annualized as a compounded capital return.
        let blown_up = synthetic_result(Currency::USD, 100.0, Currency::USD, -120.0, 30);
        assert!(blown_up.annualized_return().is_none());
    }

    #[test]
    fn json_view_contains_canonical_returns_and_null_undefined_values() -> serde_json::Result<()> {
        let result = synthetic_result(Currency::USD, 100.0, Currency::USD, 10.0, 365);
        let json = serde_json::to_value(result.to_json())?;
        assert_eq!(
            json["initial_value"],
            serde_json::to_value(result.initial_value)?
        );
        assert_eq!(json["total_return"], 0.1);
        assert!((json["annualized_return"].as_f64().expect("annual return") - 0.1).abs() < 1e-12);
        let factors = json["factor_contributions"]
            .as_object()
            .expect("factor map");
        assert_eq!(factors.len(), 9);
        assert_eq!(factors["carry"], 0.0);
        assert_eq!(factors["model_parameters"], 0.0);

        for result in [
            synthetic_result(Currency::USD, 0.0, Currency::USD, 10.0, 365),
            synthetic_result(Currency::USD, -100.0, Currency::USD, 10.0, 365),
            synthetic_result(Currency::USD, 100.0, Currency::EUR, 10.0, 365),
        ] {
            let json = serde_json::to_value(result.to_json())?;
            assert!(json["total_return"].is_null());
            assert!(json["annualized_return"].is_null());
            assert!(json["factor_contributions"]["carry"].is_null());
        }
        let leveraged_loss = synthetic_result(Currency::USD, 100.0, Currency::USD, -220.0, 365);
        let json = serde_json::to_value(leveraged_loss.to_json())?;
        assert_eq!(json["total_return"], -2.2);
        assert!(json["annualized_return"].is_null());
        Ok(())
    }

    #[test]
    fn floating_horizon_materializes_crossed_fixing_for_all_methods() -> crate::Result<()> {
        use finstack_quant_core::dates::Tenor;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;

        let origin = date!(2025 - 01 - 02);
        for issue_date in [origin, date!(2025 - 01 - 03)] {
            let mut bond = Bond::floating(
                "HORIZON-FRN",
                Money::from((1_000_000_i64, Currency::USD)),
                "USD-SOFR-3M",
                150,
                issue_date,
                date!(2026 - 01 - 03),
                Tenor::quarterly(),
                DayCount::Act360,
                "USD-OIS",
            )?;
            if let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec {
                spec.rate_spec.reset_lag_days = 0;
            }
            let instrument: Arc<dyn Instrument> = Arc::new(bond);
            let market = MarketContext::new()
                .insert(
                    DiscountCurve::builder("USD-OIS")
                        .base_date(origin)
                        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.9)])
                        .build()?,
                )
                .insert(
                    ForwardCurve::builder("USD-SOFR-3M", 0.25)
                        .base_date(origin)
                        .day_count(DayCount::Act360)
                        .knots([(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
                        .build()?,
                );
            let scenario = ScenarioSpec {
                id: "crossed-reset".into(),
                operations: vec![OperationSpec::TimeRollForward {
                    period: "4D".into(),
                    apply_shocks: false,
                    roll_mode: crate::TimeRollMode::CalendarDays,
                }],
                ..Default::default()
            };
            let mut expected_market = market.clone();
            let mut inventory = vec![instrument.clone_box()];
            let mut ctx = ExecutionContext {
                market: &mut expected_market,
                as_of: origin,
                model: None,
                instruments: Some(&mut inventory),
                rate_bindings: None,
                calendar: None,
            };
            ScenarioEngine::new().apply(&scenario, &mut ctx)?;
            let expected_value = instrument.value(&expected_market, date!(2025 - 01 - 06))?;

            for method in ["parallel", "waterfall", "metrics_based", "taylor"] {
                let result = HorizonAnalysis::new(
                    attribution_method_from_str(method)?,
                    FinstackConfig::default(),
                )
                .compute(&instrument, &market, origin, &scenario)?;
                assert_eq!(result.horizon_days, Some(4), "{method}");
                assert_eq!(result.terminal_value, expected_value, "{method}");
                assert!(result.total_return().is_finite(), "{method}");
                assert!(
                    result.scenario_report.warnings.is_empty(),
                    "{method}: {:?}",
                    result.scenario_report.warnings
                );
            }
            assert!(market.get_series("FIXING:USD-SOFR-3M").is_err());
        }
        Ok(())
    }

    #[derive(Clone)]
    struct CountingEndpoints {
        attributes: Attributes,
        origin: Date,
        initial_prices: Arc<std::sync::atomic::AtomicUsize>,
        terminal_prices: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl finstack_quant_cashflows::CashflowScheduleSource for CountingEndpoints {
        fn raw_cashflow_schedule(
            &self,
            _: &MarketContext,
            _: Date,
        ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule>
        {
            Ok(
                finstack_quant_cashflows::builder::CashFlowSchedule::from_parts(
                    Vec::new(),
                    finstack_quant_cashflows::builder::Notional::par(100.0, Currency::USD)?,
                    DayCount::Act365F,
                    Default::default(),
                ),
            )
        }
    }

    impl Instrument for CountingEndpoints {
        fn id(&self) -> &str {
            "COUNT-ENDPOINTS"
        }
        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn attributes(&self) -> &Attributes {
            &self.attributes
        }
        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }
        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }
        fn base_value(
            &self,
            market: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            use std::sync::atomic::Ordering;
            if as_of == self.origin {
                self.initial_prices.fetch_add(1, Ordering::Relaxed);
            } else if market.get_discount("USD-OIS")?.base_date() == as_of {
                self.terminal_prices.fetch_add(1, Ordering::Relaxed);
            }
            Money::new(100.0, Currency::USD)
        }
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(Default::default())
        }
    }

    #[test]
    fn horizon_prices_each_endpoint_once() -> crate::Result<()> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let origin = date!(2025 - 01 - 15);
        let initial_prices = Arc::new(AtomicUsize::new(0));
        let terminal_prices = Arc::new(AtomicUsize::new(0));
        let instrument: Arc<dyn Instrument> = Arc::new(CountingEndpoints {
            attributes: Attributes::new(),
            origin,
            initial_prices: Arc::clone(&initial_prices),
            terminal_prices: Arc::clone(&terminal_prices),
        });
        let scenario = ScenarioSpec {
            id: "count-endpoints".into(),
            operations: vec![OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: crate::TimeRollMode::CalendarDays,
            }],
            ..Default::default()
        };
        let result = HorizonAnalysis::default().compute(
            &instrument,
            &test_market(origin)?,
            origin,
            &scenario,
        )?;
        assert_eq!(result.initial_value, result.terminal_value);
        assert_eq!(initial_prices.load(Ordering::Relaxed), 1);
        assert_eq!(terminal_prices.load(Ordering::Relaxed), 1);
        Ok(())
    }
}
