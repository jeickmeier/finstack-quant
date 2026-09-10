//! Convertible contract mapping and node exercise decisions.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::convertible::{
    ConversionEvent, ConversionPolicy, ConvertibleBond, SoftCallTrigger,
};
use finstack_quant_models::trees::tree_framework::map_date_to_step;

/// Convertible bond valuator implementing the TZ logic
pub(super) struct ConvertibleBondValuator {
    /// Conversion ratio (shares per bond) - used for standard conversion policies.
    conversion_ratio: f64,
    /// Face value of the bond
    pub(super) face_value: f64,
    /// Coupon cashflows mapped to tree steps
    pub(super) coupon_map: HashMap<usize, f64>,
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
    /// Conversion price per share (for soft-call trigger evaluation).
    conversion_price: f64,
    /// Optional soft-call trigger condition.
    soft_call_trigger: Option<SoftCallTrigger>,
    /// Per-step risk-free discount factors: `rf_step_dfs[i] = curve.df(t_{i+1}) / curve.df(t_i)`.
    /// Uses the full discount curve term structure instead of a flat rate.
    pub(super) rf_step_dfs: Vec<f64>,
    /// Per-step risky discount factors (includes credit spread, adjusted for recovery).
    ///
    /// With recovery rate R:
    /// `risky_fwd_adj = risky_fwd * (1 - R) + rf_fwd * R`
    ///
    /// At R=0 this equals the raw credit-curve forward (zero-recovery TZ model).
    /// At R=1 this equals the risk-free forward (no credit effect).
    pub(super) risky_step_dfs: Vec<f64>,
    /// Equity volatility (stored for soft-call trigger adjustment).
    volatility: f64,
    /// Bond maturity date (for date-to-step mapping in conversion policies).
    maturity: Date,
    /// Number of tree steps (for date-to-step mapping in conversion policies).
    num_steps: usize,
    /// Whether the bond carries dividend protection (`AdjustPrice` or
    /// `AdjustRatio`). When set, the conversion ratio accretes at the
    /// dividend yield: `ratio(t) = ratio_0 * exp(q * t)`; see
    /// [`ConvertibleBondValuator::conversion_value`].
    pub(super) dividend_protected: bool,
}

