//! Convertible bond pricing model using Tsiveriotis-Zhang tree.
//!
//! Implements a hybrid debt-equity pricing model that:
//! 1. Uses `CashFlowBuilder` to generate the bond's coupon schedule
//! 2. Applies Tsiveriotis-Zhang tree decomposition to capture the equity conversion option
//!    while accounting for credit risk on the cash-only component.
//! 3. Handles call/put provisions and various conversion policies
//!
//! Public API:
//! - `price_convertible_bond`: Present value using selected tree type
//! - `calculate_convertible_greeks`: Tree-based Greeks and price (central differences)
//! - `calculate_parity`: Equity parity ratio
//! - `calculate_conversion_premium`: Conversion premium versus equity value
//! - `calculate_accrued_interest`: Accrued coupon interest as of valuation date

use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PriceId;
use finstack_quant_core::HashMap;
use finstack_quant_core::InputError;
use finstack_quant_core::{Error, Result};

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::{
    market_inputs::resolve_dividend_yield, ConversionEvent, ConversionPolicy, ConvertibleBond,
};
use crate::metrics::bump_discount_curve_parallel;
use crate::models::trees::tree_framework::map_date_to_step;
use crate::models::{single_factor_equity_state, EvolutionParams, TreeGreeks};

/// Compute the conversion value for any conversion policy given the spot price.
///
/// This standalone function handles all `ConversionPolicy` variants, including
/// `MandatoryVariable` with its three-regime variable delivery ratio. Used by both
/// the tree terminal/interior nodes and the at-maturity early-exit path.
///
/// For standard policies: `conversion_ratio * spot`.
/// For `MandatoryVariable`:
///   - `spot <= lower_price`: `(face / lower_price) * spot` (max shares, loss)
///   - `lower < spot <= upper`: `face` (variable ratio delivers par)
///   - `spot > upper_price`: `(face / upper_price) * spot` (min shares, capped)
pub(crate) fn compute_conversion_value(bond: &ConvertibleBond, spot: f64) -> Result<f64> {
    match &bond.conversion.policy {
        ConversionPolicy::MandatoryVariable {
            upper_conversion_price,
            lower_conversion_price,
            ..
        } => {
            if *lower_conversion_price <= 0.0 || *upper_conversion_price <= 0.0 {
                return Err(Error::Validation(format!(
                    "Conversion prices must be positive: lower={}, upper={}",
                    lower_conversion_price, upper_conversion_price
                )));
            }
            // Reject inverted bounds. Without this guard the three-regime payoff
            // below collapses degenerately (no `lower < spot <= upper` regime
            // can fire) and produces NaN-adjacent values that propagate
            // silently into PV. Data-entry inversion at trade capture is the
            // most likely source.
            if *lower_conversion_price > *upper_conversion_price {
                return Err(Error::Validation(format!(
                    "MandatoryVariable conversion bounds inverted: lower={lower_conversion_price} \
                     must be <= upper={upper_conversion_price}"
                )));
            }
            let face = bond.notional.amount();
            if spot <= *lower_conversion_price {
                Ok((face / lower_conversion_price) * spot)
            } else if spot <= *upper_conversion_price {
                Ok(face)
            } else {
                Ok((face / upper_conversion_price) * spot)
            }
        }
        _ => {
            let conversion_ratio =
                bond.effective_conversion_ratio()
                    .ok_or(Error::Input(InputError::NotFound {
                        id: "conversion_ratio_or_price".to_string(),
                    }))?;
            Ok(spot * conversion_ratio)
        }
    }
}

/// Tree model type selection for convertible bond pricing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertibleTreeType {
    /// Use binomial tree (CRR)
    Binomial(usize), // number of steps
    /// Use trinomial tree
    Trinomial(usize), // number of steps
}

impl Default for ConvertibleTreeType {
    fn default() -> Self {
        Self::Binomial(200)
    }
}

/// Convertible bond valuator implementing the TZ logic
pub(crate) struct ConvertibleBondValuator {
    /// Conversion ratio (shares per bond) - used for standard conversion policies.
    conversion_ratio: f64,
    /// Face value of the bond
    face_value: f64,
    /// Coupon cashflows mapped to tree steps
    coupon_map: HashMap<usize, f64>,
    /// Call prices mapped to tree steps (step -> price).
    /// For exercise periods, every step within the period maps to the call price.
    call_map: HashMap<usize, f64>,
    /// Put prices mapped to tree steps (step -> price).
    /// For exercise periods, every step within the period maps to the put price.
    put_map: HashMap<usize, f64>,
    /// Conversion policy
    conversion_policy: ConversionPolicy,
    /// Base date for time calculations
    base_date: Date,
    /// Day-count convention for time mapping in the tree.
    day_count: DayCount,
    day_count_frequency: Option<Tenor>,
    /// Conversion price per share (for soft-call trigger evaluation).
    conversion_price: f64,
    /// Optional soft-call trigger condition.
    soft_call_trigger: Option<super::SoftCallTrigger>,
    /// Per-step risk-free discount factors: `rf_step_dfs[i] = curve.df(t_{i+1}) / curve.df(t_i)`.
    /// Uses the full discount curve term structure instead of a flat rate.
    rf_step_dfs: Vec<f64>,
    /// Per-step risky discount factors (includes credit spread, adjusted for recovery).
    ///
    /// With recovery rate R:
    /// `risky_fwd_adj = risky_fwd * (1 - R) + rf_fwd * R`
    ///
    /// At R=0 this equals the raw credit-curve forward (zero-recovery TZ model).
    /// At R=1 this equals the risk-free forward (no credit effect).
    risky_step_dfs: Vec<f64>,
    /// Equity volatility (stored for soft-call trigger adjustment).
    volatility: f64,
    /// Bond maturity date (for date-to-step mapping in conversion policies).
    maturity: Date,
    /// Number of tree steps (for date-to-step mapping in conversion policies).
    num_steps: usize,
}

