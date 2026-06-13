//! Margin calculation result types.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_margin::{ImMethodology, NettingSetId, SimmSensitivities};
use std::fmt;

use crate::types::PositionId;

/// Error returned when attempting to aggregate margin results with mismatched currencies.
///
/// This error occurs when [`PortfolioMarginResult::add_netting_set`] is called with a
/// netting set margin in a currency different from the portfolio's base currency.
/// Use [`PortfolioMarginResult::add_netting_set_with_fx`] to handle cross-currency
/// aggregation with explicit FX conversion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrencyMismatchError {
    /// The netting set that caused the error.
    pub netting_set_id: NettingSetId,
    /// Currency of the netting set margin.
    pub netting_set_currency: Currency,
    /// Expected base currency of the portfolio margin result.
    pub base_currency: Currency,
}

impl fmt::Display for CurrencyMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Currency mismatch for netting set {:?}: expected {} but got {}. \
             Use add_netting_set_with_fx() for cross-currency aggregation.",
            self.netting_set_id, self.base_currency, self.netting_set_currency
        )
    }
}

impl std::error::Error for CurrencyMismatchError {}

/// Margin results for a single netting set.
#[derive(Debug, Clone)]
pub struct NettingSetMargin {
    /// Netting set identifier
    pub netting_set_id: NettingSetId,
    /// Calculation date
    pub as_of: Date,
    /// Initial margin requirement
    pub initial_margin: Money,
    /// Variation margin requirement
    pub variation_margin: Money,
    /// Total margin (IM + positive VM)
    pub total_margin: Money,
    /// Number of positions in the netting set
    pub position_count: usize,
    /// IM methodology used
    pub im_methodology: ImMethodology,
    /// Aggregated sensitivities (for SIMM breakdown)
    pub sensitivities: Option<SimmSensitivities>,
    /// Breakdown by risk class (for SIMM)
    pub im_breakdown: HashMap<String, Money>,
}

impl NettingSetMargin {
    /// Create a new netting set margin result.
    ///
    /// # Returns
    ///
    /// Netting-set margin result with total margin computed as
    /// `initial_margin + max(variation_margin, 0)`.
    #[must_use]
    pub fn new(
        netting_set_id: NettingSetId,
        as_of: Date,
        initial_margin: Money,
        variation_margin: Money,
        position_count: usize,
        im_methodology: ImMethodology,
    ) -> Self {
        let currency = initial_margin.currency();
        let total = Money::new(
            initial_margin.amount() + variation_margin.amount().max(0.0),
            currency,
        );

        Self {
            netting_set_id,
            as_of,
            initial_margin,
            variation_margin,
            total_margin: total,
            position_count,
            im_methodology,
            sensitivities: None,
            im_breakdown: HashMap::default(),
        }
    }

    /// Add SIMM breakdown information.
    ///
    /// # Returns
    ///
    /// The updated result for fluent chaining.
    pub fn with_simm_breakdown(
        mut self,
        sensitivities: SimmSensitivities,
        breakdown: HashMap<String, Money>,
    ) -> Self {
        self.sensitivities = Some(sensitivities);
        self.im_breakdown = breakdown;
        self
    }

    /// Check if this is a cleared netting set.
    ///
    /// # Returns
    ///
    /// `true` when the netting set identifier represents a cleared venue.
    #[must_use]
    pub fn is_cleared(&self) -> bool {
        self.netting_set_id.is_cleared()
    }
}

/// Portfolio-wide margin calculation results.
#[derive(Debug, Clone)]
pub struct PortfolioMarginResult {
    /// Calculation date
    pub as_of: Date,
    /// Base currency for aggregated figures
    pub base_currency: Currency,
    /// Total initial margin across all netting sets
    pub total_initial_margin: Money,
    /// Total variation margin across all netting sets
    pub total_variation_margin: Money,
    /// Total margin requirement
    pub total_margin: Money,
    /// Results by netting set
    pub by_netting_set: HashMap<NettingSetId, NettingSetMargin>,
    /// Number of positions included in margin calculation (i.e. those that
    /// successfully landed in a netting set with computed margin).
    pub total_positions: usize,
    /// Number of positions for which the engine did not produce a margin
    /// figure. This is the count of `portfolio.positions.len() -
    /// total_positions` and therefore conflates two qualitatively different
    /// cases: (a) positions whose instruments are not marginable at all and
    /// (b) positions whose sensitivity computation failed and were
    /// recorded in [`Self::degraded_positions`]. Callers that need to
    /// distinguish the two should subtract `degraded_positions.len()` from
    /// this count to recover the count of truly non-marginable positions.
    pub positions_without_margin: usize,
    /// Positions whose sensitivity or VM valuation failed during aggregation.
    /// Each entry pairs the position id with the originating error message.
    /// These positions are also counted in [`Self::positions_without_margin`].
    pub degraded_positions: Vec<(PositionId, String)>,
}

