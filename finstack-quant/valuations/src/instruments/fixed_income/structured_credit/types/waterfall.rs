//! Waterfall type definitions for structured credit instruments.
//!
//! This module contains all data structures for waterfall distribution:
//! - Payment recipients and calculation methods
//! - Tier structures and allocation modes
//! - Coverage-test positions (`PaymentType::CoverageTest` tiers carrying
//!   `CoverageTestSpec`s) for OC/IC diversion
//! - Result types for waterfall execution
//!
//! Execution logic is in `crate::instruments::fixed_income::structured_credit::pricing::waterfall`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::ExplanationTrace;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::HashMap;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Recipient of waterfall payments
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum RecipientType {
    /// Service provider (trustee, admin, rating agency, etc.)
    ServiceProvider(String),
    /// Manager fee (type indicates senior/subordinated/incentive)
    ManagerFee(ManagementFeeType),
    /// Tranche payment
    Tranche(String),
    /// Equity/residual distribution
    Equity,
    /// Reserve account (credit enhancement replenishment)
    ReserveAccount(String),
}

/// Type of management fee
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum ManagementFeeType {
    /// Senior variant.
    Senior,
    /// Subordinated variant.
    Subordinated,
    /// Incentive variant.
    Incentive,
}

/// Rounding convention for payments
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum RoundingConvention {
    /// Round to nearest precision
    #[default]
    Nearest,
    /// Round down (floor)
    Floor,
    /// Round up (ceiling)
    Ceiling,
}

