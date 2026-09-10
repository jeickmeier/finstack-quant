//! Margin calculation result types.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_margin::{ImCollateralResult, ImMethodology, NettingSetId, SimmSensitivities};

use crate::types::PositionId;

/// Margin results for a single netting set.
#[derive(Debug, Clone)]
pub struct NettingSetMargin {
    /// Netting set identifier
    pub netting_set_id: NettingSetId,
    /// Contractual CSA ID used for one-time IM terms, absent without an OTC CSA.
    pub csa_id: Option<String>,
    /// Calculation date
    pub as_of: Date,
    /// Gross model initial margin before CSA collateral terms.
    pub initial_margin: Money,
    /// Signed desk VM outflow: positive posts, negative collections.
    pub variation_margin: Money,
    /// Total margin (IM + positive VM)
    pub total_margin: Money,
    /// Number of positions in the netting set
    pub position_count: usize,
    /// IM methodology used
    pub im_methodology: ImMethodology,
    /// Whether IM uses an approximate model, including historical SIMM or the CCP proxy.
    pub is_approximate: bool,
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
    pub fn new(
        netting_set_id: NettingSetId,
        as_of: Date,
        initial_margin: Money,
        variation_margin: Money,
        position_count: usize,
        im_methodology: ImMethodology,
    ) -> finstack_quant_core::Result<Self> {
        let currency = initial_margin.currency();
        let total = Money::new(
            initial_margin.amount() + variation_margin.amount().max(0.0),
            currency,
        )?;
        Ok(Self {
            netting_set_id,
            csa_id: None,
            as_of,
            initial_margin,
            variation_margin,
            total_margin: total,
            position_count,
            im_methodology,
            is_approximate: false,
            sensitivities: None,
            im_breakdown: HashMap::default(),
        })
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
    /// Gross model initial margin across all netting sets before CSA terms
    pub total_initial_margin: Money,
    /// Signed desk VM outflow across all netting sets; collections are negative.
    pub total_variation_margin: Money,
    /// Gross IM plus positive VM by netting set; this is not a collateral transfer
    pub total_margin: Money,
    /// One-way IM accounts keyed by contractual CSA ID, in each CSA's currency.
    pub by_csa: HashMap<String, ImCollateralResult>,
    /// Sum of target IM account balances, converted to the reporting currency.
    pub total_required_im_collateral: Money,
    /// Signed IM transfers across CSAs in reporting currency; positive posts.
    pub total_im_transfer: Money,
    /// Required IM held in segregated custody, in reporting currency.
    pub total_segregated_im: Money,
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
    /// Create a new portfolio margin result.
    ///
    /// # Returns
    ///
    /// Empty portfolio margin report initialized in the supplied base currency.
    #[must_use]
    pub(crate) fn new(as_of: Date, base_currency: Currency) -> Self {
        Self {
            as_of,
            base_currency,
            total_initial_margin: Money::from((0_i64, base_currency)),
            total_variation_margin: Money::from((0_i64, base_currency)),
            total_margin: Money::from((0_i64, base_currency)),
            by_netting_set: HashMap::default(),
            by_csa: HashMap::default(),
            total_required_im_collateral: Money::from((0_i64, base_currency)),
            total_im_transfer: Money::from((0_i64, base_currency)),
            total_segregated_im: Money::from((0_i64, base_currency)),
            total_positions: 0,
            positions_without_margin: 0,
            degraded_positions: Vec::new(),
        }
    }

    /// Add a netting set margin result already expressed in the base currency.
    ///
    /// # Arguments
    ///
    /// * `result` - Completed netting-set result in `self.base_currency`; the
    ///   aggregator converts every netting set before calling this.
    pub(crate) fn add_netting_set(
        &mut self,
        result: NettingSetMargin,
    ) -> finstack_quant_core::Result<()> {
        debug_assert_eq!(result.initial_margin.currency(), self.base_currency);

        let im = result.initial_margin.amount();
        let vm = result.variation_margin.amount();

        self.total_initial_margin =
            Money::new(self.total_initial_margin.amount() + im, self.base_currency)?;
        self.total_variation_margin = Money::new(
            self.total_variation_margin.amount() + vm,
            self.base_currency,
        )?;
        self.total_margin = Money::new(
            self.total_margin.amount() + result.total_margin.amount(),
            self.base_currency,
        )?;
        self.total_positions += result.position_count;
        self.by_netting_set
            .insert(result.netting_set_id.clone(), result);
        Ok(())
    }

    /// Record a degraded position with the corresponding error message.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position whose margin calculation degraded.
    /// * `message` - Human-readable reason for the degradation.
    pub(crate) fn add_degraded_position(
        &mut self,
        position_id: PositionId,
        message: impl Into<String>,
    ) {
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
            Money::from((5_000_000_i64, Currency::USD)),
            Money::from((1_000_000_i64, Currency::USD)),
            10,
            ImMethodology::Simm,
        )
        .expect("valid new fixture");

        assert_eq!(result.initial_margin.amount(), 5_000_000.0);
        assert_eq!(result.variation_margin.amount(), 1_000_000.0);
        assert_eq!(result.total_margin.amount(), 6_000_000.0);
        assert!(!result.is_cleared());
    }

    #[test]
    fn test_portfolio_margin_aggregation() {
        let mut portfolio_result = PortfolioMarginResult::new(test_date(), Currency::USD);

        let bilateral = NettingSetMargin::new(
            NettingSetId::bilateral("BANK_A", "CSA_001"),
            test_date(),
            Money::from((5_000_000_i64, Currency::USD)),
            Money::from((1_000_000_i64, Currency::USD)),
            10,
            ImMethodology::Simm,
        )
        .expect("valid new fixture");
        portfolio_result
            .add_netting_set(bilateral)
            .expect("valid add_netting_set fixture");

        let cleared = NettingSetMargin::new(
            NettingSetId::cleared("LCH"),
            test_date(),
            Money::from((3_000_000_i64, Currency::USD)),
            Money::from((500_000_i64, Currency::USD)),
            5,
            ImMethodology::ClearingHouse,
        )
        .expect("valid new fixture");
        portfolio_result
            .add_netting_set(cleared)
            .expect("valid add_netting_set fixture");

        assert_eq!(portfolio_result.by_netting_set.len(), 2);
        assert_eq!(portfolio_result.total_initial_margin.amount(), 8_000_000.0);
        assert_eq!(
            portfolio_result.total_variation_margin.amount(),
            1_500_000.0
        );
        assert_eq!(portfolio_result.total_positions, 15);
        assert_eq!(portfolio_result.total_margin.amount(), 9_500_000.0);
    }
}
