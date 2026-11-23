//! Public period-by-period and aggregate simulation diagnostics.

use super::state::{SimulationState, pool_composition};
use crate::structured_credit::types::{Tranche, WaterfallDistribution};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// One coverage test as the waterfall executor evaluated it in a period.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CoverageTestDiagnostic {
    /// `CoverageTestSpec::id` of the test.
    pub test_id: String,
    /// Ratio the executor computed (OC: collateral ÷ notes; IC: interest ÷ due).
    /// `None` when the test has no claim outstanding: the notes it covers are
    /// retired (OC) or owe no interest (IC), so the denominator is zero and no
    /// ratio exists. Such a test passes.
    pub ratio: Option<f64>,
    /// Trigger level the ratio was tested against.
    pub trigger_level: f64,
    /// `ratio − trigger_level`; negative while the test fails. `None` when `ratio`
    /// is, for the same reason.
    pub cushion: Option<f64>,
    /// Whether the test passed in this period.
    pub passing: bool,
}

/// When one coverage test first failed over a simulation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CoverageTestBreach {
    /// Id of the test: `CoverageTestDiagnostic::test_id` for a test the
    /// waterfall carries (`OC_<tranche>`), or `OC_<tranche>_trigger` /
    /// `IC_<tranche>_trigger` for a tranche's own `oc_trigger` / `ic_trigger`.
    pub test_id: String,
    /// First payment date this simulation found the test failing; a later cure
    /// does not clear it. `None` when it never failed.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub first_breach: Option<Date>,
}

/// One tranche's credit enhancement at a period's end.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TrancheCreditEnhancement {
    /// Id of the tranche (`Tranche::id`).
    pub tranche_id: String,
    /// Share of the collateral beneath this tranche, as a decimal (`0.30` is
    /// 30%): `(C − N) ÷ C`, where `C` is the collateral balance plus the
    /// controlled-accumulation funding account and `N` the balances of this
    /// tranche and every tranche paid ahead of it (`payment_priority` at or
    /// below its own). Deferred interest is not in `N`. Negative when the notes
    /// exceed the collateral, and zero when there is no collateral. Pari-passu
    /// classes rank by `payment_priority`, so each is enhanced only by those
    /// ranked after it; an equity class's value is the collateral left above
    /// the whole structure.
    pub credit_enhancement: f64,
}

/// Where cash paid to a tranche outside the waterfall came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NonWaterfallSource {
    /// Reserve-account interest routed straight to a named tranche
    /// (`ReserveInterestDestination::Tranche`).
    ReserveInterest,
    /// A deal call or clean-up call redemption, including the surplus paid to
    /// the most junior class after every note is redeemed.
    Redemption,
    /// Excess-spread account and retained interest released at deal end.
    SpreadAccountRelease,
    /// Controlled-accumulation funding account and retained principal released
    /// at deal end.
    PrincipalAccountRelease,
    /// Reserve account released at deal end.
    ReserveAccountRelease,
    /// A recovery claim still pending at deal end, settled at its lagged date.
    RecoveryRelease,
}

/// Cash paid to one tranche outside the waterfall.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct NonWaterfallPayment {
    /// Date the tranche received the cash. A recovery settling after the last
    /// period carries its own later date here.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Id of the tranche paid (`Tranche::id`), equity classes included.
    pub tranche_id: String,
    /// Where the cash came from.
    pub source: NonWaterfallSource,
    /// Part booked to the tranche's interest flows.
    pub interest: Money,
    /// Part booked to the tranche's principal flows, net of `incentive_fee`.
    pub principal: Money,
    /// Manager incentive fee withheld from the cash before it reached an
    /// equity class; zero otherwise. Already counted in the period's
    /// `fees_paid`, and not part of what the tranche received.
    pub incentive_fee: Money,
}

impl NonWaterfallPayment {
    /// A payment of `interest` and `principal` with no incentive fee withheld.
    pub(super) fn new(
        payment_date: Date,
        tranche_id: &str,
        source: NonWaterfallSource,
        interest: Money,
        principal: Money,
    ) -> Self {
        Self {
            payment_date,
            tranche_id: tranche_id.to_string(),
            source,
            interest,
            principal,
            incentive_fee: Money::from((0_i64, interest.currency())),
        }
    }
}

