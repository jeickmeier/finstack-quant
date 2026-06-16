//! Delta to underlying volatility index calculator for volatility index futures.
//!
//! Computes the exposure to the underlying volatility index level.
//!
//! # Market Standard Formula
//!
//! **DeltaVol:** Δ_vol = ±contracts × face_value
//!
//! Where:
//! - contracts = notional / face_value
//! - sign = +1 for long, -1 for short
//!
//! # Note
//!
//! This metric represents the dollar exposure to a one-point move in the
//! underlying volatility index. For VIX futures with a $1000 multiplier,
//! a 1-point move in VIX results in a $1000 P&L per contract.

use crate::define_metric_calculator;
use crate::instruments::equity::vol_index_future::VolatilityIndexFuture;

define_metric_calculator!(
    /// Delta to volatility index calculator for volatility index futures.
    DeltaVolCalculator,
    instrument = VolatilityIndexFuture,
    calc = |future, _ctx| Ok(future.delta_vol())
);
