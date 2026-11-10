use super::*;
use crate::structured_credit::types::pool::weighted_avg_rating_factor;
/// De minimis threshold for write-down recording (avoids noise from fp rounding).
pub(super) const WRITEDOWN_DE_MINIMIS: f64 = 0.01;

/// Cleanup-call premium; currently zero because the deal has no premium term.
/// Internal state for period-by-period simulation.
pub(super) struct SimulationState<'a> {
    /// AssetPool state (SoA layout)
    pub(super) pool_state: PoolState,
    /// Total pool outstanding (sum of balances)
    pub(super) pool_outstanding: Money,
    pub(super) recovery_queue: RecoveryQueue,
    pub(super) tranche_balances: FxHashMap<String, Money>,
    /// Deferred (PIK) interest per tranche, carried forward to next period.
    pub(super) deferred_interest: FxHashMap<String, Money>,
    pub(super) results: FxHashMap<String, TrancheCashflows>,
    pub(super) prev_date: Option<Date>,
    pub(super) currency: Currency,
    pub(super) recovery_lag_months: u32,
    /// Asset pool; owned once a reinvestment purchase has added a synthetic row.
    pub(super) pool: std::borrow::Cow<'a, AssetPool>,
    pub(super) tranches: &'a TrancheStructure,
    pub(super) closing_date: Date,
    pub(super) pool_balance_cleanup_threshold: f64,
    pub(super) tranche_recipient_keys: Vec<RecipientType>,
    /// Independently evolving OC/IC breach states for this simulation path.
    pub(super) tranche_triggers: Vec<super::triggers::TrancheTriggerState>,
    /// Projected hedge-swap flows for this run, bucketed per period by
    /// `hedges::period_hedge_flows`.
    pub(super) hedge_schedules: Vec<super::hedges::HedgeSchedule>,
    /// Annualized excess spread realized in each simulated period, oldest
    /// first; the early-amortization excess-spread test reads the last three.
    pub(super) excess_spread_history: Vec<f64>,
    /// Per-period deal accounting for [`SimulationDiagnostics::periods`].
    pub(super) period_diagnostics: Vec<PeriodDiagnostics>,
    /// Payments to tranches outside the waterfall not yet attached to a period
    /// record (see [`PeriodDiagnostics::non_waterfall_payments`]).
    pub(super) pending_non_waterfall_payments: Vec<NonWaterfallPayment>,
    /// Whether an early-amortization event has occurred; once set the
    /// revolving period stays closed.
    pub(super) early_amortization_triggered: bool,
    /// Card master trust: the per-asset investor flow base fixed at the end
    /// of the revolving period (`CardPortfolioSpec::investor_flow_base`);
    /// `None` while revolving or without a card model.
    pub(super) card_flow_base: Option<Vec<f64>>,
    /// Manager incentive fee applied to the terminal sweeps and the call
    /// residual reaching equity (the waterfall carries its own recipients).
    pub(super) incentive_fee: Option<IncentiveFeeSpec>,
    /// When collateral losses are booked into `cumulative_realized_loss`.
    pub(super) loss_recognition: LossRecognition,
    /// Whether the reserve balance retires note principal at legal final
    /// (`ReserveAccountSpec::covers_principal_at_final`); `false` releases
    /// it to the residual holder.
    pub(super) reserve_covers_principal_at_final: bool,
    /// Net-WAC carryover outstanding per capped tranche: interest the
    /// available-funds cap withheld and not yet repaid from excess.
    pub(super) carryover_balance: FxHashMap<String, Money>,
    /// Payment date of the early-amortization (or acceleration) event, once
    /// one has fired.
    pub(super) early_amortization_date: Option<Date>,
    /// Index of the next scheduled `tranche_draws` entry not yet applied.
    pub(super) next_tranche_draw: usize,
    /// Every lender draw applied, as `(tranche id, payment date, amount)`.
    pub(super) tranche_draws: Vec<(String, Date, Money)>,
    /// Cumulative net loss realized in this scenario
    /// (`default_amount * (1 - recovery_rate)`), accumulated period by period.
    ///
    /// "Realized" here means realized *within the simulated path*: it uses the
    /// expected recovery at the point of default rather than lagged cash
    /// recoveries (the INTEX/Moody's Analytics convention — loss allocation
    /// reflects economic loss at default, not the cash-timing of recovery
    /// receipts). It is a running realized loss, not a forward-looking expected
    /// loss; trap/early-amortization/step-down triggers key off it as a fraction
    /// of the original pool.
    pub(super) cumulative_realized_loss: f64,
    /// Loss already reflected in the supplied current note balances; not allocated again.
    pub(super) initial_realized_loss: f64,
    /// Cumulative net loss that exceeds the structure's total absorbable
    /// notional (every tranche fully written down). Surfaced rather than
    /// silently dropped by the loss-allocation `min(...)` cap, so the
    /// per-period cash-conservation check can account for it.
    pub(super) cumulative_loss_unallocated: f64,
    /// Original (cut-off) pool balance, the base of every "fraction of the
    /// original pool" quantity: cumulative-loss triggers, the clean-up call
    /// factor and the cumulative-loss/timing default curves. See
    /// `AssetPool::original_balance_or_reconstructed`.
    pub(super) original_pool_balance: Money,
    /// Performing pool balance at simulation start (excluding pre-defaulted assets).
    /// Used as denominator for loss allocation percentage.
    pub(super) performing_pool_balance: Money,
    /// Pre-computed tranche indices sorted by loss allocation order:
    /// equity (first loss) → subordinated → mezzanine → senior.
    /// Computed once, reused every period.
    pub(super) loss_alloc_order: Vec<usize>,
    /// Balance-weighted average collateral age (WALA) in months at closing,
    /// derived from each asset's `acquisition_date`. PSA/SDA seasoning ramps
    /// are keyed off LOAN age, not deal age, so seasoned collateral must
    /// start partway up the ramp. Assets without an `acquisition_date`
    /// contribute zero age (collateral assumed new at closing).
    pub(super) closing_seasoning_months: u32,
    /// Current reserve account balance.
    pub(super) reserve_balance: Money,
    /// Current excess-spread (spread-account) balance. Carries period to period:
    /// funded from captured residual interest and drawn to cover debt interest
    /// shortfalls. See `ExcessSpreadSpec`.
    pub(super) spread_account: Money,
    /// Current controlled-accumulation principal funding account balance. During
    /// the accumulation period, collected pool principal is held here (investor
    /// balances flat) and released as a bullet at the accumulation end. See
    /// `ControlledAccumulationSpec`.
    pub(super) principal_funding_account: Money,
    /// Interest retained by the waterfall, available to later interest tiers.
    pub(super) undistributed_interest: Money,
    /// Principal retained by the waterfall, available for later capital payments.
    pub(super) undistributed_principal: Money,
    /// Additive shift applied to FLOATING rate projections this period,
    /// so OAS-simulated coupons follow the same rate path as the discounting.
    /// Zero for every non-OAS run, making this exact identity there.
    pub(super) floating_rate_shift: f64,
    /// Cumulative collateral draws funded from the reserve account.
    pub(super) draws_from_reserve: Money,
    /// Cumulative collateral draws funded from principal collections.
    pub(super) draws_from_principal: Money,
    /// Cumulative collateral draws that could not be funded.
    pub(super) cumulative_unfunded_draws: Money,
    /// Cumulative revolver repayments diverted to replenish the reserve.
    pub(super) reserve_replenished: Money,
    /// End-of-period reserve balances.
    pub(super) reserve_balance_path: Vec<(Date, Money)>,
    /// Reserve interest earned per period, before routing.
    pub(super) reserve_interest_paid: Vec<(Date, Money)>,
}

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

