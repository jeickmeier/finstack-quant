//! Instrument-backed collateral for structured-credit pools.
//!
//! A pool may hold real instruments — the universal [`Bond`] in every form the
//! bond module supports, [`TermLoan`] (including delayed-draw commitments) and
//! [`RevolvingCredit`] — instead of balance-and-rate [`PoolAsset`] rows or
//! representative lines. The instruments are retained on the pool so the
//! simulation engine can drive period flows from their own schedules; for
//! every existing balance consumer (pool statistics, coverage tests, loss
//! allocation) they are materialized into [`PoolAsset`] rows by
//! [`InstrumentCollateral::materialize`].
//!
//! Call and put options on the collateral need an exercise rule the pool
//! engine can evaluate period by period; [`CallExercisePolicy`] and
//! [`PutExercisePolicy`] carry the deal-level rule with per-instrument
//! overrides. [`ReserveInterestDestination`] says where the earnings on the
//! deal reserve account go.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{HashSet, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::AssetType;
use super::pool::PoolAsset;
use crate::cashflow::builder::FloatingRateSpec;
use crate::instruments::fixed_income::bond::{Bond, CashflowSpec};
use crate::instruments::fixed_income::revolving_credit::{BaseRateSpec, RevolvingCredit};
use crate::instruments::fixed_income::term_loan::{RateSpec, TermLoan};

/// Rule the pool engine applies to issuer call options on the collateral.
///
/// Set at the deal level on [`InstrumentCollateral::call_exercise`] and
/// overridable per instrument through [`InstrumentExerciseOverride`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "policy", rename_all = "snake_case", deny_unknown_fields)]
pub enum CallExercisePolicy {
    /// Never exercise: the instrument runs to its contractual schedule.
    #[default]
    Contractual,
    /// The issuer calls at the first call date, at that date's call price.
    FirstCall,
    /// Redeem at the yield-to-worst date implied by the instrument's quoted
    /// clean price. Requires a quoted price on every instrument it applies to.
    Worst,
    /// The issuer calls at the next call date when the refinancing rate (the
    /// index forward, or the simulated path rate, plus the instrument's own
    /// spread) is below the current coupon by more than `threshold_bp`.
    RefinancingIncentive {
        /// Minimum coupon-over-refinancing-rate saving, in basis points, that
        /// triggers exercise.
        threshold_bp: f64,
    },
}

/// Rule the pool engine applies to holder put options on the collateral.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "policy", rename_all = "snake_case", deny_unknown_fields)]
pub enum PutExercisePolicy {
    /// Never exercise.
    #[default]
    Never,
    /// The holder puts at the first put date, at that date's put price.
    FirstPut,
    /// The holder puts at the next put date when the reinvestment rate (the
    /// par forward on the deal discount curve to the instrument's maturity,
    /// plus the simulated path's shift) is above the current coupon by more
    /// than `threshold_bp`. Floating instruments never exercise under it.
    ReinvestmentIncentive {
        /// Minimum reinvestment-rate-over-coupon pickup, in basis points,
        /// that triggers exercise.
        threshold_bp: f64,
    },
}

/// Per-instrument override of the deal-level exercise policies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InstrumentExerciseOverride {
    /// Identifier of the bond or term loan the override applies to.
    pub id: InstrumentId,
    /// Call policy for this instrument; `None` keeps the deal-level policy.
    #[serde(default)]
    pub call: Option<CallExercisePolicy>,
    /// Put policy for this instrument; `None` keeps the deal-level policy.
    #[serde(default)]
    pub put: Option<PutExercisePolicy>,
}

/// Where the interest earned on the deal reserve account is paid.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReserveInterestDestination {
    /// Interest proceeds available to the waterfall (default).
    #[default]
    Waterfall,
    /// Paid directly to one tranche (the equity tranche, typically) before the
    /// waterfall runs. Excluded from the interest-coverage numerator.
    Tranche {
        /// Identifier of the receiving tranche; must exist in the deal.
        tranche_id: InstrumentId,
    },
    /// Capitalized into the reserve account.
    Retain,
}

impl ReserveInterestDestination {
    /// `true` for the default destination, used to omit it from serialized pools.
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Waterfall)
    }
}