/// How to calculate payment amount
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum PaymentCalculation {
    /// Fixed amount
    FixedAmount {
        /// Amount.
        amount: Money,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Percentage of collateral balance
    PercentageOfCollateral {
        /// Rate.
        rate: f64,
        /// Annualized.
        annualized: bool,
        /// Day count convention for annualization.
        day_count: Option<finstack_quant_core::dates::DayCount>,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Percentage of the specially serviced collateral balance (CMBS special
    /// servicing fee): the balances of pool assets carrying a
    /// `special_servicing` spec.
    PercentageOfSpecialServiced {
        /// Rate.
        rate: f64,
        /// Annualized.
        annualized: bool,
        /// Day count convention for annualization.
        day_count: Option<finstack_quant_core::dates::DayCount>,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Interest due on tranche
    TrancheInterest {
        /// Tranche id.
        tranche_id: String,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Principal payment to tranche
    TranchePrincipal {
        /// Tranche id.
        tranche_id: String,
        /// Balance the regular principal pass amortizes the tranche down to;
        /// `None` pays it in full. A coverage-test cure diversion at an earlier
        /// position pays toward zero and counts toward this target, so the
        /// regular pass only completes the remaining distance to it.
        target_balance: Option<Money>,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// All remaining cash
    ResidualCash,
    /// Reserve account replenishment (up to target balance).
    ///
    /// The current reserve balance is passed dynamically via `WaterfallContext`
    /// at execution time, not stored here, because the balance changes each period.
    ReserveReplenishment {
        /// Target reserve balance the account should be replenished to.
        target_balance: Money,
    },
    /// Tranche interest with the coupon rate capped (available-funds / net-WAC cap).
    ///
    /// Identical to [`PaymentCalculation::TrancheInterest`] except the effective
    /// annualized coupon is capped at `cap_rate`, modelling an available-funds
    /// cap where a tranche cannot be paid more interest than the collateral's
    /// net weighted-average coupon supports. The cap defines the *claim*, not
    /// just the allocation: the capped-off coupon is never owed, so it does not
    /// defer, does not PIK, and does not enter IC coverage or excess-spread
    /// sizing (no carryforward in this variant).
    CappedTrancheInterest {
        /// Tranche id.
        tranche_id: String,
        /// Cap on the annualized coupon rate (decimal, e.g. `0.03` = 3%).
        cap_rate: f64,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Net-WAC carryover repayment: the tranche's carryover balance brought
    /// into the period (`amount`, set by the engine each period from the
    /// interest its available-funds cap withheld in earlier periods), paid
    /// from the cash reaching this recipient and recorded as interest on the
    /// tranche outside its capped claim.
    NetWacCarryover {
        /// Tranche id.
        tranche_id: String,
        /// Carryover balance outstanding at the period's opening.
        amount: Money,
    },
    /// Manager incentive fee: `share_pct` of the cash reaching this recipient
    /// that lies above the equity hurdle. The hurdle is tested on the
    /// [`EquityHistory`] in the `WaterfallContext` plus every equity
    /// distribution earlier in the same waterfall run plus this cash; the
    /// cash that lifts the equity IRR exactly to `hurdle_irr`
    /// ([`EquityHistory::hurdle_shortfall`]) passes to equity untouched and
    /// the manager shares only in the excess. Nothing is paid while the
    /// hurdle is unreachable, or when no equity history is supplied. The
    /// standard template places one recipient in the principal tier ahead of
    /// `equity_principal` and one ahead of the residual.
    IncentiveFee {
        /// Equity IRR hurdle as an annual decimal.
        hurdle_irr: f64,
        /// Share of the residual paid once the hurdle is met, in `[0, 1]`.
        share_pct: f64,
    },
}

/// Equity's cash history to date, the input of an incentive-fee IRR test.
#[derive(Debug, Clone, PartialEq)]
pub struct EquityHistory {
    /// Date the equity capital was invested (the deal closing date).
    pub invested_on: Date,
    /// Equity capital invested at closing (the equity notes' original par).
    pub invested: Money,
    /// Every distribution to equity so far, in date order.
    pub distributions: Vec<(Date, Money)>,
}

impl EquityHistory {
    /// Annual IRR of the equity investment against its distributions to date
    /// plus `candidate` paid on `payment_date`, or `None` when no IRR exists
    /// (no capital invested, or no sign change yet).
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Date the candidate distribution would be paid.
    /// * `candidate` - Cash that would reach equity on `payment_date`.
    #[must_use]
    pub fn irr_with(&self, payment_date: Date, candidate: Money) -> Option<f64> {
        if self.invested.amount() <= 0.0 {
            return None;
        }
        let mut flows: Vec<(Date, f64)> = Vec::with_capacity(self.distributions.len() + 2);
        flows.push((self.invested_on, -self.invested.amount()));
        flows.extend(
            self.distributions
                .iter()
                .map(|(date, amount)| (*date, amount.amount())),
        );
        flows.push((payment_date, candidate.amount()));
        finstack_quant_core::cashflow::xirr(&flows, None)
            .ok()
            .filter(|irr| irr.is_finite())
    }

    /// Cash on `payment_date` that lifts the equity IRR to `hurdle_irr`,
    /// capped at `candidate`: zero when the hurdle is already earned, all of
    /// `candidate` when even that much leaves the IRR below the hurdle (or no
    /// IRR exists). The manager's incentive share applies to
    /// `candidate − shortfall`.
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Date the candidate distribution would be paid.
    /// * `candidate` - Total cash that could reach equity on `payment_date`.
    /// * `hurdle_irr` - Equity IRR hurdle as an annual decimal.
    #[must_use]
    pub fn hurdle_shortfall(&self, payment_date: Date, candidate: Money, hurdle_irr: f64) -> Money {
        let currency = candidate.currency();
        let total = candidate.amount().max(0.0);
        let irr_at = |cash: f64| {
            Money::new(cash, currency)
                .ok()
                .and_then(|money| self.irr_with(payment_date, money))
        };
        if irr_at(0.0).is_some_and(|irr| irr >= hurdle_irr) {
            return Money::from((0_i64, currency));
        }
        if !irr_at(total).is_some_and(|irr| irr >= hurdle_irr) {
            return candidate;
        }
        // IRR is monotone in the candidate cash: bisect for the crossing.
        let (mut low, mut high) = (0.0_f64, total);
        for _ in 0..64 {
            let mid = 0.5 * (low + high);
            if irr_at(mid).is_some_and(|irr| irr >= hurdle_irr) {
                high = mid;
            } else {
                low = mid;
            }
            if high - low <= 0.005 {
                break;
            }
        }
        Money::new(high, currency).unwrap_or(candidate)
    }
}

/// Declarative, additively-applied waterfall rules layered onto a deal's base
/// waterfall.
///
/// Each sub-spec is optional; when none are present the resolved waterfall is
/// identical to the base waterfall. Applied by
/// [`crate::instruments::fixed_income::structured_credit::resolve_waterfall`].
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
    ///   `months_from_closing` (so [`crate::instruments::fixed_income::structured_credit::resolve_waterfall`]'s
    ///   step lookup is unambiguous).
    /// - `controlled_accumulation.start_date <= bullet_date`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` describing the first violation found.
    pub(crate) fn validate(
        &self,
        tranches: &super::tranches::TrancheStructure,
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

/// Allocation mode within a tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum AllocationMode {
    /// Pay recipients sequentially in order until tier allocation exhausted
    Sequential,
    /// Distribute proportionally by weight or equally if no weights
    ProRata,
}

/// Payment type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    /// Fee payment funded from interest collections.
    Fee,
    /// Coupon payment funded from interest collections.
    Interest,
    /// Capital repayment funded from principal collections.
    Principal,
    /// Excess-interest distribution; does not retire equity principal.
    Residual,
    /// Coverage-test position: the tier pays nobody itself. While any of its
    /// [`WaterfallTier::tests`] fails, the interest still undistributed at
    /// this point in the waterfall is diverted (up to the cure amount) per the
    /// test's [`CoverageTestAction`], so only tiers ranked below the test can
    /// lose cash to it.
    CoverageTest,
}

/// Individual payment recipient within a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    /// Unique identifier
    pub id: String,
    /// Recipient type
    pub recipient_type: RecipientType,
    /// How to calculate payment amount
    pub calculation: PaymentCalculation,
    /// Weight for pro-rata distribution (None = equal weight)
    pub weight: Option<f64>,
}

impl Recipient {
    /// Create a new recipient
    pub fn new(
        id: impl Into<String>,
        recipient_type: RecipientType,
        calculation: PaymentCalculation,
    ) -> Self {
        Self {
            id: id.into(),
            recipient_type,
            calculation,
            weight: None,
        }
    }

    /// Set weight for pro-rata allocation
    #[must_use]
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Create a fixed fee recipient
    #[must_use]
    pub fn fixed_fee(id: impl Into<String>, provider: impl Into<String>, amount: Money) -> Self {
        Self::new(
            id,
            RecipientType::ServiceProvider(provider.into()),
            PaymentCalculation::FixedAmount {
                amount,
                rounding: None,
            },
        )
    }

    /// Create a tranche interest recipient
    #[must_use]
    pub fn tranche_interest(id: impl Into<String>, tranche_id: impl Into<String>) -> Self {
        let tranche_id_str = tranche_id.into();
        Self::new(
            id,
            RecipientType::Tranche(tranche_id_str.clone()),
            PaymentCalculation::TrancheInterest {
                tranche_id: tranche_id_str,
                rounding: None,
            },
        )
    }

    /// Create a tranche principal recipient
    #[must_use]
    pub fn tranche_principal(
        id: impl Into<String>,
        tranche_id: impl Into<String>,
        target_balance: Option<Money>,
    ) -> Self {
        let tranche_id_str = tranche_id.into();
        Self::new(
            id,
            RecipientType::Tranche(tranche_id_str.clone()),
            PaymentCalculation::TranchePrincipal {
                tranche_id: tranche_id_str,
                target_balance,
                rounding: None,
            },
        )
    }
}

/// Collection account(s) a waterfall tier draws on.
///
/// Interest and principal proceeds are separate accounts in the executor.
/// A tier normally draws on the account matching its [`PaymentType`]; a
/// `Fee` or `Interest` tier may instead be allowed to top up from principal
/// proceeds (the CLO principal-waterfall convention that senior fees and
/// senior note interest shortfalls are paid from principal before any note
/// is redeemed). Cash taken from principal this way is reported as
/// [`WaterfallDistribution::principal_used_for_interest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FundingSource {
    /// Interest proceeds only (the default for fee, interest and residual tiers).
    Interest,
    /// Principal proceeds only (the default for principal tiers).
    Principal,
    /// Interest proceeds first, then principal proceeds for any shortfall.
    InterestThenPrincipal,
}

impl FundingSource {
    /// Account a tier of `payment_type` draws on when it sets no explicit
    /// funding source.
    ///
    /// # Arguments
    ///
    /// * `payment_type` - Tier classification; `Principal` tiers draw on
    ///   principal proceeds, every other tier on interest proceeds.
    #[must_use]
    pub fn default_for(payment_type: PaymentType) -> Self {
        match payment_type {
            PaymentType::Principal => Self::Principal,
            _ => Self::Interest,
        }
    }
}

/// Waterfall tier: a payment step with recipients, or a coverage-test
/// position ([`PaymentType::CoverageTest`]) carrying `tests` and no recipients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallTier {
    /// Unique tier identifier
    pub id: String,
    /// Priority order (lower = higher priority)
    pub priority: usize,
    /// Recipients in this tier (empty for a coverage-test tier)
    pub recipients: Vec<Recipient>,
    /// Payment type classification
    pub payment_type: PaymentType,
    /// How to allocate within tier
    pub allocation_mode: AllocationMode,
    /// Coverage tests evaluated at this position (only for
    /// [`PaymentType::CoverageTest`] tiers; empty otherwise). Every test in
    /// one tier shares the same [`CoverageTestAction`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<CoverageTestSpec>,
    /// Collection account(s) this tier draws on; `None` uses
    /// [`FundingSource::default_for`] the tier's `payment_type`. See
    /// [`Self::effective_funding`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding: Option<FundingSource>,
}

impl WaterfallTier {
    /// Create a new waterfall tier
    #[must_use]
    pub fn new(id: impl Into<String>, priority: usize, payment_type: PaymentType) -> Self {
        Self {
            id: id.into(),
            priority,
            recipients: Vec::new(),
            payment_type,
            allocation_mode: AllocationMode::Sequential,
            tests: Vec::new(),
            funding: None,
        }
    }

    /// Set the collection account(s) this tier draws on.
    ///
    /// # Arguments
    ///
    /// * `source` - Account(s) to pay this tier from; `InterestThenPrincipal`
    ///   lets a fee or interest tier top up from principal proceeds.
    #[must_use]
    pub fn funding(mut self, source: FundingSource) -> Self {
        self.funding = Some(source);
        self
    }

    /// Account(s) this tier draws on: the explicit `funding`, or the default
    /// for its `payment_type`.
    #[must_use]
    pub fn effective_funding(&self) -> FundingSource {
        self.funding
            .unwrap_or_else(|| FundingSource::default_for(self.payment_type))
    }

    /// Create a coverage-test tier at `priority` carrying `tests`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique tier identifier.
    /// * `priority` - Position in the waterfall (lower runs first); `0` lets
    ///   [`WaterfallBuilder::add_tier`] assign the next priority.
    /// * `tests` - Coverage tests evaluated at this position; they must share
    ///   one [`CoverageTestAction`].
    #[must_use]
    pub fn coverage_tests(
        id: impl Into<String>,
        priority: usize,
        tests: Vec<CoverageTestSpec>,
    ) -> Self {
        Self {
            id: id.into(),
            priority,
            recipients: Vec::new(),
            payment_type: PaymentType::CoverageTest,
            allocation_mode: AllocationMode::Sequential,
            tests,
            funding: None,
        }
    }

    /// Add a recipient to this tier
    #[must_use]
    pub fn add_recipient(mut self, recipient: Recipient) -> Self {
        self.recipients.push(recipient);
        self
    }

    /// Set allocation mode
    #[must_use]
    pub fn allocation_mode(mut self, mode: AllocationMode) -> Self {
        self.allocation_mode = mode;
        self
    }

    /// Tranche ids whose interest claim this tier pays.
    pub(crate) fn interest_tranche_ids(&self) -> impl Iterator<Item = &str> {
        self.recipients
            .iter()
            .filter_map(|recipient| match &recipient.calculation {
                PaymentCalculation::TrancheInterest { tranche_id, .. }
                | PaymentCalculation::CappedTrancheInterest { tranche_id, .. } => {
                    Some(tranche_id.as_str())
                }
                _ => None,
            })
    }
}

/// Result of waterfall distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallDistribution {
    /// Payment date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Total available cash at start
    pub total_available: Money,

    /// Tier-level allocations
    pub tier_allocations: Vec<(String, Money)>,

    /// Distributions by recipient
    pub distributions: BTreeMap<RecipientType, Money>,
    /// The PRINCIPAL portion of `distributions`, per recipient.
    ///
    /// SC-M28: `distributions` aggregates a tranche's interest and principal
    /// into one number, because `RecipientType::Tranche(id)` is the same key
    /// for both. Step 5 therefore had to RE-DERIVE the split by assuming
    /// interest is satisfied first — which silently reclassified a diverted
    /// OC-cure principal payment as interest whenever that tranche carried a
    /// shortfall, leaving the balance unretired so the cure could not reduce
    /// the very denominator it was sized to fix.
    ///
    /// The waterfall already knows the answer: `PaymentCalculation`
    /// distinguishes `TranchePrincipal` from `TrancheInterest`. Reporting it
    /// here makes the classification authoritative instead of reconstructed.
    pub principal_distributions: BTreeMap<RecipientType, Money>,
    /// Detailed payment records
    pub payment_records: Vec<PaymentRecord>,

    /// Coverage test results (test_name, value, passed)
    pub coverage_tests: Vec<(String, f64, bool)>,

    /// Total diverted cash
    pub diverted_cash: Money,
    /// Remaining undistributed cash
    pub remaining_cash: Money,
    /// Undistributed interest retained for subsequent interest-waterfall periods.
    pub remaining_interest: Money,
    /// Undistributed principal retained for reinvestment or debt repayment.
    pub remaining_principal: Money,
    /// Principal proceeds spent on fee or interest tiers whose
    /// [`FundingSource`] is `InterestThenPrincipal`. Already netted out of
    /// `remaining_principal`; reported so the principal account's use is
    /// visible.
    pub principal_used_for_interest: Money,
    /// Whether any diversions occurred
    pub had_diversions: bool,
    /// Diversion reason if applicable
    pub diversion_reason: Option<String>,
    /// Detailed diverted payment records.
    #[serde(default)]
    pub diverted_amounts: Vec<DiversionRecord>,

    /// Recovery proceeds included in this period's available cash.
    /// Tracked separately from principal collections for trustee report reconciliation.
    #[serde(default = "WaterfallDistribution::zero_usd")]
    pub recovery_proceeds: Money,

    /// Optional explanation trace (enabled via ExplainOpts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplanationTrace>,
}

impl WaterfallDistribution {
    fn zero_usd() -> Money {
        Money::from((0_i64, finstack_quant_core::currency::Currency::USD))
    }
}

/// Record of a diverted payment from one tier to another recipient.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiversionRecord {
    /// Source tier where cash originated.
    pub source_tier: String,
    /// Recipient identifier that received the diverted cash.
    pub target_tranche: String,
    /// Diverted amount.
    pub amount: Money,
    /// Human-readable reason for diversion.
    pub reason: String,
}

/// Record of individual payment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PaymentRecord {
    /// Tier id
    pub tier_id: String,
    /// Recipient id within tier
    pub recipient_id: String,
    /// Priority
    pub priority: usize,
    /// Recipient
    pub recipient: RecipientType,
    /// Requested amount
    pub requested_amount: Money,
    /// Paid amount
    pub paid_amount: Money,
    /// Shortfall
    pub shortfall: Money,
    /// Diverted
    pub diverted: bool,
}

