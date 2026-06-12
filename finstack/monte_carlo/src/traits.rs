//! Core traits for Monte Carlo simulation and pricing.
//!
//! This module defines the fundamental abstractions that enable composable,
//! testable Monte Carlo simulation:
//!
//! - `RandomStream`: RNG abstraction with splittable streams
//! - `StochasticProcess`: SDE specification (drift, diffusion)
//! - `Discretization`: Time-stepping schemes
//! - `PathState`: State information at a point along a path
//! - `Payoff`: Payoff computation with currency safety
//!
//! Times are expressed as year fractions, and process-specific quantities such
//! as rates, dividend yields, and volatilities are generally quoted in decimals
//! rather than basis points unless a process module says otherwise.
//!
//! # Example
//!
//! The example below shows the most important interaction in this module:
//! a process maps raw state-vector entries into named [`state_keys`] so a payoff
//! can read them through [`PathState`] during `on_event`.
//!
//! ```rust
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use finstack_monte_carlo::traits::{
//!     state_keys, PathState, Payoff, StochasticProcess,
//! };
//!
//! struct SpotOnlyProcess;
//!
//! impl StochasticProcess for SpotOnlyProcess {
//!     fn dim(&self) -> usize { 1 }
//!
//!     fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
//!         out[0] = 0.0;
//!     }
//!
//!     fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
//!         out[0] = 0.0;
//!     }
//!
//!     fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
//!         state.set(state_keys::SPOT, x[0]);
//!     }
//! }
//!
//! #[derive(Clone, Default)]
//! struct IntrinsicValuePayoff {
//!     terminal_spot: f64,
//! }
//!
//! impl Payoff for IntrinsicValuePayoff {
//!     fn on_event(&mut self, state: &mut PathState) {
//!         self.terminal_spot = state.spot().unwrap_or(0.0);
//!     }
//!
//!     fn value(&self, currency: Currency) -> Money {
//!         Money::new((self.terminal_spot - 100.0).max(0.0), currency)
//!     }
//!
//!     fn reset(&mut self) {
//!         self.terminal_spot = 0.0;
//!     }
//! }
//! ```

use super::paths::CashflowType;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::HashMap;
use smallvec::SmallVec;

/// Random stream trait for generating random numbers.
///
/// Implementations must be cheap enough to clone for serial execution. Parallel
/// execution additionally requires deterministic stream splitting; each stream
/// is independent and can be split into substreams identified by a unique ID.
pub trait RandomStream: Clone + Send + Sync {
    /// Split this stream into a new independent substream.
    ///
    /// The `stream_id` should be unique across all substreams to ensure independence.
    /// This enables deterministic parallel execution where each path gets its own stream.
    ///
    /// Returns `None` when the underlying generator cannot be split into
    /// independent streams (e.g. Sobol quasi-random sequences). Callers should
    /// check [`RandomStream::supports_splitting`] before requesting per-path
    /// streams, and treat `None` as a validation error.
    fn split(&self, stream_id: u64) -> Option<Self>
    where
        Self: Sized;

    /// Fill a buffer with uniform random numbers in the open interval (0, 1).
    ///
    /// This is a vectorized operation for efficiency.
    ///
    /// Implementations must never return exactly 0.0 or 1.0: consumers feed
    /// these values into inverse CDFs and logs, and antithetic sampling
    /// mirrors them as `1 − u`, so either boundary value would produce
    /// infinities. Use a grid-centred mapping such as `(bits + 0.5)·2⁻⁵³`.
    fn fill_u01(&mut self, out: &mut [f64]);

    /// Fill a buffer with standard normal random numbers N(0,1).
    ///
    /// Implementations may use Box-Muller, inverse CDF, or other transforms.
    fn fill_std_normals(&mut self, out: &mut [f64]);

    /// Generate a single uniform random number in [0, 1).
    fn next_u01(&mut self) -> f64 {
        let mut buf = [0.0];
        self.fill_u01(&mut buf);
        buf[0]
    }

    /// Generate a single standard normal random number.
    fn next_std_normal(&mut self) -> f64 {
        let mut buf = [0.0];
        self.fill_std_normals(&mut buf);
        buf[0]
    }

    /// Returns whether this RNG supports stream splitting for parallel execution.
    ///
    /// Returns `true` if `split(stream_id)` produces independent streams for any `stream_id`.
    /// Returns `false` for quasi-random sequences (e.g., Sobol) that cannot be meaningfully split.
    ///
    /// The MC engine checks this before enabling parallel mode.
    fn supports_splitting(&self) -> bool {
        true // Default: most PRNGs support splitting
    }
}

/// Antithetic mirror of a [`RandomStream`].
///
/// Reads the same underlying draws as the wrapped stream and mirrors them:
/// normals are negated and uniforms mapped `u → 1 − u`. The engine hands a
/// mirror of the primary leg's stream to the antithetic payoff's
/// [`Payoff::on_path_start`], so per-path payoff randomness (e.g. default
/// thresholds) stays antithetically coupled across the pair instead of being
/// drawn independently — independent draws silently destroy the variance
/// reduction for that randomness while leaving the estimator unbiased.
#[derive(Clone)]
pub(crate) struct MirroredStream<R: RandomStream>(pub(crate) R);

impl<R: RandomStream> RandomStream for MirroredStream<R> {
    fn split(&self, stream_id: u64) -> Option<Self> {
        self.0.split(stream_id).map(MirroredStream)
    }

    fn fill_u01(&mut self, out: &mut [f64]) {
        self.0.fill_u01(out);
        for x in out {
            *x = 1.0 - *x;
        }
    }

    fn fill_std_normals(&mut self, out: &mut [f64]) {
        self.0.fill_std_normals(out);
        for x in out {
            *x = -*x;
        }
    }

    fn supports_splitting(&self) -> bool {
        self.0.supports_splitting()
    }
}

/// Standard named keys for values stored in [`PathState`].
pub mod state_keys {
    use std::sync::Mutex;

    pub use crate::indexed_spot_table::INDEXED_SPOT_INLINE;

    use crate::indexed_spot_table::INDEXED_SPOT_TABLE;

    /// Spot price (equity/FX)
    pub const SPOT: &str = "spot";
    /// Stochastic volatility (Heston, etc.)
    pub const VARIANCE: &str = "variance";
    /// Short rate (Hull-White, etc.)
    pub const SHORT_RATE: &str = "short_rate";
    /// Time (in years from valuation)
    pub const TIME: &str = "time";
    /// Step index
    pub const STEP: &str = "step";
    /// FX rate (for multi-asset/quanto products)
    pub const FX_RATE: &str = "fx_rate";
    /// Equity spot (for multi-asset, when SPOT refers to FX)
    pub const EQUITY_SPOT: &str = "equity_spot";
    /// Current NPV of remaining cashflows (for MTM)
    pub const NPV_CURRENT: &str = "npv_current";
    /// NPV from previous timestep (for MTM)
    pub const NPV_PREVIOUS: &str = "npv_previous";
    /// Mark-to-market P&L (change in NPV)
    pub const MTM_PNL: &str = "mtm_pnl";

    static INDEXED_SPOT_OVERFLOW: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    /// Return the canonical state key for the indexed spot of a multi-asset process.
    ///
    /// Indices `0..INDEXED_SPOT_INLINE` (currently 128) map directly to static
    /// `&str` literals with no allocation. Indices above that bound trigger an
    /// interning step that **leaks one heap allocation per distinct index**
    /// into a process-wide [`std::sync::Mutex`]-protected cache. This is
    /// deliberate — the keys must outlive any path simulation and live for
    /// `'static`, and `Box::leak` is the simplest way to obtain that guarantee.
    ///
    /// In practice the leak budget is tiny: a multi-asset basket of 200 names
    /// leaks ~72 strings (indices 128..200) on first use, then nothing more.
    /// Workloads that genuinely need thousands of indexed spots should consider
    /// raising [`INDEXED_SPOT_INLINE`] at the source rather than relying on
    /// the overflow path.
    pub fn indexed_spot(index: usize) -> &'static str {
        if index < INDEXED_SPOT_INLINE {
            INDEXED_SPOT_TABLE[index]
        } else {
            indexed_spot_overflow(index)
        }
    }

    #[allow(clippy::expect_used)] // Mutex poisoning indicates prior panic; propagate.
    fn indexed_spot_overflow(index: usize) -> &'static str {
        let mut cache = INDEXED_SPOT_OVERFLOW
            .lock()
            .expect("indexed spot overflow cache mutex should not be poisoned");
        let base = INDEXED_SPOT_INLINE;
        while base + cache.len() <= index {
            let next = base + cache.len();
            let key = Box::leak(format!("spot_{next}").into_boxed_str());
            cache.push(key);
        }
        cache[index - base]
    }
}