impl ConvertibleBondValuator {
    /// Create a new convertible bond valuator with full term structure discount factors.
    ///
    /// Unlike the flat-rate approach, this extracts per-step discount factors from the
    /// risk-free and credit curves, capturing the full shape of the yield curve.
    pub(crate) fn new(
        bond: &ConvertibleBond,
        cashflow_schedule: &CashFlowSchedule,
        time_to_maturity: f64,
        steps: usize,
        base_date: Date,
        market_context: &MarketContext,
        volatility: f64,
    ) -> Result<Self> {
        // Use effective conversion ratio (includes anti-dilution adjustments)
        let conversion_ratio = bond.effective_conversion_ratio().ok_or_else(|| {
            Error::internal("convertible tree pricer requires effective conversion ratio")
        })?;

        // Map cashflows to tree steps
        let day_count_frequency = bond
            .fixed_coupon
            .as_ref()
            .map(|coupon| coupon.freq)
            .or_else(|| bond.floating_coupon.as_ref().map(|coupon| coupon.freq));
        let day_count_ctx = DayCountContext {
            frequency: day_count_frequency,
            ..Default::default()
        };
        let dt = time_to_maturity / steps as f64;
        let mut time_steps = Vec::with_capacity(steps + 1);
        let mut step_dates = Vec::with_capacity(steps + 1);
        let total_calendar_days = (bond.maturity - base_date).whole_days();

        for i in 0..=steps {
            time_steps.push(i as f64 * dt);
            let offset_days = if i == steps {
                total_calendar_days
            } else {
                ((total_calendar_days as f64) * (i as f64 / steps as f64)).round() as i64
            };
            step_dates.push(base_date + time::Duration::days(offset_days));
        }

        // Process coupon cashflows (exclude reset-only events) using schedule day count
        let mut coupon_map: HashMap<usize, f64> = HashMap::default();
        for cf in cashflow_schedule.coupons() {
            if cf.date <= base_date {
                continue;
            }
            let bounded_step = map_date_to_step(
                base_date,
                cf.date,
                bond.maturity,
                steps,
                cashflow_schedule.day_count,
                day_count_ctx,
            )?;
            *coupon_map.entry(bounded_step).or_insert(0.0) += cf.amount.amount();
        }

        // Map call/put schedules to tree steps, supporting exercise periods.
        let mut call_map: HashMap<usize, f64> = HashMap::default();
        let mut put_map: HashMap<usize, f64> = HashMap::default();

        if let Some(ref call_put) = bond.call_put {
            for call in &call_put.calls {
                if call.end_date > base_date && call.start_date <= bond.maturity {
                    let floor_price = bond.notional.amount() * (call.price_pct_of_par / 100.0);
                    let start_step = map_date_to_step(
                        base_date,
                        call.start_date.max(base_date),
                        bond.maturity,
                        steps,
                        cashflow_schedule.day_count,
                        day_count_ctx,
                    )?;

                    // Exercise period: map all steps from start to end
                    let end_step = map_date_to_step(
                        base_date,
                        call.end_date.min(bond.maturity),
                        bond.maturity,
                        steps,
                        cashflow_schedule.day_count,
                        day_count_ctx,
                    )?;

                    let reference_curve = if let Some(spec) = &call.make_whole {
                        Some((
                            market_context.get_discount(&spec.reference_curve_id)?,
                            spec.spread_bps / 10_000.0,
                        ))
                    } else {
                        None
                    };

                    // For overlapping call windows (e.g., step-down calls), the issuer
                    // will select the *cheapest* call price available at each step.
                    for (s, &exercise_date) in step_dates
                        .iter()
                        .enumerate()
                        .take(end_step + 1)
                        .skip(start_step)
                    {
                        let call_price = if let Some((curve, spread)) = &reference_curve {
                            let mut pv_remaining = 0.0;
                            for cashflow in cashflow_schedule
                                .flows
                                .iter()
                                .filter(|cashflow| cashflow.date > exercise_date)
                            {
                                let df = curve.df_between_dates(exercise_date, cashflow.date)?;
                                let tau = curve.day_count().year_fraction(
                                    exercise_date,
                                    cashflow.date,
                                    finstack_quant_core::dates::DayCountContext::default(),
                                )?;
                                pv_remaining +=
                                    cashflow.amount.amount() * df * (-spread * tau).exp();
                            }
                            floor_price.max(pv_remaining)
                        } else {
                            floor_price
                        };
                        call_map
                            .entry(s)
                            .and_modify(|p| *p = p.min(call_price))
                            .or_insert(call_price);
                    }
                }
            }

            for put in &call_put.puts {
                if put.end_date > base_date && put.start_date <= bond.maturity {
                    let put_price = bond.notional.amount() * (put.price_pct_of_par / 100.0);
                    let start_step = map_date_to_step(
                        base_date,
                        put.start_date.max(base_date),
                        bond.maturity,
                        steps,
                        cashflow_schedule.day_count,
                        day_count_ctx,
                    )?;

                    let end_step = map_date_to_step(
                        base_date,
                        put.end_date.min(bond.maturity),
                        bond.maturity,
                        steps,
                        cashflow_schedule.day_count,
                        day_count_ctx,
                    )?;

                    // For overlapping put windows, the holder will select the *highest*
                    // put price available at each step.
                    for s in start_step..=end_step {
                        put_map
                            .entry(s)
                            .and_modify(|p| *p = p.max(put_price))
                            .or_insert(put_price);
                    }
                }
            }
        }

        // Derive conversion price from notional / ratio
        let conversion_price = if conversion_ratio > 0.0 {
            bond.notional.amount() / conversion_ratio
        } else {
            0.0
        };

        // ---- M1: Per-step discount factors from full term structure ----
        let rf_curve = market_context.get_discount(bond.discount_curve_id.as_str())?;
        let credit_curve = if let Some(credit_id) = &bond.credit_curve_id {
            if credit_id != &bond.discount_curve_id {
                Some(market_context.get_discount(credit_id.as_str())?)
            } else {
                None
            }
        } else {
            None
        };

        let recovery = match bond.recovery_rate {
            Some(r) if !r.is_finite() || !(0.0..=1.0).contains(&r) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Convertible bond {} has recovery_rate={r}; expected finite value in \
                     [0.0, 1.0] (was previously clamped silently, which masked invalid input)",
                    bond.id.as_str()
                )));
            }
            Some(r) => r,
            None => 0.0,
        };

        let mut rf_step_dfs = Vec::with_capacity(steps);
        let mut risky_step_dfs = Vec::with_capacity(steps);

        for i in 0..steps {
            let step_start = step_dates[i];
            let step_end = step_dates[i + 1];
            let rf_fwd = rf_curve.df_between_dates(step_start, step_end)?;
            rf_step_dfs.push(rf_fwd);

            if let Some(ref cc) = credit_curve {
                let raw_risky_fwd = cc.df_between_dates(step_start, step_end)?;
                // Blend risky and risk-free using recovery:
                //   adjusted = risky * (1 - R) + rf * R
                // At R=0: pure zero-recovery TZ model.
                // At R=1: cash component discounted at risk-free (no credit effect).
                let risky_fwd = raw_risky_fwd * (1.0 - recovery) + rf_fwd * recovery;
                risky_step_dfs.push(risky_fwd);
            } else {
                risky_step_dfs.push(rf_fwd);
            }
        }

        if let ConversionPolicy::MandatoryVariable {
            upper_conversion_price,
            lower_conversion_price,
            ..
        } = &bond.conversion.policy
        {
            if *lower_conversion_price <= 0.0 || *upper_conversion_price <= 0.0 {
                return Err(Error::Validation(format!(
                    "Conversion prices must be positive: lower={}, upper={}",
                    lower_conversion_price, upper_conversion_price
                )));
            }
            if *lower_conversion_price > *upper_conversion_price {
                return Err(Error::Validation(format!(
                    "MandatoryVariable conversion bounds inverted: lower={lower_conversion_price} \
                     must be <= upper={upper_conversion_price}"
                )));
            }
        }

        Ok(Self {
            conversion_ratio,
            face_value: bond.notional.amount(),
            coupon_map,
            call_map,
            put_map,
            conversion_policy: bond.conversion.policy.clone(),
            base_date,
            day_count: cashflow_schedule.day_count,
            day_count_frequency,
            conversion_price,
            soft_call_trigger: bond.soft_call_trigger.clone(),
            rf_step_dfs,
            risky_step_dfs,
            volatility,
            maturity: bond.maturity,
            num_steps: steps,
        })
    }

    /// Whether conversion is mandatory (forced) when allowed, regardless of optimality.
    ///
    /// For `MandatoryOn` and `MandatoryVariable` policies, the holder **must** convert
    /// at the specified date -- even if conversion value is below redemption value.
    /// This correctly models PERCS/DECS where holders bear downside equity risk.
    fn conversion_is_mandatory(&self) -> bool {
        matches!(
            self.conversion_policy,
            ConversionPolicy::MandatoryOn(_) | ConversionPolicy::MandatoryVariable { .. }
        )
    }

    /// Check if conversion is allowed at a given time step.
    ///
    /// Date-based policies (`MandatoryOn`, `Window`, `MandatoryVariable`) use
    /// `map_date_to_step` to find the nearest tree step, avoiding floating-point
    /// comparison issues that could cause conversion to never trigger.
    ///
    /// For `PriceTrigger`, we use a barrier approximation: the node spot price
    /// is compared against the trigger threshold.
    fn conversion_allowed(&self, step: usize, node_spot: f64) -> Result<bool> {
        let ctx = DayCountContext {
            frequency: self.day_count_frequency,
            ..Default::default()
        };
        let allowed = match &self.conversion_policy {
            ConversionPolicy::Voluntary => true,
            ConversionPolicy::MandatoryOn(date) => {
                // Map the mandatory date to its nearest tree step
                let target_step = map_date_to_step(
                    self.base_date,
                    *date,
                    self.maturity,
                    self.num_steps,
                    self.day_count,
                    ctx,
                )?;
                step == target_step
            }
            ConversionPolicy::Window { start, end } => {
                let start_step = map_date_to_step(
                    self.base_date,
                    *start,
                    self.maturity,
                    self.num_steps,
                    self.day_count,
                    ctx,
                )?;
                let end_step = map_date_to_step(
                    self.base_date,
                    *end,
                    self.maturity,
                    self.num_steps,
                    self.day_count,
                    ctx,
                )?;
                step >= start_step && step <= end_step
            }
            ConversionPolicy::UponEvent(event) => {
                // PriceTrigger uses barrier approximation in the tree.
                // QualifiedIpo / ChangeOfControl cannot be modeled in a tree
                // (they require external event probability); treated as no conversion.
                match event {
                    ConversionEvent::PriceTrigger {
                        threshold,
                        lookback_days: _,
                    } => {
                        // Barrier approximation: node spot must exceed threshold.
                        // The lookback_days would ideally require path-dependent modeling;
                        // here we use the instantaneous spot as a first-order approximation.
                        node_spot >= *threshold
                    }
                    ConversionEvent::QualifiedIpo | ConversionEvent::ChangeOfControl => false,
                }
            }
            ConversionPolicy::MandatoryVariable {
                conversion_date, ..
            } => {
                let target_step = map_date_to_step(
                    self.base_date,
                    *conversion_date,
                    self.maturity,
                    self.num_steps,
                    self.day_count,
                    ctx,
                )?;
                step == target_step
            }
        };
        Ok(allowed)
    }

    /// Compute the conversion value at a given node, accounting for variable delivery
    /// ratios under `MandatoryVariable` policies (PERCS/DECS/ACES).
    ///
    /// For standard policies, conversion value = conversion_ratio * spot.
    /// For `MandatoryVariable`:
    ///   - spot <= lower_price: max_ratio * spot = (face/lower_price) * spot (loss)
    ///   - lower_price < spot <= upper_price: face value (variable ratio delivers par)
    ///   - spot > upper_price: min_ratio * spot = (face/upper_price) * spot (capped upside)
    fn conversion_value(&self, spot: f64) -> f64 {
        match &self.conversion_policy {
            ConversionPolicy::MandatoryVariable {
                upper_conversion_price,
                lower_conversion_price,
                ..
            } => {
                if spot <= *lower_conversion_price {
                    (self.face_value / lower_conversion_price) * spot
                } else if spot <= *upper_conversion_price {
                    self.face_value
                } else {
                    (self.face_value / upper_conversion_price) * spot
                }
            }
            _ => spot * self.conversion_ratio,
        }
    }

    /// Get call price at a given step (if callable)
    fn call_price_at_step(&self, step: usize) -> Option<f64> {
        self.call_map.get(&step).copied()
    }

    /// Get put price at a given step (if puttable)
    fn put_price_at_step(&self, step: usize) -> Option<f64> {
        self.put_map.get(&step).copied()
    }

    /// Check if the soft-call trigger is satisfied, with adjustment for the
    /// multi-day observation window.
    ///
    /// The standard 20-of-30 observation window is approximated by raising the
    /// effective trigger level. Since the tree models a single spot per node
    /// (not the path over the observation window), we adjust the barrier upward
    /// to account for the probability of *sustaining* the level.
    ///
    /// ## Adjustment methodology
    ///
    /// The Broadie-Glasserman-Kou (1997) correction for discrete barrier
    /// monitoring shifts the barrier by `exp(beta * sigma * sqrt(dt))` where
    /// `beta = zeta(1/2) / sqrt(2*pi) ≈ 0.5826` and `dt` is the monitoring
    /// interval. That correction applies to a single discrete observation.
    ///
    /// For the "k-of-n days above" requirement, no closed-form correction
    /// exists. We use a heuristic that scales the BGK-style adjustment by the
    /// required fraction `k/n`, reflecting that higher required fractions make
    /// the trigger harder to satisfy. The `0.5826` constant is rounded to the
    /// exact BGK beta. This is intentionally conservative (slightly over-adjusts).
    ///
    /// ## Reference
    ///
    /// Broadie, M., Glasserman, P., & Kou, S. (1997). "A Continuity Correction
    /// for Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349.
    fn soft_call_triggered(&self, node_spot: f64) -> bool {
        match self.soft_call_trigger {
            Some(ref trigger) => {
                let nominal_trigger = self.conversion_price * (trigger.threshold_pct / 100.0);

                let required_fraction =
                    trigger.required_days_above as f64 / trigger.observation_days.max(1) as f64;

                // BGK β = −ζ(1/2) / √(2π), taken from the canonical
                // definition in the MC crate so the analytical and MC stacks
                // cannot drift apart. The single-observation BGK shift uses the
                // **per-observation monitoring interval** dt (daily monitoring =
                // 1 business day = 1/252y), NOT the full observation-window length
                // — using the window inflated the shift by ≈√(observation_days).
                // Scaled by `required_fraction` (k/n) for the sustained
                // "k-of-n days above" requirement (heuristic extension).
                const BGK_BETA: f64 =
                    finstack_quant_monte_carlo::barriers::corrections::GOBET_MIRI_BETA;
                const MONITORING_DT: f64 = 1.0 / 252.0;
                let adj = BGK_BETA * required_fraction * self.volatility * MONITORING_DT.sqrt();
                let effective_trigger = nominal_trigger * (1.0 + adj);

                node_spot >= effective_trigger
            }
            None => true,
        }
    }
}