/// Deal accounting for one payment period, recorded after the waterfall.
///
/// Balances are end-of-period; collections, defaults, recoveries and fees are
/// the period's amounts; composition statistics are balance-weighted over the
/// performing pool at period end.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PeriodDiagnostics {
    /// Payment date of the period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Collateral balance at period end.
    pub pool_balance: Money,
    /// `pool_balance` divided by the original (cut-off) pool balance
    /// (`AssetPool::original_balance_or_reconstructed`).
    pub pool_factor: f64,
    /// Weighted-average coupon (annual decimal) of the performing fixed-rate
    /// collateral at period end: the same population as `AssetPool::wac`. `None`
    /// when no performing fixed-rate balance remains.
    pub wac: Option<f64>,
    /// Balance-weighted spread of the performing floating-rate collateral (basis
    /// points). `None` when no performing floating-rate balance remains.
    pub weighted_avg_spread_bp: Option<f64>,
    /// Balance-weighted Moody's rating factor of the performing collateral. `None`
    /// when no performing balance remains.
    pub warf: Option<f64>,
    /// Interest collected from the pool (including servicer advances).
    pub interest_collections: Money,
    /// Scheduled and prepaid principal collected from the pool.
    pub principal_collections: Money,
    /// Par that defaulted (charged off) this period.
    pub defaults: Money,
    /// Recovery cash released to the waterfall this period.
    pub recoveries: Money,
    /// Principal recycled into replacement collateral this period.
    pub reinvested_par: Money,
    /// Fees paid through the waterfall this period.
    pub fees_paid: Money,
    /// Reserve account balance at period end.
    pub reserve_balance: Money,
    /// Excess-spread account balance at period end.
    pub spread_account: Money,
    /// Controlled-accumulation funding account at period end.
    pub funding_account: Money,
    /// Delinquent collateral balance at period end (delinquency model).
    pub delinquent_balance: Money,
    /// Servicer advances of missed principal and interest outstanding at
    /// period end (not yet repaid by cures or liquidations).
    pub servicer_advances_outstanding: Money,
    /// Excess spread realized this period, annualized, as a decimal of the
    /// opening pool balance (`0.02` is 2% a year): `(I − D − F − L) ÷ P × 12 ÷ m`.
    /// `I` is `interest_collections`, `D` the debt coupons due this period, `F`
    /// the service-provider and manager fees paid through the waterfall (not
    /// special-servicing fees, already out of `I`), `L` the period's defaults less
    /// pool recoveries, floored at zero, `P` the pool balance before this period's
    /// flows and `m` the months in the period. The early-amortization excess-spread
    /// test reads this number. Zero when `P` is zero and on a settlement record,
    /// where no waterfall ran.
    pub excess_spread: f64,
    /// Every coverage test the executor evaluated this period.
    pub coverage_tests: Vec<CoverageTestDiagnostic>,
    /// How this period's waterfall ran: who was paid, how much, and what was
    /// left. Its recipient-keyed totals are not serialized; `payment_records`
    /// carries the same cash.
    pub waterfall: WaterfallDistribution,
    /// Cash paid to tranches outside the waterfall: reserve interest routed to
    /// a tranche, and, on the deal's last period, its redemption and terminal
    /// releases. Together with `waterfall.payment_records` this is every
    /// tranche's cash: per tranche and date, the two sum to the tranche's
    /// `cashflows`, and to its `interest_flows` plus `principal_flows`.
    pub non_waterfall_payments: Vec<NonWaterfallPayment>,
    /// Every tranche's credit enhancement on this period's end-of-period
    /// balances, in `TrancheStructure` order. The step-down trigger
    /// `MinCreditEnhancement` reads the senior tranche's value from the same
    /// function on the next payment date, before that date's principal is paid,
    /// so with no losses or draws in between it tests this period's number.
    pub credit_enhancement: Vec<TrancheCreditEnhancement>,
}

