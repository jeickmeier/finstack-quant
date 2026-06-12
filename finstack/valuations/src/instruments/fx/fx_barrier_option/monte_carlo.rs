//! FX barrier option payoffs with quanto adjustments.
//!
//! Extends the barrier framework for FX options, including quanto barriers
//! where the barrier and/or payoff are in different currencies.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_monte_carlo::payoff::barrier::{BarrierOptionPayoff, BarrierType, OptionKind};
use finstack_monte_carlo::traits::PathState;
use finstack_monte_carlo::traits::Payoff;

/// FX barrier option payoff (call or put).
///
/// Wraps the generic [`BarrierOptionPayoff`] with FX currency metadata for use
/// by the FX barrier Monte Carlo pricer.
///
/// # FX Barrier Types
///
/// - **Up-and-out**: Option knocked out if FX rate rises above barrier
/// - **Up-and-in**: Option activated if FX rate rises above barrier
/// - **Down-and-out**: Option knocked out if FX rate falls below barrier
/// - **Down-and-in**: Option activated if FX rate falls below barrier
///
/// # Quanto Barriers
///
/// Quanto FX barriers — where barrier monitoring and payoff settlement are in
/// different currencies — are intentionally **not** supported here. Correctly
/// pricing a quanto requires a 2D correlated equity/FX process; a scalar drift
/// adjustment on this 1D GBM payoff cannot represent it (the same reasoning
/// `QuantoOption` documents for its unsupported MC path). A constructor that
/// silently priced a "quanto" barrier identically to a non-quanto one would be
/// misleading, so no such constructor is exposed. The FX barrier pricer builds
/// the GBM drift directly as `r_dom - r_for`.
#[derive(Debug, Clone)]
pub struct FxBarrierPayoff {
    /// Underlying barrier call (reuses existing infrastructure)
    inner: BarrierOptionPayoff,
    /// Base currency (underlying currency, formerly foreign_currency)
    pub base_currency: Currency,
    /// Quote currency (settlement currency, formerly domestic_currency)
    pub quote_currency: Currency,
}

impl FxBarrierPayoff {
    /// Create a new FX barrier option (call or put).
    ///
    /// # Arguments
    ///
    /// * `strike` - Strike price (in foreign currency units)
    /// * `barrier` - Barrier level (in foreign currency units)
    /// * `barrier_type` - Type of barrier (up/down, in/out)
    /// * `option_kind` - Whether this is a call or put
    /// * `notional` - Notional amount
    /// * `maturity_step` - Step index at maturity
    /// * `sigma` - FX volatility
    /// * `dt` - Time step size
    /// * `use_gobet_miri` - Use Gobet-Miri barrier adjustment
    /// * `base_currency` - Underlying currency
    /// * `quote_currency` - Settlement currency
    /// * `rebate` - Optional rebate paid at maturity if barrier condition met
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        strike: f64,
        barrier: f64,
        barrier_type: BarrierType,
        option_kind: OptionKind,
        notional: f64,
        maturity_step: usize,
        sigma: f64,
        dt: f64,
        use_gobet_miri: bool,
        base_currency: Currency,
        quote_currency: Currency,
        rebate: Option<f64>,
    ) -> Result<Self> {
        let time_grid = finstack_monte_carlo::time_grid::TimeGrid::uniform(
            dt * maturity_step as f64,
            maturity_step,
        )?;
        let inner = BarrierOptionPayoff::new(
            strike,
            barrier,
            barrier_type,
            option_kind,
            rebate,
            notional,
            maturity_step,
            sigma,
            &time_grid,
            use_gobet_miri,
        );

        Ok(Self {
            inner,
            base_currency,
            quote_currency,
        })
    }

    /// Pay knock-out rebates at the hit time, compounded forward at the
    /// (domestic) continuously compounded `rate` — see
    /// [`BarrierOptionPayoff::with_rebate_at_hit`].
    #[must_use]
    pub fn with_rebate_at_hit(mut self, rate: f64) -> Self {
        self.inner = self.inner.with_rebate_at_hit(rate);
        self
    }

    /// Create a standard FX barrier with continuous monitoring.
    ///
    /// `use_gobet_miri` defaults to `false` to match the
    /// `FxBarrierOption::use_gobet_miri` instrument default. Callers that need
    /// discrete-monitoring correction should call `Self::new` directly.
    #[allow(clippy::too_many_arguments)]
    pub fn standard(
        strike: f64,
        barrier: f64,
        barrier_type: BarrierType,
        option_kind: OptionKind,
        notional: f64,
        maturity_step: usize,
        sigma: f64,
        dt: f64,
        base_currency: Currency,
        quote_currency: Currency,
    ) -> Result<Self> {
        Self::new(
            strike,
            barrier,
            barrier_type,
            option_kind,
            notional,
            maturity_step,
            sigma,
            dt,
            false, // continuous monitoring; matches FxBarrierOption default
            base_currency,
            quote_currency,
            None,
        )
    }
}

impl Payoff for FxBarrierPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        // Delegate to inner barrier call. The FX rate is carried in PathState
        // as the simulated spot.
        self.inner.on_event(state);
    }

    fn value(&self, currency: Currency) -> Money {
        self.inner.value(currency)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_barrier_standard_creation() {
        let fx_barrier = FxBarrierPayoff::standard(
            1.15,
            1.20,
            BarrierType::UpAndOut,
            OptionKind::Call,
            1_000_000.0,
            100,
            0.12,
            0.01,
            Currency::EUR,
            Currency::USD,
        )
        .expect("valid standard FX barrier should construct");

        assert_eq!(fx_barrier.base_currency, Currency::EUR);
        assert_eq!(fx_barrier.quote_currency, Currency::USD);
    }

    #[test]
    fn test_fx_barrier_new_constructs_with_currency_metadata() {
        let fx_barrier = FxBarrierPayoff::new(
            1.15,
            1.20,
            BarrierType::UpAndOut,
            OptionKind::Call,
            1_000_000.0,
            100,
            0.12,
            0.01,
            true,
            Currency::EUR,
            Currency::USD,
            Some(0.02),
        )
        .expect("valid FX barrier should construct");

        assert_eq!(fx_barrier.base_currency, Currency::EUR);
        assert_eq!(fx_barrier.quote_currency, Currency::USD);
    }
}
