//! Declarative waterfall rules (available-funds cap, reserve and spread
//! accounts, target OC, step-down, shifting interest, early amortization,
//! controlled accumulation) layered onto a deal's base waterfall each period.

use super::*;

/// Declarative, additively-applied waterfall rules layered onto a deal's base
/// waterfall.
///
/// Each sub-spec is optional; when none are present the resolved waterfall is
/// identical to the base waterfall. The simulation engine applies them to
/// each period's copy of the waterfall.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallRules {
    /// Available-funds / net-WAC cap on named tranches' interest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afc: Option<AfcSpec>,
    /// Excess-spread (spread-account) capture and draw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excess_spread: Option<ExcessSpreadSpec>,
    /// Step-down: switch principal allocation to pro-rata after a date when the
    /// step-down trigger passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_down: Option<StepDownSpec>,
    /// Shifting interest: senior receives a scheduled (declining) share of
    /// principal. Mutually exclusive with `step_down` (both govern principal
    /// allocation); `shifting_interest` takes precedence when both are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifting_interest: Option<ShiftingInterestSpec>,
    /// Early amortization: end a revolving period early on a performance breach
    /// (master-trust style), switching the deal into amortization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_amortization: Option<EarlyAmortizationSpec>,
    /// Controlled accumulation: accumulate principal into a funding account and
    /// repay the investor as a bullet at the accumulation end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controlled_accumulation: Option<ControlledAccumulationSpec>,
    /// Reserve account target, replenishment, excess release and final
    /// principal cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve: Option<ReserveAccountSpec>,
    /// Targeted-overcollateralization amortization: notes are paid down each
    /// period to the amount that holds the pool's overcollateralization at
    /// the target, with the excess released to the residual holder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_oc: Option<TargetOcSpec>,
}

