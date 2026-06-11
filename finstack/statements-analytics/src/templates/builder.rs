//! Extension trait for ModelBuilder to support templates.

use super::real_estate;
use super::roll_forward;
use super::vintage;
use finstack_statements::builder::{ModelBuilder, Ready};
use finstack_statements::error::Result;

/// Extension methods for `ModelBuilder` to support high-level modeling templates.
pub trait TemplatesExtension<State> {
    /// Add a roll-forward structure (Beginning + Increases - Decreases = Ending)
    /// with a zero opening balance in the first period.
    fn add_roll_forward(
        self,
        name: &str,
        increases: &[&str],
        decreases: &[&str],
    ) -> Result<ModelBuilder<State>>;

    /// Add a roll-forward structure with an explicit opening balance used in
    /// the first period (where no prior ending balance exists).
    fn add_roll_forward_with_opening(
        self,
        name: &str,
        increases: &[&str],
        decreases: &[&str],
        opening: f64,
    ) -> Result<ModelBuilder<State>>;
}

impl<State> TemplatesExtension<State> for ModelBuilder<State> {
    fn add_roll_forward(
        self,
        name: &str,
        increases: &[&str],
        decreases: &[&str],
    ) -> Result<ModelBuilder<State>> {
        roll_forward::add_roll_forward(self, name, increases, decreases)
    }

    fn add_roll_forward_with_opening(
        self,
        name: &str,
        increases: &[&str],
        decreases: &[&str],
        opening: f64,
    ) -> Result<ModelBuilder<State>> {
        roll_forward::add_roll_forward_with_opening(self, name, increases, decreases, opening)
    }
}

/// Extension methods for `ModelBuilder<Ready>` (requires periods).
pub trait VintageExtension {
    /// Add a vintage buildup (cohort analysis) structure.
    fn add_vintage_buildup(
        self,
        name: &str,
        new_volume_node: &str,
        decay_curve: &[f64],
    ) -> Result<ModelBuilder<Ready>>;
}

impl VintageExtension for ModelBuilder<Ready> {
    fn add_vintage_buildup(
        self,
        name: &str,
        new_volume_node: &str,
        decay_curve: &[f64],
    ) -> Result<ModelBuilder<Ready>> {
        vintage::add_vintage_buildup(self, name, new_volume_node, decay_curve)
    }
}

/// Extension methods for real estate operating statement templates.
pub trait RealEstateExtension {
    /// Add a standard NOI buildup: total revenue/expenses and NOI.
    fn add_noi_buildup(
        self,
        total_revenue_node: &str,
        revenue_nodes: &[&str],
        total_expenses_node: &str,
        expense_nodes: &[&str],
        noi_node: &str,
    ) -> Result<ModelBuilder<Ready>>;

    /// Add a standard NCF buildup: NOI minus CapEx items.
    fn add_ncf_buildup(
        self,
        noi_node: &str,
        capex_nodes: &[&str],
        ncf_node: &str,
    ) -> Result<ModelBuilder<Ready>>;

    /// Add a full rent roll with PGI/EGI decomposition, concessions, vacancy, and optional renewal.
    ///
    /// This is the canonical rent roll entry point. Creates per-lease nodes and aggregated totals.
    fn add_rent_roll(
        self,
        leases: &[real_estate::LeaseSpec],
        nodes: &real_estate::RentRollOutputNodes,
    ) -> Result<ModelBuilder<Ready>>;

    /// Add a full property operating statement template (rent roll -> EGI -> NOI -> NCF).
    fn add_property_operating_statement(
        self,
        leases: &[real_estate::LeaseSpec],
        other_income_nodes: &[&str],
        opex_nodes: &[&str],
        capex_nodes: &[&str],
        management_fee: Option<real_estate::ManagementFeeSpec>,
        nodes: &real_estate::PropertyTemplateNodes,
    ) -> Result<ModelBuilder<Ready>>;
}

impl RealEstateExtension for ModelBuilder<Ready> {
    fn add_noi_buildup(
        self,
        total_revenue_node: &str,
        revenue_nodes: &[&str],
        total_expenses_node: &str,
        expense_nodes: &[&str],
        noi_node: &str,
    ) -> Result<ModelBuilder<Ready>> {
        real_estate::add_noi_buildup(
            self,
            total_revenue_node,
            revenue_nodes,
            total_expenses_node,
            expense_nodes,
            noi_node,
        )
    }

    fn add_ncf_buildup(
        self,
        noi_node: &str,
        capex_nodes: &[&str],
        ncf_node: &str,
    ) -> Result<ModelBuilder<Ready>> {
        real_estate::add_ncf_buildup(self, noi_node, capex_nodes, ncf_node)
    }

    fn add_rent_roll(
        self,
        leases: &[real_estate::LeaseSpec],
        nodes: &real_estate::RentRollOutputNodes,
    ) -> Result<ModelBuilder<Ready>> {
        real_estate::add_rent_roll(self, leases, nodes)
    }

    fn add_property_operating_statement(
        self,
        leases: &[real_estate::LeaseSpec],
        other_income_nodes: &[&str],
        opex_nodes: &[&str],
        capex_nodes: &[&str],
        management_fee: Option<real_estate::ManagementFeeSpec>,
        nodes: &real_estate::PropertyTemplateNodes,
    ) -> Result<ModelBuilder<Ready>> {
        real_estate::add_property_operating_statement(
            self,
            leases,
            other_income_nodes,
            opex_nodes,
            capex_nodes,
            management_fee,
            nodes,
        )
    }
}
