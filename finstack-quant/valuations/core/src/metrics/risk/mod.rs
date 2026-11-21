//! Risk configuration and scenario wire types.
//!
//! Full Historical VaR calculation (`calculate_var`, `GenericHVar`,
//! `GenericExpectedShortfall`, scenario application, and tabular export) lives
//! in the umbrella crate `finstack_quant_valuations::metrics::risk`.

pub(crate) mod market_history;
pub(crate) mod risk_factors;
pub mod var_calculator;

pub use market_history::{MarketHistory, MarketScenario, RiskFactorShift, UnshiftedHazardPolicy};
pub use risk_factors::RiskFactorType;
pub use var_calculator::{VarConfig, VarMethod};