impl WaterfallRules {
    /// Validate the declarative rules against the deal's tranche structure.
    ///
    /// A malformed rule set otherwise misprices *silently*: an unresolved
    /// tranche-id reference makes its cap/lock-out a no-op, and an out-of-range
    /// fraction (e.g. `5` entered for `5%`) never trips its trigger. This guard
    /// converts those into explicit `Error::Validation`s at deal-build / pricing
    /// time. Checked here:
    ///
    /// - `afc.capped_tranches` and `shifting_interest.senior_id` resolve to real
    ///   tranches; `afc.net_wac_fee_bp` is finite and non-negative.
    /// - Loss/enhancement/share fractions (`trap_loss_pct`,
    ///   `MaxCumulativeLoss`, `MinCreditEnhancement`, `senior_pct`,
    ///   `max_cumulative_loss`) lie in `[0, 1]`; `MinOcRatio` is finite and
    ///   non-negative (a ratio, so it may exceed 1).
    /// - `excess_spread.target_balance` is non-negative.
    /// - The shifting-interest schedule is non-empty and strictly ascending in
    ///   `months_from_closing` (so the per-period shifting-interest step lookup
    ///   is unambiguous).
    /// - `controlled_accumulation.start_date <= bullet_date`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` describing the first violation found.
    pub(crate) fn validate(
        &self,
        tranches: &super::super::tranches::TrancheStructure,
        currency: Currency,
    ) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let has_tranche = |id: &str| tranches.tranches.iter().any(|t| t.id.as_str() == id);
        let unit = |v: f64, what: &str| -> finstack_quant_core::Result<()> {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(invalid(format!(
                    "waterfall_rules: {what} must be a fraction in [0, 1], got {v}"
                )));
            }
            Ok(())
        };

        if let Some(afc) = &self.afc {
            for id in &afc.capped_tranches {
                if !has_tranche(id) {
                    return Err(invalid(format!(
                        "waterfall_rules: afc.capped_tranches references unknown tranche '{id}'"
                    )));
                }
            }
            if let Some(bp) = afc.net_wac_fee_bp {
                if !bp.is_finite() || bp < 0.0 {
                    return Err(invalid(format!(
                        "waterfall_rules: afc.net_wac_fee_bp must be finite and non-negative, got {bp}"
                    )));
                }
            }
        }

        if let Some(es) = &self.excess_spread {
            if es.target_balance.amount() < 0.0 {
                return Err(invalid(format!(
                    "waterfall_rules: excess_spread.target_balance must be non-negative, got {}",
                    es.target_balance.amount()
                )));
            }
            if let Some(trap) = es.trap_loss_pct {
                unit(trap, "excess_spread.trap_loss_pct")?;
            }
        }

        let validate_trigger = |trigger: &StepDownTrigger,
                                rule: &str|
         -> finstack_quant_core::Result<()> {
            match trigger {
                StepDownTrigger::MaxCumulativeLoss(v) => {
                    unit(*v, &format!("{rule} trigger MaxCumulativeLoss"))
                }
                StepDownTrigger::MinCreditEnhancement(v) => {
                    unit(*v, &format!("{rule} trigger MinCreditEnhancement"))
                }
                StepDownTrigger::MaxDelinquency(v) => {
                    unit(*v, &format!("{rule} trigger MaxDelinquency"))
                }
                StepDownTrigger::MinOcRatio(v) => {
                    if !v.is_finite() || *v < 0.0 {
                        return Err(invalid(format!(
                            "waterfall_rules: {rule} trigger MinOcRatio must be finite and non-negative, got {v}"
                        )));
                    }
                    Ok(())
                }
            }
        };

        if let Some(sd) = &self.step_down {
            for trigger in &sd.triggers {
                validate_trigger(trigger, "step_down")?;
            }
        }

        if let Some(si) = &self.shifting_interest {
            if !has_tranche(&si.senior_id) {
                return Err(invalid(format!(
                    "waterfall_rules: shifting_interest.senior_id references unknown tranche '{}'",
                    si.senior_id
                )));
            }
            if si.schedule.is_empty() {
                return Err(invalid(
                    "waterfall_rules: shifting_interest.schedule must have at least one step"
                        .to_string(),
                ));
            }
            for trigger in &si.triggers {
                validate_trigger(trigger, "shifting_interest")?;
            }
            let mut prev: Option<u32> = None;
            for step in &si.schedule {
                unit(step.senior_pct, "shifting_interest senior_pct")?;
                if let Some(p) = prev {
                    if step.months_from_closing <= p {
                        return Err(invalid(format!(
                            "waterfall_rules: shifting_interest.schedule must be strictly ascending in \
                             months_from_closing (found {} after {p})",
                            step.months_from_closing
                        )));
                    }
                }
                prev = Some(step.months_from_closing);
            }
        }

        if let Some(reserve) = &self.reserve {
            reserve.target.validate(currency)?;
        }
        if let Some(target_oc) = &self.target_oc {
            target_oc.validate()?;
        }

        if let Some(ea) = &self.early_amortization {
            if let Some(max_loss) = ea.max_cumulative_loss {
                unit(max_loss, "early_amortization.max_cumulative_loss")?;
            }
            if let Some(floor) = ea.min_excess_spread_3m {
                if !floor.is_finite() {
                    return Err(invalid(format!(
                        "waterfall_rules: early_amortization.min_excess_spread_3m must be finite, got {floor}"
                    )));
                }
            }
        }

        if let Some(ca) = &self.controlled_accumulation {
            if ca.start_date > ca.bullet_date {
                return Err(invalid(format!(
                    "waterfall_rules: controlled_accumulation.start_date ({}) must be on or before \
                     bullet_date ({})",
                    ca.start_date, ca.bullet_date
                )));
            }
        }

        Ok(())
    }
}

/// Available-funds cap (net-WAC cap) specification.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AfcSpec {
    /// Ids of tranches whose interest coupon is capped at the collateral's
    /// (net) weighted-average coupon.
    pub capped_tranches: Vec<String>,
    /// Senior fee load (annualized basis points) ranking ahead of the capped
    /// interest — typically servicing plus trustee fees. Subtracted from the
    /// gross collateral WAC to form the **net**-WAC cap. When `None` the cap is
    /// the gross collateral WAC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_wac_fee_bp: Option<f64>,
    /// Net-WAC carryover: the interest the cap withholds from each capped
    /// tranche accrues as a carryover balance (no interest on it) repaid from
    /// excess interest through a `net_wac_carryover` tier ahead of the
    /// incentive fee and the residual. Off by default: the capped-off coupon
    /// is then simply never owed.
    #[serde(default)]
    pub carryover: bool,
}

/// Target balance of the deal reserve account, re-evaluated every period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum ReserveTarget {
    /// A fixed amount in the deal currency.
    Fixed(Money),
    /// A decimal fraction of the current pool balance (performing collateral
    /// plus any accumulation funding account), so the target amortizes with
    /// the pool.
    PctOfCurrent(f64),
    /// A decimal fraction of the original (cut-off) pool balance: a floor
    /// that does not amortize.
    PctOfOriginal(f64),
    /// The larger of two targets, typically `PctOfCurrent` with a
    /// `PctOfOriginal` floor.
    Max(Box<ReserveTarget>, Box<ReserveTarget>),
}

