use super::*;
use finstack_quant_core::types::CreditRating;

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
    pub(super) tranche_balances: HashMap<String, Money>,
    /// Deferred (PIK) interest per tranche, carried forward to next period.
    pub(super) deferred_interest: HashMap<String, Money>,
    pub(super) results: HashMap<String, TrancheCashflows>,
    pub(super) prev_date: Option<Date>,
    pub(super) base_currency: Currency,
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
    /// Servicer advances of missed P&I not yet reimbursed from recoveries.
    pub(super) servicer_advances_outstanding: f64,
    /// Annualized excess spread realized in each simulated period, oldest
    /// first; the early-amortization excess-spread test reads the last three.
    pub(super) excess_spread_history: Vec<f64>,
    /// Per-period deal accounting for [`SimulationDiagnostics::periods`].
    pub(super) period_diagnostics: Vec<PeriodDiagnostics>,
    /// Whether an early-amortization event has occurred; once set the
    /// revolving period stays closed.
    pub(super) early_amortization_triggered: bool,
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
    /// Total pool balance at simulation start (including defaulted assets).
    /// Used for cleanup call pool factor calculation.
    pub(super) total_pool_balance: Money,
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
    pub(super) pool_wala_months: u32,
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
    /// SC-M13: additive shift applied to FLOATING rate projections this period,
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
    pub(super) reserve_balance_path: DatedFlows,
    /// Reserve interest earned per period, before routing.
    pub(super) reserve_interest_paid: DatedFlows,
}

/// One coverage test as the waterfall executor evaluated it in a period.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CoverageTestDiagnostic {
    /// `CoverageTestSpec::id` of the test.
    pub test_id: String,
    /// Ratio the executor computed (OC: collateral ÷ notes; IC: interest ÷ due).
    pub ratio: f64,
    /// Trigger level the ratio was tested against.
    pub trigger_level: f64,
    /// `ratio − trigger_level`; negative while the test fails.
    pub cushion: f64,
    /// Whether the test passed in this period.
    pub passing: bool,
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
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Collateral balance at period end.
    pub pool_balance: Money,
    /// `pool_balance` divided by the pool balance at simulation start.
    pub pool_factor: f64,
    /// Balance-weighted coupon of the fixed-rate collateral (decimal).
    pub weighted_avg_coupon: f64,
    /// Balance-weighted spread of the floating-rate collateral (basis points).
    pub weighted_avg_spread_bp: f64,
    /// Balance-weighted Moody's rating factor of the collateral.
    pub warf: f64,
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
    /// Annualized excess spread realized this period (decimal).
    pub excess_spread: f64,
    /// Every coverage test the executor evaluated this period.
    pub coverage_tests: Vec<CoverageTestDiagnostic>,
}

/// Deal-level accounting produced alongside the tranche cashflows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimulationDiagnostics {
    /// Per-period deal accounting, in payment order.
    #[serde(default)]
    pub periods: Vec<PeriodDiagnostics>,
    /// Reserve-account balance at the end of each simulated period.
    pub reserve_balance_path: DatedFlows,
    /// Reserve interest earned each period, before routing to its destination.
    pub reserve_interest_paid: DatedFlows,
    /// Collateral draws funded from the reserve account over the simulation.
    pub draws_from_reserve: Money,
    /// Collateral draws funded from principal collections over the simulation.
    pub draws_from_principal: Money,
    /// Collateral draws that could not be funded over the simulation.
    pub unfunded_draws: Money,
    /// Revolver repayments diverted to replenish the reserve over the simulation.
    pub reserve_replenished: Money,
}