/// Real instruments held as pool collateral.
///
/// Exactly one collateral representation may be populated on an
/// [`super::AssetPool`]: `assets`, `rep_lines` or `instruments`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InstrumentCollateral {
    /// Bonds in any form: fixed, floating, step-up, amortizing, custom
    /// cashflows, with call/put schedules and return floors.
    #[serde(default)]
    pub bonds: Vec<Bond>,
    /// Term loans, including delayed-draw commitments, PIK and call schedules.
    #[serde(default)]
    pub term_loans: Vec<TermLoan>,
    /// Revolving credit facilities.
    #[serde(default)]
    pub revolvers: Vec<RevolvingCredit>,
    /// Deal-level call exercise rule.
    #[serde(default)]
    pub call_exercise: CallExercisePolicy,
    /// Deal-level put exercise rule.
    #[serde(default)]
    pub put_exercise: PutExercisePolicy,
    /// Per-instrument exercise overrides.
    #[serde(default)]
    pub overrides: Vec<InstrumentExerciseOverride>,
}

/// One instrument of the collateral, borrowed by kind.
#[derive(Debug, Clone, Copy)]
pub enum CollateralInstrument<'a> {
    /// A bond.
    Bond(&'a Bond),
    /// A term loan.
    TermLoan(&'a TermLoan),
    /// A revolving credit facility.
    Revolver(&'a RevolvingCredit),
}

impl<'a> CollateralInstrument<'a> {
    /// Instrument identifier, borrowed from the collateral.
    pub fn id(self) -> &'a InstrumentId {
        match self {
            Self::Bond(bond) => &bond.id,
            Self::TermLoan(loan) => &loan.id,
            Self::Revolver(facility) => &facility.id,
        }
    }

    /// `true` when the instrument carries at least one issuer call option.
    pub fn has_calls(self) -> bool {
        match self {
            Self::Bond(bond) => bond
                .call_put
                .as_ref()
                .is_some_and(|schedule| !schedule.calls.is_empty()),
            Self::TermLoan(loan) => loan
                .call_schedule
                .as_ref()
                .is_some_and(|schedule| !schedule.calls.is_empty()),
            Self::Revolver(_) => false,
        }
    }

    /// Quoted clean price from the instrument's pricing overrides, if any.
    pub fn quoted_clean_price(self) -> Option<f64> {
        match self {
            Self::Bond(bond) => {
                bond.instrument_pricing_overrides
                    .market_quotes
                    .quoted_clean_price
            }
            Self::TermLoan(loan) => {
                loan.instrument_pricing_overrides
                    .market_quotes
                    .quoted_clean_price
            }
            Self::Revolver(_) => None,
        }
    }
}

impl InstrumentCollateral {
    /// Number of instruments across the three lists.
    pub fn len(&self) -> usize {
        self.bonds.len() + self.term_loans.len() + self.revolvers.len()
    }

    /// `true` when no instrument is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate every instrument in a stable order: bonds, term loans, revolvers.
    pub fn iter(&self) -> impl Iterator<Item = CollateralInstrument<'_>> {
        self.bonds
            .iter()
            .map(CollateralInstrument::Bond)
            .chain(self.term_loans.iter().map(CollateralInstrument::TermLoan))
            .chain(self.revolvers.iter().map(CollateralInstrument::Revolver))
    }

    /// Effective call policy for one instrument (override, else deal level).
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier to look up in `overrides`.
    pub fn call_policy_for(&self, id: &InstrumentId) -> CallExercisePolicy {
        self.overrides
            .iter()
            .find(|o| &o.id == id)
            .and_then(|o| o.call.clone())
            .unwrap_or_else(|| self.call_exercise.clone())
    }

    /// Effective put policy for one instrument (override, else deal level).
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier to look up in `overrides`.
    pub fn put_policy_for(&self, id: &InstrumentId) -> PutExercisePolicy {
        self.overrides
            .iter()
            .find(|o| &o.id == id)
            .and_then(|o| o.put.clone())
            .unwrap_or_else(|| self.put_exercise.clone())
    }

    /// Validate the collateral against the pool base currency.
    ///
    /// Checks each instrument's own invariants, that every monetary field is
    /// in `base_currency`, that identifiers are unique across the three lists,
    /// that every override names a held instrument, and that a `Worst` call
    /// policy only applies to instruments carrying a quoted clean price.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Pool base currency every instrument must use.
    pub fn validate(&self, base_currency: Currency) -> Result<()> {
        let mut seen: HashSet<&str> = HashSet::default();
        for instrument in self.iter() {
            let id = instrument.id();
            if !seen.insert(id.as_str()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "instrument collateral contains duplicate id '{id}'"
                )));
            }
            match instrument {
                CollateralInstrument::Bond(bond) => {
                    bond.validate()?;
                    require_currency(bond.notional.currency(), base_currency, id)?;
                }
                CollateralInstrument::TermLoan(loan) => {
                    loan.validate()?;
                    require_currency(loan.currency, base_currency, id)?;
                    require_currency(loan.notional_limit.currency(), base_currency, id)?;
                }
                CollateralInstrument::Revolver(facility) => {
                    facility.validate()?;
                    require_currency(facility.commitment_amount.currency(), base_currency, id)?;
                }
            }
        }
        for override_ in &self.overrides {
            if !seen.contains(override_.id.as_str()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "exercise override names unknown instrument '{}'",
                    override_.id
                )));
            }
        }
        for instrument in self.iter() {
            let id = instrument.id();
            if instrument.has_calls()
                && self.call_policy_for(id) == CallExercisePolicy::Worst
                && instrument.quoted_clean_price().is_none()
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "call policy 'worst' requires a quoted clean price on instrument '{id}' \
                     (set instrument_pricing_overrides.market_quotes.quoted_clean_price)"
                )));
            }
        }
        Ok(())
    }

    /// Sum of current outstanding balances.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Currency of the returned total.
    pub fn total_balance(&self, base_currency: Currency) -> Result<Money> {
        self.iter()
            .try_fold(Money::from((0_i64, base_currency)), |acc, instrument| {
                acc.checked_add(current_balance(instrument, None)?)
            })
    }

    /// Materialize every instrument into a [`PoolAsset`] row.
    ///
    /// Balances are the current outstanding amounts (a revolver's drawn
    /// amount, a delayed-draw loan's cumulative draws to `closing_date`, a
    /// bond's notional); coupons are the contractual current rate, with the
    /// spread and index recorded separately for floating instruments;
    /// commitments are carried for revolvers and delayed-draw loans. The
    /// instrument-schedule flow source replaces these opening balances with
    /// the schedule's outstanding at the valuation date.
    ///
    /// # Arguments
    ///
    /// * `closing_date` - Deal closing date, used to count delayed draws that
    ///   have already occurred and as the acquisition date of the rows.
    pub(crate) fn materialize(&self, closing_date: Date) -> Result<Vec<PoolAsset>> {
        let mut rows = Vec::with_capacity(self.len());
        for bond in &self.bonds {
            let (rate, spread_bp, index_id) = bond_economics(&bond.cashflow_spec)?;
            rows.push(PoolAsset {
                id: bond.id.clone(),
                asset_type: AssetType::HighYieldBond { industry: None },
                balance: bond.notional,
                rate,
                spread_bp,
                index_id,
                maturity: bond.maturity,
                index_floor: None,
                credit_quality: None,
                industry: None,
                obligor_id: None,
                is_defaulted: false,
                recovery_amount: None,
                default_date: None,
                purchase_price: None,
                acquisition_date: None,
                origination_date: Some(bond.issue_date),
                day_count: bond.cashflow_spec.day_count(),
                smm_override: None,
                mdr_override: None,
                recovery_rate: None,
                commitment: None,
                contractual_payment: None,
                amortization_term_months: None,
                io_months: None,
                market_price_pct: None,
                delinquency_buckets: None,
                balloon: None,
                prepayment_penalty: None,
                special_servicing: None,
                noi: None,
                liquidation: None,
            });
        }
        for loan in &self.term_loans {
            let (rate, spread_bp, index_id) = match &loan.rate {
                RateSpec::Fixed { rate_bp } => (f64::from(*rate_bp) / 10_000.0, None, None),
                RateSpec::Floating(spec) => floating_economics(spec)?,
            };
            rows.push(PoolAsset {
                id: loan.id.clone(),
                asset_type: AssetType::FirstLienLoan { industry: None },
                balance: current_balance(CollateralInstrument::TermLoan(loan), Some(closing_date))?,
                rate,
                spread_bp,
                index_id,
                maturity: loan.maturity,
                index_floor: None,
                credit_quality: None,
                industry: None,
                obligor_id: None,
                is_defaulted: false,
                recovery_amount: None,
                default_date: None,
                purchase_price: None,
                acquisition_date: None,
                origination_date: Some(loan.issue_date),
                day_count: loan.day_count,
                smm_override: None,
                mdr_override: None,
                recovery_rate: None,
                commitment: loan.ddtl.as_ref().map(|ddtl| ddtl.commitment_limit),
                contractual_payment: None,
                amortization_term_months: None,
                io_months: None,
                market_price_pct: None,
                delinquency_buckets: None,
                balloon: None,
                prepayment_penalty: None,
                special_servicing: None,
                noi: None,
                liquidation: None,
            });
        }
        for facility in &self.revolvers {
            let (rate, spread_bp, index_id) = match &facility.base_rate_spec {
                BaseRateSpec::Fixed { rate } => (*rate, None, None),
                BaseRateSpec::Floating(spec) => floating_economics(spec)?,
            };
            rows.push(PoolAsset {
                id: facility.id.clone(),
                asset_type: AssetType::RevolverLoan { industry: None },
                balance: facility.drawn_amount,
                rate,
                spread_bp,
                index_id,
                maturity: facility.maturity,
                index_floor: None,
                credit_quality: None,
                industry: None,
                obligor_id: None,
                is_defaulted: false,
                recovery_amount: None,
                default_date: None,
                purchase_price: None,
                acquisition_date: None,
                origination_date: Some(facility.commitment_date),
                day_count: facility.day_count,
                smm_override: None,
                mdr_override: None,
                recovery_rate: Some(facility.recovery_rate),
                commitment: Some(facility.commitment_amount),
                contractual_payment: None,
                amortization_term_months: None,
                io_months: None,
                market_price_pct: None,
                delinquency_buckets: None,
                balloon: None,
                prepayment_penalty: None,
                special_servicing: None,
                noi: None,
                liquidation: None,
            });
        }
        Ok(rows)
    }
}

