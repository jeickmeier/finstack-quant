//! Real estate operating statement templates.
//!
//! These helpers provide a consistent NOI / NCF buildup pattern for
//! property-level models.
//!
//! All template inputs are expressed per model period unless a field says
//! otherwise. For example, if the model is quarterly then rent, other income,
//! OpEx, and CapEx inputs are all interpreted as quarterly amounts.
//!
//! The richer rent-roll APIs generate both aggregated property nodes and
//! per-lease detail nodes so that underwriting outputs remain explainable.
//!
//! # Limitations
//!
//! The templates cover gross-lease NOI/NCF buildups. The following CRE
//! features are **not** generated and must be added as explicit model nodes
//! by the caller when needed:
//!
//! - **NNN / expense recoveries**: no reimbursement-income nodes are
//!   generated from OpEx (triple-net or expense-stop recoveries).
//! - **Percentage rent**: no sales-breakpoint overage rent.
//! - **TI / LC**: tenant improvements and leasing commissions on rollover
//!   are not modeled; include them in the CapEx nodes if material.
//! - **Valuation/credit metric helpers**: no cap-rate value, DSCR,
//!   debt-yield, or LTV node generators — the CRE covenant template
//!   (`finstack_quant_covenants::templates::CovenantTemplate::RealEstate`) expects metric nodes
//!   (`dscr`, `debt_yield`, `ltv`) that the caller must define.

mod buildup;
mod property;
mod rent_roll;

pub use buildup::{add_ncf_buildup, add_noi_buildup};
pub use property::{
    ManagementFeeBase, ManagementFeeSpec, PropertyTemplateNodes, add_property_operating_statement,
};
pub use rent_roll::{
    FreeRentWindowSpec, LeaseGrowthConvention, LeaseSpec, RenewalSpec, RentRollOutputNodes,
    RentStepSpec, add_rent_roll,
};
