//! Portfolio-level P&L attribution.
//!
//! Aggregates instrument-level attribution across all positions in a portfolio,
//! with currency conversion to portfolio base currency.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use finstack_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_parallel, default_attribution_metrics,
    AttributionMethod, CorrelationsAttribution, CreditCurvesAttribution, ExecutionPolicy,
    FxAttribution, InflationCurvesAttribution, PnlAttribution, RatesCurvesAttribution,
    ScalarsAttribution, VolAttribution,
};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::summation::NeumaierAccumulator;
use finstack_core::money::{fx::FxQuery, Money};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Portfolio-level P&L attribution result.
///
/// Aggregates P&L attribution across all positions with currency conversion
/// to portfolio base currency.
///
/// # FX Translation Effects
///
/// For positions denominated in currencies other than the portfolio's base currency,
/// the attribution includes FX translation effects. The `total_pnl` field represents
/// the **all-in P&L** including both:
///
/// 1. **Instrument-level P&L** converted to base currency at T₁ FX rates
/// 2. **FX translation P&L** from the revaluation of opening principal
///
/// The decomposition is:
///
/// ```text
/// total_pnl = sum(factor_pnl_at_T1_FX) + fx_translation_pnl + residual
/// ```
///
/// Where each factor bucket (carry, rates, credit, vol, etc.) is converted to
/// base currency using the T₁ FX rate. This means the implicit FX translation
/// of the P&L *flow* (i.e., `PnL_native × (FX_T1 - FX_T0)`) is absorbed into
/// each factor bucket rather than isolated in `fx_translation_pnl`.
///
/// `fx_translation_pnl` captures **only** the revaluation of the opening
/// principal:
///
/// ```text
/// fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
/// ```
///
/// This convention is consistent with systems that convert factor P&L at
/// closing rates and report principal revaluation separately.
///
/// # Note on by_position Attribution
///
/// The `by_position` map contains instrument-currency attribution before FX
/// translation effects are applied. To reconcile with `total_pnl`, apply the
/// FX rates and add the principal revaluation effect.
///
/// # Conventions
///
/// The portfolio-level aggregates are reported in portfolio base currency,
/// while `by_position` remains in each instrument's native currency so callers
/// can inspect raw instrument attribution before FX translation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioAttribution {
    /// Total portfolio P&L in base currency.
    ///
    /// This is the **all-in P&L** that includes:
    /// - All factor attributions converted to base currency at T₁ rates
    /// - FX translation effects from opening principal revaluation
    ///
    /// Note: This differs from a simple sum of factor attributions because
    /// cross-currency positions include FX translation P&L on the principal.
    pub total_pnl: Money,

    /// Carry P&L (theta + accruals) in base currency.
    pub carry: Money,

    /// Interest rate curves P&L in base currency.
    pub rates_curves_pnl: Money,

    /// Credit hazard curves P&L in base currency.
    pub credit_curves_pnl: Money,

    /// Inflation curves P&L in base currency.
    pub inflation_curves_pnl: Money,

    /// Base correlation curves P&L in base currency.
    pub correlations_pnl: Money,

    /// FX rate changes P&L in base currency.
    ///
    /// This captures FX exposure within instruments (e.g., cross-currency swaps),
    /// not the translation effect from converting instrument P&L to base currency.
    pub fx_pnl: Money,

    /// FX translation P&L from revaluing opening principal to base currency.
    ///
    /// For cross-currency positions, this captures the effect of FX rate changes
    /// on the T₀ position value:
    ///
    /// ```text
    /// fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
    /// ```
    ///
    /// Note: The implicit FX translation of each factor's P&L flow
    /// (converting native-currency factor P&L at T₁ FX rather than T₀ FX) is
    /// absorbed into the respective factor buckets (carry, rates, etc.) and is
    /// **not** included here.
    ///
    /// This is separate from `fx_pnl` which captures FX exposure within instruments.
    pub fx_translation_pnl: Money,

    /// Cross-factor interaction P&L in base currency.
    ///
    /// Aggregated from each position's native-currency `cross_factor_pnl`
    /// after conversion to portfolio base currency.
    #[serde(default = "default_zero_usd_money")]
    pub cross_factor_pnl: Money,

    /// Implied volatility changes P&L in base currency.
    pub vol_pnl: Money,

    /// Model parameters P&L in base currency.
    pub model_params_pnl: Money,

    /// Market scalars P&L in base currency.
    pub market_scalars_pnl: Money,

    /// Residual P&L (unexplained) in base currency.
    pub residual: Money,

    /// Attribution by position in instrument-native currency.
    ///
    /// Note: These values are in each instrument's native currency and do not
    /// include FX translation effects. Use the portfolio-level aggregates for
    /// base-currency totals.
    pub by_position: IndexMap<PositionId, PnlAttribution>,

    /// Aggregate rates curves detail (optional).
    pub rates_detail: Option<RatesCurvesAttribution>,

    /// Aggregate credit curves detail (optional).
    pub credit_detail: Option<CreditCurvesAttribution>,

    /// Aggregate inflation curves detail (optional).
    pub inflation_detail: Option<InflationCurvesAttribution>,

    /// Aggregate correlations detail (optional).
    pub correlations_detail: Option<CorrelationsAttribution>,

    /// Aggregate FX detail (optional).
    pub fx_detail: Option<FxAttribution>,

    /// Aggregate volatility detail (optional).
    pub vol_detail: Option<VolAttribution>,

    /// Aggregate scalars detail (optional).
    pub scalars_detail: Option<ScalarsAttribution>,

    /// True if any constituent position's attribution was flagged invalid
    /// (for example, a non-finite factor sensitivity — see
    /// [`PnlAttribution::result_invalid`]). When `true`, the portfolio
    /// aggregates and [`PortfolioAttribution::reconciliation_check`] are not
    /// trustworthy and must not be relied on for reporting.
    ///
    /// Defaults to `false`; results serialized before this field existed
    /// deserialize as `false`.
    #[serde(default)]
    pub result_invalid: bool,
}