/// Implementation of Tsiveriotis-Zhang tree pricing logic.
///
/// Uses per-step discount factors from the full term structure instead of
/// flat-rate discounting. The equity component is discounted at the risk-free
/// forward rate and the cash component at the recovery-adjusted risky forward
/// rate, both extracted step-by-step from the respective discount curves.
///
/// ## Credit model
///
/// The risky step discount factors are adjusted for recovery:
///
/// ```text
/// risky_fwd_adj = risky_fwd * (1 - R) + rf_fwd * R
/// ```
///
/// where `R` is the recovery rate (0.0 to 1.0). At R=0 this reduces to the
/// zero-recovery TZ model. Setting R=0.40 (ISDA standard for senior unsecured)
/// reflects that bondholders recover 40% of face value on default, reducing
/// the effective credit spread impact on the cash component.
struct TsiveriotisZhangEngine<'a> {
    valuator: &'a ConvertibleBondValuator,
    steps: usize,
    time_to_maturity: f64,
}

impl<'a> TsiveriotisZhangEngine<'a> {
    fn price(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        tree_type: ConvertibleTreeType,
    ) -> Result<(f64, f64)> {
        // Extract parameters
        let spot = *initial_vars
            .get("spot")
            .ok_or(Error::Input(InputError::NotFound {
                id: "spot price".to_string(),
            }))?;
        let volatility =
            *initial_vars
                .get("volatility")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "volatility".to_string(),
                }))?;
        let risk_free_rate =
            *initial_vars
                .get("interest_rate")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "interest_rate".to_string(),
                }))?;
        let dividend_yield =
            *initial_vars
                .get("dividend_yield")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "dividend_yield".to_string(),
                }))?;

        let dt = self.time_to_maturity / self.steps as f64;

        // Evolution parameters for the recombining tree.
        //
        // KNOWN LIMITATION (drift-discount mismatch):
        // The tree evolution uses a single short rate (instantaneous forward at t=0)
        // for the CRR/trinomial up/down factors and probabilities to preserve the
        // recombining tree structure. Backward induction uses per-step forward
        // discount factors from the full term structure. Using the short rate
        // (rather than the average zero rate to maturity) reduces the drift
        // mismatch but does not eliminate it for non-flat curves.
        //
        // A fully consistent implementation would require per-step evolution
        // parameters, which breaks standard CRR recombination.
        let params = match tree_type {
            ConvertibleTreeType::Binomial(_) => {
                EvolutionParams::equity_crr(volatility, risk_free_rate, dividend_yield, dt)?
            }
            ConvertibleTreeType::Trinomial(_) => {
                EvolutionParams::equity_trinomial(volatility, risk_free_rate, dividend_yield, dt)?
            }
        };

        // State tracking: (Total Value, Cash Component)
        let mut values: Vec<(f64, f64)> = Vec::with_capacity(2 * self.steps + 1);

        // Helper to get spot at (step, node).
        //
        // Trinomial node spot: for a recombining trinomial the recombination
        // identity is `up·down = middle²`, so the spot at `net` net up-moves
        // after `step` total moves depends only on `net` and equals
        //   S₀ · up^net · middle^(step − net).
        // The previous form `up^max(net,0) · down^max(-net,0)` silently
        // assumed `up·down = 1` (it dropped the middle factor entirely) and is
        // malformed for any trinomial whose middle factor is not 1. Using the
        // explicit `middle_factor` is correct for any recombining trinomial.
        let trinomial_middle = params.middle_factor.unwrap_or(1.0);
        let get_spot = |step: usize, node: usize| -> f64 {
            match tree_type {
                ConvertibleTreeType::Binomial(_) => {
                    let ups = node as i32;
                    let downs = step as i32 - node as i32;
                    spot * params.up_factor.powi(ups) * params.down_factor.powi(downs)
                }
                ConvertibleTreeType::Trinomial(_) => {
                    let net_moves = node as i32 - step as i32;
                    // `powi` accepts negative exponents (u^(-k) = 1/u^k), so
                    // this is correct for both up and down net moves.
                    spot * params.up_factor.powi(net_moves)
                        * trinomial_middle.powi(step as i32 - net_moves)
                }
            }
        };

        // 1. Terminal Step
        let num_nodes = match tree_type {
            ConvertibleTreeType::Binomial(n) => n + 1,
            ConvertibleTreeType::Trinomial(n) => 2 * n + 1,
        };

        let mandatory = self.valuator.conversion_is_mandatory();

        for i in 0..num_nodes {
            let node_spot = get_spot(self.steps, i);
            let conversion_val = self.valuator.conversion_value(node_spot);

            let coupon = self
                .valuator
                .coupon_map
                .get(&self.steps)
                .copied()
                .unwrap_or(0.0);
            let redemption_val = self.valuator.face_value;

            let can_convert = self.valuator.conversion_allowed(self.steps, node_spot)?;

            let (ex_coupon_total, ex_coupon_cash) = if can_convert && mandatory {
                // Mandatory conversion: holder must convert regardless of optimality.
                // For PERCS/DECS below the lower strike, this correctly reflects
                // the holder bearing equity downside risk.
                (conversion_val, 0.0)
            } else if can_convert && conversion_val > redemption_val {
                (conversion_val, 0.0)
            } else {
                (redemption_val, redemption_val)
            };

            // Coupon entitlement is independent of the exercise choice under
            // the public contract (there is no coupon-forfeiture flag). Make
            // the exercise decision ex-coupon, then add the date's coupon as
            // a cash component.
            values.push((ex_coupon_total + coupon, ex_coupon_cash + coupon));
        }

        // 2. Backward Induction. Double-buffer the value layers so each per-step
        // layer reuses one allocation (cleared) instead of allocating a fresh Vec.
        let mut next_values: Vec<(f64, f64)> = Vec::with_capacity(values.len());
        for step in (0..self.steps).rev() {
            let current_num_nodes = match tree_type {
                ConvertibleTreeType::Binomial(_) => step + 1,
                ConvertibleTreeType::Trinomial(_) => 2 * step + 1,
            };

            // Per-step discount factors from full term structure
            let df_rf = self.valuator.rf_step_dfs[step];
            let df_risky = self.valuator.risky_step_dfs[step];

            next_values.clear();

            for i in 0..current_num_nodes {
                let (exp_total, exp_cash) = match tree_type {
                    ConvertibleTreeType::Binomial(_) => {
                        let (v_up, c_up) = values[i + 1];
                        let (v_down, c_down) = values[i];

                        (
                            params.prob_up * v_up + params.prob_down * v_down,
                            params.prob_up * c_up + params.prob_down * c_down,
                        )
                    }
                    ConvertibleTreeType::Trinomial(_) => {
                        let (v_up, c_up) = values[i + 2];
                        let (v_mid, c_mid) = values[i + 1];
                        let (v_down, c_down) = values[i];

                        let pm = params.prob_middle.unwrap_or(0.0);
                        (
                            params.prob_up * v_up + pm * v_mid + params.prob_down * v_down,
                            params.prob_up * c_up + pm * c_mid + params.prob_down * c_down,
                        )
                    }
                };

                // TZ discounting: equity at risk-free, cash at risky
                let equity_part = (exp_total - exp_cash) * df_rf;
                let cash_part = exp_cash * df_risky;
                let continuation_total = equity_part + cash_part;
                let continuation_cash = cash_part;

                // Exercise decisions are ex-coupon. The coupon is added after
                // conversion/call/put resolution so no branch can overwrite it.
                let coupon = self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0);

                // Node decision logic
                let node_spot = get_spot(step, i);

                // 1. Conversion (uses variable delivery for MandatoryVariable)
                let conversion_val = self.valuator.conversion_value(node_spot);
                let can_convert = self.valuator.conversion_allowed(step, node_spot)?;

                let mut final_total = continuation_total;
                let mut final_cash = continuation_cash;

                if can_convert && mandatory {
                    // Mandatory conversion: forced regardless of optimality.
                    final_total = conversion_val;
                    final_cash = 0.0;
                } else if can_convert && conversion_val > final_total {
                    final_total = conversion_val;
                    final_cash = 0.0;
                }

                // 2. Call (Issuer minimizes value).
                //
                // Tsiveriotis-Zhang call rule:
                //   value = min(continuation, max(call_price, conversion_value))
                // The issuer calls only when doing so *reduces* the bond value,
                // i.e. when `continuation > value_if_called`, where
                // `value_if_called` is the holder's optimal response to a call:
                //   - convert (→ conversion_value, all-equity) iff conversion
                //     is permitted at this step AND conversion_value exceeds
                //     the call price;
                //   - otherwise accept the cash call price (all-cash).
                //
                // Critically the conversion branch must be gated on
                // `can_convert`: a previous form used `conversion_val` in the
                // cash/equity split unconditionally, which forced a conversion
                // (cash = 0) even at steps where conversion is not allowed,
                // corrupting both the value and the cash component that feeds
                // the next credit-risky discounting step.
                // Uses adjusted soft-call trigger with observation window correction.
                let call_allowed = self.valuator.soft_call_triggered(node_spot);

                if call_allowed {
                    if let Some(call_price) = self.valuator.call_price_at_step(step) {
                        // Holder converts in response to a call only if
                        // conversion is genuinely permitted here and is worth
                        // more than the cash call price.
                        let holder_converts = can_convert && conversion_val >= call_price;
                        let value_if_called = if holder_converts {
                            conversion_val
                        } else {
                            call_price
                        };

                        // Issuer calls iff it strictly lowers the bond value.
                        if final_total > value_if_called {
                            if holder_converts {
                                final_total = conversion_val;
                                final_cash = 0.0;
                            } else {
                                // Cash redemption at the call price: the whole
                                // payoff is a cash (credit-risky) component.
                                final_total = call_price;
                                final_cash = call_price;
                            }
                        }
                    }
                }

                // 3. Put (Holder maximizes value)
                if let Some(put_price) = self.valuator.put_price_at_step(step) {
                    if final_total < put_price {
                        final_total = put_price;
                        final_cash = final_total;
                    }
                }

                final_total += coupon;
                final_cash += coupon;

                next_values.push((final_total, final_cash));
            }
            std::mem::swap(&mut values, &mut next_values);
        }

        Ok(values[0])
    }
}