impl ReserveTarget {
    /// The target for one period.
    ///
    /// # Arguments
    ///
    /// * `current_pool` - Current pool balance in currency units (performing
    ///   collateral plus the accumulation funding account).
    /// * `original_pool` - Original (cut-off) pool balance in currency units.
    #[must_use]
    pub fn resolve(&self, current_pool: f64, original_pool: f64) -> f64 {
        match self {
            Self::Fixed(amount) => amount.amount().max(0.0),
            Self::PctOfCurrent(pct) => (pct * current_pool).max(0.0),
            Self::PctOfOriginal(pct) => (pct * original_pool).max(0.0),
            Self::Max(a, b) => a
                .resolve(current_pool, original_pool)
                .max(b.resolve(current_pool, original_pool)),
        }
    }

    fn validate(&self, currency: Currency) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        match self {
            Self::Fixed(amount) => {
                if amount.currency() != currency || amount.amount() < 0.0 {
                    return Err(invalid(format!(
                        "waterfall_rules: reserve.target must be a non-negative {currency} amount, got {amount}"
                    )));
                }
            }
            Self::PctOfCurrent(pct) | Self::PctOfOriginal(pct) => {
                if !pct.is_finite() || !(0.0..=1.0).contains(pct) {
                    return Err(invalid(format!(
                        "waterfall_rules: reserve.target fraction must be in [0, 1], got {pct}"
                    )));
                }
            }
            Self::Max(a, b) => {
                a.validate(currency)?;
                b.validate(currency)?;
            }
        }
        Ok(())
    }
}

fn default_true() -> bool {
    true
}

/// Targeted overcollateralization amortization (auto and consumer ABS).
///
/// Each period the notes are paid down to the amount that leaves the pool's
/// overcollateralization (`pool − notes`) at the target: the larger of
/// `pct_of_current` of the pool balance after the period's collections and
/// `floor_pct_of_original` of the cut-off balance. The required principal
/// distribution is `max(0, notes − max(pool − target, 0))`, paid to the
/// notes by priority from interest proceeds first and principal proceeds
/// for the rest; the collections above it are released to the residual
/// holder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TargetOcSpec {
    /// Target overcollateralization as a decimal fraction of the current
    /// pool balance (after the period's collections).
    pub pct_of_current: f64,
    /// Floor on the target as a decimal fraction of the original (cut-off)
    /// pool balance; `0.0` for no floor.
    #[serde(default)]
    pub floor_pct_of_original: f64,
}

impl TargetOcSpec {
    /// Target overcollateralization for one period, in currency units.
    ///
    /// # Arguments
    ///
    /// * `current_pool` - Pool balance after the period's collections.
    /// * `original_pool` - Original (cut-off) pool balance.
    #[must_use]
    pub fn target(&self, current_pool: f64, original_pool: f64) -> f64 {
        (self.pct_of_current * current_pool)
            .max(self.floor_pct_of_original * original_pool)
            .max(0.0)
    }

    fn validate(&self) -> finstack_quant_core::Result<()> {
        for (value, what) in [
            (self.pct_of_current, "target_oc.pct_of_current"),
            (
                self.floor_pct_of_original,
                "target_oc.floor_pct_of_original",
            ),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "waterfall_rules: {what} must be a fraction in [0, 1], got {value}"
                )));
            }
        }
        Ok(())
    }
}

/// Deal reserve account rules for the template waterfall.
///
/// The account starts at `AssetPool::reserve_account`. Every period the
/// target is resolved from the live pool; a `ReserveReplenishment` recipient
/// after the last note coupon and the junior fees tops the account up from
/// interest proceeds (`replenish`), the balance above the target is released
/// into the waterfall's interest proceeds (`release_excess`), and the account
/// covers senior fees and note coupons whenever interest proceeds fall short.
/// At legal final the balance retires note principal by priority
/// (`covers_principal_at_final`) or goes straight to the residual holder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReserveAccountSpec {
    /// Required balance, re-evaluated every period.
    pub target: ReserveTarget,
    /// Top the account up from interest proceeds after the note coupons and
    /// junior fees. Defaults to `true`.
    #[serde(default = "default_true")]
    pub replenish: bool,
    /// Release any balance above the target into the period's interest
    /// proceeds. Defaults to `true`.
    #[serde(default = "default_true")]
    pub release_excess: bool,
    /// At legal final, apply the balance to unpaid note principal by
    /// priority before the residual holder. Defaults to `true`; `false`
    /// releases it to the residual holder.
    #[serde(default = "default_true")]
    pub covers_principal_at_final: bool,
}

