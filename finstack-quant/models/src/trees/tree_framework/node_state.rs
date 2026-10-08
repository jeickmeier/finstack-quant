//! Shared node, evolution, and backward-induction components for pricing trees.
//!
/// State handed to a [`TreeValuator`](super::TreeValuator) at one lattice node.
///
/// Each tree fills the fields it models and leaves the rest `None`: equity
/// trees set `spot`, short-rate trees set `interest_rate`, and the
/// rates-credit tree sets `interest_rate`, `hazard_rate` and (at interior
/// nodes) `df`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeState {
    /// Time step index (0 to N)
    pub step: usize,
    /// Option-adjusted spread the tree is being priced at, in basis points
    /// (continuously compounded). Zero for trees priced without a spread.
    pub oas_bp: f64,
    /// Underlying asset price at this node
    pub spot: Option<f64>,
    /// Short rate at this node as a decimal annual rate
    pub interest_rate: Option<f64>,
    /// Hazard rate (default intensity) at this node as a decimal annual rate
    pub hazard_rate: Option<f64>,
    /// Discount factor over the interval starting at this node
    pub df: Option<f64>,
}

impl NodeState {
    /// Get spot price
    #[inline]
    pub fn spot(&self) -> Option<f64> {
        self.spot
    }

    /// Get interest rate
    #[inline]
    pub fn interest_rate(&self) -> Option<f64> {
        self.interest_rate
    }

    /// Get hazard rate
    #[inline]
    pub fn hazard_rate(&self) -> Option<f64> {
        self.hazard_rate
    }

    /// Get discount factor
    #[inline]
    pub fn discount_factor(&self) -> Option<f64> {
        self.df
    }
}