/// Resolved market data identifiers for Greek bumping.
struct ResolvedIds {
    spot_id: PriceId,
    vol_id: String,
}

/// Extracted equity market state.
struct EquityState {
    spot: f64,
    spot_scalar: finstack_quant_core::market_data::scalars::MarketScalar,
    volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
    time_to_maturity: f64,
    resolved_ids: ResolvedIds,
}

/// Extract equity market state from market context.
///
/// Uses **Act/365F** for all process/option time calculations (time_to_maturity,
/// vol surface lookup, drift estimation). This is deliberately decoupled from
/// the bond's coupon accrual day count, which can be 30/360 or other conventions.
fn extract_equity_state(
    bond: &ConvertibleBond,
    ctx: &MarketContext,
    as_of: Date,
    _accrual_day_count: DayCount,
) -> Result<EquityState> {
    let underlying_id = bond
        .underlying_equity_id
        .as_deref()
        .ok_or_else(|| Error::internal("convertible pricing requires underlying equity spot"))?;

    // Get spot price, preserving the original scalar variant for type-safe bumping
    let spot_scalar = ctx.get_price(underlying_id)?.clone();
    let spot = match &spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
            if money.currency() != bond.notional.currency() {
                return Err(Error::CurrencyMismatch {
                    expected: bond.notional.currency(),
                    actual: money.currency(),
                });
            }
            money.amount()
        }
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
    };

    let discount_curve = ctx.get_discount(bond.discount_curve_id.as_str())?;
    // Use Act/365F for process time (tree steps, vol lookups, curve DF queries).
    // This is standard for equity option models and ensures consistency with
    // discount curve time axis (which defaults to Act/365F).
    let process_dc = DayCount::Act365F;
    let time_to_maturity = process_dc.year_fraction(
        as_of,
        bond.maturity,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;

    // Use the short-rate (instantaneous forward at t=0) for tree drift rather
    // than the average zero rate to maturity. This better approximates the
    // local risk-neutral drift at each step when combined with the per-step
    // discount factors used in backward induction.
    //
    // Approximated as -ln(DF(epsilon))/epsilon with epsilon = 1/252 (~1 day).
    // Falls back to zero rate to maturity when TTM is very short.
    let risk_free_rate = if time_to_maturity > 0.0 {
        let next_day = as_of + time::Duration::days(1);
        let epsilon = process_dc.year_fraction(
            as_of,
            next_day,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let df_short = discount_curve.df_between_dates(as_of, next_day)?;
        if df_short > 0.0 {
            -df_short.ln() / epsilon
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Resolve volatility
    let mut vol_candidates: Vec<String> = Vec::new();
    if let Some(id) = bond.attributes.get_meta("vol_surface_id") {
        vol_candidates.push(id.to_string());
    }
    vol_candidates.push(format!("{}-VOL", underlying_id));
    if let Some(stripped) = underlying_id.strip_suffix("-SPOT") {
        vol_candidates.push(format!("{}-VOL", stripped));
    }
    let (volatility, resolved_vol_id) =
        resolve_volatility_with_id(ctx, &vol_candidates, time_to_maturity, spot)?;

    // Resolve dividend yield
    let dividend_yield = resolve_dividend_yield(ctx, bond)?;

    let resolved_ids = ResolvedIds {
        spot_id: underlying_id.into(),
        vol_id: resolved_vol_id,
    };

    Ok(EquityState {
        spot,
        spot_scalar,
        volatility,
        dividend_yield,
        risk_free_rate,
        time_to_maturity,
        resolved_ids,
    })
}

/// Resolve volatility and return both the value and the resolved ID.
fn resolve_volatility_with_id(
    ctx: &MarketContext,
    candidate_ids: &[String],
    time_to_maturity: f64,
    spot: f64,
) -> Result<(f64, String)> {
    let mut first_missing: Option<String> = None;

    for id in candidate_ids {
        match ctx.get_price(id) {
            Ok(finstack_quant_core::market_data::scalars::MarketScalar::Unitless(vol)) => {
                return Ok((*vol, id.clone()));
            }
            Ok(_) => {}
            Err(err) => {
                if matches!(err, Error::Input(InputError::NotFound { .. })) {
                    if first_missing.is_none() {
                        first_missing = Some(id.clone());
                    }
                } else {
                    return Err(err);
                }
            }
        }

        match ctx.get_surface(id) {
            Ok(surface) => {
                let vol = surface.value_clamped(time_to_maturity, spot);
                return Ok((vol, id.clone()));
            }
            Err(err) => {
                if matches!(err, Error::Input(InputError::NotFound { .. })) {
                    if first_missing.is_none() {
                        first_missing = Some(id.clone());
                    }
                    continue;
                }
                return Err(err);
            }
        }
    }

    let missing_id = first_missing.unwrap_or_else(|| "volatility".to_string());
    Err(Error::from(InputError::NotFound { id: missing_id }))
}

/// Aggregated data required for tree pricing
struct PricingInputs {
    cashflow_schedule: CashFlowSchedule,
    spot: f64,
    volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
    time_to_maturity: f64,
    resolved_ids: ResolvedIds,
    /// Original spot scalar from market context, preserved for type-safe bumping.
    spot_scalar: finstack_quant_core::market_data::scalars::MarketScalar,
}

/// Prepare all necessary inputs for pricing and greeks calculation.
fn prepare_for_pricing(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    as_of: Date,
) -> Result<PricingInputs> {
    let cashflow_schedule = build_convertible_schedule(bond)?;
    let day_count = cashflow_schedule.day_count;
    let eq = extract_equity_state(bond, market_context, as_of, day_count)?;

    Ok(PricingInputs {
        cashflow_schedule,
        spot: eq.spot,
        volatility: eq.volatility,
        dividend_yield: eq.dividend_yield,
        risk_free_rate: eq.risk_free_rate,
        time_to_maturity: eq.time_to_maturity,
        resolved_ids: eq.resolved_ids,
        spot_scalar: eq.spot_scalar,
    })
}

/// Internal pricing function that reuses pre-computed `PricingInputs`.
///
/// Avoids redundant `prepare_for_pricing` when the caller already has the inputs
/// (e.g., `calculate_convertible_greeks` for the base price).
fn price_convertible_bond_with_inputs(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    inputs: &PricingInputs,
    tree_type: ConvertibleTreeType,
    as_of: Date,
) -> Result<Money> {
    if as_of > bond.maturity {
        return Ok(Money::new(0.0, bond.notional.currency()));
    }

    if inputs.time_to_maturity <= 0.0 {
        let maturity_coupon: f64 = inputs
            .cashflow_schedule
            .coupons()
            .filter(|cf| cf.date == bond.maturity)
            .map(|cf| cf.amount.amount())
            .sum();

        let redemption_value = bond.notional.amount() + maturity_coupon;
        let conversion_value = compute_conversion_value(bond, inputs.spot)?;

        let is_mandatory = matches!(
            bond.conversion.policy,
            ConversionPolicy::MandatoryOn(_) | ConversionPolicy::MandatoryVariable { .. }
        );
        let can_convert = match &bond.conversion.policy {
            ConversionPolicy::Voluntary => true,
            ConversionPolicy::MandatoryOn(date) => *date == bond.maturity,
            ConversionPolicy::MandatoryVariable {
                conversion_date, ..
            } => *conversion_date == bond.maturity,
            ConversionPolicy::Window { start, end } => {
                *start <= bond.maturity && bond.maturity <= *end
            }
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger { threshold, .. }) => {
                inputs.spot >= *threshold
            }
            ConversionPolicy::UponEvent(
                ConversionEvent::QualifiedIpo | ConversionEvent::ChangeOfControl,
            ) => false,
        };
        let payoff = if is_mandatory && can_convert {
            conversion_value
        } else if can_convert {
            redemption_value.max(conversion_value)
        } else {
            redemption_value
        };

        return Ok(Money::new(payoff, bond.notional.currency()));
    }

    let steps = match tree_type {
        ConvertibleTreeType::Binomial(n) | ConvertibleTreeType::Trinomial(n) => n,
    };

    let valuator = ConvertibleBondValuator::new(
        bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        steps,
        as_of,
        market_context,
        inputs.volatility,
    )?;

    let initial_vars = single_factor_equity_state(
        inputs.spot,
        inputs.risk_free_rate,
        inputs.dividend_yield,
        inputs.volatility,
    );

    let engine = TsiveriotisZhangEngine {
        valuator: &valuator,
        steps,
        time_to_maturity: inputs.time_to_maturity,
    };

    let (pv_amount, _) = engine.price(initial_vars, tree_type)?;

    Ok(Money::new(pv_amount, bond.notional.currency()))
}

/// Main pricing function for convertible bonds
pub fn price_convertible_bond(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    tree_type: ConvertibleTreeType,
    as_of: Date,
) -> Result<Money> {
    if as_of > bond.maturity {
        return Ok(Money::new(0.0, bond.notional.currency()));
    }
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    price_convertible_bond_with_inputs(bond, market_context, &inputs, tree_type, as_of)
}

/// Calculate Greeks for a convertible bond using central finite differences.
///
/// All Greeks use full repricing with bumped market contexts to ensure consistency
/// with the full term structure discounting (M1). Each bump correctly propagates
/// through the entire pricing pipeline including per-step discount factor extraction.
///
/// # Greek Definitions
///
/// - **Delta**: `(P(S+h) - P(S-h)) / (2h)` where `h = bump_pct * S`
/// - **Gamma**: `(P(S+h) - 2*P(S) + P(S-h)) / h^2`
/// - **Vega**: `(P(σ+0.01) - P(σ-0.01)) / (vol_up - vol_down) * 0.01` — per 1% absolute vol move
/// - **Rho**: `(P(r+1bp) - P(r-1bp)) / 2` — per 1bp parallel curve shift (bp-count)
/// - **Theta**: `P(t+1d) - P(t)` — change per calendar day
pub fn calculate_convertible_greeks(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    tree_type: ConvertibleTreeType,
    bump_size: Option<f64>,
    as_of: Date,
) -> Result<TreeGreeks> {
    let bump_pct = bump_size.unwrap_or(0.01);

    // Resolve market data and compute base price in one pass.
    // The base price is computed inline to avoid a second prepare_for_pricing call
    // (which would duplicate cashflow schedule build and market data resolution).
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    let base_price =
        price_convertible_bond_with_inputs(bond, market_context, &inputs, tree_type, as_of)?;

    let mut greeks = TreeGreeks {
        price: base_price.amount(),
        delta: 0.0,
        gamma: 0.0,
        vega: 0.0,
        theta: 0.0,
        rho: 0.0,
    };

    // ---- Delta & Gamma: bump equity spot (central differences) ----
    let h_spot = bump_pct * inputs.spot;
    if h_spot > 0.0 {
        let bump_scalar = |amount: f64| -> finstack_quant_core::market_data::scalars::MarketScalar {
            match &inputs.spot_scalar {
                finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                    finstack_quant_core::market_data::scalars::MarketScalar::Price(
                        finstack_quant_core::money::Money::new(amount, money.currency()),
                    )
                }
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(_) => {
                    finstack_quant_core::market_data::scalars::MarketScalar::Unitless(amount)
                }
            }
        };

        let market_up = market_context.clone().insert_price(
            inputs.resolved_ids.spot_id.as_str(),
            bump_scalar(inputs.spot + h_spot),
        );
        let market_down = market_context.clone().insert_price(
            inputs.resolved_ids.spot_id.as_str(),
            bump_scalar(inputs.spot - h_spot),
        );

        let price_up = price_convertible_bond(bond, &market_up, tree_type, as_of)?.amount();
        let price_down = price_convertible_bond(bond, &market_down, tree_type, as_of)?.amount();

        greeks.delta = (price_up - price_down) / (2.0 * h_spot);
        greeks.gamma = (price_up - 2.0 * greeks.price + price_down) / (h_spot * h_spot);
    }

    // ---- Vega: bump volatility (B1: central differences) ----
    {
        let h_vol = 0.01; // 1% absolute
        let vol_down = (inputs.volatility - h_vol).max(1e-6); // Guard against negative vol
        let vol_up = inputs.volatility + h_vol;
        let actual_width = vol_up - vol_down; // May differ from 2*h_vol when clamped

        use finstack_quant_core::market_data::scalars::MarketScalar;
        let market_vol_up = market_context
            .clone()
            .insert_price(&inputs.resolved_ids.vol_id, MarketScalar::Unitless(vol_up));
        let market_vol_down = market_context.clone().insert_price(
            &inputs.resolved_ids.vol_id,
            MarketScalar::Unitless(vol_down),
        );

        let price_vol_up = price_convertible_bond(bond, &market_vol_up, tree_type, as_of)?.amount();
        let price_vol_down =
            price_convertible_bond(bond, &market_vol_down, tree_type, as_of)?.amount();

        // Vega per 1% vol move: central difference with actual bump width.
        // (P_up - P_down) / actual_width gives per-unit-vol sensitivity;
        // multiply by 0.01 to convert to "per 1% absolute vol move" convention.
        // When bumps are symmetric (actual_width == 0.02), this simplifies to
        // (P_up - P_down) / 2.0 as expected.
        greeks.vega = (price_vol_up - price_vol_down) / actual_width * 0.01;
    }

    // ---- Rho: bump discount curve (B2: central differences) ----
    {
        let h_rate = 1.0; // 1bp in bp-count units (BumpSpec::parallel_bp convention)
        let market_rate_up =
            bump_discount_curve_parallel(market_context, &bond.discount_curve_id, h_rate)?;
        let market_rate_down =
            bump_discount_curve_parallel(market_context, &bond.discount_curve_id, -h_rate)?;

        let price_rate_up =
            price_convertible_bond(bond, &market_rate_up, tree_type, as_of)?.amount();
        let price_rate_down =
            price_convertible_bond(bond, &market_rate_down, tree_type, as_of)?.amount();

        // Rho per 1bp: central difference
        greeks.rho = (price_rate_up - price_rate_down) / 2.0;
    }

    // ---- Theta: 1-day roll (forward difference), reported per calendar day ----
    {
        if inputs.time_to_maturity > 1.0 / 365.25 {
            if let Some(next_day) = as_of.next_day() {
                // `roll_forward(1)` realizes one day of forwards on every
                // curve (discount curves renormalize by DF(1d), hazard
                // curves preserve hazard rates via conditional survival), so
                // theta = price(rolled, t+1d) - price(t) captures both carry
                // and roll-down. See module documentation
                // (realized-forward roll semantics).
                //
                // A roll can fail when a curve is too sparse to retain ≥ 2
                // knots after the shift; in that case we fall back to a
                // no-roll reprice. Because the pricer discounts relative to
                // `as_of`, the fallback yields nearly the same discounting
                // effect, but leaves curve base dates anchored at `t`.
                let rolled_market = match market_context.roll_forward(1) {
                    Ok(m) => m,
                    Err(_) => market_context.clone(),
                };
                let fwd_price = price_convertible_bond(bond, &rolled_market, tree_type, next_day)?;
                // Theta = P(t+1d) - P(t), reported as change per calendar day.
                greeks.theta = fwd_price.amount() - greeks.price;
            }
        }
    }

    Ok(greeks)
}

/// Build the convertible bond cashflow schedule using common builder flow.
pub(crate) fn build_convertible_schedule(bond: &ConvertibleBond) -> Result<CashFlowSchedule> {
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(bond.notional, bond.issue_date, bond.maturity);
    if let Some(fixed_spec) = &bond.fixed_coupon {
        let _ = builder.fixed_cf(fixed_spec.clone());
    }
    if let Some(floating_spec) = &bond.floating_coupon {
        let _ = builder.floating_cf(floating_spec.clone());
    }
    builder.build_with_curves(None)
}

/// Calculate convertible bond parity
pub fn calculate_parity(bond: &ConvertibleBond, current_spot: f64) -> f64 {
    let Some(conversion_ratio) = bond.effective_conversion_ratio() else {
        return 0.0;
    };

    (current_spot * conversion_ratio) / bond.notional.amount()
}

/// Calculate conversion premium
pub fn calculate_conversion_premium(
    bond_price: f64,
    current_spot: f64,
    conversion_ratio: f64,
) -> f64 {
    let conversion_value = current_spot * conversion_ratio;
    if conversion_value > 0.0 {
        (bond_price / conversion_value) - 1.0
    } else {
        0.0
    }
}

/// Compute the settlement date for a convertible bond.
///
/// If `settlement_days` is set, adds that many weekdays to `as_of`.
/// Otherwise returns `as_of` unchanged.
pub fn settlement_date(bond: &ConvertibleBond, as_of: Date) -> Date {
    match bond.settlement_days {
        Some(days) if days > 0 => as_of.add_weekdays(days as i32),
        _ => as_of,
    }
}

/// Calculate accrued interest for a convertible bond.
///
/// Accrued interest is computed as of the **settlement date** (trade date +
/// `settlement_days` business days). If `settlement_days` is not set, `as_of`
/// is used directly.
///
/// Finds the accrual period containing the settlement date from the cashflow
/// schedule and computes the pro-rata portion of the coupon that has accrued.
///
/// Returns 0.0 for zero-coupon convertibles or if the date is outside all
/// accrual periods.
pub fn calculate_accrued_interest(bond: &ConvertibleBond, as_of: Date) -> Result<f64> {
    if bond.fixed_coupon.is_none() && bond.floating_coupon.is_none() {
        return Ok(0.0); // Zero-coupon
    }

    let settle = settlement_date(bond, as_of);

    let schedule = build_convertible_schedule(bond)?;
    let coupons: Vec<_> = schedule.coupons().collect();

    if coupons.is_empty() {
        return Ok(0.0);
    }

    // ACT/ACT (ISMA) requires the coupon frequency and reference period; without
    // them year_fraction errors (MissingFrequencyForActActIsma). Propagate that
    // error rather than silently treating accrued as zero — which would corrupt
    // the dirty/clean price and the OAS target for ISMA-day-count convertibles.
    let frequency = bond
        .fixed_coupon
        .as_ref()
        .map(|c| c.freq)
        .or_else(|| bond.floating_coupon.as_ref().map(|c| c.freq));

    let mut period_start = bond.issue_date;
    for cf in &coupons {
        let period_end = cf.date;
        if settle >= period_start && settle < period_end {
            let dc_ctx = finstack_quant_core::dates::DayCountContext {
                frequency,
                coupon_period: Some((period_start, period_end)),
                ..Default::default()
            };
            let period_yf = schedule
                .day_count
                .year_fraction(period_start, period_end, dc_ctx)?;
            let accrued_yf = schedule
                .day_count
                .year_fraction(period_start, settle, dc_ctx)?;

            if period_yf > 0.0 {
                let fraction = accrued_yf / period_yf;
                return Ok(cf.amount.amount() * fraction);
            }
            return Ok(0.0);
        }
        period_start = period_end;
    }

    Ok(0.0)
}

// ========================= REGISTRY PRICER =========================

/// Registry pricer for Convertible Bond using Tsiveriotis-Zhang tree-based pricing.
pub(crate) struct ConvertibleTreePricer;

impl ConvertibleTreePricer {
    /// Create a new convertible bond tree pricer.
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for ConvertibleTreePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for ConvertibleTreePricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::Convertible,
            crate::pricer::ModelKey::Tree,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        let convertible = instrument
            .as_any()
            .downcast_ref::<crate::instruments::fixed_income::convertible::ConvertibleBond>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::Convertible,
                    instrument.key(),
                )
            })?;

        let pv = price_convertible_bond(convertible, market, ConvertibleTreeType::default(), as_of)
            .map_err(|e| {
                crate::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    crate::pricer::PricingErrorContext::default(),
                )
            })?;

        Ok(crate::results::ValuationResult::stamped(
            convertible.id(),
            as_of,
            pv,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
    use crate::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::Month;

    fn create_test_bond() -> ConvertibleBond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let conversion_spec = ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        };

        let fixed_coupon = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        ConvertibleBond {
            id: "TEST_CONVERTIBLE".to_string().into(),
            notional: Money::new(1000.0, Currency::USD),
            issue_date: issue,
            maturity,
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            settlement_days: None,
            recovery_rate: None,
            conversion: conversion_spec,
            underlying_equity_id: Some("AAPL".to_string()),
            call_put: None,
            soft_call_trigger: None,
            fixed_coupon: Some(fixed_coupon),
            floating_coupon: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        }
    }

    fn create_test_market_context() -> MarketContext {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("should succeed");

        MarketContext::new()
            .insert(discount_curve)
            .insert_price("AAPL", MarketScalar::Unitless(150.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
    }

    #[test]
    fn coupon_on_valuation_date_is_not_added_to_tree_step_zero() {
        let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let market = create_test_market_context();
        let bond = create_test_bond();
        let inputs = prepare_for_pricing(&bond, &market, as_of).expect("pricing inputs");
        let valuator = ConvertibleBondValuator::new(
            &bond,
            &inputs.cashflow_schedule,
            inputs.time_to_maturity,
            50,
            as_of,
            &market,
            inputs.volatility,
        )
        .expect("valuator");

        assert!(
            !valuator.coupon_map.contains_key(&0),
            "coupon dated exactly on as_of must not be added at tree step 0"
        );
    }

    #[test]
    fn test_convertible_bond_parity() {
        let bond = create_test_bond();
        let parity = calculate_parity(&bond, 150.0);
        assert!((parity - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_convertible_bond_pricing() {
        let bond = create_test_bond();
        let market_context = create_test_market_context();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let price = price_convertible_bond(
            &bond,
            &market_context,
            ConvertibleTreeType::Binomial(50),
            as_of,
        );

        assert!(price.is_ok());
        let price = price.expect("should succeed");

        let conversion_value = 150.0 * 10.0;
        assert!(price.amount() >= conversion_value);
        assert!(price.amount() > 1000.0 && price.amount() < 2000.0);
    }

    #[test]
    fn test_convertible_pricing_at_maturity_uses_payoff() {
        let bond = create_test_bond();
        let market_context = create_test_market_context();
        let as_of = bond.maturity;

        let price = price_convertible_bond(
            &bond,
            &market_context,
            ConvertibleTreeType::Binomial(10),
            as_of,
        )
        .expect("should price");

        let conversion_value = 150.0 * 10.0;
        assert!((price.amount() - conversion_value).abs() < 1e-6);
    }

    #[test]
    fn test_convertible_greeks_calculation() {
        let bond = create_test_bond();
        let market_context = create_test_market_context();

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let greeks = calculate_convertible_greeks(
            &bond,
            &market_context,
            ConvertibleTreeType::Binomial(50),
            Some(0.01),
            as_of,
        );

        assert!(greeks.is_ok());
        let greeks = greeks.expect("should succeed");

        assert!(greeks.delta > 0.0);
        assert!(greeks.gamma >= -1e-6);
        assert!(greeks.price > 1000.0);
    }

    /// Item 8 regression: theta must roll the discount curve to `t+1d`, not
    /// reprice at `t+1d` against the curve still anchored at `t`.
    ///
    /// On a non-flat curve, rolling the curve forward changes the discount
    /// factors (roll-down). The pre-fix theta repriced at the next day with
    /// the *same* market context, so it omitted curve roll-down entirely. This
    /// test computes the no-roll theta explicitly and asserts the reported
    /// theta — which now rolls the curve — differs from it on a steep curve.
    #[test]
    fn theta_rolls_the_discount_curve() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let as_of = issue;
        let bond = create_test_bond();

        // A steeply-sloped discount curve so curve roll-down is material.
        let steep_curve = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([
                (0.0, 1.0),
                (0.5, 0.95),
                (1.0, 0.88),
                (5.0, 0.55),
                (10.0, 0.30),
            ])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("steep curve");
        let market = MarketContext::new()
            .insert(steep_curve)
            .insert_price("AAPL", MarketScalar::Unitless(150.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

        let tree = ConvertibleTreeType::Binomial(80);
        let greeks =
            calculate_convertible_greeks(&bond, &market, tree, Some(0.01), as_of).expect("greeks");
        assert!(greeks.theta.is_finite(), "theta must be finite");

        // No-roll theta: reprice at t+1d with the SAME (un-rolled) market —
        // exactly the pre-fix behavior.
        let next_day = as_of.next_day().expect("next day");
        let base_price = greeks.price;
        let no_roll_price = price_convertible_bond(&bond, &market, tree, next_day)
            .expect("no-roll price")
            .amount();
        let no_roll_theta = no_roll_price - base_price;

        // The reported theta rolls the curve, so on this steep curve it must
        // differ from the no-roll theta.
        assert!(
            (greeks.theta - no_roll_theta).abs() > 1e-6,
            "theta {} should differ from the no-roll theta {no_roll_theta} on a \
             steep curve — curve roll-down must be included",
            greeks.theta
        );
    }

    #[test]
    fn test_accrued_interest() {
        let bond = create_test_bond();
        // Mid-period: ~3 months into a 6-month coupon period
        let mid = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let accrued = calculate_accrued_interest(&bond, mid).expect("should compute");
        // ~half of semi-annual coupon (5%/2 * 1000 = 25, half ~ 12.5)
        assert!(accrued > 5.0 && accrued < 20.0, "accrued = {}", accrued);
    }

    #[test]
    fn test_mandatory_conversion_forced_at_loss() {
        // DECS/PERCS: mandatory conversion even when conversion_value < redemption.
        // Spot=50, ratio=10, notional=1000 → conversion_value=500 < 1000.
        // Mandatory bond at maturity should price at conversion value, not redemption.
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let mut bond = create_test_bond();
        bond.conversion.policy = ConversionPolicy::MandatoryOn(maturity);

        // Market with OTM spot: conversion_value = 50 * 10 = 500 < 1000 face
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("should succeed");

        let market = MarketContext::new()
            .insert(discount_curve)
            .insert_price("AAPL", MarketScalar::Unitless(50.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

        // At maturity: forced conversion at loss
        let price_at_mat =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(10), maturity)
                .expect("should price");

        // conversion_value = 50 * 10 = 500 (must convert, can't choose 1000 redemption)
        assert!(
            (price_at_mat.amount() - 500.0).abs() < 1.0,
            "Mandatory at maturity should force conversion: got {}",
            price_at_mat.amount()
        );

        // Before maturity: should be below straight bond floor due to forced conversion risk
        let price_before =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), issue)
                .expect("should price");

        assert!(
            price_before.amount() < 1000.0,
            "Mandatory OTM bond should price below par: got {}",
            price_before.amount()
        );
    }

    /// Item 2 regression: the call branch must not force a conversion at a
    /// node where conversion is *not* permitted.
    ///
    /// Construct a callable bond whose conversion is gated on
    /// `ChangeOfControl` — an event the tree cannot model, so
    /// `conversion_allowed` is `false` at every node. With a very high equity
    /// spot the conversion *value* (ratio·spot) is enormous, but the holder
    /// can never actually convert. The bond is economically a callable
    /// straight bond: its value is capped by the call price and must stay far
    /// below the conversion value.
    ///
    /// The pre-fix code used `conversion_val` in the call branch's cash/equity
    /// split unconditionally, so at a call node it set `(conversion_val, 0)` —
    /// forcing a conversion that is not allowed and inflating the price to the
    /// conversion value. The fix gates the conversion response on
    /// `can_convert`, so the called bond correctly redeems in cash.
    #[test]
    fn call_branch_does_not_force_disallowed_conversion() {
        use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
        use crate::instruments::fixed_income::convertible::ConversionEvent;

        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let as_of = issue;

        let mut bond = create_test_bond();
        // Conversion only on a change-of-control — never modellable in the
        // tree, so `conversion_allowed` is false at every node.
        bond.conversion.policy = ConversionPolicy::UponEvent(ConversionEvent::ChangeOfControl);
        // Callable for the whole life at 102% of par.
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: issue,
                end_date: maturity,
                price_pct_of_par: 102.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });

        // Very high spot: conversion value (ratio 10 × spot 5000 = 50,000) is
        // ~50× the 1,000 face. A callable non-convertible bond must ignore it.
        let base_date = issue;
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("curve");
        let market = MarketContext::new()
            .insert(discount_curve)
            .insert_price("AAPL", MarketScalar::Unitless(5000.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

        let price =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(60), as_of)
                .expect("should price");

        let conversion_value = 10.0 * 5000.0; // ratio × spot = 50,000
        let call_price = 1000.0 * 1.02; // 102% of par = 1,020

        // The bond must be valued as a callable straight bond: nowhere near
        // the conversion value, and capped around the call price (plus the
        // PV of coupons until the call).
        assert!(
            price.amount() < conversion_value / 10.0,
            "callable non-convertible bond priced at {} — far too close to the \
             conversion value {conversion_value}; the call branch is forcing a \
             disallowed conversion",
            price.amount()
        );
        // Sanity: it should be a sensible callable-bond value, near the call
        // price (the issuer calls the deep-premium bond), well under 2× par.
        assert!(
            price.amount() > 0.0 && price.amount() < 2.0 * call_price,
            "callable straight-bond value out of range: {}",
            price.amount()
        );
    }

    #[test]
    fn test_thirty_360_day_count_corporate_convention() {
        // Verify that 30/360 day count (US corporate standard) works correctly.
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let conversion_spec = ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: super::super::AntiDilutionPolicy::None,
            dividend_adjustment: super::super::DividendAdjustment::None,
            dilution_events: Vec::new(),
        };

        let fixed_coupon = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Thirty360, // US corporate convention
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let bond = ConvertibleBond {
            id: "TEST_30360".to_string().into(),
            notional: Money::new(1000.0, Currency::USD),
            issue_date: issue,
            maturity,
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            settlement_days: None,
            recovery_rate: None,
            conversion: conversion_spec,
            underlying_equity_id: Some("AAPL".to_string()),
            call_put: None,
            soft_call_trigger: None,
            fixed_coupon: Some(fixed_coupon),
            floating_coupon: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        };

        let market = create_test_market_context();
        let as_of = issue;

        let price =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), as_of)
                .expect("30/360 should price successfully");

        // Same economics as Act365F, should be in similar range
        let conversion_value = 150.0 * 10.0;
        assert!(price.amount() >= conversion_value);
        assert!(
            price.amount() > 1000.0 && price.amount() < 2000.0,
            "30/360 price out of range: {}",
            price.amount()
        );

        // Verify accrued interest works with 30/360
        let mid = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let accrued = calculate_accrued_interest(&bond, mid).expect("should compute");
        assert!(
            accrued > 5.0 && accrued < 20.0,
            "30/360 accrued should be reasonable: {}",
            accrued
        );
    }

    /// Item 11 regression: the trinomial node-spot grid must be built with the
    /// proper middle factor, `S₀ · up^net · middle^(step − net)`.
    ///
    /// For a recombining trinomial the recombination identity is
    /// `up·down = middle²`. The previous `up^max(net,0)·down^max(-net,0)` form
    /// dropped the middle factor and is only valid when `up·down = 1`. With
    /// the corrected formula the trinomial spot grid is well-formed, so a
    /// trinomial price must converge to the binomial price for the same bond
    /// (both are consistent lattice discretizations of the same process).
    #[test]
    fn trinomial_spot_grid_well_formed_matches_binomial() {
        let bond = create_test_bond();
        let market = create_test_market_context();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let binomial =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(400), as_of)
                .expect("binomial price")
                .amount();
        let trinomial =
            price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(400), as_of)
                .expect("trinomial price")
                .amount();

        // Both lattices discretize the same process; with 400 steps they must
        // agree closely. A malformed trinomial grid would diverge sharply.
        let rel_diff = (binomial - trinomial).abs() / binomial.max(1.0);
        assert!(
            rel_diff < 0.01,
            "trinomial price {trinomial} should match binomial {binomial} \
             within 1% (rel diff {rel_diff:.4}); a malformed spot grid would diverge"
        );
    }

    #[test]
    fn mandatory_variable_inverted_bounds_rejected_at_pricing() {
        // Data-entry inversion: lower > upper. Without the new guard, the
        // three-regime payoff in compute_conversion_value would silently fall
        // into the wrong branch and produce non-monotone PV. Pricing must
        // reject up front with a Validation error naming both bounds.
        let mut bond = create_test_bond();
        let conversion_date =
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        bond.conversion.policy = ConversionPolicy::MandatoryVariable {
            conversion_date,
            upper_conversion_price: 80.0,  // intentionally < lower
            lower_conversion_price: 120.0, // intentionally > upper
        };

        let market = create_test_market_context();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let err = price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), as_of)
            .expect_err("inverted bounds must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("inverted") && msg.contains("120") && msg.contains("80"),
            "error must name the inverted bounds, got: {msg}"
        );
    }

    #[test]
    fn mandatory_variable_inverted_bounds_rejected_in_compute_conversion_value() {
        // Direct call site (used at-maturity early-exit and reachable from
        // greeks recomputation).
        let mut bond = create_test_bond();
        let conversion_date =
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        bond.conversion.policy = ConversionPolicy::MandatoryVariable {
            conversion_date,
            upper_conversion_price: 50.0,
            lower_conversion_price: 200.0,
        };
        let err =
            compute_conversion_value(&bond, 100.0).expect_err("inverted bounds must be rejected");
        assert!(format!("{err}").contains("inverted"));
    }

    /// Regression test: recovery_rate must be validated explicitly, not
    /// silently clamped to [0.0, 1.0]. Out-of-range or non-finite values
    /// previously masked invalid inputs (e.g., a typo of 1.5 producing a
    /// silently-changed 1.0 PV).
    #[test]
    fn convertible_recovery_rate_out_of_bounds_errors() {
        let mut bond = create_test_bond();
        let market = create_test_market_context();
        let as_of = Date::from_calendar_date(2025, Month::June, 1).expect("valid date");
        let tree_type = ConvertibleTreeType::Binomial(50);

        // Above 1.0 — previously clamped to 1.0, now rejected.
        bond.recovery_rate = Some(1.5);
        let err = price_convertible_bond(&bond, &market, tree_type, as_of)
            .expect_err("recovery_rate=1.5 must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("recovery_rate") && msg.contains("TEST_CONVERTIBLE"),
            "error must mention recovery_rate and bond id; got: {msg}"
        );

        // Negative — previously clamped to 0.0, now rejected.
        bond.recovery_rate = Some(-0.1);
        let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
            .expect_err("negative recovery_rate must be rejected");

        // NaN — previously clamped to 0.0, now rejected.
        bond.recovery_rate = Some(f64::NAN);
        let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
            .expect_err("NaN recovery_rate must be rejected");

        // None — backwards-compat: still treated as 0.0 (no recovery).
        bond.recovery_rate = None;
        let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
            .expect("None recovery_rate must remain valid (defaults to 0.0)");
    }
}