/// Indexed state variable key for O(1) array access in the MC inner loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StateKey {
    /// Spot price (equity/FX)
    Spot = 0,
    /// Stochastic variance (Heston, etc.)
    Variance = 1,
    /// Short rate (Hull-White, etc.)
    ShortRate = 2,
    /// Time in years from valuation
    Time = 3,
    /// Step index
    Step = 4,
    /// FX rate (multi-asset/quanto)
    FxRate = 5,
    /// Equity spot (multi-asset, when Spot refers to FX)
    EquitySpot = 6,
    /// Current NPV of remaining cashflows
    NpvCurrent = 7,
    /// NPV from previous timestep
    NpvPrevious = 8,
    /// Mark-to-market P&L
    MtmPnl = 9,
    /// Credit spread
    CreditSpread = 10,
    /// Pathwise money-market account `B(t) = exp(∫₀ᵗ r(s) ds)` (numeraire)
    BankAccount = 11,
}

const STATE_ARRAY_LEN: usize = 12;

/// Resolve a string key to its fixed-slot `StateKey`, if it matches a known key.
fn resolve_key(key: &str) -> Option<StateKey> {
    match key {
        "spot" => Some(StateKey::Spot),
        "variance" => Some(StateKey::Variance),
        "short_rate" => Some(StateKey::ShortRate),
        "time" => Some(StateKey::Time),
        "step" => Some(StateKey::Step),
        "fx_rate" => Some(StateKey::FxRate),
        "equity_spot" => Some(StateKey::EquitySpot),
        "npv_current" => Some(StateKey::NpvCurrent),
        "npv_previous" => Some(StateKey::NpvPrevious),
        "mtm_pnl" => Some(StateKey::MtmPnl),
        "credit_spread" => Some(StateKey::CreditSpread),
        "bank_account" => Some(StateKey::BankAccount),
        _ => None,
    }
}