impl PortfolioMarginResult {
    /// Number of portfolio positions that genuinely have no margin spec
    /// (i.e. non-marginable instruments), excluding positions that the
    /// SIMM engine attempted but failed to score. This is the
    /// `positions_without_margin - degraded_positions.len()` figure that
    /// risk reports usually want when distinguishing "instrument types we
    /// don't margin" from "we tried to margin this but the calc broke".
    #[must_use]
    pub fn truly_non_marginable_count(&self) -> usize {
        self.positions_without_margin
            .saturating_sub(self.degraded_positions.len())
    }
}

impl PortfolioMarginResult {
    /// Create a new portfolio margin result.
    ///
    /// # Returns
    ///
    /// Empty portfolio margin report initialized in the supplied base currency.
    #[must_use]
    pub fn new(as_of: Date, base_currency: Currency) -> Self {
        Self {
            as_of,
            base_currency,
            total_initial_margin: Money::new(0.0, base_currency),
            total_variation_margin: Money::new(0.0, base_currency),
            total_margin: Money::new(0.0, base_currency),
            by_netting_set: HashMap::default(),
            total_positions: 0,
            positions_without_margin: 0,
            degraded_positions: Vec::new(),
        }
    }

    /// Add a netting set margin result.
    ///
    /// # Errors
    ///
    /// Returns an error if the netting set margin currency differs from the
    /// portfolio's base currency. Cross-currency margin aggregation requires
    /// explicit FX conversion via [`Self::add_netting_set_with_fx`].
    pub fn add_netting_set(
        &mut self,
        result: NettingSetMargin,
    ) -> Result<(), CurrencyMismatchError> {
        // Validate currency matches base currency
        let ns_currency = result.initial_margin.currency();
        if ns_currency != self.base_currency {
            return Err(CurrencyMismatchError {
                netting_set_id: result.netting_set_id,
                netting_set_currency: ns_currency,
                base_currency: self.base_currency,
            });
        }

        let im = result.initial_margin.amount();
        let vm = result.variation_margin.amount();

        self.total_initial_margin =
            Money::new(self.total_initial_margin.amount() + im, self.base_currency);
        self.total_variation_margin = Money::new(
            self.total_variation_margin.amount() + vm,
            self.base_currency,
        );
        self.total_margin = Money::new(
            self.total_margin.amount() + result.total_margin.amount(),
            self.base_currency,
        );
        self.total_positions += result.position_count;
        self.by_netting_set
            .insert(result.netting_set_id.clone(), result);

        Ok(())
    }

    /// Add a netting set margin result with explicit FX conversion.
    ///
    /// Use this method when the netting set margin is in a different currency
    /// than the portfolio's base currency. The provided FX rate converts from
    /// the netting set currency to the base currency.
    ///
    /// # Arguments
    ///
    /// * `result` - The netting set margin to add
    /// * `fx_rate` - FX rate to convert from netting set currency to base currency
    ///   (e.g., if netting set is EUR and base is USD, rate is EUR/USD)
    pub fn add_netting_set_with_fx(
        &mut self,
        result: NettingSetMargin,
        fx_rate: f64,
    ) -> crate::Result<()> {
        if !fx_rate.is_finite() || fx_rate <= 0.0 {
            return Err(crate::Error::invalid_input(format!(
                "invalid FX rate for margin aggregation on netting set {:?}: rate must be positive and finite, got {fx_rate}",
                result.netting_set_id
            )));
        }

        let im = result.initial_margin.amount() * fx_rate;
        let vm = result.variation_margin.amount() * fx_rate;
        let mut converted = NettingSetMargin::new(
            result.netting_set_id,
            result.as_of,
            Money::new(im, self.base_currency),
            Money::new(vm, self.base_currency),
            result.position_count,
            result.im_methodology,
        );
        converted.sensitivities = result
            .sensitivities
            .map(|sensitivities| sensitivities.scaled_to_currency(self.base_currency, fx_rate));
        converted.im_breakdown = result
            .im_breakdown
            .into_iter()
            .map(|(label, amount)| {
                (
                    label,
                    Money::new(amount.amount() * fx_rate, self.base_currency),
                )
            })
            .collect();
        let total = converted.total_margin.amount();

        self.total_initial_margin =
            Money::new(self.total_initial_margin.amount() + im, self.base_currency);
        self.total_variation_margin = Money::new(
            self.total_variation_margin.amount() + vm,
            self.base_currency,
        );
        self.total_margin = Money::new(self.total_margin.amount() + total, self.base_currency);
        self.total_positions += converted.position_count;
        self.by_netting_set
            .insert(converted.netting_set_id.clone(), converted);
        Ok(())
    }

    /// Get the number of netting sets.
    ///
    /// # Returns
    ///
    /// Number of tracked netting sets.
    #[must_use]
    pub fn netting_set_count(&self) -> usize {
        self.by_netting_set.len()
    }

