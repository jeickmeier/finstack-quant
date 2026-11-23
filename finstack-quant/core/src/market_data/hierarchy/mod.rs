//! Market data hierarchy for organizational grouping of market targets.
//!
//! Provides a tree structure where each node holds exact insertion-ordered child
//! entries, key-value metadata tags, and typed
//! [`MarketDataTarget`](crate::market_data::bumps::MarketDataTarget) associations
//! pointing into `MarketContext`'s flat storage.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::market_data::bumps::MarketDataTarget;
//! use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
//! use finstack_quant_core::types::CurveId;
//!
//! let hierarchy = MarketDataHierarchy::builder()
//!     .add_node("Rates/USD/OIS")
//!         .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
//!     .add_node("Credit/US/IG/Financials")
//!         .tag("sector", "Financials")
//!         .targets([
//!             MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")),
//!             MarketDataTarget::HazardCurve(CurveId::from("GS-5Y")),
//!         ])
//!     .build()
//!     .expect("valid hierarchy");
//! ```

mod builder;
mod resolution;
mod tree;

pub use builder::HierarchyBuilder;
pub use resolution::{
    HierarchyTarget, ResolutionMode, ResolvedTargetMatch, TagFilter, TagPredicate,
};
pub use tree::{HierarchyNode, HierarchyNodeEntry, MarketDataHierarchy};
pub(crate) use tree::{parse_contract_json, parse_path, validate_segment, validate_tag_key};