/// Deal-health metrics for this period's step-down trigger evaluation.
///
/// Computes, on current balances: cumulative loss (fraction of the *original*
/// pool), the overcollateralization ratio (pool ÷ rated non-equity notes), and
/// senior credit enhancement ([`tranche_credit_enhancement`] of the lowest
/// payment-priority tranche). See [`StepDownTrigger`].
///
/// [`StepDownTrigger`]: crate::structured_credit::StepDownTrigger
pub(super) fn step_down_metrics(
    state: &SimulationState,
) -> crate::structured_credit::pricing::resolve::StepDownMetrics {
    // Include the controlled-accumulation funding account. That principal
    // has left the asset balances but is still collateral for the notes, so
    // omitting it depresses both the OC ratio and credit enhancement during
    // accumulation and can trip a step-down trigger that has not truly fired.
    // Same reasoning as the coverage-test numerator in `simulate_period`.
    let pool = state.pool_outstanding.amount() + state.principal_funding_account.amount();
    let delinquent: f64 = state
        .pool_state
        .delinquent
        .iter()
        .map(|buckets| super::delinquency::delinquent_balance(buckets))
        .sum();
    let delinquency_rate = if state.pool_outstanding.amount() > 0.0 {
        delinquent / state.pool_outstanding.amount()
    } else {
        0.0
    };
    let cumulative_loss_fraction = if state.original_pool_balance.amount() > 0.0 {
        state.cumulative_realized_loss / state.original_pool_balance.amount()
    } else {
        0.0
    };
    let rated_note_balance: f64 = state
        .tranches
        .tranches
        .iter()
        .filter(|t| t.seniority != TrancheSeniority::Equity)
        .map(|t| {
            state
                .tranche_balances
                .get(t.id.as_str())
                .map_or(0.0, |m| m.amount())
        })
        .sum();
    crate::structured_credit::pricing::resolve::StepDownMetrics {
        delinquency_rate,
        cumulative_loss_fraction,
        oc_ratio: if rated_note_balance > 0.0 {
            pool / rated_note_balance
        } else {
            f64::INFINITY
        },
        credit_enhancement: state
            .tranches
            .tranches
            .iter()
            .min_by_key(|t| t.payment_priority)
            .map_or(0.0, |senior| tranche_credit_enhancement(state, senior)),
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
fn first_coverage_breaches(
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

/// Loop-invariant pieces of the initial [`SimulationState`], computed once
/// per pricing run.
///
/// The stochastic engine constructs an identical initial state for every
/// scenario path; only the mutable simulation fields differ (they start from
/// the same values and diverge through the waterfall). Freezing the registry
/// lookup, balance scans, WALA loop, and loss-order sort into a template and
/// cloning the per-path copies is bit-identical — map insertion order is
/// preserved exactly, so FxHashMap bucket layout and every `.values()`
/// iteration order downstream stay unchanged.
pub(crate) struct StateTemplate {
    results: FxHashMap<String, TrancheCashflows>,
    tranche_balances: FxHashMap<String, Money>,
    deferred_interest: FxHashMap<String, Money>,
    tranche_recipient_keys: Vec<RecipientType>,
    pool_state: PoolState,
    loss_alloc_order: Vec<usize>,
    closing_seasoning_months: u32,
    currency: Currency,
    total_pool_balance: Money,
    original_pool_balance: Money,
    performing_pool_balance: Money,
    pool_balance_cleanup_threshold: f64,
}

impl StateTemplate {
    pub(crate) fn new(
        pool: &AssetPool,
        tranches: &TrancheStructure,
        closing_date: Date,
    ) -> Result<Self> {
        let currency = pool.get_currency();
        let pool_balance_cleanup_threshold = embedded_registry()?.pool_balance_cleanup_threshold();

        let results: FxHashMap<String, TrancheCashflows> = tranches
            .tranches
            .iter()
            .map(|t| {
                (
                    t.id.to_string(),
                    TrancheCashflows {
                        tranche_id: t.id.to_string(),
                        cashflows: Vec::new(),
                        detailed_flows: Vec::new(),
                        accrual_periods: Vec::new(),
                        interest_flows: Vec::new(),
                        principal_flows: Vec::new(),
                        pik_flows: Vec::new(),
                        deferred_flows: Vec::new(),
                        writedown_flows: Vec::new(),
                        final_balance: t.current_balance,
                        total_interest: Money::from((0_i64, currency)),
                        total_principal: Money::from((0_i64, currency)),
                        total_pik: Money::from((0_i64, currency)),
                        total_deferred: Money::from((0_i64, currency)),
                        total_writedown: Money::from((0_i64, currency)),
                    },
                )
            })
            .collect();

        let tranche_balances: FxHashMap<String, Money> = tranches
            .tranches
            .iter()
            .map(|t| (t.id.to_string(), t.current_balance))
            .collect();

        // Map each tranche to its waterfall distribution key.
        // Equity tranches receive residual via RecipientType::Equity in the
        // standard waterfall, so their key must match that variant.
        let tranche_recipient_keys: Vec<RecipientType> = tranches
            .tranches
            .iter()
            .map(|t| {
                if t.seniority == TrancheSeniority::Equity {
                    RecipientType::Equity
                } else {
                    RecipientType::Tranche(t.id.to_string())
                }
            })
            .collect();

        let pool_state = PoolState::from_pool(pool);

        let deferred_interest: FxHashMap<String, Money> = tranches
            .tranches
            .iter()
            .map(|t| (t.id.to_string(), t.deferred_interest))
            .collect();

        let total_pool_balance = pool
            .total_balance()
            .unwrap_or(Money::from((0_i64, currency)));
        // The original (cut-off) balance every "fraction of the original pool"
        // quantity is stated against; equals the current balance for a
        // new-issue pool, the tallies' reconstruction for a seasoned one.
        let original_pool_balance = pool.original_balance_or_reconstructed()?;

        // Performing balance excludes pre-defaulted assets. Used as denominator
        // for loss allocation — pre-defaulted assets are already priced into the
        // deal structure and should not trigger additional write-downs.
        let performing_pool_balance = pool.performing_balance().unwrap_or(total_pool_balance);

        // Junior-first loss order by `payment_priority` (total order from
        // `assign_priorities`), not the four-level seniority enum.
        let mut loss_alloc_order: Vec<usize> = (0..tranches.tranches.len()).collect();
        loss_alloc_order.sort_by(|&a, &b| {
            tranches.tranches[b]
                .payment_priority
                .cmp(&tranches.tranches[a].payment_priority)
        });

        let closing_seasoning_months = pool.seasoning_months(closing_date, closing_date);

        Ok(Self {
            results,
            tranche_balances,
            deferred_interest,
            tranche_recipient_keys,
            pool_state,
            loss_alloc_order,
            closing_seasoning_months,
            currency,
            total_pool_balance,
            original_pool_balance,
            performing_pool_balance,
            pool_balance_cleanup_threshold,
        })
    }
}

impl<'a> SimulationState<'a> {
    /// Build a fresh per-path state from the shared [`StateTemplate`].
    ///
    /// Clones the template's maps and vectors in declaration order so every
    /// path's FxHashMaps have identical key sets and insertion sequences,
    /// keeping downstream `.values()` iteration order path-independent.
    pub(super) fn from_template(
        template: &StateTemplate,
        pool: &'a AssetPool,
        tranches: &'a TrancheStructure,
        closing_date: Date,
        state_date: Date,
        recovery_lag_months: u32,
    ) -> Self {
        let mut recovery_queue = RecoveryQueue::new();
        for asset in &pool.assets {
            if let (true, Some(date), Some(amount)) =
                (asset.defaulted, asset.default_date, asset.recovery_amount)
            {
                recovery_queue.add_recovery(date, amount, asset.balance);
            }
        }
        let initial_realized_loss =
            (pool.cumulative_defaults.amount().max(
                template.total_pool_balance.amount() - template.performing_pool_balance.amount(),
            ) - pool.cumulative_recoveries.amount()
                - pool
                    .assets
                    .iter()
                    .filter(|asset| asset.defaulted)
                    .filter_map(|asset| asset.recovery_amount)
                    .map(|amount| amount.amount())
                    .sum::<f64>())
            .max(0.0);
        Self {
            pool_state: template.pool_state.clone(),
            pool_outstanding: template.performing_pool_balance,
            recovery_queue,
            tranche_balances: template.tranche_balances.clone(),
            deferred_interest: template.deferred_interest.clone(),
            results: template.results.clone(),
            prev_date: Some(state_date),
            currency: template.currency,
            recovery_lag_months,
            pool: std::borrow::Cow::Borrowed(pool),
            tranches,
            closing_date,
            pool_balance_cleanup_threshold: template.pool_balance_cleanup_threshold,
            tranche_recipient_keys: template.tranche_recipient_keys.clone(),
            tranche_triggers: super::triggers::initial_states(tranches),
            hedge_schedules: Vec::new(),
            excess_spread_history: Vec::new(),
            period_diagnostics: Vec::new(),
            pending_non_waterfall_payments: Vec::new(),
            early_amortization_triggered: false,
            card_flow_base: None,
            incentive_fee: None,
            loss_recognition: LossRecognition::AtDefault,
            reserve_covers_principal_at_final: true,
            carryover_balance: FxHashMap::default(),
            early_amortization_date: None,
            next_tranche_draw: 0,
            tranche_draws: Vec::new(),
            cumulative_realized_loss: initial_realized_loss,
            initial_realized_loss,
            cumulative_loss_unallocated: 0.0,
            original_pool_balance: template.original_pool_balance,
            performing_pool_balance: template.performing_pool_balance,
            loss_alloc_order: template.loss_alloc_order.clone(),
            closing_seasoning_months: template.closing_seasoning_months,
            reserve_balance: pool.reserve_account,
            spread_account: pool.excess_spread_account,
            principal_funding_account: Money::from((0_i64, template.currency)),
            undistributed_interest: Money::from((0_i64, template.currency)),
            undistributed_principal: pool.collection_account,
            floating_rate_shift: 0.0,
            draws_from_reserve: Money::from((0_i64, template.currency)),
            draws_from_principal: Money::from((0_i64, template.currency)),
            cumulative_unfunded_draws: Money::from((0_i64, template.currency)),
            reserve_replenished: Money::from((0_i64, template.currency)),
            reserve_balance_path: Vec::new(),
            reserve_interest_paid: Vec::new(),
        }
    }

    /// Test-only convenience: builds a [`StateTemplate`] and instantiates a
    /// state from it in one call.
    ///
    /// Production paths go through [`SimulationState::from_template`] so the
    /// initial-state computation exists in exactly one place.
    #[cfg(test)]
    pub(super) fn new(
        pool: &'a AssetPool,
        tranches: &'a TrancheStructure,
        closing_date: Date,
        state_date: Date,
        recovery_lag_months: u32,
    ) -> Result<Self> {
        let template = StateTemplate::new(pool, tranches, closing_date)?;
        Ok(Self::from_template(
            &template,
            pool,
            tranches,
            closing_date,
            state_date,
            recovery_lag_months,
        ))
    }

    pub(super) fn is_pool_exhausted(&self) -> bool {
        self.pool_outstanding.amount() <= self.pool_balance_cleanup_threshold
    }

    /// Finalize tranche results and return them with the deal-level accounting.
    pub(super) fn finalize_with_diagnostics(
        mut self,
    ) -> (FxHashMap<String, TrancheCashflows>, SimulationDiagnostics) {
        let periods = std::mem::take(&mut self.period_diagnostics);
        let diagnostics = SimulationDiagnostics {
            first_breaches: first_coverage_breaches(&periods, &self.tranche_triggers),
            periods,
            reserve_balance_path: std::mem::take(&mut self.reserve_balance_path),
            reserve_interest_paid: std::mem::take(&mut self.reserve_interest_paid),
            draws_from_reserve: self.draws_from_reserve,
            draws_from_principal: self.draws_from_principal,
            unfunded_draws: self.cumulative_unfunded_draws,
            reserve_replenished: self.reserve_replenished,
            early_amortization_date: self.early_amortization_date,
            tranche_draws: std::mem::take(&mut self.tranche_draws),
        };
        for (tranche_id, res) in self.results.iter_mut() {
            let mut final_balance = self
                .tranche_balances
                .get(tranche_id)
                .copied()
                .unwrap_or(Money::from((0_i64, self.currency)));
            if final_balance.amount() < 0.0 && final_balance.amount().abs() <= WRITEDOWN_DE_MINIMIS
            {
                final_balance = Money::from((0_i64, self.currency));
            }
            res.final_balance = final_balance;

            for (date, amount) in &res.interest_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow::new(
                        *date,
                        None,
                        *amount,
                        CFKind::Fixed,
                        0.0,
                        None,
                    ));
                }
            }
            for (date, amount) in &res.principal_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow::new(
                        *date,
                        None,
                        *amount,
                        CFKind::Amortization,
                        0.0,
                        None,
                    ));
                }
            }
            // Include write-down flows in detailed_flows so NPV and
            // risk analytics capture the full economic picture.
            // Write-downs represent permanent loss of notional and are
            // classified as DefaultedNotional (negative = loss to holder).
            for (date, amount) in &res.writedown_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow::new(
                        *date,
                        None,
                        amount.checked_neg(),
                        CFKind::DefaultedNotional,
                        0.0,
                        None,
                    ));
                }
            }
            for (draw_tranche_id, date, amount) in &diagnostics.tranche_draws {
                if draw_tranche_id == tranche_id && amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow::new(
                        *date,
                        None,
                        amount.checked_neg(),
                        CFKind::Notional,
                        0.0,
                        None,
                    ));
                }
            }

            // `detailed_flows` is assembled by category (interest, then
            // principal, then write-downs); sort by date so downstream consumers
            // see a single chronologically-ordered stream. Stable so flows that
            // share a date keep their category order. NPV is unaffected (each
            // flow carries its own date), but any consumer assuming date order —
            // e.g. terminal residual sweeps dated at the last pay date — no
            // longer sees them interleaved out of order.
            res.detailed_flows.sort_by_key(|cf| cf.date);
        }

        (self.results, diagnostics)
    }
}