fn parse_indexed_spot_key(key: &str) -> Option<usize> {
    key.strip_prefix("spot_")?.parse::<usize>().ok()
}

#[derive(Debug, Clone, Default)]
struct PathStateExtras {
    indexed_spots: SmallVec<[Option<f64>; 4]>,
    dynamic: HashMap<&'static str, f64>,
    cashflows: Vec<(f64, f64, CashflowType)>,
}

/// State information for a point along a Monte Carlo path.
///
/// Uses a fixed-size array for known state keys (O(1) access) and stores rarer
/// dynamic/indexed state plus cashflows in an optional sidecar allocation.
#[derive(Debug, Clone)]
pub struct PathState {
    /// Time step index (0 = initial, N = final)
    pub step: usize,
    /// Time in years from valuation date
    pub time: f64,
    /// Fixed-slot storage for known state keys (indexed by `StateKey`)
    fixed: [f64; STATE_ARRAY_LEN],
    /// Bitmask tracking which fixed slots contain valid values
    fixed_set: u16,
    extras: Option<Box<PathStateExtras>>,
    uniform_random: f64,
}

impl PathState {
    fn sync_core_fields(&mut self) {
        self.set_key(StateKey::Time, self.time);
        self.set_key(StateKey::Step, self.step as f64);
    }

    fn extras(&self) -> Option<&PathStateExtras> {
        self.extras.as_deref()
    }

    fn extras_mut(&mut self) -> &mut PathStateExtras {
        self.extras
            .get_or_insert_with(|| Box::new(PathStateExtras::default()))
            .as_mut()
    }

    /// Create a new path state.
    pub fn new(step: usize, time: f64) -> Self {
        let mut state = Self {
            step,
            time,
            fixed: [0.0; STATE_ARRAY_LEN],
            fixed_set: 0,
            extras: None,
            uniform_random: 0.0,
        };
        state.sync_core_fields();
        state
    }

    /// Create a path state with initial variables.
    pub fn with_vars(step: usize, time: f64, vars: HashMap<&'static str, f64>) -> Self {
        let mut ps = Self::new(step, time);
        for (key, value) in vars {
            ps.set(key, value);
        }
        ps
    }

    /// Set a state variable by indexed key (fast path, no hashing).
    #[inline]
    pub fn set_key(&mut self, key: StateKey, value: f64) {
        let idx = key as usize;
        self.fixed[idx] = value;
        self.fixed_set |= 1 << idx;
    }

    /// Get a state variable by indexed key (fast path, no hashing).
    #[inline]
    pub fn get_key(&self, key: StateKey) -> Option<f64> {
        let idx = key as usize;
        if self.fixed_set & (1 << idx) != 0 {
            Some(self.fixed[idx])
        } else {
            None
        }
    }

    /// Set the uniform random value for this timestep.
    pub fn set_uniform_random(&mut self, u: f64) {
        self.uniform_random = u;
    }

    /// Get the uniform random value for this timestep.
    pub fn uniform_random(&self) -> f64 {
        self.uniform_random
    }

