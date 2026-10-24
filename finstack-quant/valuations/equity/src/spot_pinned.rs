//! Spot-pinned finite-difference Greeks for products struck at inception spot.
//!
//! An autocallable or cliquet without an observed `initial_level` takes its
//! strike reference from the current spot. A spot bump would then move the
//! strike with the spot and the bumped PV would barely change, so the spot
//! Greeks first fix the reference level at the unbumped spot.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::PriceId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::helpers::scalar_price_amount;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::marker::PhantomData;
use std::sync::Arc;

/// Instrument whose strike reference can be fixed at the current spot.
pub(crate) trait SpotPinnable: Instrument + Clone + 'static {
    /// Spot price ID and quote currency of an instrument whose reference level
    /// is still unobserved, or `None` when the reference level is already fixed.
    fn unpinned_spot(&self) -> Option<(&PriceId, Currency)>;

    /// Copy of the instrument with its reference level fixed at `spot`.
    ///
    /// # Arguments
    ///
    /// * `spot` - Unbumped spot level, in the underlying's price units.
    /// * `as_of` - Valuation date on which `spot` is observed.
    fn pinned_at(&self, spot: f64, as_of: Date) -> Self;
}

/// Runs a spot-bumping FD calculator against the spot-pinned instrument.
///
/// Wrap only calculators that bump spot (delta, gamma, vanna). Vol-only
/// bumps such as volga leave spot, and therefore the unpinned strike, unchanged.
pub(crate) struct SpotPinnedFd<I, C> {
    inner: C,
    _instrument: PhantomData<fn() -> I>,
}

impl<I, C> SpotPinnedFd<I, C> {
    /// Wrap `inner` so it sees the spot-pinned instrument.
    ///
    /// # Arguments
    ///
    /// * `inner` - Finite-difference calculator that bumps spot.
    pub(crate) const fn new(inner: C) -> Self {
        Self {
            inner,
            _instrument: PhantomData,
        }
    }
}

impl<I: SpotPinnable, C: MetricCalculator> MetricCalculator for SpotPinnedFd<I, C> {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &I = context.instrument_as()?;
        let Some((spot_id, currency)) = inst.unpinned_spot() else {
            return self.inner.calculate(context);
        };
        let spot = scalar_price_amount(context.curves.get_price(spot_id)?, currency)?;
        let pinned = inst.pinned_at(spot, context.as_of);
        context.with_instrument(Arc::new(pinned), |ctx| self.inner.calculate(ctx))
    }
}