fn default_zero_usd_money() -> Money {
    Money::new(0.0, Currency::USD)
}

/// Report from reconciling position-level P&L attribution against portfolio totals.
///
/// Verifies that the sum of all factor P&L buckets plus FX translation equals `total_pnl`.
#[derive(Debug, Clone)]
pub struct ReconciliationReport {
    /// Total residual: `total_pnl - (sum of factor buckets + fx_translation_pnl)`.
    pub total_residual: f64,
    /// Whether the reconciliation passes within tolerance.
    pub is_reconciled: bool,
    /// Tolerance used for the check.
    pub tolerance: f64,
}

struct PositionAttributionData {
    position_id: PositionId,
    pos_attr: PnlAttribution,
    val_t0_native: Money,
    inst_ccy: Currency,
}

#[derive(Copy, Clone)]
enum FactorBucket {
    TotalPnl = 0,
    Carry = 1,
    RatesCurvesPnl = 2,
    CreditCurvesPnl = 3,
    InflationCurvesPnl = 4,
    CorrelationsPnl = 5,
    FxPnl = 6,
    FxTranslationPnl = 7,
    CrossFactorPnl = 8,
    VolPnl = 9,
    ModelParamsPnl = 10,
    MarketScalarsPnl = 11,
    Residual = 12,
}

const N_BUCKETS: usize = 13;

/// Private helper that aggregates portfolio-level factor P&L buckets using
/// Neumaier summation. An enum-keyed array eliminates per-field duplication
/// and makes the bucket-to-output mapping a single point of truth.
struct FactorAccumulator {
    buckets: [NeumaierAccumulator; N_BUCKETS],
    /// Set once any folded-in position carried `result_invalid = true`.
    result_invalid: bool,
}

impl FactorAccumulator {
    fn new() -> Self {
        Self {
            buckets: [NeumaierAccumulator::new(); N_BUCKETS],
            result_invalid: false,
        }
    }

    fn add(&mut self, b: FactorBucket, x: f64) {
        self.buckets[b as usize].add(x);
    }

    fn total(&self, b: FactorBucket) -> f64 {
        self.buckets[b as usize].current()
    }