impl PeriodDiagnostics {
    /// The record of a payment date from the end-of-period state, with no cash
    /// moved: the base each period adds its flows to.
    pub(super) fn from_state(state: &SimulationState, payment_date: Date) -> Result<Self> {
        let zero = Money::from((0_i64, state.currency));
        let pool_factor = if state.original_pool_balance.amount() > 0.0 {
            state.pool_outstanding.amount() / state.original_pool_balance.amount()
        } else {
            0.0
        };
        let (wac, weighted_avg_spread_bp, warf) = pool_composition(state)?;
        let delinquent: f64 = state
            .pool_state
            .delinquent
            .iter()
            .map(|buckets| super::delinquency::delinquent_balance(buckets))
            .sum();
        Ok(Self {
            payment_date,
            pool_balance: state.pool_outstanding,
            pool_factor,
            wac,
            weighted_avg_spread_bp,
            warf,
            interest_collections: zero,
            principal_collections: zero,
            defaults: zero,
            recoveries: zero,
            reinvested_par: zero,
            fees_paid: zero,
            reserve_balance: state.reserve_balance,
            spread_account: state.spread_account,
            funding_account: state.principal_funding_account,
            delinquent_balance: Money::from_model_value(delinquent, state.currency)?,
            servicer_advances_outstanding: Money::from_model_value(
                state.servicer_advances_outstanding(),
                state.currency,
            )?,
            excess_spread: 0.0,
            coverage_tests: Vec::new(),
            waterfall: WaterfallDistribution::empty(payment_date, state.currency),
            non_waterfall_payments: Vec::new(),
            credit_enhancement: state
                .tranches
                .tranches
                .iter()
                .map(|tranche| TrancheCreditEnhancement {
                    tranche_id: tranche.id.to_string(),
                    credit_enhancement: tranche_credit_enhancement(state, tranche),
                })
                .collect(),
        })
    }
}

/// Deal-level accounting produced alongside the tranche cashflows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SimulationDiagnostics {
    /// Per-period deal accounting, in payment order. A deal with no payment
    /// period left that still settles cash (a fully defaulted pool with
    /// recoveries pending, or a funded reserve) has one record, holding that
    /// settlement in `non_waterfall_payments`.
    #[serde(default)]
    pub periods: Vec<PeriodDiagnostics>,
    /// Reserve-account balance at the end of each simulated period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub reserve_balance_path: Vec<(Date, Money)>,
    /// Reserve interest earned each period, before routing to its destination.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub reserve_interest_paid: Vec<(Date, Money)>,
    /// Collateral draws funded from the reserve account over the simulation.
    pub draws_from_reserve: Money,
    /// Collateral draws funded from principal collections over the simulation.
    pub draws_from_principal: Money,
    /// Collateral draws that could not be funded over the simulation.
    pub unfunded_draws: Money,
    /// Revolver repayments diverted to replenish the reserve over the simulation.
    pub reserve_replenished: Money,
    /// Payment date on which an early-amortization event (or a coverage-test
    /// acceleration) ended the revolving period, when one fired.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub early_amortization_date: Option<Date>,
    /// Lender draws applied to notes, as `(tranche id, payment date, amount)`.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(String, finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub tranche_draws: Vec<(String, Date, Money)>,
    /// First failure of each coverage test, one entry per test sorted by
    /// `test_id`. Covers every test the waterfall carries (each also on
    /// `periods[i].coverage_tests`) and every tranche `oc_trigger` /
    /// `ic_trigger`, whatever its consequence (only a breached `DivertCashFlow`
    /// trigger is on `coverage_tests`, for the periods it is in breach). A deal
    /// with no coverage test has no entries, and a test the waterfall carries is
    /// listed only if a period evaluated it, so a settlement with no waterfall run
    /// lists none of those.
    pub first_breaches: Vec<CoverageTestBreach>,
}

