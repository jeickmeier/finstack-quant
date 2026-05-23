use finstack_core::market_data::context::MarketContext;
use finstack_core::HashMap;

use super::state_keys;

/// Map of state variables for a tree node
pub type StateVariables = HashMap<&'static str, f64>;

/// Complete state information for a node in the pricing tree
#[derive(Clone)]
pub struct NodeState<'a> {
    /// Time step index (0 to N)
    pub step: usize,
    /// Time in years from valuation date
    pub time: f64,
    /// Map of all state variables at this node (reference to avoid cloning)
    pub vars: &'a StateVariables,
    /// Access to market context for additional data
    pub market_context: &'a MarketContext,
    /// Barrier state tracking (if applicable)
    pub barrier_state: Option<BarrierState>,
    /// Cached spot price for performance (avoids hash lookup)
    pub spot: Option<f64>,
    /// Cached interest rate for performance (avoids hash lookup)
    pub interest_rate: Option<f64>,
    /// Cached hazard rate for performance (avoids hash lookup)
    pub hazard_rate: Option<f64>,
    /// Cached discount factor for performance (avoids hash lookup)
    pub df: Option<f64>,
}

/// Simple barrier state tracking for barrier options
#[derive(Debug, Clone, Copy, Default)]
pub struct BarrierState {
    /// Whether barrier has been hit during the path
    pub barrier_hit: bool,
    /// Barrier level (for checking)
    pub barrier_level: f64,
    /// Barrier type
    pub barrier_type: BarrierType,
}

/// Types of barrier conditions
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarrierType {
    /// Up-and-out (option knocks out when spot > barrier)
    #[default]
    UpAndOut,
    /// Up-and-in (option knocks in when spot > barrier)
    UpAndIn,
    /// Down-and-out (option knocks out when spot < barrier)
    DownAndOut,
    /// Down-and-in (option knocks in when spot < barrier)
    DownAndIn,
}

/// Pre-extracted state variable cache to avoid redundant HashMap lookups in hot paths.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CachedValues {
    /// Spot price
    pub spot: Option<f64>,
    /// Interest rate
    pub interest_rate: Option<f64>,
    /// Hazard rate (default intensity)
    pub hazard_rate: Option<f64>,
    /// Discount factor
    pub df: Option<f64>,
}

impl<'a> NodeState<'a> {
    /// Create a new node state
    pub fn new(
        step: usize,
        time: f64,
        vars: &'a StateVariables,
        market_context: &'a MarketContext,
    ) -> Self {
        let cached = CachedValues {
            spot: vars.get(state_keys::SPOT).copied(),
            interest_rate: vars.get(state_keys::INTEREST_RATE).copied(),
            hazard_rate: vars.get(state_keys::HAZARD_RATE).copied(),
            df: vars.get(state_keys::DF).copied(),
        };

        Self {
            step,
            time,
            vars,
            market_context,
            barrier_state: None,
            spot: cached.spot,
            interest_rate: cached.interest_rate,
            hazard_rate: cached.hazard_rate,
            df: cached.df,
        }
    }

    /// Create a new node state with pre-extracted cached values.
    ///
    /// Avoids redundant HashMap lookups when the caller already knows the values.
    /// Used in hot paths (backward induction) where we just inserted the values.
    #[inline]
    pub(crate) fn with_cached(
        step: usize,
        time: f64,
        vars: &'a StateVariables,
        market_context: &'a MarketContext,
        cached: CachedValues,
    ) -> Self {
        Self {
            step,
            time,
            vars,
            market_context,
            barrier_state: None,
            spot: cached.spot,
            interest_rate: cached.interest_rate,
            hazard_rate: cached.hazard_rate,
            df: cached.df,
        }
    }

    /// Create a new node state with barrier tracking and pre-extracted cached values.
    #[inline]
    pub(crate) fn with_cached_barrier(
        step: usize,
        time: f64,
        vars: &'a StateVariables,
        market_context: &'a MarketContext,
        barrier_state: BarrierState,
        cached: CachedValues,
    ) -> Self {
        Self {
            step,
            time,
            vars,
            market_context,
            barrier_state: Some(barrier_state),
            spot: cached.spot,
            interest_rate: cached.interest_rate,
            hazard_rate: cached.hazard_rate,
            df: cached.df,
        }
    }