/// Collateral valuation rules for the OC tests: rating haircuts, the value
/// carried for defaulted collateral, the excess-CCC bucket and discount
/// obligations (CLO indenture conventions). Percentages are percent values
/// (`7.5` = 7.5%); haircuts are decimal fractions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CoverageRules {
    /// Haircut applied to the par of performing collateral by rating bucket,
    /// as a decimal fraction (`0.5` carries the asset at half par). Ratings
    /// are looked up by their letter bucket (`B+`, `B` and `B-` all read the
    /// `B` entry, see `CreditRating::bucket`). An `NR` entry applies to
    /// unrated asset rows supplied by the caller; reinvestment purchases and
    /// materialized instrument collateral are unrated by construction and
    /// stay at par. Empty (the indenture par-value convention) by default.
    #[serde(default)]
    pub rating_haircuts: BTreeMap<CreditRating, f64>,
    /// Value carried for defaulted collateral whose recovery cash has not yet
    /// arrived.
    #[serde(default)]
    pub defaulted_valuation: DefaultedValuation,
    /// Excess-CCC bucket: collateral rated CCC+ and below beyond
    /// `threshold_pct` of the performing pool is carried at market value or
    /// excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccc_bucket: Option<CccBucketRule>,
    /// Discount obligations: collateral bought below `price_threshold_pct` of
    /// par is carried at its purchase price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_obligation: Option<DiscountObligationRule>,
    /// Advance rates and concentration limits evaluated by
    /// `CoverageTestType::BorrowingBase` tests; `None` makes such a test a
    /// validation error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrowing_base: Option<super::borrowing_base::BorrowingBaseRules>,
}