    fn add_converted(
        &mut self,
        pos_attr: &PnlAttribution,
        convert: &impl Fn(Money) -> Result<Money>,
    ) -> Result<()> {
        // A single invalid constituent makes the whole aggregate untrustworthy.
        self.result_invalid |= pos_attr.result_invalid;
        self.add(
            FactorBucket::TotalPnl,
            convert(pos_attr.total_pnl)?.amount(),
        );
        self.add(FactorBucket::Carry, convert(pos_attr.carry)?.amount());
        self.add(
            FactorBucket::RatesCurvesPnl,
            convert(pos_attr.rates_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::CreditCurvesPnl,
            convert(pos_attr.credit_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::InflationCurvesPnl,
            convert(pos_attr.inflation_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::CorrelationsPnl,
            convert(pos_attr.correlations_pnl)?.amount(),
        );
        self.add(FactorBucket::FxPnl, convert(pos_attr.fx_pnl)?.amount());
        self.add(
            FactorBucket::CrossFactorPnl,
            convert(pos_attr.cross_factor_pnl)?.amount(),
        );
        self.add(FactorBucket::VolPnl, convert(pos_attr.vol_pnl)?.amount());
        self.add(
            FactorBucket::ModelParamsPnl,
            convert(pos_attr.model_params_pnl)?.amount(),
        );
        self.add(
            FactorBucket::MarketScalarsPnl,
            convert(pos_attr.market_scalars_pnl)?.amount(),
        );
        self.add(FactorBucket::Residual, convert(pos_attr.residual)?.amount());
        Ok(())
    }

    fn add_fx_translation(&mut self, amount: f64) {
        self.add(FactorBucket::FxTranslationPnl, amount);
        self.add(FactorBucket::TotalPnl, amount);
    }

    fn into_portfolio_attribution(
        self,
        base_ccy: Currency,
        by_position: IndexMap<PositionId, PnlAttribution>,
    ) -> PortfolioAttribution {
        PortfolioAttribution {
            total_pnl: Money::new(self.total(FactorBucket::TotalPnl), base_ccy),
            carry: Money::new(self.total(FactorBucket::Carry), base_ccy),
            rates_curves_pnl: Money::new(self.total(FactorBucket::RatesCurvesPnl), base_ccy),
            credit_curves_pnl: Money::new(self.total(FactorBucket::CreditCurvesPnl), base_ccy),
            inflation_curves_pnl: Money::new(
                self.total(FactorBucket::InflationCurvesPnl),
                base_ccy,
            ),
            correlations_pnl: Money::new(self.total(FactorBucket::CorrelationsPnl), base_ccy),
            fx_pnl: Money::new(self.total(FactorBucket::FxPnl), base_ccy),
            fx_translation_pnl: Money::new(self.total(FactorBucket::FxTranslationPnl), base_ccy),
            cross_factor_pnl: Money::new(self.total(FactorBucket::CrossFactorPnl), base_ccy),
            vol_pnl: Money::new(self.total(FactorBucket::VolPnl), base_ccy),
            model_params_pnl: Money::new(self.total(FactorBucket::ModelParamsPnl), base_ccy),
            market_scalars_pnl: Money::new(self.total(FactorBucket::MarketScalarsPnl), base_ccy),
            residual: Money::new(self.total(FactorBucket::Residual), base_ccy),
            by_position,
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: self.result_invalid,
        }
    }
}

fn attribute_single_position(
    position: &crate::position::Position,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: &AttributionMethod,
) -> Result<PositionAttributionData> {
    let value_at_t0 = || {
        position
            .instrument
            .value(market_t0, as_of_t0)
            .map_err(|e| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Attribution T0 valuation failed: {}", e),
            })
    };

    let (mut pos_attr, val_t0_native_unit) = match method {
        AttributionMethod::Parallel => {
            let attr = attribute_pnl_parallel(
                &position.instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                config,
                ExecutionPolicy::Serial,
            )
            .map_err(|e| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Attribution failed: {}", e),
            })?;

            (attr, value_at_t0()?)
        }

        AttributionMethod::Waterfall(ref order) => {
            let attr = finstack_attribution::attribute_pnl_waterfall(
                &position.instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                config,
                order.clone(),
                false,
                None,
            )
            .map_err(|e| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Attribution failed: {}", e),
            })?;

            (attr, value_at_t0()?)
        }

        AttributionMethod::MetricsBased => {
            let metrics = default_attribution_metrics();

            let val_t0 = position
                .instrument
                .price_with_metrics(
                    market_t0,
                    as_of_t0,
                    &metrics,
                    finstack_valuations::instruments::PricingOptions::default(),
                )
                .map_err(|e: finstack_core::Error| Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: format!("Attribution T0 valuation failed: {}", e),
                })?;

            let val_t1 = position
                .instrument
                .price_with_metrics(
                    market_t1,
                    as_of_t1,
                    &metrics,
                    finstack_valuations::instruments::PricingOptions::default(),
                )
                .map_err(|e: finstack_core::Error| Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: format!("Attribution T1 valuation failed: {}", e),
                })?;

