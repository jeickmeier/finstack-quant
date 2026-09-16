//! Stochastic pricing results.

use finstack_quant_core::money::Money;

use super::config::PricingMode;
use crate::instruments::fixed_income::structured_credit::types::TrancheSeniority;

/// Stochastic pricing result for a structured credit deal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct StochasticPricingResult {
    /// Net present value of the deal
    pub npv: Money,

    /// Clean price (percentage of notional), UNADJUSTED for accrued interest.
    ///
    /// SC-m29: this equals [`Self::dirty_price`]. The deal-level stochastic
    /// result carries no per-tranche interest flows, so accrued cannot be
    /// computed here (the same constraint documented on the accrued
    /// calculator). It is reported unadjusted rather than fabricated: the
    /// error is at most one period's accrued interest, in a known direction.
    /// Use `calculate_tranche_metrics` when an accrued-adjusted clean price is
    /// required.
    pub clean_price: f64,

    /// Dirty price (percentage of notional), including accrued interest.
    ///
    /// This is the authoritative price: it is the present value of all future
    /// cashflows from the valuation date, which by definition contains the
    /// accrued portion of the next coupon.
    pub dirty_price: f64,

    /// Expected loss (probability-weighted average loss)
    pub expected_loss: Money,

    /// Unexpected loss (loss standard deviation)
    pub unexpected_loss: Money,

    /// Expected shortfall (tail risk metric)
    pub expected_shortfall: Money,

    /// ES confidence level used
    pub es_confidence: f64,

    /// Standard error of the mean PV estimate
    pub pv_std_error: f64,

    /// 95% confidence interval for the mean PV
    pub pv_confidence_interval: (f64, f64),

    /// Number of scenario paths
    pub num_paths: usize,

    /// Pricing mode used.
    pub pricing_mode: PricingMode,

    /// Tranche-level results
    pub tranche_results: Vec<TranchePricingResult>,

    /// Fraction of paths on which a collateral draw could not be funded from
    /// the reserve account and that period's principal collections (`0.0`
    /// for pools without draws).
    #[serde(default)]
    pub unfunded_draw_path_fraction: f64,

    /// Mean over paths of the collateral draws funded through the reserve
    /// account and principal collections (revolver utilization increases,
    /// delayed draws and loan-equivalent draws at default).
    pub expected_collateral_draws: Money,

    /// Mean over paths of the draw option cost: the value to the deal of its
    /// revolvers' simulated draws having been made at the contractual margin
    /// instead of each path's fair spread, obtained per path as the present
    /// value of the actual tranche cashflows less that of a counterfactual
    /// run where every draw accrues at its fair spread. Negative when spreads
    /// widen; zero for pools without stochastic revolvers.
    pub draw_option_cost: Money,

    /// Per-path draw option cost in path order (same currency as `npv`),
    /// populated only for pools with stochastic revolvers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub draw_option_cost_paths: Vec<f64>,
}

impl StochasticPricingResult {
    /// Create a new pricing result.
    ///
    /// # Arguments
    ///
    /// * `npv` - Deal net present value and result currency.
    /// * `expected_loss` - Probability-weighted expected loss in the result currency.
    /// * `num_paths` - Number of simulated or enumerated scenario paths.
    /// * `pricing_mode` - Exact typed mode and mode parameters used for pricing.
    pub fn new(
        npv: Money,
        expected_loss: Money,
        num_paths: usize,
        pricing_mode: PricingMode,
    ) -> Self {
        let currency = npv.currency();
        Self {
            npv,
            clean_price: 0.0,
            dirty_price: 0.0,
            expected_loss,
            unexpected_loss: Money::from((0_i64, currency)),
            expected_shortfall: Money::from((0_i64, currency)),
            es_confidence: 0.95,
            pv_std_error: 0.0,
            pv_confidence_interval: (0.0, 0.0),
            num_paths,
            pricing_mode,
            tranche_results: Vec::new(),
            unfunded_draw_path_fraction: 0.0,
            expected_collateral_draws: Money::from((0_i64, currency)),
            draw_option_cost: Money::from((0_i64, currency)),
            draw_option_cost_paths: Vec::new(),
        }
    }

    /// Set unexpected loss.
    pub fn with_unexpected_loss(mut self, ul: Money) -> Self {
        self.unexpected_loss = ul;
        self
    }

    /// Set expected shortfall.
    pub fn with_expected_shortfall(mut self, es: Money, confidence: f64) -> Self {
        self.expected_shortfall = es;
        self.es_confidence = confidence;
        self
    }
}