/// How defaulted collateral enters the OC numerator until its recovery cash
/// arrives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DefaultedValuation {
    /// At the modeled recovery value of the pending claims (the default).
    #[default]
    Recovery,
    /// At `pct` percent of the defaulted par (a market-value convention).
    MarketValue {
        /// Percent of defaulted par carried (`40.0` = 40%).
        pct: f64,
    },
}

/// Excess-CCC bucket rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CccBucketRule {
    /// Share of the performing pool, in percent, that CCC+ and lower rated
    /// collateral may occupy at par (`7.5` = 7.5%).
    pub threshold_pct: f64,
    /// `true` carries the excess at the assets' `market_price_pct` (assets
    /// without a price stay at par); `false` excludes the excess entirely.
    pub carry_at_market_value: bool,
}

/// Discount-obligation rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscountObligationRule {
    /// Purchase price, in percent of par, below which an asset is a discount
    /// obligation carried at its purchase price (`80.0` = 80% of par).
    pub price_threshold_pct: f64,
}

impl CoverageRules {
    /// Standard CLO par-value test rules: performing collateral at par (no
    /// rating haircuts), defaulted collateral at recovery, a 7.5% CCC bucket
    /// carried at market value and an 80% discount-obligation threshold.
    #[must_use]
    pub fn clo_standard() -> Self {
        Self {
            rating_haircuts: BTreeMap::new(),
            defaulted_valuation: DefaultedValuation::Recovery,
            ccc_bucket: Some(CccBucketRule {
                threshold_pct: 7.5,
                carry_at_market_value: true,
            }),
            discount_obligation: Some(DiscountObligationRule {
                price_threshold_pct: 80.0,
            }),
            borrowing_base: None,
        }
    }

    /// Whether the rules leave every test at plain par with defaulted
    /// collateral at recovery.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rating_haircuts.is_empty()
            && self.defaulted_valuation == DefaultedValuation::Recovery
            && self.ccc_bucket.is_none()
            && self.discount_obligation.is_none()
            && self.borrowing_base.is_none()
    }

    /// Whether any rule changes the value of performing collateral.
    #[must_use]
    pub fn adjusts_collateral(&self) -> bool {
        !self.rating_haircuts.is_empty()
            || self.ccc_bucket.is_some()
            || self.discount_obligation.is_some()
    }

    /// Reject non-finite or out-of-range parameters.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when a haircut is outside `[0, 1]`, a
    /// market-value or threshold percent is outside `[0, 100]`, or any value
    /// is non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if let Some(rules) = &self.borrowing_base {
            rules.validate()?;
        }
        let invalid = |msg: &str| finstack_quant_core::Error::Validation(msg.to_string());
        if self
            .rating_haircuts
            .values()
            .any(|h| !h.is_finite() || !(0.0..=1.0).contains(h))
        {
            return Err(invalid(
                "coverage rating haircuts must be decimal fractions in [0, 1]",
            ));
        }
        if let DefaultedValuation::MarketValue { pct } = self.defaulted_valuation {
            if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
                return Err(invalid(
                    "defaulted market value must be a percent in [0, 100]",
                ));
            }
        }
        if let Some(bucket) = self.ccc_bucket {
            if !bucket.threshold_pct.is_finite() || !(0.0..=100.0).contains(&bucket.threshold_pct) {
                return Err(invalid(
                    "CCC bucket threshold must be a percent in [0, 100]",
                ));
            }
        }
        if let Some(rule) = self.discount_obligation {
            if !rule.price_threshold_pct.is_finite() || rule.price_threshold_pct <= 0.0 {
                return Err(invalid(
                    "discount obligation price threshold must be a positive percent of par",
                ));
            }
        }
        Ok(())
    }
}

