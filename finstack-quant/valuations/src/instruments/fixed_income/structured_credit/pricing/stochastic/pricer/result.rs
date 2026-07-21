//! Stochastic pricing results.

use finstack_quant_core::money::Money;

/// Stochastic pricing result for a structured credit deal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

    /// Pricing mode used
    pub pricing_mode: String,

    /// Tranche-level results
    pub tranche_results: Vec<TranchePricingResult>,
}

impl StochasticPricingResult {
    /// Create a new pricing result.
    pub fn new(npv: Money, expected_loss: Money, num_paths: usize) -> Self {
        let currency = npv.currency();
        Self {
            npv,
            clean_price: 0.0,
            dirty_price: 0.0,
            expected_loss,
            unexpected_loss: Money::new(0.0, currency),
            expected_shortfall: Money::new(0.0, currency),
            es_confidence: 0.95,
            pv_std_error: 0.0,
            pv_confidence_interval: (0.0, 0.0),
            num_paths,
            pricing_mode: "Tree".to_string(),
            tranche_results: Vec::new(),
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

    /// Set tranche results.
    pub fn with_tranche_results(mut self, results: Vec<TranchePricingResult>) -> Self {
        self.tranche_results = results;
        self
    }

    /// Get loss ratio (EL / notional).
    pub fn loss_ratio(&self) -> f64 {
        let npv_val = self.npv.amount();
        if npv_val.abs() > 1e-10 {
            self.expected_loss.amount() / npv_val.abs()
        } else {
            0.0
        }
    }
}

/// Tranche-level pricing result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TranchePricingResult {
    /// Tranche identifier
    pub tranche_id: String,

    /// Tranche seniority level
    pub seniority: String,

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
}

impl TranchePricingResult {
    /// Create a new tranche pricing result.
    pub fn new(tranche_id: String, seniority: String, npv: Money) -> Self {
        let currency = npv.currency();
        Self {
            tranche_id,
            seniority,
            npv,
            expected_loss: Money::new(0.0, currency),
            unexpected_loss: Money::new(0.0, currency),
            expected_shortfall: Money::new(0.0, currency),
            attachment: 0.0,
            detachment: 1.0,
            average_life: 0.0,
            spread: 0.0,
            credit_duration: 0.0,
        }
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

    /// Get loss multiple (EL / thickness).
    pub fn loss_multiple(&self) -> f64 {
        let thickness = self.thickness();
        if thickness.abs() > 1e-10 {
            self.expected_loss.amount() / thickness
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_stochastic_result_creation() {
        let currency = Currency::USD;
        let npv = Money::new(1_000_000.0, currency);
        let el = Money::new(50_000.0, currency);

        let result = StochasticPricingResult::new(npv, el, 1000);

        assert_eq!(result.num_paths, 1000);
        assert!(result.expected_loss.amount() > 0.0);
    }

    #[test]
    fn test_tranche_result_creation() {
        let currency = Currency::USD;
        let npv = Money::new(100_000.0, currency);

        let tranche = TranchePricingResult::new("A".to_string(), "Senior".to_string(), npv)
            .with_subordination(0.20, 1.00);

        assert!((tranche.thickness() - 0.80).abs() < 1e-10);
    }

    #[test]
    fn test_builder_pattern() {
        let currency = Currency::USD;
        let npv = Money::new(1_000_000.0, currency);
        let el = Money::new(50_000.0, currency);
        let ul = Money::new(75_000.0, currency);
        let es = Money::new(100_000.0, currency);

        let result = StochasticPricingResult::new(npv, el, 1000)
            .with_unexpected_loss(ul)
            .with_expected_shortfall(es, 0.99);

        assert!(result.unexpected_loss.amount() > 0.0);
        assert!((result.es_confidence - 0.99).abs() < 1e-10);
    }
}