/// Balance-weighted composition of the performing pool: fixed-rate coupon,
/// floating spread (bp) and Moody's rating factor, each `None` when its
/// population is empty.
pub(super) fn pool_composition(
    state: &SimulationState,
) -> Result<(Option<f64>, Option<f64>, Option<f64>)> {
    let mut fixed_balance = 0.0_f64;
    let mut coupon = 0.0_f64;
    let mut floating_balance = 0.0_f64;
    let mut spread = 0.0_f64;
    for (i, balance) in state.pool_state.balances.iter().enumerate() {
        if *balance <= 0.0 || state.pool_state.defaulted[i] {
            continue;
        }
        match (
            state.pool_state.curve_indices[i],
            state.pool_state.spread_bp[i],
        ) {
            (Some(_), Some(spread_bp)) => {
                floating_balance += balance;
                spread += balance * spread_bp;
            }
            _ => {
                fixed_balance += balance;
                coupon += balance * state.pool_state.rates[i];
            }
        }
    }
    let weighted = |sum: f64, total: f64| (total > 0.0).then(|| sum / total);
    Ok((
        weighted(coupon, fixed_balance),
        weighted(spread, floating_balance),
        weighted_avg_rating_factor(
            state
                .pool_state
                .balances
                .iter()
                .zip(&state.pool_state.defaulted)
                .zip(&state.pool.assets)
                .filter(|((_, defaulted), _)| !**defaulted)
                .map(|((balance, _), asset)| (*balance, asset.rating)),
        )?,
    ))
}