    /// Get a state variable by string key.
    /// Routes known keys to the fixed array; unknown keys to the dynamic HashMap.
    pub fn get(&self, key: &str) -> Option<f64> {
        if let Some(sk) = resolve_key(key) {
            self.get_key(sk)
        } else if let Some(index) = parse_indexed_spot_key(key) {
            self.extras()
                .and_then(|extras| extras.indexed_spots.get(index))
                .copied()
                .flatten()
        } else {
            self.extras()
                .and_then(|extras| extras.dynamic.get(key))
                .copied()
        }
    }

    /// Get a state variable with a default value.
    pub fn get_or(&self, key: &str, default: f64) -> f64 {
        self.get(key).unwrap_or(default)
    }

    /// Set a state variable by string key.
    /// Routes known keys to the fixed array; unknown keys to the dynamic HashMap.
    pub fn set(&mut self, key: &'static str, value: f64) {
        if let Some(sk) = resolve_key(key) {
            self.set_key(sk, value);
        } else if let Some(index) = parse_indexed_spot_key(key) {
            self.set_indexed_spot(index, value);
        } else {
            self.extras_mut().dynamic.insert(key, value);
        }
    }

    /// Set a multi-asset indexed spot without hashing a string key.
    pub fn set_indexed_spot(&mut self, index: usize, value: f64) {
        let indexed_spots = &mut self.extras_mut().indexed_spots;
        if indexed_spots.len() <= index {
            indexed_spots.resize_with(index + 1, || None);
        }
        indexed_spots[index] = Some(value);
    }

    /// Update the public step/time fields and their keyed representations together.
    pub fn set_step_time(&mut self, step: usize, time: f64) {
        self.step = step;
        self.time = time;
        self.sync_core_fields();
    }

    /// Merge all state variables into `out` (clears `out` first).
    ///
    /// Prefer this over [`Self::vars`] on hot paths to reuse a single `HashMap`
    /// allocation instead of cloning the dynamic map and allocating a fresh map
    /// on every call.
    pub fn collect_vars(&self, out: &mut HashMap<&'static str, f64>) {
        out.clear();

        let dyn_len = self.extras().map(|e| e.dynamic.len()).unwrap_or(0);
        let indexed_count = self
            .extras()
            .map(|e| e.indexed_spots.iter().filter(|v| v.is_some()).count())
            .unwrap_or(0);
        let fixed_count = self.fixed_set.count_ones() as usize;
        out.reserve(dyn_len + indexed_count + fixed_count);

        if let Some(extras) = self.extras() {
            for (k, v) in &extras.dynamic {
                out.insert(*k, *v);
            }
        }

        let key_names: [&'static str; STATE_ARRAY_LEN] = [
            state_keys::SPOT,
            state_keys::VARIANCE,
            state_keys::SHORT_RATE,
            state_keys::TIME,
            state_keys::STEP,
            state_keys::FX_RATE,
            state_keys::EQUITY_SPOT,
            state_keys::NPV_CURRENT,
            state_keys::NPV_PREVIOUS,
            state_keys::MTM_PNL,
            "credit_spread",
            "bank_account",
        ];
        for (i, key_name) in key_names.iter().enumerate().take(STATE_ARRAY_LEN) {
            if self.fixed_set & (1 << i) != 0 {
                out.insert(*key_name, self.fixed[i]);
            }
        }

        if let Some(extras) = self.extras() {
            for (index, value) in extras.indexed_spots.iter().enumerate() {
                if let Some(value) = value {
                    out.insert(state_keys::indexed_spot(index), *value);
                }
            }
        }
    }

    /// Collect all state variables into a HashMap.
    /// Merges fixed-slot values with dynamic values. This allocates --
    /// prefer `get`/`get_key` or [`Self::collect_vars`] on hot paths.
    pub fn vars(&self) -> HashMap<&'static str, f64> {
        let mut map = HashMap::<&'static str, f64>::default();
        self.collect_vars(&mut map);
        map
    }

    /// Get spot price (convenience method).
    #[inline]
    pub fn spot(&self) -> Option<f64> {
        self.get_key(StateKey::Spot)
    }

    /// Get variance (convenience method).
    #[inline]
    pub fn variance(&self) -> Option<f64> {
        self.get_key(StateKey::Variance)
    }

    /// Add a cashflow to this state.
    pub fn add_cashflow(&mut self, time: f64, amount: f64) {
        self.extras_mut()
            .cashflows
            .push((time, amount, CashflowType::Other));
    }

    /// Add a typed cashflow to this state.
    pub fn add_typed_cashflow(&mut self, time: f64, amount: f64, cf_type: CashflowType) {
        self.extras_mut().cashflows.push((time, amount, cf_type));
    }

    /// Drain all cashflows into a callback while retaining the underlying capacity.
    pub(crate) fn drain_cashflows<F>(&mut self, mut sink: F)
    where
        F: FnMut(f64, f64, CashflowType),
    {
        if let Some(extras) = self.extras.as_deref_mut() {
            for (time, amount, cf_type) in extras.cashflows.drain(..) {
                sink(time, amount, cf_type);
            }
        }
    }

    /// Take all cashflows from this state, leaving it empty.
    pub fn take_cashflows(&mut self) -> Vec<(f64, f64, CashflowType)> {
        self.extras
            .as_deref_mut()
            .map_or_else(Vec::new, |extras| std::mem::take(&mut extras.cashflows))
    }

    /// Get a reference to cashflows (for inspection, not taking ownership).
    pub fn cashflows(&self) -> &[(f64, f64, CashflowType)] {
        self.extras()
            .map_or_else(|| &[][..], |extras| extras.cashflows.as_slice())
    }
}

