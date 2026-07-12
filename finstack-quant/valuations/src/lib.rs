#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

//! Financial instrument pricing, risk, and cashflow analysis.
//!
//! This crate provides a deterministic valuation engine for fixed income,
//! equity, credit, and derivative instruments. Built on accounting-grade numerics
//! (Decimal by default), currency safety, and stable wire formats.
//!
//! # Features
//!
//! - **Instrument pricing**: NPV, yields, spreads across 40+ instrument types
//! - **Risk metrics**: DV01, CS01, Greeks, bucketed sensitivities, time decay
//! - **Cashflow generation**: Schedule building with amortization, floating rates, and caps
//! - **Calibration**: Bootstrap and optimize curves (discount, forward, hazard, volatility)
//! - **Monte Carlo**: Path generation, variance reduction, LSM for early exercise
//! - **Analytical formulas**: Black-Scholes, SABR, barrier options, Asian options
//! - **Registry-based pricing**: Type-safe dispatch without macro complexity
//! - **Metrics framework**: Composable calculators with dependency resolution
//!
//! # Documentation Conventions
//!
//! Public rustdoc in `finstack-quant-valuations` follows a few crate-wide rules:
//!
//! - **Prefer typed rates and spreads in examples**: when an API accepts either raw
//!   decimals or typed wrappers, examples should usually favor
//!   [`finstack_quant_core::types::Rate`] and related typed constructors such as
//!   `Rate::from_percent(5.0)` or `Rate::from_decimal(0.05)`.
//! - **Treat metrics as explicit contracts**: values stored in
//!   [`crate::results::ValuationResult::measures`] are not all currency amounts. Their
//!   units, sign conventions, and bump conventions are defined by
//!   [`crate::metrics::MetricId`].
//! - **State market conventions near the API**: when behavior depends on day count,
//!   calendars, compounding, settlement, quote style, or curve-role assumptions, the
//!   rustdoc for that public API should say so directly.
//! - **Cite canonical sources when the model matters**: public APIs implementing a
//!   market convention, pricing model, or numerical method should include
//!   `# References` sections pointing to `docs/REFERENCES.md#anchor`.
//!
//! # Architecture
//!
//! ```text
//! Instruments ──> Pricer Registry ──> Pricing Models
//!      │               │                     │
//!      │               ├─ Discounting        ├─ Analytical (Black-Scholes, SABR)
//!      │               ├─ Tree-based         ├─ Monte Carlo (GBM, Heston)
//!      │               ├─ Monte Carlo        └─ Specialized (CDS, Convertibles)
//!      │               └─ Custom
//!      │
//!      └──> Metrics Registry ──> Risk Calculators
//!                  │                   │
//!                  │                   ├─ Greeks (delta, gamma, vega, theta, rho)
//!                  │                   ├─ DV01/CS01 (bucketed and total)
//!                  │                   ├─ Spreads (Z-spread, OAS, ASW)
//!                  │                   └─ Custom metrics
//!                  │
//!                  └──> ValuationResult (PV + Metrics + Metadata)
//! ```
//!
//! # Quick Start
//!
//! ## Basic Bond Pricing
//!
//! ```rust
//! use finstack_quant_valuations::instruments::Bond;
//! use finstack_quant_valuations::pricer::{standard_registry, ModelKey};
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::market_data::MarketContext;
//! use finstack_quant_core::types::Rate;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create pricing registry
//! let registry = standard_registry();
//!
//! // Build a fixed-rate bond
//! let issue = create_date(2025, Month::January, 15)?;
//! let maturity = create_date(2030, Month::January, 15)?;
//! let bond = Bond::fixed(
//!     "US-BOND-001",
//!     Money::new(1_000_000.0, Currency::USD),
//!     Rate::from_percent(5.0),
//!     issue,
//!     maturity,
//!     "USD-OIS"       // Discount curve ID
//! );
//!
//! // Create market context with curves
//! // Note: Market context requires calibrated discount curves.
//! // In practice, populate with discount curves via calibration module.
//! // let market = MarketContext::new();
//! // let as_of = create_date(2025, Month::January, 1)?;
//!
//! // Price the bond (requires populated market context)
//! // let result = registry.price_with_metrics(
//! //     &bond, ModelKey::Discounting, &market, as_of,
//! //     &[], PricingOptions::default(),
//! // )?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Risk Metrics
//!
//! ```rust
//! use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_valuations::pricer::ModelKey;
//! use finstack_quant_core::market_data::MarketContext;
//! use finstack_quant_core::types::Rate;
//! # use finstack_quant_core::currency::Currency;
//! # use finstack_quant_core::money::Money;
//! # use finstack_quant_core::dates::create_date;
//! # use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let issue = create_date(2025, Month::January, 15)?;
//! # let maturity = create_date(2030, Month::January, 15)?;
//! # let bond = Bond::fixed("US-BOND-001", Money::new(1_000_000.0, Currency::USD),
//! #     Rate::from_percent(5.0), issue, maturity, "USD-OIS");
//! # let market = MarketContext::new();
//! # let as_of = create_date(2025, Month::January, 1)?;
//!
//! let metrics_to_compute = vec![
//!     MetricId::Ytm,
//!     MetricId::DurationMod,  // Modified duration
//!     MetricId::Convexity,
//!     MetricId::Dv01,
//! ];
//!
//! let default_opts = PricingOptions::default();
//! let hazard_rate_opts = PricingOptions::default().with_model(ModelKey::HazardRate);
//!
//! // Note: Requires populated market context with the curves needed by the
//! // selected pricing path.
//! // let result = bond.price_with_metrics(&market, as_of, &metrics_to_compute, default_opts)?;
//! // let hazard_result = bond.price_with_metrics(&market, as_of, &metrics_to_compute, hazard_rate_opts)?;
//! // println!("YTM: {:.2}%", result.metric(MetricId::Ytm).unwrap_or(0.0) * 100.0);
//! // println!("DV01: ${:.2}", result.metric(MetricId::Dv01).unwrap_or(0.0));
//! # Ok(())
//! # }
//! ```
//!
//! ## Calibration — building a `MarketContext` from raw quotes
//!
//! The canonical path is JSON-in / `MarketContext`-out. Pass a
//! [`calibration::api::schema::CalibrationEnvelope`] through
//! [`calibration::api::engine::execute`] and rehydrate the result:
//!
//! ```rust
//! use finstack_quant_valuations::calibration::api::{engine, schema::CalibrationEnvelope};
//! use finstack_quant_core::market_data::context::MarketContext;
//!
//! let envelope_json = r#"{"schema":"finstack_quant.calibration","plan":{"id":"empty","description":null,"quote_sets":{},"steps":[],"settings":{}}}"#;
//! let envelope: CalibrationEnvelope =
//!     serde_json::from_str(envelope_json).expect("parse envelope");
//! let result = engine::execute(&envelope).expect("calibration succeeded");
//! let market = MarketContext::try_from(result.result.final_market)
//!     .expect("rehydrate market");
//! // `market` is now ready for valuations, attribution, scenarios, portfolio.
//! let _ = market;
//! ```
//!
//! See [`calibration`] crate-level docs for the full two-track structure
//! (`plan.steps` for bootstrapping vs snapshot-only entries in `market_data`),
//! and `finstack-quant/valuations/examples/market_bootstrap/` for canonical
//! envelope JSON examples.
//!
//! # Module Organization
//!
//! - [`crate::calibration`]: Curve and surface calibration from market quotes
//! - [`finstack_quant_cashflows`]: Cashflow schedule generation and aggregation
//! - [`crate::instruments`]: Financial instrument definitions (bonds, swaps, options, etc.)
//! - [`crate::metrics`]: Risk metric calculators and registry
//! - [`crate::pricer`]: Pricing dispatch and registry infrastructure
//! - [`crate::results`]: Valuation result envelopes and metadata
//! - [`crate::constants`]: Common numerical constants (basis points, etc.)
//! - [`finstack_quant_covenants`]: Covenant checking for structured products
//! - [`finstack_quant_margin`]: Margin, collateral, VM/IM, and regulatory capital helpers
//! - [`crate::schema`]: JSON Schema generation for API contracts
//!
//! # Semantic Contracts
//!
//! The main user-facing semantic contracts are:
//!
//! - [`crate::metrics::MetricId`]: the authoritative glossary for metric meanings,
//!   units, and bump/sign conventions.
//! - [`crate::results::ValuationResult`]: the canonical result envelope for PV,
//!   metrics, and metadata.
//! - [`crate::instruments::Instrument`]: the common pricing and dependency
//!   contract for all supported instruments.
//!
//! # API Layers
//!
//! The public API is organized into three layers:
//!
//! ## Layer 1: Core API (Most Common)
//! - [`crate::instruments`]: Financial instrument types (bonds, swaps, options, etc.)
//! - [`crate::pricer`]: Pricing registry and dispatch
//!   ([`crate::pricer::PricerRegistry`], [`crate::pricer::standard_registry`])
//! - [`crate::metrics`]: Risk metric calculation
//!   ([`crate::metrics::MetricId`], [`crate::metrics::standard_registry`])
//! - [`crate::results`]: Valuation result envelopes
//!   ([`crate::results::ValuationResult`])
//! - [`crate::calibration::api`]: Calibration schema and execution engine
//! - [`crate::prelude`]: Convenient re-exports of commonly used types
//!
//! ## Layer 2: Extended API (Less Common)
//! - [`finstack_quant_margin`]: Margin calculations (VM/IM/CSA) for collateralized derivatives
//! - `finstack-quant-attribution`: P&L attribution analysis built on valuation results
//! - [`finstack_quant_covenants`]: Covenant checking for structured products
//! - [`finstack_quant_cashflows`]: Advanced cashflow schedule builders
//! - [`crate::instruments`]: Shared traits and parameters, plus
//!   [`crate::instruments::pricing`] for reusable schedules and pricing helpers
//! - [`crate::models`]: Reusable pricing-model and numerical-method primitives
//! - [`crate::market`]: Market quote schemas and conventions
//! - [`crate::calibration::bumps`]: Shared re-calibration helpers for scenarios
//!
//! ## Layer 3: Internal API (Use with Caution)
//! - Individual pricer implementations (use via [`crate::pricer::PricerRegistry`] instead)
//! - Calibration solvers (use via [`crate::calibration::api`] instead)
//! - Low-level market data helpers
//!
//! For most users, Layer 1 + `prelude` imports are sufficient.
//! Import with `use finstack_quant_valuations::prelude::*;` to get started quickly.
//!
//! # Supported Instruments
//!
//! ## Fixed Income
//! - `Bond`: Fixed and floating-rate bonds, callable/putable, amortizing
//! - `InterestRateSwap`: Plain vanilla and basis swaps
//! - `Swaption`: European and Bermudan swaptions
//! - `CapFloor`: Interest rate caps and floors
//! - `Deposit`: Money market deposits
//! - `ForwardRateAgreement`: FRAs
//! - `InterestRateFuture`: Futures contracts
//!
//! ## Credit
//! - `CreditDefaultSwap`: Single-name CDS
//! - `CDSIndex`: Credit indices (CDX, iTraxx)
//! - `CDSTranche`: Synthetic CDO tranches
//! - `CDSOption`: Options on CDS
//! - `StructuredCredit`: ABS, RMBS, CMBS, CLO
//!
//! ## Equity & FX
//! - `Equity`: Equity spot positions
//! - `EquityOption`: Vanilla equity options
//! - `FxSpot`: FX spot positions
//! - `FxOption`: Vanilla FX options (Garman-Kohlhagen)
//! - `FxSwap`: FX forwards and swaps
//! - `Basket`: Multi-asset baskets
//!
//! ## Exotic Options
//! - `AsianOption`: Asian (average price/strike) options
//! - `BarrierOption`: Barrier options (knock-in/out)
//! - `LookbackOption`: Lookback options
//! - `Autocallable`: Autocallable notes
//! - `CliquetOption`: Cliquet/ratchet options
//! - `QuantoOption`: Quanto options
//!
//! ## Structured Products
//! - `ConvertibleBond`: Convertible bonds
//! - `Repo`: Repurchase agreements
//! - `VarianceSwap`: Variance and volatility swaps
//! - `PrivateMarketsFund`: Private equity/credit funds
//! - `RevolvingCredit`: Revolving credit facilities
//!
//! # Pricing Models
//!
//! ## Analytical
//! - **Black-Scholes-Merton**: European options on equity and FX
//! - **Black (1976)**: Caps, floors, swaptions
//! - **Garman-Kohlhagen**: FX options
//! - **SABR**: Stochastic volatility surface interpolation
//! - **Barrier formulas**: Rubinstein-Reiner barrier options
//! - **Asian approx**: Turnbull-Wakeman and geometric averaging
//!
//! ## Tree Methods
//! - **Binomial trees**: Cox-Ross-Rubinstein, Jarrow-Rudd
//! - **Trinomial trees**: Short rate models, convertibles
//! - **Hull-White**: Interest rate trees for callable bonds
//!
//! ## Monte Carlo
//! - **Geometric Brownian Motion**: Standard equity/FX simulation
//! - **Heston**: Stochastic volatility with Andersen QE discretization
//! - **Longstaff-Schwartz**: American and Bermudan options via LSM
//! - **Variance reduction**: Antithetic variates, control variates
//!
//! # Determinism and Reproducibility
//!
//! All pricing and calibration is deterministic by default:
//! - Decimal arithmetic via [`rust_decimal`] ensures consistent results
//! - Monte Carlo uses seedable RNGs with stable algorithms
//! - Parallel execution produces identical results to serial
//! - Calibration solvers use deterministic iteration orders
//!
//! # Performance
//!
//! - **Allocation-aware data paths**: compact Rust structs, deterministic maps,
//!   and JSON/table exports for downstream DataFrame consumers
//! - **Caching**: Intermediate results (curves, cashflows) cached per valuation
//! - **Parallelism**: Rayon-backed paths preserve deterministic results
//! - **Lazy evaluation**: Metrics computed only when requested
//!
//! # Error Handling
//!
//! All public APIs return `Result<T, finstack_quant_core::Error>` with structured error types:
//! - `CurveNotFound`: Missing discount or forward curve
//! - `InvalidInstrument`: Inconsistent instrument parameters
//! - `CalibrationFailed`: Calibration did not converge
//! - `error::PricingError`: Pricing calculation failed
//!
//! # Feature Flags
//!
//! - `ts_export`: Enable TypeScript type export support for schema tooling.
//!
//! # References
//!
//! - Curve construction and discounting: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Fixed-income risk conventions: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Black-style option pricing: `docs/REFERENCES.md#black-1976`
//! - Normal-model option pricing: `docs/REFERENCES.md#bachelier-1900`
//! - SABR volatility: `docs/REFERENCES.md#hagan-2002-sabr`
//!
//! # See Also
//!
//! - `finstack_quant_core`: Core primitives (Money, dates, curves, expressions)
//! - `finstack_quant_statements`: Financial statement modeling
//! - `finstack_quant_portfolio`: Multi-instrument portfolio aggregation
//! - `finstack_quant_scenarios`: Scenario analysis and stress testing