/// Current outstanding balance of one instrument.
///
/// Delayed-draw loans count the draws dated on or before `as_of` (all draws
/// when `as_of` is `None`); every other instrument reports its notional or
/// drawn amount.
fn current_balance(instrument: CollateralInstrument<'_>, as_of: Option<Date>) -> Result<Money> {
    match instrument {
        CollateralInstrument::Bond(bond) => Ok(bond.notional),
        CollateralInstrument::TermLoan(loan) => match &loan.ddtl {
            Some(ddtl) => ddtl
                .draws
                .iter()
                .filter(|draw| {
                    draw.date >= ddtl.availability_start
                        && draw.date <= ddtl.availability_end
                        && as_of.is_none_or(|date| draw.date <= date)
                })
                .try_fold(Money::from((0_i64, loan.currency)), |acc, draw| {
                    acc.checked_add(draw.amount)
                }),
            None => Ok(loan.notional_limit),
        },
        CollateralInstrument::Revolver(facility) => Ok(facility.drawn_amount),
    }
}

fn require_currency(actual: Currency, expected: Currency, id: &InstrumentId) -> Result<()> {
    if actual != expected {
        return Err(finstack_quant_core::Error::Validation(format!(
            "instrument '{id}' is denominated in {actual} but the pool base currency is {expected}"
        )));
    }
    Ok(())
}