/// Stochastic process specification (SDE).
///
/// A stochastic process defines the drift and diffusion coefficients
/// for a system of SDEs:
///
/// ```text
/// dX_t = μ(t, X_t) dt + Σ(t, X_t) dW_t
/// ```
///
/// where μ is the drift vector and Σ is the diffusion matrix (or diagonal).
/// Implementers are responsible for documenting the economic meaning and units
/// of each state variable in their concrete process modules.
///
/// # Example
///
/// GBM for equity under risk-neutral measure:
///
/// ```text
/// dS_t = (r - q) S_t dt + σ S_t dW_t
/// ```
pub trait StochasticProcess: Send + Sync {
    /// Number of state variables.
    fn dim(&self) -> usize;

    /// Number of independent Brownian motions (may differ from dim).
    ///
    /// For example, Heston has dim=2 (S, v) but may have 2 correlated BMs.
    fn num_factors(&self) -> usize {
        self.dim()
    }

    /// Compute drift vector: μ(t, x).
    ///
    /// # Arguments
    ///
    /// * `t` - Time in years
    /// * `x` - Current state vector (length = dim)
    /// * `out` - Output drift vector (length = dim)
    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]);

    /// Compute diffusion matrix or diagonal: Σ(t, x).
    ///
    /// For diagonal diffusion, `out` contains the diagonal elements.
    /// For full matrix, `out` is row-major (or Cholesky factor).
    ///
    /// # Arguments
    ///
    /// * `t` - Time in years
    /// * `x` - Current state vector (length = dim)
    /// * `out` - Output diffusion vector/matrix
    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]);

    /// Optional factor correlation matrix (`num_factors × num_factors`, row-major).
    ///
    /// When present, the engine treats the simulation shocks `z` as the output
    /// of `L · z_raw` where `L L^T` reconstructs this correlation matrix and
    /// `z_raw` are independent standard normals generated by the RNG. Any
    /// [`Discretization`] that applies correlation to the shocks itself (e.g.
    /// [`QeHeston`](crate::discretization::QeHeston)) should return `true` from
    /// [`Discretization::applies_correlation_internally`]; the engine then
    /// skips its Cholesky transform and passes the raw independent shocks
    /// unchanged.
    ///
    /// # Returns
    ///
    /// `None` when the factors are structurally independent or the process
    /// uses a non-standard noise basis (e.g. fractional Brownian motion
    /// increments, factor loadings) that the engine cannot correlate via a
    /// standard Cholesky transform. Otherwise a symmetric positive-semidefinite
    /// matrix with unit diagonal in row-major order.
    ///
    /// The returned vector is consumed once per pricing invocation to build a
    /// Cholesky factor, so the allocation cost is amortised over the full
    /// simulation and is not on a hot path.
    fn factor_correlation(&self) -> Option<Vec<f64>> {
        None
    }

    /// Whether this process requires externally injected noise in some factor
    /// slots (e.g. Riemann-Liouville Volterra increments and driving normals
    /// for rough-volatility models).
    ///
    /// The generic [`crate::engine::McEngine`] fills every factor slot with
    /// i.i.d. N(0,1) draws, which is silently wrong for such processes — the
    /// variance path would receive uncorrelated, unscaled noise with no
    /// error. `McEngine` rejects processes that return `true`; drive them
    /// through [`crate::engine_fractional::simulate_path_fractional`] (or a
    /// dedicated pricer) instead.
    fn requires_injected_noise(&self) -> bool {
        false
    }

    /// Identifier of the dedicated discretization this process requires, if
    /// any.
    ///
    /// Some processes carry dynamics a generic scheme cannot see — discrete
    /// dividends ([`crate::process::gbm_dividends::GbmWithDividends`]) or
    /// Poisson jumps ([`crate::process::bates::BatesProcess`]). Pairing them
    /// with generic Euler-style schemes type-checks but silently simulates
    /// only the diffusion, producing systematically wrong prices. Returning
    /// `Some(id)` here makes the engine reject any scheme whose
    /// [`Discretization::scheme_id`] differs from `id`.
    fn dedicated_scheme(&self) -> Option<&'static str> {
        None
    }

    /// Populate a [`PathState`] from the raw state vector.
    ///
    /// This is the bridge between process-specific storage and payoff logic.
    /// Implementations should map raw state-vector indices to stable semantic
    /// keys such as [`state_keys::SPOT`] or [`state_keys::SHORT_RATE`] so payoffs
    /// can read the values without knowing the process's raw layout. Override
    /// this method whenever the default equity-style mapping is not sufficient.
    ///
    /// Default mapping (suitable for GBM and Heston-like models):
    /// - `x[0]` => `SPOT`
    /// - `x[1]` => `VARIANCE` (if dim >= 2)
    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if !x.is_empty() {
            state.set(state_keys::SPOT, x[0]);
        }
        if x.len() >= 2 {
            state.set(state_keys::VARIANCE, x[1]);
        }
    }
}