extern crate self as finstack_quant_valuations;

/// Curve and surface calibration tooling.
pub mod calibration;
/// Cashflow schedule generation and builders.
pub(crate) use finstack_quant_cashflows as cashflow;
/// Shared numerical constants and basis point helpers.
pub mod constants;
pub(crate) mod contract_specs;
/// Copula, factor, and recovery models for credit correlation.
///
/// Provides reusable correlation infrastructure used across credit instruments
/// (CDS tranche pricing, structured credit engines, portfolio credit risk).
pub mod correlation;
/// Error types for pricing and valuation workflows.
pub mod error;
/// Market quotes and conventions
pub mod market;
/// Pricing models and numerical methods.
pub mod models;
/// Convenient re-exports for pricing and risk calculations.
pub mod prelude;
/// Pricing dispatch and registry infrastructure.
pub mod pricer;
/// Valuation result envelopes and metadata.
pub mod results;
/// JSON Schema generation for API contracts.
pub mod schema;
pub(crate) mod serde_defaults;

/// Hidden support paths for exported macros.
#[doc(hidden)]
pub mod __private {
    /// Cashflow crate path used by exported macros without exposing `cashflow`.
    pub use finstack_quant_cashflows;
}

#[macro_use]
/// Financial instrument definitions and builders.
pub mod instruments;
/// Risk metric calculators and registries.
pub mod metrics;