            let attr = attribute_pnl_metrics_based(
                &position.instrument,
                market_t0,
                market_t1,
                &val_t0,
                &val_t1,
                as_of_t0,
                as_of_t1,
            )
            .map_err(|e| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Attribution failed: {}", e),
            })?;

            (attr, val_t0.value)
        }

        AttributionMethod::Taylor(ref taylor_config) => {
            let attr = finstack_attribution::attribute_pnl_taylor(
                &position.instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                taylor_config,
                ExecutionPolicy::Serial,
            )
            .map_err(|e| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Taylor attribution failed: {}", e),
            })?;

            (attr, value_at_t0()?)
        }
    };

    pos_attr.scale(position.scale_factor());
    let val_t0_native = position.scale_value(val_t0_native_unit);
    let inst_ccy = pos_attr.total_pnl.currency();

    Ok(PositionAttributionData {
        position_id: position.position_id.clone(),
        pos_attr,
        val_t0_native,
        inst_ccy,
    })
}

/// Perform P&L attribution for an entire portfolio.
///
/// Attributes each position's P&L and aggregates to portfolio base currency.
/// Each position is attributed using the specified method (Parallel, Waterfall,
/// or MetricsBased), and the results are converted to the portfolio's base
/// currency with explicit FX translation P&L tracking.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to attribute
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `as_of_t0` - Valuation date at T₀ (typically yesterday for day-over-day)
/// * `as_of_t1` - Valuation date at T₁ (typically today for day-over-day)
/// * `config` - Finstack configuration
/// * `method` - Attribution methodology (Parallel, Waterfall, or MetricsBased)
///
/// # Returns
///
/// Portfolio-level attribution with per-position breakdown.
///
/// # Examples
///
/// ```ignore
/// use finstack_portfolio::attribution::attribute_portfolio_pnl;
/// use finstack_attribution::AttributionMethod;
/// use finstack_core::config::FinstackConfig;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_portfolio::Portfolio;
/// use time::macros::date;
///
/// # fn main() -> finstack_portfolio::Result<()> {
/// let as_of_t0 = date!(2025-11-20);  // Yesterday
/// let as_of_t1 = date!(2025-11-21);  // Today
///
/// # let portfolio: Portfolio = unimplemented!("Provide your portfolio");
/// # let market_t0: MarketContext = unimplemented!("Provide market at t0");
/// # let market_t1: MarketContext = unimplemented!("Provide market at t1");
/// # let config: FinstackConfig = unimplemented!("Provide finstack config");
/// let attribution = attribute_portfolio_pnl(
///     &portfolio,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
///     AttributionMethod::Parallel,
/// )?;
///
/// println!("Portfolio P&L: {}", attribution.total_pnl);
/// println!("Total Carry: {}", attribution.carry);
/// println!("FX Translation: {}", attribution.fx_translation_pnl);
///
/// // Drill down to specific position
/// if let Some(pos_attr) = attribution.by_position.get("POS_001") {
///     println!("Position POS_001 P&L: {}", pos_attr.total_pnl);
/// }
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Parametric risk-reporting background:
///   `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
pub fn attribute_portfolio_pnl(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: AttributionMethod,
) -> Result<PortfolioAttribution> {
    let base_ccy = portfolio.base_ccy;

    use rayon::prelude::*;
    let position_data: Vec<PositionAttributionData> = portfolio
        .positions
        .par_iter()
        .map(|position| {
            attribute_single_position(
                position, market_t0, market_t1, as_of_t0, as_of_t1, config, &method,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let mut acc = FactorAccumulator::new();
    let mut by_position: IndexMap<PositionId, PnlAttribution> = IndexMap::new();

    // Hoisted out of the per-position loop: the closure captures `market_t1`,
    // `base_ccy`, and `as_of_t1` by reference and is reused for every field of
    // every position. Delegates to the shared `crate::fx::convert_to_base`
    // helper so the FX lookup + error mapping stay consistent with the rest of
    // the portfolio crate.
    let convert = |money: Money| -> Result<Money> {
        crate::fx::convert_to_base(money, as_of_t1, market_t1, base_ccy)
    };

    for data in position_data {
        let PositionAttributionData {
            position_id,
            pos_attr,
            val_t0_native,
            inst_ccy,
        } = data;

        acc.add_converted(&pos_attr, &convert)?;

        if inst_ccy != base_ccy {
            let fx_t0 = market_t0.fx().ok_or_else(|| {
                Error::MissingMarketData("FX matrix at T0 not available".to_string())
            })?;
            let fx_t1 = market_t1.fx().ok_or_else(|| {
                Error::MissingMarketData("FX matrix at T1 not available".to_string())
            })?;

            let query_t0 = FxQuery::new(inst_ccy, base_ccy, as_of_t0);
            let rate_t0 = fx_t0
                .rate(query_t0)
                .map_err(|_| Error::FxConversionFailed {
                    from: inst_ccy,
                    to: base_ccy,
                })?;

            let query_t1 = FxQuery::new(inst_ccy, base_ccy, as_of_t1);
            let rate_t1 = fx_t1
                .rate(query_t1)
                .map_err(|_| Error::FxConversionFailed {
                    from: inst_ccy,
                    to: base_ccy,
                })?;

            let principal_amount = val_t0_native.amount();
            let principal_translation = principal_amount * (rate_t1.rate - rate_t0.rate);

            let total_translation = principal_translation;
            acc.add_fx_translation(total_translation);
        }

        by_position.insert(position_id, pos_attr);
    }

    Ok(acc.into_portfolio_attribution(base_ccy, by_position))
}

impl PortfolioAttribution {
    /// Export portfolio attribution as CSV string.
    ///
    /// Returns summary row with total attribution by factor.
    pub fn to_csv(&self) -> String {
        let mut lines = Vec::new();

        // Header
        lines.push(
            "total,carry,rates_curves,credit_curves,inflation_curves,\
             correlations,fx,fx_translation,cross_factor,vol,model_params,market_scalars,residual"
                .to_string(),
        );

        // Data row
        lines.push(format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.total_pnl.amount(),
            self.carry.amount(),
            self.rates_curves_pnl.amount(),
            self.credit_curves_pnl.amount(),
            self.inflation_curves_pnl.amount(),
            self.correlations_pnl.amount(),
            self.fx_pnl.amount(),
            self.fx_translation_pnl.amount(),
            self.cross_factor_pnl.amount(),
            self.vol_pnl.amount(),
            self.model_params_pnl.amount(),
            self.market_scalars_pnl.amount(),
            self.residual.amount(),
        ));

        lines.join("\n")
    }

    /// Export position-by-position detail as CSV string.
    ///
    /// Returns one row per position with full factor breakdown.
    pub fn position_detail_to_csv(&self) -> String {
        let mut lines = Vec::new();

        // Header
        lines.push(
            "position_id,total,carry,rates_curves,credit_curves,\
             inflation_curves,correlations,fx,vol,model_params,\
             market_scalars,residual"
                .to_string(),
        );

        // Data rows (one per position)
        for (position_id, pos_attr) in &self.by_position {
            lines.push(format!(
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                position_id,
                pos_attr.total_pnl.amount(),
                pos_attr.carry.amount(),
                pos_attr.rates_curves_pnl.amount(),
                pos_attr.credit_curves_pnl.amount(),
                pos_attr.inflation_curves_pnl.amount(),
                pos_attr.correlations_pnl.amount(),
                pos_attr.fx_pnl.amount(),
                pos_attr.vol_pnl.amount(),
                pos_attr.model_params_pnl.amount(),
                pos_attr.market_scalars_pnl.amount(),
                pos_attr.residual.amount(),
            ));
        }

        lines.join("\n")
    }

    /// Generate explanation tree for portfolio attribution.
    pub fn explain(&self) -> String {
        let mut lines = Vec::new();

        let fmt = |amount: &Money, total: &Money| -> String {
            let pct = if total.amount().abs() > 1e-10 {
                (amount.amount() / total.amount()) * 100.0
            } else {
                0.0
            };
            format!("{} ({:.1}%)", amount, pct)
        };

        lines.push(format!("Portfolio P&L: {}", self.total_pnl));
        lines.push(format!("  ├─ Carry: {}", fmt(&self.carry, &self.total_pnl)));
        lines.push(format!(
            "  ├─ Rates Curves: {}",
            fmt(&self.rates_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Credit Curves: {}",
            fmt(&self.credit_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Inflation: {}",
            fmt(&self.inflation_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Correlations: {}",
            fmt(&self.correlations_pnl, &self.total_pnl)
        ));
        lines.push(format!("  ├─ FX: {}", fmt(&self.fx_pnl, &self.total_pnl)));
        lines.push(format!(
            "  ├─ FX Translation: {}",
            fmt(&self.fx_translation_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Cross Factor: {}",
            fmt(&self.cross_factor_pnl, &self.total_pnl)
        ));
        lines.push(format!("  ├─ Vol: {}", fmt(&self.vol_pnl, &self.total_pnl)));
        lines.push(format!(
            "  ├─ Model Params: {}",
            fmt(&self.model_params_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Market Scalars: {}",
            fmt(&self.market_scalars_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  └─ Residual: {}",
            fmt(&self.residual, &self.total_pnl)
        ));

        lines.join("\n")
    }

    /// Check that the sum of all factor P&L buckets plus FX translation
    /// reconciles against `total_pnl` within the given tolerance.
    ///
    /// This uses the portfolio-level (base-currency) aggregates, so no
    /// additional FX conversion is needed.
    ///
    /// If [`PortfolioAttribution::result_invalid`] is set, `is_reconciled` is
    /// forced to `false` regardless of the numeric residual: an aggregate built
    /// from a corrupted constituent must never be reported as reconciled.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Absolute tolerance in base-currency units (e.g. 0.01
    ///   for one-cent precision).
    pub fn reconciliation_check(&self, tolerance: f64) -> ReconciliationReport {
        let mut acc = NeumaierAccumulator::new();
        acc.add(self.carry.amount());
        acc.add(self.rates_curves_pnl.amount());
        acc.add(self.credit_curves_pnl.amount());
        acc.add(self.inflation_curves_pnl.amount());
        acc.add(self.correlations_pnl.amount());
        acc.add(self.fx_pnl.amount());
        acc.add(self.cross_factor_pnl.amount());
        acc.add(self.vol_pnl.amount());
        acc.add(self.model_params_pnl.amount());
        acc.add(self.market_scalars_pnl.amount());
        acc.add(self.residual.amount());
        acc.add(self.fx_translation_pnl.amount());

        let total_residual = self.total_pnl.amount() - acc.total();
        // A portfolio aggregated from an invalid constituent cannot be trusted
        // to reconcile, even if the (equally corrupted) buckets happen to net
        // to within tolerance — never report `is_reconciled` in that case.
        let is_reconciled = !self.result_invalid && total_residual.abs() <= tolerance;

        ReconciliationReport {
            total_residual,
            is_reconciled,
            tolerance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn factor_bucket_indices_are_unique_and_cover_n_buckets() {
        let buckets = [
            FactorBucket::TotalPnl,
            FactorBucket::Carry,
            FactorBucket::RatesCurvesPnl,
            FactorBucket::CreditCurvesPnl,
            FactorBucket::InflationCurvesPnl,
            FactorBucket::CorrelationsPnl,
            FactorBucket::FxPnl,
            FactorBucket::FxTranslationPnl,
            FactorBucket::CrossFactorPnl,
            FactorBucket::VolPnl,
            FactorBucket::ModelParamsPnl,
            FactorBucket::MarketScalarsPnl,
            FactorBucket::Residual,
        ];
        assert_eq!(buckets.len(), N_BUCKETS);
        let mut seen = [false; N_BUCKETS];
        for b in buckets {
            let idx = b as usize;
            assert!(!seen[idx], "duplicate index {idx}");
            seen[idx] = true;
        }
        assert!(seen.iter().all(|&v| v), "gap in bucket indices");
    }

    fn sample_position_attr(
        position_id: &str,
        total: f64,
        carry: f64,
        residual: f64,
    ) -> PnlAttribution {
        let mut attr = PnlAttribution::new(
            Money::new(total, Currency::USD),
            position_id,
            date!(2026 - 01 - 02),
            date!(2026 - 01 - 03),
            AttributionMethod::Parallel,
        );
        attr.carry = Money::new(carry, Currency::USD);
        attr.rates_curves_pnl = Money::new(total - carry - residual, Currency::USD);
        attr.residual = Money::new(residual, Currency::USD);
        attr
    }

    #[test]
    fn test_portfolio_attribution_structure() {
        let base_ccy = Currency::USD;
        let zero = Money::new(0.0, base_ccy);

        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::new(1000.0, base_ccy),
            carry: Money::new(100.0, base_ccy),
            rates_curves_pnl: Money::new(500.0, base_ccy),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: Money::new(400.0, base_ccy),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let csv = portfolio_attr.to_csv();
        assert!(csv.contains("total"));
        assert!(csv.contains("1000"));
    }

    #[test]
    fn test_default_metrics_nonempty() {
        assert!(!default_attribution_metrics().is_empty());
    }

    #[test]
    fn test_position_detail_to_csv_includes_each_position_breakdown() {
        let mut by_position = IndexMap::new();
        by_position.insert(
            PositionId::from("POS_A"),
            sample_position_attr("POS_A", 120.0, 10.0, 5.0),
        );
        by_position.insert(
            PositionId::from("POS_B"),
            sample_position_attr("POS_B", -20.0, -2.0, 1.0),
        );

        let zero = Money::new(0.0, Currency::USD);
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::new(100.0, Currency::USD),
            carry: Money::new(8.0, Currency::USD),
            rates_curves_pnl: Money::new(87.0, Currency::USD),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: Money::new(5.0, Currency::USD),
            by_position,
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let csv = portfolio_attr.position_detail_to_csv();
        assert!(csv.contains("position_id,total,carry"));
        assert!(csv.contains("POS_A,120"));
        assert!(csv.contains("POS_B,-20"));
    }

    #[test]
    fn test_explain_formats_percentages_and_zero_total_safely() {
        let zero = Money::new(0.0, Currency::USD);
        let explained = PortfolioAttribution {
            total_pnl: Money::new(200.0, Currency::USD),
            carry: Money::new(20.0, Currency::USD),
            rates_curves_pnl: Money::new(100.0, Currency::USD),
            credit_curves_pnl: Money::new(10.0, Currency::USD),
            inflation_curves_pnl: Money::new(5.0, Currency::USD),
            correlations_pnl: Money::new(15.0, Currency::USD),
            fx_pnl: Money::new(25.0, Currency::USD),
            fx_translation_pnl: Money::new(10.0, Currency::USD),
            cross_factor_pnl: zero,
            vol_pnl: Money::new(5.0, Currency::USD),
            model_params_pnl: Money::new(5.0, Currency::USD),
            market_scalars_pnl: Money::new(3.0, Currency::USD),
            residual: Money::new(2.0, Currency::USD),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };
        let rendered = explained.explain();
        assert!(rendered.contains("Portfolio P&L: USD 200.00"));
        assert!(rendered.contains("Carry: USD 20.00 (10.0%)"));
        assert!(rendered.contains("FX Translation: USD 10.00 (5.0%)"));
        assert!(rendered.contains("Residual: USD 2.00 (1.0%)"));

        let zero_total = PortfolioAttribution {
            total_pnl: zero,
            carry: Money::new(5.0, Currency::USD),
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: Money::new(-5.0, Currency::USD),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };
        let zero_rendered = zero_total.explain();
        assert!(zero_rendered.contains("Carry: USD 5.00 (0.0%)"));
        assert!(zero_rendered.contains("Residual: USD -5.00 (0.0%)"));
    }

    #[test]
    fn test_reconciliation_check_passes_for_consistent_attribution() {
        let base_ccy = Currency::USD;
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::new(200.0, base_ccy),
            carry: Money::new(20.0, base_ccy),
            rates_curves_pnl: Money::new(100.0, base_ccy),
            credit_curves_pnl: Money::new(10.0, base_ccy),
            inflation_curves_pnl: Money::new(5.0, base_ccy),
            correlations_pnl: Money::new(15.0, base_ccy),
            fx_pnl: Money::new(25.0, base_ccy),
            fx_translation_pnl: Money::new(10.0, base_ccy),
            cross_factor_pnl: Money::new(0.0, base_ccy),
            vol_pnl: Money::new(5.0, base_ccy),
            model_params_pnl: Money::new(5.0, base_ccy),
            market_scalars_pnl: Money::new(3.0, base_ccy),
            residual: Money::new(2.0, base_ccy),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(
            report.is_reconciled,
            "expected reconciliation to pass, residual = {}",
            report.total_residual
        );
        assert!(
            report.total_residual.abs() < 1e-10,
            "residual should be ~0, got {}",
            report.total_residual
        );
    }

    #[test]
    fn reconciliation_check_includes_cross_factor_pnl() {
        let base_ccy = Currency::USD;
        let zero = Money::new(0.0, base_ccy);
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::new(107.0, base_ccy),
            carry: Money::new(20.0, base_ccy),
            rates_curves_pnl: Money::new(80.0, base_ccy),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            vol_pnl: zero,
            cross_factor_pnl: Money::new(7.0, base_ccy),
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(report.is_reconciled);
        assert!(report.total_residual.abs() < 1e-10);
    }

    #[test]
    fn test_reconciliation_check_fails_when_totals_mismatch() {
        let base_ccy = Currency::USD;
        let zero = Money::new(0.0, base_ccy);
        // total_pnl deliberately mismatches the sum of factor buckets
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::new(1000.0, base_ccy),
            carry: Money::new(100.0, base_ccy),
            rates_curves_pnl: Money::new(500.0, base_ccy),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(
            !report.is_reconciled,
            "expected reconciliation to fail for mismatched totals"
        );
        assert!(
            (report.total_residual - 400.0).abs() < 1e-10,
            "residual should be 400.0, got {}",
            report.total_residual
        );
    }

    /// Audit (Portfolio): a single invalid constituent must flag the whole
    /// aggregate invalid so downstream reporting can refuse to trust it.
    #[test]
    fn portfolio_aggregation_propagates_position_result_invalid() {
        let good = sample_position_attr("GOOD", 50.0, 5.0, 2.0);
        let mut bad = sample_position_attr("BAD", 100.0, 10.0, 5.0);
        bad.result_invalid = true;

        let identity = |m: Money| -> Result<Money> { Ok(m) };
        let mut acc = FactorAccumulator::new();
        acc.add_converted(&good, &identity)
            .expect("same-currency add");
        acc.add_converted(&bad, &identity)
            .expect("same-currency add");

        let portfolio = acc.into_portfolio_attribution(Currency::USD, IndexMap::new());
        assert!(
            portfolio.result_invalid,
            "one invalid position must flag the whole portfolio invalid"
        );
    }

    /// Audit (Portfolio): `reconciliation_check` must never report a result as
    /// reconciled when `result_invalid` is set, even if the corrupted buckets
    /// happen to net to within tolerance.
    #[test]
    fn reconciliation_check_fails_when_result_invalid_even_if_numbers_net() {
        let base = Currency::USD;
        let zero = Money::new(0.0, base);
        let attr = PortfolioAttribution {
            total_pnl: Money::new(100.0, base),
            carry: Money::new(100.0, base),
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: true,
        };
        // The buckets net exactly to total_pnl, so a numeric-only check would
        // pass — the result_invalid gate must still force is_reconciled false.
        let report = attr.reconciliation_check(0.01);
        assert!(
            !report.is_reconciled,
            "must not report reconciled when result_invalid is set"
        );
    }
}