/// Tranche-level pricing result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TranchePricingResult {
    /// Tranche identifier
    pub tranche_id: String,

    /// Tranche seniority level.
    pub seniority: TrancheSeniority,

    /// Net present value
    pub npv: Money,

    /// Expected loss
    pub expected_loss: Money,

    /// Unexpected loss
    pub unexpected_loss: Money,

    /// Expected shortfall
    pub expected_shortfall: Money,

    /// Attachment point (percentage)
    pub attachment: f64,

    /// Detachment point (percentage)
    pub detachment: f64,

    /// Average life (years)
    pub average_life: f64,

    /// Weighted average spread to LIBOR/SOFR
    pub spread: f64,

    /// Credit duration (price sensitivity to credit spread)
    pub credit_duration: f64,

    /// This tranche's share of the deal's draw option cost: the mean over
    /// paths of its present value on the actual run less that on the
    /// counterfactual run where revolver draws accrue at their fair spread.
    /// Tranche shares sum to the deal's `draw_option_cost` on every path.
    pub draw_option_cost: Money,
}

impl TranchePricingResult {
    /// Create a new tranche pricing result.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Stable identifier of the priced tranche.
    /// * `seniority` - Canonical typed seniority serialized in snake_case.
    /// * `npv` - Tranche net present value and result currency.
    pub fn new(tranche_id: String, seniority: TrancheSeniority, npv: Money) -> Self {
        let currency = npv.currency();
        Self {
            tranche_id,
            seniority,
            npv,
            expected_loss: Money::from((0_i64, currency)),
            unexpected_loss: Money::from((0_i64, currency)),
            expected_shortfall: Money::from((0_i64, currency)),
            attachment: 0.0,
            detachment: 1.0,
            average_life: 0.0,
            spread: 0.0,
            credit_duration: 0.0,
            draw_option_cost: Money::from((0_i64, currency)),
        }
    }

    /// Set the tranche's share of the deal draw option cost.
    pub fn with_draw_option_cost(mut self, cost: Money) -> Self {
        self.draw_option_cost = cost;
        self
    }

    /// Set attachment and detachment points.
    pub fn with_subordination(mut self, attachment: f64, detachment: f64) -> Self {
        self.attachment = attachment;
        self.detachment = detachment;
        self
    }

    /// Set risk metrics.
    pub fn with_risk_metrics(
        mut self,
        expected_loss: Money,
        unexpected_loss: Money,
        expected_shortfall: Money,
    ) -> Self {
        self.expected_loss = expected_loss;
        self.unexpected_loss = unexpected_loss;
        self.expected_shortfall = expected_shortfall;
        self
    }

    /// Set average life.
    pub fn with_average_life(mut self, wal: f64) -> Self {
        self.average_life = wal;
        self
    }

    /// Set credit duration.
    pub fn with_credit_duration(mut self, duration: f64) -> Self {
        self.credit_duration = duration;
        self
    }

    /// Get thickness (width of the tranche).
    pub fn thickness(&self) -> f64 {
        self.detachment - self.attachment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_stochastic_result_creation() {
        let currency = Currency::USD;
        let npv = Money::from((1_000_000_i64, currency));
        let el = Money::from((50_000_i64, currency));

        let result = StochasticPricingResult::new(npv, el, 1000, PricingMode::Tree);

        assert_eq!(result.num_paths, 1000);
        assert!(result.expected_loss.amount() > 0.0);
    }

    #[test]
    fn test_tranche_result_creation() {
        let currency = Currency::USD;
        let npv = Money::from((100_000_i64, currency));

        let tranche = TranchePricingResult::new("A".to_string(), TrancheSeniority::Senior, npv)
            .with_subordination(0.20, 1.00);

        assert!((tranche.thickness() - 0.80).abs() < 1e-10);
        assert_eq!(
            serde_json::to_value(&tranche).expect("serialize")["seniority"],
            "senior"
        );
    }

    #[test]
    fn test_builder_pattern() {
        let currency = Currency::USD;
        let npv = Money::from((1_000_000_i64, currency));
        let el = Money::from((50_000_i64, currency));
        let ul = Money::from((75_000_i64, currency));
        let es = Money::from((100_000_i64, currency));

        let result = StochasticPricingResult::new(npv, el, 1000, PricingMode::Tree)
            .with_unexpected_loss(ul)
            .with_expected_shortfall(es, 0.99);

        assert!(result.unexpected_loss.amount() > 0.0);
        assert!((result.es_confidence - 0.99).abs() < 1e-10);
    }
}
