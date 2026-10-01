//! Repurchase agreements priced from the cash-lender perspective.
//!
//! [`Repo`] supports term, open-with-an-initial-maturity, and overnight
//! contracts. Its full contractual schedule contains three classified entries
//! on two business-day-adjusted payment dates: a `Notional` outflow at the
//! start, then a `Notional` principal repayment and `Fixed` simple repo interest
//! at maturity. Valuation-date filtering may omit settled entries.
//! [`CollateralType::Special`] may adjust the repo rate; the haircut determines
//! required collateral but does not change those cashflows or base PV.
//!
//! # Margining boundary
//!
//! [`Repo`] implements [`finstack_quant_margin::Marginable`] and can carry an
//! optional [`finstack_quant_margin::RepoMarginSpec`]. Variation-margin MTM
//! delegates to repo PV, while SIMM returns a simple 3M/6M interest-rate-delta
//! approximation.
//!
//! Margin configuration does not automatically add margin calls, cash-margin
//! interest, substitutions, collateral prices, or haircuts to base PV. Those
//! cashflows are caller-generated through `finstack-quant-margin`. The repo
//! implementation uses a per-instrument netting-set identifier, so the
//! `NetExposure` and `Triparty` modes do not by themselves implement
//! cross-trade netting or tri-party-agent operations.
//!
//! These parameters cover selected GMRA-style maintenance terms; they are not
//! a complete GMRA legal or lifecycle implementation. Open-repo notice,
//! termination and roll events, securities-income/manufactured payments,
//! failed-delivery and default/close-out remedies, rehypothecation and
//! reinvestment economics, and currency/funding basis beyond supplied market
//! data are outside this model.

pub(crate) mod metrics;
mod types;

pub use types::{CollateralSpec, CollateralType, Repo, RepoBuilder, RepoType};

// Builder is generated via derive on `Repo`.