fn decimal_to_f64(value: Decimal, what: &str) -> Result<f64> {
    value.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{what} {value} cannot be represented as f64"
        ))
    })
}

/// `(rate, spread_bp, index_id)` for a floating-rate spec: the row's coupon
/// is the spread until the index is projected by the engine.
fn floating_economics(spec: &FloatingRateSpec) -> Result<(f64, Option<f64>, Option<String>)> {
    let spread_bp = decimal_to_f64(spec.spread_bp, "floating spread")?;
    Ok((
        spread_bp / 10_000.0,
        Some(spread_bp),
        Some(spec.index_id.to_string()),
    ))
}

/// `(rate, spread_bp, index_id)` for a bond cashflow specification.
fn bond_economics(spec: &CashflowSpec) -> Result<(f64, Option<f64>, Option<String>)> {
    match spec {
        CashflowSpec::Fixed(fixed) => Ok((decimal_to_f64(fixed.rate, "bond coupon")?, None, None)),
        CashflowSpec::Floating(floating) => floating_economics(&floating.rate_spec),
        CashflowSpec::StepUp(step_up) => Ok((
            decimal_to_f64(step_up.initial_rate, "step-up initial coupon")?,
            None,
            None,
        )),
        CashflowSpec::Amortizing { base, .. } => bond_economics(base),
    }
}
