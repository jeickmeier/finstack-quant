//! Bond-specific metric calculators split into per-metric modules.
//!
//! This module provides metric calculators for bond-specific risk and valuation metrics.
//!
//! # Available Metrics
//!
//! ## Price and Yield Metrics
//! - **Accrued Interest**: Interest accrued since last coupon payment
//! - **Clean Price**: Quoted price excluding accrued interest
//! - **Dirty Price**: Clean price plus accrued interest
//! - **Yield to Maturity (YTM)**: Internal rate of return
//! - **Yield to Worst (YTW)**: Minimum yield across call/put/maturity paths
//! - **Japanese simple yield**: Tokyo 単利 closed form (`japanese_simple_yield`)
//! - **Moosmüller YTM**: Simple first period, then periodic (`moosmuller_ytm`)
//!
//! ## Risk Metrics
//! - **Macaulay Duration**: Weighted average time to cashflows
//! - **Modified Duration**: Interest rate sensitivity measure
//! - **Convexity**: Curvature of price/yield relationship
//! - **DV01**: Dollar value of 1bp rate change
//! - **CS01**: Credit spread sensitivity
//! - **Recovery01**: PV change per +1% recovery on a hazard-priced bond
//! - **Jump to Default**: Recovery of par less current value, on a hazard-priced bond
//! - **Spread Duration**: Quote-reproducing Z-spread sensitivity normalized by
//!   the Z-spread base PV into years
//!
//! ## Spread Metrics
//! - **Z-Spread**: Zero-volatility spread over discount curve
//! - **OAS**: Option-adjusted spread (for call, put, or return-floor rights)
//! - **I-Spread**: Interpolated spread (YTM - par swap rate)
//! - **G-Spread**: Same yield over the par yield of a named government curve
//! - **Discount Margin**: Spread measure for floating-rate notes
//! - **Asset Swap Spreads**: Par and market asset swap spreads
//!
//! # See Also
//!
//! - `register_bond_metrics` for registering all bond metrics with the
//!   `FIXED_INCOME` family
//! - [`finstack_quant_valuations_core::metrics`] for the metrics framework

/// Accrued interest calculator
pub(crate) mod accrued;
pub(crate) mod carry;
/// Convexity calculator
pub(crate) mod convexity;
/// CS01 calculators with z-spread fallback for bonds without credit curves
pub(crate) mod cs01;
/// Recovery01 and jump-to-default on hazard-priced bonds
mod default_risk;
/// Macaulay duration calculator
pub(crate) mod duration_macaulay;
/// Modified duration calculator
pub(crate) mod duration_modified;
/// Option-aware bond DV01 calculator
pub(crate) mod dv01;
/// Effective duration and convexity for bonds with embedded options
pub(crate) mod effective;
/// Price, yield, and spread metrics
pub(crate) mod price_yield_spread;
mod register;
/// MOIC and XIRR return-floor verification metrics
pub(crate) mod return_metrics;
/// Quote-reproducing risk view for bond bump-based sensitivities
mod risk_view;
/// Quote-reproducing Z-spread duration
mod spread_duration;
/// Weighted Average Life calculator
pub(crate) mod wal;
/// Yield-basis DV01 calculator
pub(crate) mod yield_dv01;

pub(crate) use accrued::AccruedInterestCalculator;
pub(crate) use convexity::ConvexityCalculator;
pub(crate) use cs01::{BondBucketedCs01Calculator, BondCs01Calculator};
pub(crate) use duration_macaulay::MacaulayDurationCalculator;
pub(crate) use duration_modified::ModifiedDurationCalculator;
pub(crate) use dv01::{BondBucketedDv01Calculator, BondDv01Calculator};
pub(crate) use price_yield_spread::{
    AssetSwapMarketCalculator, AssetSwapParCalculator, BondVegaCalculator, CleanPriceCalculator,
    DirtyPriceCalculator, EmbeddedOptionValueCalculator, GSpreadCalculator, ISpreadCalculator,
    JapaneseSimpleYieldCalculator, OasCalculator, YtmCalculator, YtwCalculator,
};
pub(crate) use price_yield_spread::{DiscountMarginCalculator, ZSpreadCalculator};
pub(crate) use register::{
    YieldRiskBasis, active_model, bond_risk_basis, context_workout_path, oas_pinned,
    register_bond_metrics, reprice_selected, yield_risk_basis, z_pinned,
};
pub(crate) use spread_duration::BondSpreadDurationCalculator;
pub(crate) use wal::BondWalCalculator;
pub(crate) use yield_dv01::YieldDv01Calculator;