    /// Get results for cleared vs bilateral netting sets.
    ///
    /// # Returns
    ///
    /// Tuple of `(cleared_total, bilateral_total)` in the portfolio base currency.
    #[must_use]
    pub fn cleared_bilateral_split(&self) -> (Money, Money) {
        let mut cleared = 0.0;
        let mut bilateral = 0.0;

        for result in self.by_netting_set.values() {
            if result.is_cleared() {
                cleared += result.total_margin.amount();
            } else {
                bilateral += result.total_margin.amount();
            }
        }

        (
            Money::new(cleared, self.base_currency),
            Money::new(bilateral, self.base_currency),
        )
    }

    /// Iterate over netting set results.
    ///
    /// # Returns
    ///
    /// Iterator over netting-set identifiers and their results.
    pub fn iter(&self) -> impl Iterator<Item = (&NettingSetId, &NettingSetMargin)> {
        self.by_netting_set.iter()
    }

    /// Record a degraded position with the corresponding error message.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position whose margin calculation degraded.
    /// * `message` - Human-readable reason for the degradation.
    pub fn add_degraded_position(&mut self, position_id: PositionId, message: impl Into<String>) {
        if self
            .degraded_positions
            .iter()
            .any(|(existing, _)| existing == &position_id)
        {
            return;
        }
        self.degraded_positions.push((position_id, message.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::June, 15).expect("valid date")
    }

    #[test]
    fn test_netting_set_margin_creation() {
        let id = NettingSetId::bilateral("BANK_A", "CSA_001");
        let result = NettingSetMargin::new(
            id,
            test_date(),
            Money::new(5_000_000.0, Currency::USD),
            Money::new(1_000_000.0, Currency::USD),
            10,
            ImMethodology::Simm,
        );

        assert_eq!(result.initial_margin.amount(), 5_000_000.0);
        assert_eq!(result.variation_margin.amount(), 1_000_000.0);
        assert_eq!(result.total_margin.amount(), 6_000_000.0);
        assert!(!result.is_cleared());
    }

    #[test]
    fn test_portfolio_margin_aggregation() {
        let mut portfolio_result = PortfolioMarginResult::new(test_date(), Currency::USD);

        // Add bilateral netting set
        let bilateral = NettingSetMargin::new(
            NettingSetId::bilateral("BANK_A", "CSA_001"),
            test_date(),
            Money::new(5_000_000.0, Currency::USD),
            Money::new(1_000_000.0, Currency::USD),
            10,
            ImMethodology::Simm,
        );
        portfolio_result
            .add_netting_set(bilateral)
            .expect("same currency should succeed");

        // Add cleared netting set
        let cleared = NettingSetMargin::new(
            NettingSetId::cleared("LCH"),
            test_date(),
            Money::new(3_000_000.0, Currency::USD),
            Money::new(500_000.0, Currency::USD),
            5,
            ImMethodology::ClearingHouse,
        );
        portfolio_result
            .add_netting_set(cleared)
            .expect("same currency should succeed");

        assert_eq!(portfolio_result.netting_set_count(), 2);
        assert_eq!(portfolio_result.total_initial_margin.amount(), 8_000_000.0);
        assert_eq!(
            portfolio_result.total_variation_margin.amount(),
            1_500_000.0
        );
        assert_eq!(portfolio_result.total_positions, 15);

        let (cleared_total, bilateral_total) = portfolio_result.cleared_bilateral_split();
        assert_eq!(cleared_total.amount(), 3_500_000.0);
        assert_eq!(bilateral_total.amount(), 6_000_000.0);
    }

    #[test]
    fn test_currency_mismatch_error() {
        let mut portfolio_result = PortfolioMarginResult::new(test_date(), Currency::USD);

        // Try to add EUR netting set to USD portfolio
        let eur_netting_set = NettingSetMargin::new(
            NettingSetId::bilateral("BANK_B", "CSA_002"),
            test_date(),
            Money::new(1_000_000.0, Currency::EUR),
            Money::new(200_000.0, Currency::EUR),
            5,
            ImMethodology::Simm,
        );

        let result = portfolio_result.add_netting_set(eur_netting_set);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.netting_set_currency, Currency::EUR);
        assert_eq!(err.base_currency, Currency::USD);
    }

    #[test]
    fn test_add_netting_set_with_fx() {
        let mut portfolio_result = PortfolioMarginResult::new(test_date(), Currency::USD);

        // Add EUR netting set with FX conversion (EUR/USD = 1.10)
        let eur_netting_set = NettingSetMargin::new(
            NettingSetId::bilateral("BANK_B", "CSA_002"),
            test_date(),
            Money::new(1_000_000.0, Currency::EUR),
            Money::new(200_000.0, Currency::EUR),
            5,
            ImMethodology::Simm,
        );

        let eur_usd_rate = 1.10;
        portfolio_result
            .add_netting_set_with_fx(eur_netting_set, eur_usd_rate)
            .expect("valid FX rate should convert");

        // Verify conversion: 1M EUR * 1.10 = 1.1M USD
        assert!((portfolio_result.total_initial_margin.amount() - 1_100_000.0).abs() < 1e-9);
        assert!((portfolio_result.total_variation_margin.amount() - 220_000.0).abs() < 1e-9);
    }
}