/// What a failing coverage test does with the interest it diverts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CoverageTestAction {
    /// Pay down the notes of the senior-most principal tier, in its recipient
    /// order, until the test is cured (the standard OC/IC turbo).
    #[default]
    PayDownSenior,
    /// Retain the diverted interest as principal proceeds (a CLO
    /// reinvestment OC test): it is reinvested while the reinvestment period
    /// is active and repays notes through the principal tier afterwards.
    Reinvest,
}

/// One coverage test at a position in the waterfall.
///
/// The ratio is computed on the period's collateral and note balances
/// (overcollateralization: collateral value over the tested class and every
/// class senior to it; interest coverage: interest collections net of senior
/// fees over the interest due to the same classes); the *position* of the
/// tier that carries the test decides which cash a failure can divert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CoverageTestSpec {
    /// Unique test identifier, reported in `WaterfallDistribution::coverage_tests`
    /// (`OC_<tranche>` / `IC_<tranche>` by convention).
    pub id: String,
    /// Tested class; the ratio covers this class and every class senior to it.
    pub tranche_id: String,
    /// Overcollateralization or interest coverage.
    pub kind: CoverageTestType,
    /// Minimum ratio that passes (1.20 means 120%).
    pub trigger_level: f64,
    /// What a failure does with the diverted interest.
    #[serde(default)]
    pub action: CoverageTestAction,
    /// Template placement of the test tier; `None` places it after the
    /// tested tranche's own interest tier. Used only when the deal
    /// synthesizes its waterfall; a custom waterfall places test tiers
    /// explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<CoveragePlacement>,
    /// Cap on what a failure diverts, as a percent of the interest remaining
    /// at the test tier (`50.0` diverts at most half); `None` diverts up to
    /// the cure amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divert_pct: Option<f64>,
}

/// Where the template places a coverage test tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoveragePlacement {
    /// After the interest tier of this tranche (a test on class B placed
    /// after class A's coupon diverts ahead of B's coupon).
    AfterTranche {
        /// Tranche whose interest tier the test follows.
        tranche_id: String,
    },
    /// After the junior (subordinated management) fee tier, ahead of the
    /// residual: the test diverts only what would otherwise reach equity.
    AfterJuniorFees,
}

impl CoverageTestSpec {
    fn new(tranche_id: impl Into<String>, kind: CoverageTestType, trigger_level: f64) -> Self {
        let tranche_id = tranche_id.into();
        Self {
            id: format!("{}_{tranche_id}", kind.label()),
            tranche_id,
            kind,
            trigger_level,
            action: CoverageTestAction::default(),
            placement: None,
            divert_pct: None,
        }
    }

    /// Overcollateralization test on `tranche_id` at `trigger_level`
    /// (1.20 means 120%), id `OC_<tranche_id>`, paying down senior notes on
    /// failure.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; the denominator is this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn oc(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::Oc, trigger_level)
    }

    /// Interest coverage test on `tranche_id` at `trigger_level` (1.10 means
    /// 110%), id `IC_<tranche_id>`, paying down senior notes on failure.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; interest due covers this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn ic(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::Ic, trigger_level)
    }

    /// Borrowing-base test on `tranche_id` at `trigger_level` (1.0 means the
    /// borrowing base must cover the tested class and every class senior to
    /// it), id `BB_<tranche_id>`, paying down senior notes on failure. The
    /// advance rates and concentration limits come from
    /// `CoverageRules::borrowing_base`.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; the denominator is this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn borrowing_base(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::BorrowingBase, trigger_level)
    }

    /// Set what a failure does with the diverted interest.
    #[must_use]
    pub fn with_action(mut self, action: CoverageTestAction) -> Self {
        self.action = action;
        self
    }

    /// Place the template test tier after `tranche_id`'s interest tier.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Class whose interest tier the test follows.
    #[must_use]
    pub fn after_tranche(mut self, tranche_id: impl Into<String>) -> Self {
        self.placement = Some(CoveragePlacement::AfterTranche {
            tranche_id: tranche_id.into(),
        });
        self
    }

    /// Place the test after the junior fee tier, ahead of the residual.
    #[must_use]
    pub fn after_junior_fees(mut self) -> Self {
        self.placement = Some(CoveragePlacement::AfterJuniorFees);
        self
    }

    /// Cap what a failure diverts at `pct` percent of the interest remaining
    /// at the test tier.
    ///
    /// # Arguments
    ///
    /// * `pct` - Percent of the remaining interest in `(0, 100]`.
    #[must_use]
    pub fn with_divert_pct(mut self, pct: f64) -> Self {
        self.divert_pct = Some(pct);
        self
    }

    /// Tranche whose interest tier the template places this test after
    /// (the tested tranche unless placed after another one); `None` when
    /// placed after the junior fees.
    pub fn placement_tranche(&self) -> Option<&str> {
        match &self.placement {
            None => Some(&self.tranche_id),
            Some(CoveragePlacement::AfterTranche { tranche_id }) => Some(tranche_id),
            Some(CoveragePlacement::AfterJuniorFees) => None,
        }
    }
}

/// Type of coverage test (simplified to OC/IC only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CoverageTestType {
    /// Overcollateralization test
    Oc,
    /// Interest coverage test
    Ic,
    /// Borrowing-base test: advance-rate-weighted eligible collateral (after
    /// the concentration limits in `CoverageRules::borrowing_base`) over the
    /// tested class plus every class senior to it.
    BorrowingBase,
}