impl ReserveAccountSpec {
    /// Reserve rules with replenishment, excess release and final principal
    /// cover all on.
    ///
    /// # Arguments
    ///
    /// * `target` - Required balance, re-evaluated every period.
    pub fn new(target: ReserveTarget) -> Self {
        Self {
            target,
            replenish: true,
            release_excess: true,
            covers_principal_at_final: true,
        }
    }
}

/// Excess-spread / spread-account specification.
///
/// Each period the account captures residual interest (that would otherwise be
/// distributed to equity) up to `target_balance`, and draws down to cover debt
/// tranche interest shortfalls — providing credit enhancement from excess
/// spread. At termination it pays deferred debt coupons; a breached loss trap
/// then applies remaining interest to debt principal before releasing the
/// surplus to the residual holder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExcessSpreadSpec {
    /// Target funded balance of the spread account (currency units).
    pub target_balance: Money,
    /// Optional cumulative-loss fraction (decimal, e.g. `0.05` = 5% of the
    /// original pool) at or above which terminal spread-account cash repays
    /// debt principal after deferred coupons. Any surplus reaches the residual
    /// holder. `None` releases surplus after deferred coupons without this
    /// interest-to-principal transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_loss_pct: Option<f64>,
}

/// A single step-down performance trigger.
///
/// Each variant is a per-period *health check* the deal must pass (in addition
/// to seasoning past the step-down date) for principal to switch to pro-rata.
/// All configured triggers must pass simultaneously; while any is breached the
/// deal reverts to sequential, so the switch is re-evaluated every period.
///
/// Conventions (all evaluated on *current* balances each period):
/// - cumulative loss is a fraction of the *original* pool balance;
/// - the OC ratio is `current pool balance ÷ rated (non-equity) note balance`;
/// - credit enhancement is the senior cushion `(pool − senior note) ÷ pool`,
///   where the senior note is the *single* most-senior tranche by payment
///   priority — for pari-passu senior classes (e.g. A-1/A-2) only the
///   lowest-priority one is taken as the reference.
///
/// `MaxDelinquency` reads the delinquent share of the pool, which is zero
/// unless the deal carries a `credit_model.delinquency` model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StepDownTrigger {
    /// Passes while cumulative losses (fraction of the original pool) are at or
    /// below this level.
    MaxCumulativeLoss(f64),
    /// Passes while the overcollateralization ratio (current pool ÷ rated note
    /// balance) is at or above this level.
    MinOcRatio(f64),
    /// Passes while senior credit enhancement (`(pool − senior note) ÷ pool`) is
    /// at or above this level.
    MinCreditEnhancement(f64),
    /// Passes while the delinquent balance (every bucket of the deal's
    /// delinquency model) as a fraction of the current pool balance is at or
    /// below this level.
    MaxDelinquency(f64),
}

/// Step-down specification for senior/subordinate principal allocation.
///
/// Principal is paid sequentially (senior first) until the deal seasons past
/// `step_down_date`; from then on, *if* every configured [`StepDownTrigger`]
/// passes, principal switches to pro-rata across the debt tranches, releasing
/// subordination to the juniors. While any trigger is breached the deal reverts
/// to sequential, so the switch is re-evaluated every period (non-sticky). An
/// empty `triggers` list steps down purely on the date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StepDownSpec {
    /// Earliest date principal may switch to pro-rata.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub step_down_date: Date,
    /// Performance triggers; all must pass for the step-down to take effect.
    pub triggers: Vec<StepDownTrigger>,
}

/// One step of a shifting-interest schedule: the senior's share of principal
/// from `months_from_closing` onward (until the next step).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ShiftingInterestStep {
    /// Months from closing at which this senior share takes effect.
    pub months_from_closing: u32,
    /// Senior share of principal (decimal, `1.0` = 100% lockout) from this step.
    pub senior_pct: f64,
}

/// How a shifting-interest schedule value is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum ShiftMode {
    /// Prospectus form: the schedule value is the share of the subordinates'
    /// pro-rata unscheduled principal that shifts to the senior, so the
    /// senior's unscheduled share is `senior_pct + value × (1 − senior_pct)`
    /// on live balances (`1.0` = full lockout, `0.0` = pro-rata).
    #[default]
    ShiftOfSubordinate,
    /// The schedule value is the senior's share of unscheduled principal
    /// itself.
    SeniorShare,
}