// Re-export unified valuations error type.
pub use error::{Error, Result};

/// Construct tagged bond instrument JSON from a cashflow schedule JSON payload.
///
/// This is the Rust-canonical bridge used by Python and WASM bindings for
/// `cashflows.bond_from_cashflows`. It parses a [`finstack_quant_cashflows`] schedule,
/// constructs a [`crate::instruments::fixed_income::bond::Bond`], wraps it in
/// [`crate::instruments::InstrumentJson::Bond`], and serializes the tagged
/// instrument envelope.
///
/// # Errors
///
/// Returns an error if the schedule JSON is invalid, bond construction fails, or
/// the tagged instrument cannot be serialized.
pub fn bond_from_cashflows_json(
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> finstack_quant_core::Result<String> {
    let mut schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(schedule_json).map_err(|err| {
            finstack_quant_core::Error::Validation(format!("invalid cashflow schedule JSON: {err}"))
        })?;
    finstack_quant_cashflows::validate_cashflow_schedule(&mut schedule)?;
    let bond = instruments::fixed_income::bond::Bond::from_cashflows(
        instrument_id,
        schedule,
        discount_curve_id,
        quoted_clean,
    )?;
    let instrument = instruments::InstrumentJson::Bond(bond);
    serde_json::to_string(&instrument).map_err(|err| {
        finstack_quant_core::Error::Validation(format!(
            "failed to serialize bond instrument JSON: {err}"
        ))
    })
}
