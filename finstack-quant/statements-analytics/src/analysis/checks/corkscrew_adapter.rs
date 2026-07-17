//! Adapter that converts corkscrew account definitions into structural checks.
//!
//! This bridges the corkscrew extension's configuration with the check
//! framework, allowing users who already have a [`CorkscrewConfig`] to
//! derive [`BalanceSheetArticulation`] checks automatically.

use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::Check;
use finstack_quant_statements::types::NodeId;

use crate::extensions::corkscrew::{AccountType, CorkscrewConfig};

/// Convert a [`CorkscrewConfig`] into a set of [`Check`] trait objects.
///
/// Groups accounts by [`AccountType`] and produces a single
/// [`BalanceSheetArticulation`] check that verifies
/// Assets = Liabilities + Equity across the configured accounts.
///
/// Returns an empty `Vec` if the config has no accounts.
///
/// # Arguments
///
/// * `config` - Corkscrew account configuration whose asset, liability, and
///   equity node IDs are grouped into structural checks.
pub fn corkscrew_as_checks(config: &CorkscrewConfig) -> Vec<Box<dyn Check>> {
    let mut assets: Vec<NodeId> = Vec::new();
    let mut liabilities: Vec<NodeId> = Vec::new();
    let mut equity: Vec<NodeId> = Vec::new();

    for account in &config.accounts {
        let nid = NodeId::new(&account.node_id);
        match account.account_type {
            AccountType::Asset => assets.push(nid),
            AccountType::Liability => liabilities.push(nid),
            AccountType::Equity => equity.push(nid),
        }
    }

    if assets.is_empty() && liabilities.is_empty() && equity.is_empty() {
        return Vec::new();
    }

    let check = BalanceSheetArticulation {
        assets_nodes: assets,
        liabilities_nodes: liabilities,
        equity_nodes: equity,
        tolerance: Some(config.tolerance),
    };

    vec![Box::new(check)]
}