    /// Create a new node state with barrier tracking
    pub fn new_with_barrier(
        step: usize,
        time: f64,
        vars: &'a StateVariables,
        market_context: &'a MarketContext,
        barrier_state: BarrierState,
    ) -> Self {
        // Pre-extract commonly accessed variables to avoid hash lookups in hot path
        let spot = vars.get(state_keys::SPOT).copied();
        let interest_rate = vars.get(state_keys::INTEREST_RATE).copied();
        let hazard_rate = vars.get(state_keys::HAZARD_RATE).copied();
        let df = vars.get(state_keys::DF).copied();

        Self {
            step,
            time,
            vars,
            market_context,
            barrier_state: Some(barrier_state),
            spot,
            interest_rate,
            hazard_rate,
            df,
        }
    }

    /// Get a state variable by key
    #[inline]
    pub fn get_var(&self, key: &str) -> Option<f64> {
        self.vars.get(key).copied()
    }

    /// Get a state variable by key with a default value
    #[inline]
    pub fn get_var_or(&self, key: &str, default: f64) -> f64 {
        self.vars.get(key).copied().unwrap_or(default)
    }

    /// Get spot price (convenience method, uses cached value)
    #[inline]
    pub fn spot(&self) -> Option<f64> {
        self.spot
    }

    /// Get interest rate (convenience method, uses cached value)
    #[inline]
    pub fn interest_rate(&self) -> Option<f64> {
        self.interest_rate
    }

    /// Get credit spread (convenience method)
    #[inline]
    pub fn credit_spread(&self) -> Option<f64> {
        self.get_var(state_keys::CREDIT_SPREAD)
    }

    /// Get hazard rate (convenience method, uses cached value)
    #[inline]
    pub fn hazard_rate(&self) -> Option<f64> {
        self.hazard_rate
    }

    /// Get discount factor (convenience method, uses cached value)
    #[inline]
    pub fn discount_factor(&self) -> Option<f64> {
        self.df
    }

    /// Check if barrier has been hit (for barrier options)
    pub fn is_barrier_hit(&self) -> bool {
        self.barrier_state.as_ref().is_some_and(|bs| bs.barrier_hit)
    }

    /// Update barrier state based on current spot price
    pub fn update_barrier_state(&mut self, spot_price: f64) {
        if let Some(ref mut barrier_state) = self.barrier_state {
            if !barrier_state.barrier_hit {
                let hit = match barrier_state.barrier_type {
                    BarrierType::UpAndOut | BarrierType::UpAndIn => {
                        spot_price >= barrier_state.barrier_level
                    }
                    BarrierType::DownAndOut | BarrierType::DownAndIn => {
                        spot_price <= barrier_state.barrier_level
                    }
                };
                barrier_state.barrier_hit = hit;
            }
        }
    }

    /// Check if option should be knocked out (for barrier options)
    pub fn is_knocked_out(&self) -> bool {
        if let Some(ref barrier_state) = self.barrier_state {
            barrier_state.barrier_hit
                && matches!(
                    barrier_state.barrier_type,
                    BarrierType::UpAndOut | BarrierType::DownAndOut
                )
        } else {
            false
        }
    }

    /// Check if option should be knocked in (for barrier options)
    pub fn is_knocked_in(&self) -> bool {
        if let Some(ref barrier_state) = self.barrier_state {
            barrier_state.barrier_hit
                && matches!(
                    barrier_state.barrier_type,
                    BarrierType::UpAndIn | BarrierType::DownAndIn
                )
        } else {
            true // If no barrier, always "knocked in"
        }
    }

    /// Whether the up barrier was touched at this node (discrete monitoring flag)
    pub fn barrier_touched_up(&self) -> bool {
        self.get_var(state_keys::BARRIER_TOUCHED_UP)
            .map(|v| v > 0.5)
            .unwrap_or(false)
    }

    /// Whether the down barrier was touched at this node (discrete monitoring flag)
    pub fn barrier_touched_down(&self) -> bool {
        self.get_var(state_keys::BARRIER_TOUCHED_DOWN)
            .map(|v| v > 0.5)
            .unwrap_or(false)
    }
}