impl CoverageTestType {
    /// Upper-case label used in test ids (`OC` / `IC`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Oc => "OC",
            Self::Ic => "IC",
            Self::BorrowingBase => "BB",
        }
    }
}

// WATERFALL WORKSPACE (Pre-allocated Buffers)

/// Pre-allocated workspace for waterfall execution to avoid hot-path allocations.
///
/// This struct holds reusable buffers that are cleared between periods rather than
/// reallocated. For Monte Carlo simulations with thousands of paths and hundreds
/// of periods, this significantly reduces allocation overhead.
#[derive(Debug, Clone)]
pub struct WaterfallWorkspace {
    /// Pre-allocated tier allocations buffer
    pub tier_allocations: Vec<(String, Money)>,
    /// Pre-allocated distributions map
    pub distributions: HashMap<RecipientType, Money>,
    /// Pre-allocated payment records buffer
    pub payment_records: Vec<PaymentRecord>,
    /// Pre-allocated coverage test results buffer
    pub coverage_tests: Vec<(String, f64, bool)>,
    /// Pre-allocated tranche index (built once per deal, reused across periods)
    pub tranche_index: HashMap<String, usize>,
}

impl WaterfallWorkspace {
    /// Create a new workspace with pre-allocated capacity.
    pub fn new(num_tiers: usize, num_recipients: usize, num_tranches: usize) -> Self {
        let mut distributions = HashMap::default();
        distributions.reserve(num_recipients);
        let mut tranche_index = HashMap::default();
        tranche_index.reserve(num_tranches);
        Self {
            tier_allocations: Vec::with_capacity(num_tiers),
            distributions,
            payment_records: Vec::with_capacity(num_recipients),
            coverage_tests: Vec::with_capacity(num_tranches * 2),
            tranche_index,
        }
    }

    /// Clear all buffers for reuse in the next period.
    pub fn clear(&mut self) {
        self.tier_allocations.clear();
        self.distributions.clear();
        self.payment_records.clear();
        self.coverage_tests.clear();
    }
}

impl Default for WaterfallWorkspace {
    fn default() -> Self {
        Self::new(8, 32, 8)
    }
}

/// Main waterfall engine with tier-based distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Waterfall {
    /// Ordered payment tiers, including [`PaymentType::CoverageTest`] positions
    pub tiers: Vec<WaterfallTier>,
    /// Base currency
    pub base_currency: Currency,
    /// Collateral valuation rules for the OC tests; `None` values collateral
    /// at par with defaulted assets at recovery.
    pub coverage_rules: Option<CoverageRules>,
}

impl Waterfall {
    /// Create a [`WaterfallBuilder`] for constructing a new waterfall engine.
    ///
    /// This is the preferred entry point, consistent with other builder patterns.
    #[must_use]
    pub fn builder(base_currency: Currency) -> WaterfallBuilder {
        WaterfallBuilder::new(base_currency)
    }