/// Shifting-interest principal allocation (non-agency senior/sub RMBS).
///
/// Scheduled principal is always paid pro-rata by current balance; the
/// schedule governs *unscheduled* principal (prepayments and recoveries).
/// Under [`ShiftMode::ShiftOfSubordinate`] (the default) each step is the
/// fraction of the subordinates' pro-rata share that shifts to the senior:
/// `1.0` is the full lockout, later steps (`0.7`, `0.6`, ...) release the
/// subordinates' share progressively, `0.0` is pro-rata. Under
/// [`ShiftMode::SeniorShare`] each step is the senior's share of unscheduled
/// principal directly. While any of `triggers` fails the shift reverts to the
/// full lockout regardless of the schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ShiftingInterestSpec {
    /// Id of the senior tranche that receives the shifted principal.
    pub senior_id: String,
    /// Schedule ascending by `months_from_closing`; each step's `senior_pct`
    /// is read per `mode`.
    pub schedule: Vec<ShiftingInterestStep>,
    /// How the schedule values are read; `ShiftOfSubordinate` by default.
    #[serde(default)]
    pub mode: ShiftMode,
    /// Performance tests that must all pass for the schedule to apply; while
    /// any fails the senior takes every unscheduled dollar (lockout).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<StepDownTrigger>,
}

impl ShiftingInterestSpec {
    /// Shifting interest with the prospectus reading of the schedule and no
    /// performance gating.
    ///
    /// # Arguments
    ///
    /// * `senior_id` - Id of the senior tranche the shift favours.
    /// * `schedule` - Steps ascending by `months_from_closing`, each a decimal
    ///   in `[0, 1]` read per [`ShiftMode::ShiftOfSubordinate`].
    pub fn new(senior_id: impl Into<String>, schedule: Vec<ShiftingInterestStep>) -> Self {
        Self {
            senior_id: senior_id.into(),
            schedule,
            mode: ShiftMode::default(),
            triggers: Vec::new(),
        }
    }

    /// Set how the schedule values are read.
    ///
    /// # Arguments
    ///
    /// * `mode` - [`ShiftMode`] applied to every step.
    pub fn with_mode(mut self, mode: ShiftMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the performance tests gating the shift.
    ///
    /// # Arguments
    ///
    /// * `triggers` - Tests that must all pass for the schedule to apply.
    pub fn with_triggers(mut self, triggers: Vec<StepDownTrigger>) -> Self {
        self.triggers = triggers;
        self
    }
}

/// Early-amortization specification for revolving (master-trust) deals.
///
/// While a deal's reinvestment/revolving period is active, principal is recycled
/// and the investor (tranche) balances are held flat. If cumulative losses reach
/// `max_cumulative_loss` or the trailing excess spread falls below
/// `min_excess_spread_3m`, an early-amortization event is triggered: the
/// revolving period ends immediately and the deal begins paying principal down
/// (amortizing) even before its scheduled revolving-period end.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EarlyAmortizationSpec {
    /// Cumulative-loss fraction (decimal, of the original pool balance) at or
    /// above which the revolving period ends early and amortization begins.
    /// `None` disables the loss test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cumulative_loss: Option<f64>,
    /// Annualized excess spread (decimal of the opening pool balance: pool
    /// interest less debt coupons due, fees paid and net charge-offs) whose
    /// three-period trailing average, once it falls below this floor, ends the
    /// revolving period for good. `None` disables the excess-spread test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_excess_spread_3m: Option<f64>,
}

/// Controlled-accumulation specification for revolving (master-trust) deals.
///
/// Between `start_date` and `bullet_date` the deal is in its controlled-
/// accumulation period: collected pool principal is held in a principal funding
/// account (investor balances stay flat, no pass-through paydown) rather than
/// recycled or distributed. At the first payment date on or after `bullet_date`
/// the entire account is released into the waterfall as a single bullet
/// principal payment. Accumulation is suspended while a revolving/reinvestment
/// period is still active (which recycles principal) and on early amortization
/// (which pays principal down immediately).
///
/// `start_date` is typically the revolving-period end and `bullet_date` the
/// note's expected maturity (and at/before the legal final maturity, so the
/// release occurs before the deal winds down). If the deal terminates before
/// the bullet date (cleanup call, pool exhaustion, or a bullet date beyond the
/// last payment), any residual funding-account balance is swept to the
/// outstanding tranches senior-first at deal end, so accumulated principal is
/// never stranded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ControlledAccumulationSpec {
    /// First date principal is accumulated into the funding account.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// Date the accumulated funding account is released as a bullet payment.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub bullet_date: Date,
}