impl SimulationState<'_> {
    /// Equity's cash to date for the incentive-fee hurdle: capital at closing
    /// against every recorded equity distribution (waterfall payments, reserve
    /// interest routed straight to equity and terminal sweeps).
    pub(super) fn equity_history(&self) -> Result<EquityHistory> {
        let mut invested = Money::from((0_i64, self.currency));
        let mut distributions: Vec<(Date, Money)> = Vec::new();
        for tranche in self
            .tranches
            .tranches
            .iter()
            .filter(|tranche| tranche.seniority == TrancheSeniority::Equity)
        {
            invested = invested.checked_add(tranche.original_balance)?;
            if let Some(result) = self.results.get(tranche.id.as_str()) {
                distributions.extend(
                    result
                        .interest_flows
                        .iter()
                        .chain(result.principal_flows.iter())
                        .copied(),
                );
            }
        }
        distributions.sort_by_key(|(date, _)| *date);
        Ok(EquityHistory {
            invested_on: self.closing_date,
            invested,
            distributions,
        })
    }
}

impl SimulationState<'_> {
    /// `true` per asset while its NPL timeline has not resolved.
    /// Servicer advances outstanding across the pool.
    pub(super) fn servicer_advances_outstanding(&self) -> f64 {
        self.pool_state.advances.iter().sum()
    }

    pub(super) fn unresolved_npl(&self) -> Vec<bool> {
        self.pool_state
            .liquidation
            .iter()
            .map(|spec| spec.is_some())
            .collect()
    }

    /// Live delinquency buckets and NPL flags for the eligibility rules.
    pub(super) fn live_collateral<'a>(&'a self, unresolved_npl: &'a [bool]) -> LiveCollateral<'a> {
        LiveCollateral {
            delinquent: &self.pool_state.delinquent,
            unresolved_npl,
        }
    }
}
