//! Deterministic cashflow simulation entry point for [`StructuredCredit`].

use crate::structured_credit::types::StructuredCredit;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

#[cfg(test)]
thread_local! {
    /// Deterministic simulations started on this thread; tests read it to
    /// count waterfall runs per pricing request.
    pub(crate) static SIMULATION_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl StructuredCredit {
    /// Run deterministic cashflow simulation, returning tranche flows and deal accounting.
    ///
    /// The pricing lifecycle validates this deal once and resolves the effective
    /// valuation date before starting the simulation.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context supplying discount and forward curves and
    ///   lifecycle date policy for the collateral and liabilities.
    /// * `as_of` - Requested valuation date; market lifecycle policy can resolve
    ///   an effective date before the simulation begins.
    ///
    /// # Errors
    ///
    /// Propagates validation, missing-curve, schedule and cash-conservation errors.
    #[tracing::instrument(
        name = "structured_credit.run_simulation",
        level = "debug",
        skip(self, market),
        fields(
            inst_id = %self.id.as_str(),
            deal_type = ?self.deal_type,
            as_of = %as_of,
            tranche_count = self.tranches.tranches.len(),
        ),
        err,
    )]
    pub fn run_simulation(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<super::SimulationRun> {
        #[cfg(test)]
        SIMULATION_RUNS.with(|runs| runs.set(runs.get() + 1));
        let lifecycle =
            finstack_quant_valuations_core::instruments::lifecycle::ValidatedPricingLifecycle::new(
                self,
            )?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        super::simulation_engine::simulate_with_source(self, market, effective_as_of, None)
    }
}
