//! Expected Credit Loss (ECL) framework for IFRS 9 and CECL.
//!
//! This module implements the complete ECL computation pipeline:
//!
//! - **Stage classification** ([`crate::analysis::ecl::staging`]) -- IFRS 9 three-stage impairment model
//!   with quantitative (PD delta, DPD backstops) and qualitative triggers
//! - **ECL calculation** ([`crate::analysis::ecl::engine`]) -- time-bucketed PD x LGD x EAD x DF
//!   integration with probability-weighted macro scenarios
//! - **CECL variant** ([`crate::analysis::ecl::cecl`]) -- US GAAP ASC 326 lifetime ECL with
//!   reasonable-and-supportable forecast and historical reversion
//! - **Portfolio aggregation** ([`crate::analysis::ecl::portfolio`]) -- stage breakdown, segment
//!   analysis, migration matrices, and provision waterfall
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use finstack_statements_analytics::analysis::ecl::{
//!     EclConfigBuilder, EclEngine, Exposure, QualitativeFlags, RawPdCurve, Stage,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let pd_curve = RawPdCurve::new("BBB", vec![
//!     (0.0, 0.0),
//!     (1.0, 0.02),
//!     (5.0, 0.10),
//! ])?;
//!
//! let exposure = Exposure {
//!     id: "loan-1".to_string(),
//!     segments: vec!["corporate".to_string()],
//!     ead: 1_000_000.0,
//!     eir: 0.06,
//!     remaining_maturity_years: 3.0,
//!     lgd: 0.45,
//!     days_past_due: 0,
//!     current_rating: Some("BBB".to_string()),
//!     origination_rating: Some("BBB".to_string()),
//!     qualitative_flags: QualitativeFlags::default(),
//!     consecutive_performing_periods: 0,
//!     previous_stage: None,
//! };
//!
//! let config = EclConfigBuilder::new().bucket_width(0.25).build()?;
//! let scenario = config.scenarios[0].clone();
//! let engine = EclEngine::new(config, vec![(&scenario, &pd_curve)]);
//! let result = engine.process_exposure(&exposure)?;
//!
//! assert_eq!(result.stage_result.stage, Stage::Stage1);
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - IFRS 9 Financial Instruments, Section 5.5 -- Impairment
//! - ASC 326-20 -- Financial Instruments: Credit Losses
//! - Basel Committee on Banking Supervision (2015), "Guidance on credit risk
//!   and accounting for expected credit losses"

pub(crate) mod cecl;
pub(crate) mod engine;
pub(crate) mod policy;
pub(crate) mod portfolio;
pub(crate) mod staging;
pub(crate) mod types;

// Re-export core types for convenience.
pub use types::{Exposure, PdTermStructure, QualitativeFlags, RawPdCurve, Stage};

// Re-export staging API.
pub use staging::{classify_stage, StageResult, StagingConfig, StagingTrigger};

// Re-export ECL engine API.
pub use engine::{
    compute_ecl_single, compute_ecl_weighted, compute_ecl_weighted_from_schedules, EclBucket,
    EclConfig, EclConfigBuilder, EclEngine, EclResult, ExposureEclResult, LgdType, MacroScenario,
    WeightedEclResult,
};

// Re-export CECL API.
pub use cecl::{CeclConfig, CeclEngine, CeclMethodology, CeclResult, ReversionMethod};

// Re-export policy-backed defaults for bindings and downstream default construction.
pub use policy::{
    binding_default_classify_stage_dpd_30_trigger, binding_default_classify_stage_dpd_90_trigger,
    binding_default_classify_stage_pd_delta_absolute,
    binding_default_compute_ecl_bucket_width_years, binding_default_cure_periods_stage2_to_1,
    binding_default_cure_periods_stage3_to_2, binding_default_exposure_dpd, default_cecl_config,
    default_cecl_config_from_config, default_ecl_config, default_ecl_config_from_config,
    default_staging_config, default_staging_config_from_config, ECL_POLICY_EXTENSION_KEY,
};

// Re-export portfolio aggregation.
pub use portfolio::{compute_waterfall, PortfolioEclResult, ProvisionWaterfall};