    /// Create new waterfall engine
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            tiers: Vec::new(),
            base_currency,
            coverage_rules: None,
        }
    }

    /// Add a tier
    #[must_use]
    pub fn add_tier(mut self, tier: WaterfallTier) -> Self {
        self.tiers.push(tier);
        self.tiers.sort_by_key(|t| t.priority);
        self
    }

    /// Attach collateral valuation rules for the OC tests.
    ///
    /// # Arguments
    ///
    /// * `rules` - Rating haircuts, defaulted-asset valuation, CCC bucket and
    ///   discount-obligation rules applied by every OC test.
    #[must_use]
    pub fn with_coverage_rules(mut self, rules: CoverageRules) -> Self {
        self.coverage_rules = Some(rules);
        self
    }

    /// Every coverage test in the waterfall, in tier priority order.
    pub fn coverage_tests(&self) -> impl Iterator<Item = &CoverageTestSpec> {
        let mut tiers: Vec<&WaterfallTier> = self.tiers.iter().collect();
        tiers.sort_by_key(|tier| tier.priority);
        tiers
            .into_iter()
            .filter(|tier| tier.payment_type == PaymentType::CoverageTest)
            .flat_map(|tier| tier.tests.iter())
    }

    /// Insert `test` as a coverage-test position immediately after the tier
    /// that pays `spec.placement_tranche()`'s interest (or after the last
    /// interest tier when that tranche has no interest recipient), joining an
    /// existing test tier at that position whose tests share the test's
    /// `action` and `divert_pct`, else adding a new test tier after those
    /// already there.
    /// Priorities are renumbered `1..=n` in order.
    ///
    /// # Arguments
    ///
    /// * `test` - Coverage test to place.
    pub fn insert_coverage_test(&mut self, test: CoverageTestSpec) {
        self.tiers.sort_by_key(|tier| tier.priority);
        let (placement, anchor) = match test.placement_tranche() {
            Some(tranche_id) => (
                tranche_id.to_string(),
                self.tiers
                    .iter()
                    .position(|tier| tier.interest_tranche_ids().any(|id| id == tranche_id)),
            ),
            // After the junior fees: the last fee tier that follows the
            // coupons (the template's `junior_fees`), else the last coupon.
            None => (
                "junior_fees".to_string(),
                self.tiers
                    .iter()
                    .rposition(|tier| tier.payment_type == PaymentType::Interest)
                    .map(|last_interest| {
                        self.tiers
                            .iter()
                            .enumerate()
                            .skip(last_interest + 1)
                            .take_while(|(_, tier)| {
                                matches!(
                                    tier.payment_type,
                                    PaymentType::Fee | PaymentType::CoverageTest
                                )
                            })
                            .filter(|(_, tier)| tier.payment_type == PaymentType::Fee)
                            .map(|(index, _)| index)
                            .last()
                            .unwrap_or(last_interest)
                    }),
            ),
        };
        let anchor = anchor.or_else(|| {
            self.tiers
                .iter()
                .rposition(|tier| tier.payment_type == PaymentType::Interest)
        });
        // The run of test tiers already at this position: join the one whose
        // tests share this test's action and diversion cap (a tier diverts
        // once, under one action and one cap), else append a new tier after
        // the run so tests keep their insertion order.
        let run_start = anchor.map_or(0, |i| i + 1);
        let run_end = run_start
            + self.tiers[run_start..]
                .iter()
                .take_while(|tier| tier.payment_type == PaymentType::CoverageTest)
                .count();
        let joinable = self.tiers[run_start..run_end].iter_mut().find(|tier| {
            tier.tests.first().is_none_or(|first| {
                first.action == test.action && first.divert_pct == test.divert_pct
            })
        });
        match joinable {
            Some(existing) => existing.tests.push(test),
            None => {
                let id = if run_end == run_start {
                    format!("{placement}_coverage")
                } else {
                    format!("{placement}_coverage_{}", run_end - run_start + 1)
                };
                let tier = WaterfallTier::coverage_tests(id, 0, vec![test]);
                self.tiers.insert(run_end, tier);
            }
        }
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            tier.priority = index + 1;
        }
    }

    /// Create a standard sequential waterfall for a given tranche structure.
    ///
    /// Shared skeleton:
    /// 1. Fees tier (sequential)
    /// 2. One interest tier per note class in payment-priority order, each
    ///    followed by the coverage tests placed on that class
    /// 3. Principal (sequential, by priority)
    /// 4. Equity residual
    ///
    /// Paying interest class by class (INTEX/Bloomberg ordering) lets a
    /// coverage test sit after any class, so a failing Class D test can only
    /// trap the interest ranked below Class D, and lets
    /// [`Self::fund_senior_interest_from_principal`] single out the senior
    /// coupons. Every tier draws on its default [`FundingSource`].
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Deal currency of the waterfall; tranche balances and fees must
    ///   match this currency.
    /// * `tranches` - Capital structure whose notes become sequential interest and
    ///   principal recipients.
    /// * `fees` - Senior fee recipients (a tier ahead of every note), junior
    ///   fee recipients (a tier after every note coupon, ahead of principal)
    ///   the incentive-fee recipient (ahead of `equity_principal` in the
    ///   principal tier and a tier ahead of the residual) and the net-WAC
    ///   carryover recipients (a tier after principal); empty lists omit
    ///   their tiers.
    /// * `coverage_tests` - Deal-level coverage tests, each placed after the
    ///   interest tier of its [`CoverageTestSpec::placement_tranche`].
    pub fn standard_sequential(
        base_currency: Currency,
        tranches: &super::TrancheStructure,
        fees: TemplateFees,
        coverage_tests: &[CoverageTestSpec],
    ) -> Self {
        let mut engine = Self::new(base_currency);
        let mut priority = 1;

        if !fees.senior.is_empty() {
            let fees_tier = WaterfallTier::new("fees", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential);
            let fees_tier = fees
                .senior
                .into_iter()
                .fold(fees_tier, |tier, recipient| tier.add_recipient(recipient));
            engine.tiers.push(fees_tier);
            priority += 1;
        }

        let mut sorted_tranches = tranches.tranches.clone();
        sorted_tranches.sort_by_key(|t| t.payment_priority);

        // One interest tier per class in payment-priority order, so a
        // coverage-test position can sit after any class and a funding source
        // can single out the senior coupons. Sequential allocation makes the
        // split an identity when nothing sits between the tiers.
        for tranche in &sorted_tranches {
            if tranche.seniority == super::TrancheSeniority::Equity {
                continue;
            }
            let tier = WaterfallTier::new(
                format!("{}_interest", tranche.id.as_str()),
                priority,
                PaymentType::Interest,
            )
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::tranche_interest(
                format!("{}_interest", tranche.id.as_str()),
                tranche.id.as_str(),
            ));
            engine.tiers.push(tier);
            priority += 1;
        }

        // Junior fees (the subordinated management fee) rank after every note
        // coupon and ahead of principal.
        if !fees.junior.is_empty() {
            let junior_tier = WaterfallTier::new("junior_fees", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential);
            let junior_tier = fees
                .junior
                .into_iter()
                .fold(junior_tier, |tier, recipient| tier.add_recipient(recipient));
            engine.tiers.push(junior_tier);
            priority += 1;
        }

        let mut principal_recipients = Vec::new();
        for tranche in &sorted_tranches {
            if tranche.seniority != super::TrancheSeniority::Equity {
                principal_recipients.push(Recipient::tranche_principal(
                    format!("{}_principal", tranche.id.as_str()),
                    tranche.id.as_str(),
                    None,
                ));
            }
        }

        // The manager's incentive fee shares in principal proceeds reaching
        // equity above the hurdle, ahead of the equity principal recipient.
        if let Some(incentive) = fees.incentive.as_ref() {
            let mut principal_incentive = incentive.clone();
            principal_incentive.id = "incentive_fee_principal".to_string();
            principal_recipients.push(principal_incentive);
        }
        principal_recipients.push(Recipient::new(
            "equity_principal",
            RecipientType::Equity,
            PaymentCalculation::ResidualCash,
        ));
        let principal_tier = WaterfallTier::new("principal", priority, PaymentType::Principal)
            .allocation_mode(AllocationMode::Sequential);
        let principal_tier = principal_recipients
            .into_iter()
            .fold(principal_tier, |tier, recipient| {
                tier.add_recipient(recipient)
            });
        engine.tiers.push(principal_tier);
        priority += 1;

        // Net-WAC carryover repayments come out of excess interest ahead of
        // the incentive fee and the residual.
        if !fees.carryover.is_empty() {
            let carryover_tier =
                WaterfallTier::new("net_wac_carryover", priority, PaymentType::Fee)
                    .allocation_mode(AllocationMode::Sequential);
            let carryover_tier = fees
                .carryover
                .into_iter()
                .fold(carryover_tier, |tier, recipient| {
                    tier.add_recipient(recipient)
                });
            engine.tiers.push(carryover_tier);
            priority += 1;
        }

        // The manager's incentive fee takes its share of the residual ahead
        // of equity once the equity IRR hurdle is met.
        if let Some(incentive) = fees.incentive {
            let incentive_tier = WaterfallTier::new("incentive_fee", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(incentive);
            engine.tiers.push(incentive_tier);
            priority += 1;
        }

        // Residual interest reaches equity only after every coverage-test
        // position above has been satisfied.
        let equity_tier = WaterfallTier::new("equity", priority, PaymentType::Residual)
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::new(
                "equity_distribution",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            ));
        engine.tiers.push(equity_tier);

        for test in coverage_tests {
            engine.insert_coverage_test(test.clone());
        }

        engine
    }

    /// Add a hedge-swap counterparty payment at the fee position `priority`.
    ///
    /// `SeniorFee` joins the leading fee tier, or opens one ahead of every
    /// other tier; `JuniorFee` joins the fee tier after the last note coupon
    /// (the junior fee tier), or opens one there ahead of principal.
    /// Priorities are renumbered `1..=n` in order.
    ///
    /// # Arguments
    ///
    /// * `recipient` - Fixed-amount payment to the swap counterparty for the
    ///   period.
    /// * `priority` - Fee position the payment ranks in.
    pub fn insert_hedge_payment(&mut self, recipient: Recipient, priority: super::SwapPriority) {
        self.tiers.sort_by_key(|tier| tier.priority);
        match priority {
            super::SwapPriority::SeniorFee => match self.tiers.first_mut() {
                Some(first) if first.payment_type == PaymentType::Fee => {
                    first.recipients.push(recipient);
                }
                _ => {
                    let tier = WaterfallTier::new("hedge_fees", 0, PaymentType::Fee)
                        .add_recipient(recipient);
                    self.tiers.insert(0, tier);
                }
            },
            super::SwapPriority::JuniorFee => {
                // After the last note coupon (and the coverage tests placed on
                // it), ahead of principal and the residual.
                let last_interest = self
                    .tiers
                    .iter()
                    .rposition(|tier| tier.payment_type == PaymentType::Interest);
                let anchor = self
                    .tiers
                    .iter()
                    .enumerate()
                    .position(|(index, tier)| {
                        last_interest.is_none_or(|last| index > last)
                            && matches!(
                                tier.payment_type,
                                PaymentType::Principal | PaymentType::Residual
                            )
                    })
                    .unwrap_or(self.tiers.len());
                let joins_junior_fee_tier = anchor > 0
                    && self.tiers[anchor - 1].payment_type == PaymentType::Fee
                    && last_interest.is_some_and(|last| anchor - 1 > last);
                if joins_junior_fee_tier {
                    self.tiers[anchor - 1].recipients.push(recipient);
                } else {
                    let tier = WaterfallTier::new("junior_hedge_fees", 0, PaymentType::Fee)
                        .add_recipient(recipient);
                    self.tiers.insert(anchor, tier);
                }
            }
        }
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            tier.priority = index + 1;
        }
    }

    /// Let senior fees and senior note interest top up from principal
    /// proceeds (the CLO principal-waterfall convention).
    ///
    /// Sets [`FundingSource::InterestThenPrincipal`] on every `Fee` tier ranked
    /// ahead of the first interest tier and on every `Interest` tier whose
    /// interest recipients are all non-deferrable claims of `tranches`
    /// ([`Tranche::is_non_deferrable`](super::Tranche::is_non_deferrable):
    /// senior notes unless a class says otherwise). Tiers paying any
    /// deferrable coupon are left on interest proceeds.
    ///
    /// # Arguments
    ///
    /// * `tranches` - Capital structure used to classify each interest
    ///   recipient's seniority.
    pub fn fund_senior_interest_from_principal(&mut self, tranches: &super::TrancheStructure) {
        let is_senior = |id: &str| {
            tranches
                .tranches
                .iter()
                .any(|t| t.id.as_str() == id && t.is_non_deferrable())
        };
        let first_interest = self
            .tiers
            .iter()
            .position(|tier| tier.payment_type == PaymentType::Interest)
            .unwrap_or(self.tiers.len());
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            let senior_only = match tier.payment_type {
                // Only fees ranked ahead of the notes; junior and incentive
                // fee tiers stay on interest proceeds.
                PaymentType::Fee => index < first_interest,
                PaymentType::Interest => {
                    let mut ids = tier.interest_tranche_ids().peekable();
                    ids.peek().is_some() && ids.all(is_senior)
                }
                _ => false,
            };
            if senior_only {
                tier.funding = Some(FundingSource::InterestThenPrincipal);
            }
        }
    }
}