/// Marker for processes with **GBM-style proportional diffusion**: `σ(X) = σ_const · X`.
///
/// # Numerical contract (read carefully)
///
/// A correct implementation guarantees that the diffusion coefficient is
/// strictly proportional to the state variable in each dimension, so
/// `∂σ/∂X = σ/X` holds exactly. Under that contract the
/// [`crate::discretization::Milstein`] correction term
/// `½ σ σ' (Z² − 1) Δt` reduces to its exact GBM form using `σ' ≈ σ/X`.
///
/// **Implementing this trait on a process where the diffusion is *not*
/// proportional silently corrupts every Milstein-discretised price you
/// produce** — Milstein degrades to Euler accuracy *and* applies an
/// incorrect Itô correction. The crate's `#![forbid(unsafe_code)]` posture
/// prevents this from being modelled as an `unsafe` trait, so the contract
/// is enforced by convention and code review:
///
/// - Only implement on processes whose `diffusion(t, x)` returns `σ_const ·
///   x[i]` for every state component `i` simulated by Milstein.
/// - Do **not** implement on CIR, OU, Heston variance, jump-diffusion, or
///   any stochastic-volatility variance state. None of these satisfy the
///   contract.
/// - When in doubt, omit the impl. Milstein/`LogMilstein` are bounded on
///   `ProportionalDiffusion`, so omitting the impl yields a compile error
///   rather than a silently-biased run.
///
/// This trait is intentionally safe to **call** but dangerous to **implement**.
/// New impls must be reviewed against the proportional-diffusion contract.
pub trait ProportionalDiffusion: StochasticProcess {}

/// Time discretization scheme for SDEs.
///
/// A discretization scheme advances the state from time t to t + Δt
/// given random shocks.
///
/// # Example Schemes
///
/// - **Exact**: Analytical solution (GBM, OU)
/// - **Euler-Maruyama**: First-order explicit
/// - **Milstein**: Improved for diagonal diffusion
/// - **QE**: Quadratic-exponential for CIR/Heston variance
///
/// # Arguments to `step`
///
/// * `process` - The stochastic process
/// * `t` - Current time
/// * `dt` - Time step size
/// * `x` - Current state (updated in-place)
/// * `z` - Standard normal shocks (length = num_factors)
/// * `work` - Workspace buffer for intermediate calculations
pub trait Discretization<P: StochasticProcess + ?Sized>: Send + Sync {
    /// Advance state from t to t + dt.
    ///
    /// This method updates `x` in-place using the provided random shocks `z`.
    fn step(&self, process: &P, t: f64, dt: f64, x: &mut [f64], z: &[f64], work: &mut [f64]);

    /// Workspace size required for intermediate calculations.
    fn work_size(&self, process: &P) -> usize {
        process.dim()
    }

    /// Whether this scheme applies the process's factor correlation itself.
    ///
    /// When `true`, the engine skips its own Cholesky transform of the shock
    /// vector and passes independent standard normals (or the scheme's
    /// expected non-standard driver) directly to [`Self::step`]. Schemes that
    /// encode correlation inside their update (e.g. the QE scheme for Heston
    /// which uses `ρ` in the conditional log-spot update, or fBM-based rough
    /// schemes) must return `true`.
    ///
    /// Returns `false` by default, which lets the engine build a Cholesky
    /// factor from [`StochasticProcess::factor_correlation`] and apply it to the
    /// raw shocks before each `step` call.
    fn applies_correlation_internally(&self) -> bool {
        false
    }