/// Deal-health metrics for this period's step-down trigger evaluation.
///
/// Computes, on current balances: cumulative loss (fraction of the *original*
/// pool), the overcollateralization ratio (pool ÷ rated non-equity notes), and
/// senior credit enhancement (`(pool − senior note) ÷ pool`, the senior note
/// being the lowest payment-priority tranche). See [`StepDownTrigger`].
///
/// [`StepDownTrigger`]: crate::instruments::fixed_income::structured_credit::StepDownTrigger
pub(super) fn step_down_metrics(
    state: &SimulationState,
) -> crate::instruments::fixed_income::structured_credit::pricing::resolve::StepDownMetrics {
    // N2: include the controlled-accumulation funding account. That principal
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
    let cumulative_loss_fraction = if state.total_pool_balance.amount() > 0.0 {
        state.cumulative_realized_loss / state.total_pool_balance.amount()
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
    let senior_note_balance = state
        .tranches
        .tranches
        .iter()
        .min_by_key(|t| t.payment_priority)
        .and_then(|t| state.tranche_balances.get(t.id.as_str()))
        .map_or(0.0, |m| m.amount());
    crate::instruments::fixed_income::structured_credit::pricing::resolve::StepDownMetrics {
        delinquency_rate,
        cumulative_loss_fraction,
        oc_ratio: if rated_note_balance > 0.0 {
            pool / rated_note_balance
        } else {
            f64::INFINITY
        },
        credit_enhancement: if pool > 0.0 {
            (pool - senior_note_balance) / pool
        } else {
            0.0
        },
    }
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
    results: HashMap<String, TrancheCashflows>,
    tranche_balances: HashMap<String, Money>,
    deferred_interest: HashMap<String, Money>,
    tranche_recipient_keys: Vec<RecipientType>,
    pool_state: PoolState,
    loss_alloc_order: Vec<usize>,
    pool_wala_months: u32,
    base_currency: Currency,
    total_pool_balance: Money,
    performing_pool_balance: Money,
    pool_balance_cleanup_threshold: f64,
}

impl StateTemplate {
    pub(crate) fn new(
        pool: &AssetPool,
        tranches: &TrancheStructure,
        closing_date: Date,
    ) -> Result<Self> {
        let base_currency = pool.get_base_currency();
        let pool_balance_cleanup_threshold = embedded_registry()?.pool_balance_cleanup_threshold();

        let results: HashMap<String, TrancheCashflows> = tranches
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
                        total_interest: Money::from((0_i64, base_currency)),
                        total_principal: Money::from((0_i64, base_currency)),
                        total_pik: Money::from((0_i64, base_currency)),
                        total_deferred: Money::from((0_i64, base_currency)),
                        total_writedown: Money::from((0_i64, base_currency)),
                    },
                )
            })
            .collect();

        let tranche_balances: HashMap<String, Money> = tranches
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

        let deferred_interest: HashMap<String, Money> = tranches
            .tranches
            .iter()
            .map(|t| (t.id.to_string(), t.deferred_interest))
            .collect();

        let total_pool_balance = pool
            .total_balance()
            .unwrap_or(Money::from((0_i64, base_currency)));

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

        let pool_wala_months = pool.weighted_average_seasoning(closing_date, closing_date);

        Ok(Self {
            results,
            tranche_balances,
            deferred_interest,
            tranche_recipient_keys,
            pool_state,
            loss_alloc_order,
            pool_wala_months,
            base_currency,
            total_pool_balance,
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
    #[allow(clippy::too_many_arguments)]
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
            if let (true, Some(date), Some(amount)) = (
                asset.is_defaulted,
                asset.default_date,
                asset.recovery_amount,
            ) {
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
                    .filter(|asset| asset.is_defaulted)
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
            base_currency: template.base_currency,
            recovery_lag_months,
            pool: std::borrow::Cow::Borrowed(pool),
            tranches,
            closing_date,
            pool_balance_cleanup_threshold: template.pool_balance_cleanup_threshold,
            tranche_recipient_keys: template.tranche_recipient_keys.clone(),
            tranche_triggers: super::triggers::initial_states(tranches),
            hedge_schedules: Vec::new(),
            servicer_advances_outstanding: 0.0,
            excess_spread_history: Vec::new(),
            period_diagnostics: Vec::new(),
            early_amortization_triggered: false,
            cumulative_realized_loss: initial_realized_loss,
            initial_realized_loss,
            cumulative_loss_unallocated: 0.0,
            total_pool_balance: template.total_pool_balance,
            performing_pool_balance: template.performing_pool_balance,
            loss_alloc_order: template.loss_alloc_order.clone(),
            pool_wala_months: template.pool_wala_months,
            reserve_balance: pool.reserve_account,
            spread_account: pool.excess_spread_account,
            principal_funding_account: Money::from((0_i64, template.base_currency)),
            undistributed_interest: Money::from((0_i64, template.base_currency)),
            undistributed_principal: pool.collection_account,
            floating_rate_shift: 0.0,
            draws_from_reserve: Money::from((0_i64, template.base_currency)),
            draws_from_principal: Money::from((0_i64, template.base_currency)),
            cumulative_unfunded_draws: Money::from((0_i64, template.base_currency)),
            reserve_replenished: Money::from((0_i64, template.base_currency)),
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
    ) -> (HashMap<String, TrancheCashflows>, SimulationDiagnostics) {
        let diagnostics = SimulationDiagnostics {
            periods: std::mem::take(&mut self.period_diagnostics),
            reserve_balance_path: std::mem::take(&mut self.reserve_balance_path),
            reserve_interest_paid: std::mem::take(&mut self.reserve_interest_paid),
            draws_from_reserve: self.draws_from_reserve,
            draws_from_principal: self.draws_from_principal,
            unfunded_draws: self.cumulative_unfunded_draws,
            reserve_replenished: self.reserve_replenished,
        };
        for (tranche_id, res) in self.results.iter_mut() {
            let mut final_balance = self
                .tranche_balances
                .get(tranche_id)
                .copied()
                .unwrap_or(Money::from((0_i64, self.base_currency)));
            if final_balance.amount() < 0.0 && final_balance.amount().abs() <= WRITEDOWN_DE_MINIMIS
            {
                final_balance = Money::from((0_i64, self.base_currency));
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
/// floating spread (bp) and Moody's rating factor.
pub(super) fn pool_composition(state: &SimulationState) -> Result<(f64, f64, f64)> {
    let mut fixed_balance = 0.0_f64;
    let mut coupon = 0.0_f64;
    let mut floating_balance = 0.0_f64;
    let mut spread = 0.0_f64;
    let mut rated_balance = 0.0_f64;
    let mut factor = 0.0_f64;
    for (i, balance) in state.pool_state.balances.iter().enumerate() {
        if *balance <= 0.0 || state.pool_state.is_defaulted[i] {
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
        if let Some(asset) = state.pool.assets.get(i) {
            let rating = asset.credit_quality.unwrap_or(CreditRating::NR);
            rated_balance += balance;
            factor += balance * finstack_quant_models::credit::moodys_warf_factor(rating)?;
        }
    }
    let weighted = |sum: f64, total: f64| if total > 0.0 { sum / total } else { 0.0 };
    Ok((
        weighted(coupon, fixed_balance),
        weighted(spread, floating_balance),
        weighted(factor, rated_balance),
    ))
}