/// Fee recipients of the standard template, by rank.
#[derive(Debug, Clone, Default)]
pub struct TemplateFees {
    /// Senior fees paid ahead of every note (trustee, senior management,
    /// servicing); an empty list omits the fees tier.
    pub senior: Vec<Recipient>,
    /// Junior fees paid after every note coupon and ahead of principal (the
    /// subordinated management fee); an empty list omits the tier.
    pub junior: Vec<Recipient>,
    /// Manager incentive fee on the cash reaching equity above its IRR
    /// hurdle ([`PaymentCalculation::IncentiveFee`]): placed in the principal
    /// tier ahead of `equity_principal` and in a tier ahead of the residual.
    pub incentive: Option<Recipient>,
    /// Net-WAC carryover recipients ([`PaymentCalculation::NetWacCarryover`]),
    /// one per capped tranche, in a tier after principal and ahead of the
    /// incentive fee; empty omits the tier.
    pub carryover: Vec<Recipient>,
}

/// Builder for waterfall engine
pub struct WaterfallBuilder {
    engine: Waterfall,
    next_priority: usize,
}

impl WaterfallBuilder {
    /// Create new builder
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            engine: Waterfall::new(base_currency),
            next_priority: 1,
        }
    }

    /// Add a tier
    #[must_use]
    pub fn add_tier(mut self, mut tier: WaterfallTier) -> Self {
        if tier.priority == 0 {
            tier.priority = self.next_priority;
            self.next_priority += 1;
        }
        self.engine = self.engine.add_tier(tier);
        self
    }

    /// Attach coverage test rules (haircuts, par thresholds).
    #[must_use]
    pub fn coverage_rules(mut self, rules: CoverageRules) -> Self {
        self.engine = self.engine.with_coverage_rules(rules);
        self
    }

    /// Build the waterfall engine
    pub fn build(self) -> finstack_quant_core::Result<Waterfall> {
        Ok(self.engine)
    }
}