    /// Identifier used to satisfy a process's
    /// [`StochasticProcess::dedicated_scheme`] requirement.
    ///
    /// Generic schemes keep the default `"generic"`. A scheme written for a
    /// specific process (e.g. `ExactGbmWithDividends`, `QeBates`) overrides
    /// this with a stable name so the engine can verify the pairing.
    fn scheme_id(&self) -> &'static str {
        "generic"
    }
}

// ---------------------------------------------------------------------------
// Pricing-specific traits
// ---------------------------------------------------------------------------

/// Payoff computation with currency safety.
///
/// Payoffs accumulate path information via `on_event` calls and
/// return a final `Money` value. This ensures all results carry
/// explicit currency information. The engine calls `reset()` before each path
/// and then invokes `on_event()` at the initial state and after every time step.
///
/// # Event Order
///
/// For each simulated path, the engine calls methods in this order:
///
/// 1. [`Payoff::reset`] to clear state from the previous path.
/// 2. [`Payoff::on_path_start`] with the path-local random stream.
/// 3. [`Payoff::on_event`] at the initial state and after every time step.
/// 4. [`Payoff::value`] once, after the terminal event.
///
/// Implementations should treat [`Payoff::value`] as an undiscounted payoff.
/// The generic [`crate::engine::McEngine`] applies the caller-supplied
/// discount factor outside the payoff.
///
/// # Cashflow Capture
///
/// A payoff can attach diagnostic cashflows by calling
/// [`PathState::add_cashflow`] or [`PathState::add_typed_cashflow`] during
/// [`Payoff::on_event`]. When path capture is enabled, the engine drains those
/// cashflows into the captured [`crate::paths::PathPoint`] for that event.
///
/// # Examples
///
/// ```rust
/// use finstack_core::currency::Currency;
/// use finstack_core::money::Money;
/// use finstack_monte_carlo::traits::{state_keys, PathState, Payoff};
///
/// #[derive(Clone)]
/// struct TerminalCall {
///     strike: f64,
///     terminal_spot: f64,
/// }
///
/// impl TerminalCall {
///     fn new(strike: f64) -> Self {
///         Self {
///             strike,
///             terminal_spot: 0.0,
///         }
///     }
/// }
///
/// impl Payoff for TerminalCall {
///     fn on_event(&mut self, state: &mut PathState) {
///         if let Some(spot) = state.get(state_keys::SPOT) {
///             self.terminal_spot = spot;
///         }
///     }
///
///     fn value(&self, currency: Currency) -> Money {
///         Money::new((self.terminal_spot - self.strike).max(0.0), currency)
///     }
///
///     fn reset(&mut self) {
///         self.terminal_spot = 0.0;
///     }
/// }
///
/// let mut payoff = TerminalCall::new(100.0);
/// let mut state = PathState::new(1, 1.0);
/// state.set(state_keys::SPOT, 112.0);
///
/// payoff.reset();
/// payoff.on_event(&mut state);
///
/// assert_eq!(payoff.value(Currency::USD).amount(), 12.0);
/// ```
pub trait Payoff: Send + Sync + Clone {
    /// Process one path event such as a fixing, barrier check, or cashflow date.
    ///
    /// The engine passes a mutable [`PathState`] so payoffs can record
    /// diagnostic cashflows with [`PathState::add_cashflow`] or
    /// [`PathState::add_typed_cashflow`]. Payoffs should read named state
    /// variables from the state rather than assuming a raw process vector
    /// layout.
    fn on_event(&mut self, state: &mut PathState);

    /// Compute final payoff value in the specified currency (undiscounted).
    ///
    /// The returned [`Money`] is in payoff-date units. The generic engine
    /// multiplies it by the run-level `discount_factor` after this method
    /// returns.
    fn value(&self, currency: Currency) -> Money;

    /// Reset all path-local state before simulating the next path.
    ///
    /// Implementations must clear terminal fixings, barrier flags, accumulated
    /// averages, random thresholds, and any other state that should not leak
    /// between paths.
    fn reset(&mut self);

    /// Optional payoff-level discount factor hook.
    ///
    /// The generic [`crate::engine::McEngine`] still expects an explicit
    /// `discount_factor` argument and does not infer it from this method. This
    /// hook exists for specialized pricers that own discounting internally.
    fn discount_factor(&self) -> f64 {
        1.0
    }