impl ConvertibleBondValuator {
    /// Create a new convertible bond valuator with full term structure discount factors.
    ///
    /// Unlike the flat-rate approach, this extracts per-step discount factors from the
    /// risk-free and credit curves, capturing the full shape of the yield curve.
    pub(super) fn new(
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

        // Date mapping uses the same ACT/365F clock as the equity process.
        // Coupon day counts determine amounts only, never event locations.
        let day_count_ctx = DayCountContext::default();
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

        // Process coupon cashflows (exclude reset-only events) on the model clock
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
                DayCount::Act365F,
                day_count_ctx,
            )?;
            *coupon_map.entry(bounded_step).or_insert(0.0) += cf.amount.amount();
        }

        // Map call/put schedules to tree steps, supporting exercise periods.
        let mut call_map: HashMap<usize, f64> = HashMap::default();
        let mut put_map: HashMap<usize, f64> = HashMap::default();

        if let Some(ref call_put) = bond.call_put {
            for call in &call_put.calls {
                if call.end_date >= base_date && call.start_date <= bond.maturity {
                    let floor_price = bond.notional.amount() * (call.price_pct_of_par / 100.0);
                    let start_step = map_date_to_step(
                        base_date,
                        call.start_date.max(base_date),
                        bond.maturity,
                        steps,
                        DayCount::Act365F,
                        day_count_ctx,
                    )?;

                    // Exercise period: map all steps from start to end
                    let end_step = map_date_to_step(
                        base_date,
                        call.end_date.min(bond.maturity),
                        bond.maturity,
                        steps,
                        DayCount::Act365F,
                        day_count_ctx,
                    )?;

                    let reference_curve = if let Some(spec) = &call.make_whole {
                        Some((
                            market_context.get_discount(&spec.reference_curve_id)?,
                            spec.spread_bp / 10_000.0,
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
                        let accrued = super::engine::accrued_interest_at(
                            bond,
                            cashflow_schedule,
                            exercise_date,
                        )?;
                        let call_price = if let Some((curve, spread)) = &reference_curve {
                            let mut pv_remaining = 0.0;
                            for cashflow in cashflow_schedule
                                .get_flows()
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
                            (floor_price + accrued).max(pv_remaining)
                        } else {
                            floor_price + accrued
                        };
                        call_map
                            .entry(s)
                            .and_modify(|p| *p = p.min(call_price))
                            .or_insert(call_price);
                    }
                }
            }

            for put in &call_put.puts {
                if put.end_date >= base_date && put.start_date <= bond.maturity {
                    let put_price = bond.notional.amount() * (put.price_pct_of_par / 100.0);
                    let start_step = map_date_to_step(
                        base_date,
                        put.start_date.max(base_date),
                        bond.maturity,
                        steps,
                        DayCount::Act365F,
                        day_count_ctx,
                    )?;

                    let end_step = map_date_to_step(
                        base_date,
                        put.end_date.min(bond.maturity),
                        bond.maturity,
                        steps,
                        DayCount::Act365F,
                        day_count_ctx,
                    )?;

                    // For overlapping put windows, the holder will select the *highest*
                    // put price available at each step.
                    for (s, &exercise_date) in step_dates
                        .iter()
                        .enumerate()
                        .take(end_step + 1)
                        .skip(start_step)
                    {
                        let dirty_put_price = put_price
                            + super::engine::accrued_interest_at(
                                bond,
                                cashflow_schedule,
                                exercise_date,
                            )?;
                        put_map
                            .entry(s)
                            .and_modify(|p| *p = p.max(dirty_put_price))
                            .or_insert(dirty_put_price);
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

        let recovery = match (bond.recovery_rate, bond.credit_curve_id.as_ref()) {
            (Some(r), _) if !r.is_finite() || !(0.0..=1.0).contains(&r) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Convertible bond {} has recovery_rate={r}; expected finite value in \
                     [0.0, 1.0] (was previously clamped silently, which masked invalid input)",
                    bond.id.as_str()
                )));
            }
            (Some(r), _) => r,
            (None, Some(_)) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Convertible bond {} requires an explicit recovery_rate when credit_curve_id is set",
                    bond.id.as_str()
                )));
            }
            (None, None) => 0.0,
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
                //
                // NOTE: this blend assumes the credit curve encodes ZERO-RECOVERY
                // (pure hazard) risky discounting; see `ConvertibleBond::credit_curve_id`.
                // A market recovery-adjusted spread curve would double-count (1 - R).
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
            conversion_price,
            soft_call_trigger: bond.soft_call_trigger.clone(),
            rf_step_dfs,
            risky_step_dfs,
            volatility,
            maturity: bond.maturity,
            num_steps: steps,
            dividend_protected: bond.conversion.dividend_adjustment.is_protected(),
        })
    }

    /// Whether conversion is mandatory (forced) when allowed, regardless of optimality.
    ///
    /// For `MandatoryOn` and `MandatoryVariable` policies, the holder **must** convert
    /// at the specified date -- even if conversion value is below redemption value.
    /// This correctly models PERCS/DECS where holders bear downside equity risk.
    pub(super) fn conversion_is_mandatory(&self) -> bool {
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
    ///
    /// ## Modeling scope (mandatory convertibles)
    ///
    /// For `MandatoryOn` and `MandatoryVariable`, conversion is modeled ONLY at
    /// the single tree step mapped from the mandatory conversion date. Early
    /// voluntary conversion before that date (a feature of some mandatory
    /// structures) is not modeled; before the mandatory step the holder simply
    /// carries the continuation value.
    pub(super) fn conversion_allowed(&self, step: usize, node_spot: f64) -> Result<bool> {
        let ctx = DayCountContext::default();
        let allowed = match &self.conversion_policy {
            ConversionPolicy::Voluntary => true,
            ConversionPolicy::MandatoryOn(date) => {
                // Map the mandatory date to its nearest tree step
                let target_step = map_date_to_step(
                    self.base_date,
                    *date,
                    self.maturity,
                    self.num_steps,
                    DayCount::Act365F,
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
                    DayCount::Act365F,
                    ctx,
                )?;
                let end_step = map_date_to_step(
                    self.base_date,
                    *end,
                    self.maturity,
                    self.num_steps,
                    DayCount::Act365F,
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
                    DayCount::Act365F,
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
    ///
    /// ## Dividend protection (`ratio_accretion`)
    ///
    /// `ratio_accretion` is the dividend-protection factor `exp(q * t)` at the
    /// node's step time `t` (1.0 when the bond is unprotected; see
    /// [`DividendAdjustment`](crate::instruments::fixed_income::convertible::DividendAdjustment)).
    /// The stored
    /// `conversion_ratio` already includes anti-dilution event adjustments, so
    /// protection composes multiplicatively on top of those (event-adjusted
    /// ratio first, then time accretion). For `MandatoryVariable`, the same
    /// factor is applied uniformly to every delivery-ratio regime — the
    /// max-ratio, par-delivery, and min-ratio branches all scale by
    /// `ratio_accretion` while the regime boundaries stay at their contractual
    /// spot levels.
    pub(super) fn conversion_value(&self, spot: f64, ratio_accretion: f64) -> f64 {
        let base = match &self.conversion_policy {
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
        };
        base * ratio_accretion
    }

    /// Get call price at a given step (if callable)
    pub(super) fn call_price_at_step(&self, step: usize) -> Option<f64> {
        self.call_map.get(&step).copied()
    }

    /// Get put price at a given step (if puttable)
    pub(super) fn put_price_at_step(&self, step: usize) -> Option<f64> {
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
    /// ## Modeling scope
    ///
    /// A single `soft_call_trigger` gates the ENTIRE callable life of the bond:
    /// every step of every call window is subject to the trigger. Real deals
    /// often have a soft-call period followed by an unconditional hard-call
    /// period; that structure is not representable — an unconditional hard call
    /// can only be modeled by omitting `soft_call_trigger` altogether. The
    /// trigger is evaluated on the instantaneous node spot (with the BGK-style
    /// barrier adjustment below), not on the realized path over the
    /// observation window.
    ///
    /// ## Reference
    ///
    /// Broadie, M., Glasserman, P., & Kou, S. (1997). "A Continuity Correction
    /// for Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349. `docs/REFERENCES.md#broadie-glasserman-kou-1997`
    pub(super) fn soft_call_triggered(&self, node_spot: f64) -> bool {
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
                    finstack_quant_models::monte_carlo::barriers::corrections::GOBET_MIRI_BETA;
                const MONITORING_DT: f64 = 1.0 / 252.0;
                let adj = BGK_BETA * required_fraction * self.volatility * MONITORING_DT.sqrt();
                let effective_trigger = nominal_trigger * (1.0 + adj);

                node_spot >= effective_trigger
            }
            None => true,
        }
    }
}