impl SimulationDiagnostics {
    /// Project the stored period record into nineteen ordered table columns.
    ///
    /// Columns are `date`, `pool_balance`, `pool_factor`, `wac`,
    /// `weighted_avg_spread_bp`, `warf`, `interest_collections`,
    /// `principal_collections`, `defaults`, `recoveries`, `reinvested_par`,
    /// `fees_paid`, `reserve_balance`, `reserve_interest`, `spread_account`,
    /// `funding_account`, `delinquent_balance`, `servicer_advances_outstanding`
    /// and `excess_spread`. Dates are ISO 8601 strings; `wac`,
    /// `weighted_avg_spread_bp` and `warf` are nullable Float64, null where their
    /// population is empty; all other columns are Float64, including on an empty
    /// table.
    ///
    /// Rows retain `periods` order and duplicate payment dates. Reserve interest
    /// is the first entry in `reserve_interest_paid` with the same date, or zero
    /// when absent; entries without a matching period create no rows. Monetary
    /// values retain their stored currency-unit magnitudes without currency
    /// conversion or aggregation. WAC and excess spread are annual decimals,
    /// weighted-average spread is in basis points, pool factor is a ratio and
    /// WARF is the weighted-average rating factor.
    ///
    /// # Errors
    ///
    /// Returns an error if the projected columns fail table-envelope validation.
    pub fn to_table(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_core::table::TableEnvelope> {
        use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};

        let float = |name: &str, read: &dyn Fn(&PeriodDiagnostics) -> f64| {
            TableColumn::new(
                name,
                TableColumnData::Float64(self.periods.iter().map(read).collect()),
            )
        };
        let nullable = |name: &str, read: fn(&PeriodDiagnostics) -> Option<f64>| {
            TableColumn::new(
                name,
                TableColumnData::NullableFloat64(self.periods.iter().map(read).collect()),
            )
        };
        let columns = vec![
            TableColumn::new(
                "date",
                TableColumnData::String(
                    self.periods
                        .iter()
                        .map(|period| period.payment_date.to_string())
                        .collect(),
                ),
            ),
            float("pool_balance", &|p| p.pool_balance.amount()),
            float("pool_factor", &|p| p.pool_factor),
            nullable("wac", |p| p.wac),
            nullable("weighted_avg_spread_bp", |p| p.weighted_avg_spread_bp),
            nullable("warf", |p| p.warf),
            float("interest_collections", &|p| p.interest_collections.amount()),
            float("principal_collections", &|p| {
                p.principal_collections.amount()
            }),
            float("defaults", &|p| p.defaults.amount()),
            float("recoveries", &|p| p.recoveries.amount()),
            float("reinvested_par", &|p| p.reinvested_par.amount()),
            float("fees_paid", &|p| p.fees_paid.amount()),
            float("reserve_balance", &|p| p.reserve_balance.amount()),
            float("reserve_interest", &|p| {
                self.reserve_interest_paid
                    .iter()
                    .find(|(date, _)| *date == p.payment_date)
                    .map_or(0.0, |(_, amount)| amount.amount())
            }),
            float("spread_account", &|p| p.spread_account.amount()),
            float("funding_account", &|p| p.funding_account.amount()),
            float("delinquent_balance", &|p| p.delinquent_balance.amount()),
            float("servicer_advances_outstanding", &|p| {
                p.servicer_advances_outstanding.amount()
            }),
            float("excess_spread", &|p| p.excess_spread),
        ];
        TableEnvelope::new(columns, Default::default())
    }
}

/// Credit enhancement of `tranche` on the state's current balances; see
/// [`TrancheCreditEnhancement::credit_enhancement`] for the definition.
///
/// # Arguments
///
/// * `state` - Simulation state supplying collateral and note balances.
/// * `tranche` - Tranche to measure; its `payment_priority` decides which
///   notes rank at or ahead of it.
///
/// # Returns
///
/// The decimal share of collateral beneath the tranche.
pub(super) fn tranche_credit_enhancement(state: &SimulationState, tranche: &Tranche) -> f64 {
    // The funding account's principal has left the asset balances but is
    // still collateral for the notes (see `step_down_metrics`).
    let collateral = state.pool_outstanding.amount() + state.principal_funding_account.amount();
    if collateral <= 0.0 {
        return 0.0;
    }
    let notes_at_or_ahead: f64 = state
        .tranches
        .tranches
        .iter()
        .filter(|note| note.payment_priority <= tranche.payment_priority)
        .map(|note| {
            state
                .tranche_balances
                .get(note.id.as_str())
                .map_or(0.0, |m| m.amount())
        })
        .sum();
    (collateral - notes_at_or_ahead) / collateral
}

/// First failure of each coverage test, sorted by test id: what the executor
/// reported on `periods`, merged with the tranche triggers' own breach dates.
pub(super) fn first_coverage_breaches(
    periods: &[PeriodDiagnostics],
    triggers: &[super::triggers::TrancheTriggerState],
) -> Vec<CoverageTestBreach> {
    let mut first: std::collections::BTreeMap<&str, Option<Date>> =
        std::collections::BTreeMap::new();
    for period in periods {
        for test in &period.coverage_tests {
            let date = first.entry(test.test_id.as_str()).or_insert(None);
            if !test.passing && date.is_none() {
                *date = Some(period.payment_date);
            }
        }
    }
    for trigger in triggers {
        let date = first.entry(trigger.test_id.as_str()).or_insert(None);
        *date = match (*date, trigger.first_breach) {
            (Some(reported), Some(breach)) => Some(reported.min(breach)),
            (reported, breach) => reported.or(breach),
        };
    }
    first
        .into_iter()
        .map(|(test_id, first_breach)| CoverageTestBreach {
            test_id: test_id.to_string(),
            first_breach,
        })
        .collect()
}