    /// Optional hook invoked at the start of each path with access to RNG.
    ///
    /// Use this to draw per-path random variables such as default thresholds
    /// before the first event is processed. Implementations that draw here
    /// should also clear the drawn value in [`Payoff::reset`].
    fn on_path_start<R: RandomStream>(&mut self, _rng: &mut R) {}

    /// Whether this payoff consumes the per-step uniform random draw exposed via
    /// [`PathState::uniform_random`] (e.g. the Brownian-bridge barrier
    /// correction).
    ///
    /// Returns `false` by default. The engine only draws the per-step uniform
    /// when this is `true`, so the common case (European/Asian/lookback/basket
    /// payoffs that never read it) avoids ~one RNG draw per step per path. Note
    /// that enabling/disabling this changes a payoff's RNG stream, so the value
    /// is part of a payoff's reproducibility contract.
    fn needs_uniform_random(&self) -> bool {
        false
    }

    /// The largest grid step at which this payoff expects an event, if known.
    ///
    /// The engine validates that the time grid has at least this many steps
    /// before simulating. Without this check, a maturity or fixing step
    /// beyond the grid silently never fires: a European payoff keeps its
    /// 0.0 terminal spot and an Asian average quietly shrinks — both produce
    /// systematically wrong prices with no diagnostic.
    ///
    /// Return `None` (the default) when the payoff has no fixed event
    /// schedule.
    fn max_event_step(&self) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_state_creation() {
        let state = PathState::new(5, 0.5);
        assert_eq!(state.step, 5);
        assert_eq!(state.time, 0.5);
        assert_eq!(state.get("step"), Some(5.0));
        assert_eq!(state.get("time"), Some(0.5));
        let vars = state.vars();
        assert_eq!(vars.get(state_keys::STEP), Some(&5.0));
        assert_eq!(vars.get(state_keys::TIME), Some(&0.5));
    }

    #[test]
    fn test_path_state_vars() {
        let mut state = PathState::new(0, 0.0);
        state.set(state_keys::SPOT, 100.0);
        state.set(state_keys::VARIANCE, 0.04);
        state.set_indexed_spot(1, 120.0);

        assert_eq!(state.spot(), Some(100.0));
        assert_eq!(state.variance(), Some(0.04));
        assert_eq!(state.get("spot_1"), Some(120.0));
        assert_eq!(state.vars().get("spot_1"), Some(&120.0));

        let mut buf = HashMap::<&'static str, f64>::default();
        state.collect_vars(&mut buf);
        assert_eq!(buf.get("spot_1"), Some(&120.0));
        buf.clear();
        state.collect_vars(&mut buf);
        assert_eq!(buf.get("spot_1"), Some(&120.0));

        assert_eq!(state.get("nonexistent"), None);
        assert_eq!(state.get_or("nonexistent", 42.0), 42.0);
    }

    #[test]
    fn test_indexed_spot_inline_keys() {
        assert_eq!(state_keys::indexed_spot(0), "spot_0");
        assert_eq!(state_keys::indexed_spot(127), "spot_127");
        assert_eq!(state_keys::indexed_spot(128), "spot_128");
    }

    #[test]
    fn test_path_state_cashflows() {
        let mut state = PathState::new(1, 0.25);

        assert!(state.cashflows().is_empty());

        state.add_cashflow(0.25, 1000.0);
        state.add_cashflow(0.25, 500.0);

        assert_eq!(state.cashflows().len(), 2);
        assert_eq!(state.cashflows()[0], (0.25, 1000.0, CashflowType::Other));
        assert_eq!(state.cashflows()[1], (0.25, 500.0, CashflowType::Other));

        let cashflows = state.take_cashflows();
        assert_eq!(cashflows.len(), 2);
        assert_eq!(cashflows[0], (0.25, 1000.0, CashflowType::Other));
        assert_eq!(cashflows[1], (0.25, 500.0, CashflowType::Other));

        assert!(state.cashflows().is_empty());

        state.add_typed_cashflow(0.5, 2000.0, CashflowType::Interest);
        state.add_typed_cashflow(0.5, 100.0, CashflowType::Principal);
        assert_eq!(state.cashflows().len(), 2);
        assert_eq!(state.cashflows()[0], (0.5, 2000.0, CashflowType::Interest));
        assert_eq!(state.cashflows()[1], (0.5, 100.0, CashflowType::Principal));

        let mut drained = Vec::new();
        state.drain_cashflows(|time, amount, cf_type| drained.push((time, amount, cf_type)));
        assert_eq!(drained.len(), 2);
        assert!(state.cashflows().is_empty());

        state.add_cashflow(0.75, 50.0);
        assert_eq!(state.cashflows(), &[(0.75, 50.0, CashflowType::Other)]);
    }
}
