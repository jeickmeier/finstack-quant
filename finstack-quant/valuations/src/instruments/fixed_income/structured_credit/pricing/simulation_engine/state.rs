use super::*;

/// De minimis threshold for write-down recording (avoids noise from fp rounding).
pub(super) const WRITEDOWN_DE_MINIMIS: f64 = 0.01;

/// Cleanup-call premium; currently zero because the deal has no premium term.
pub(super) fn cleanup_call_premium(_instrument: &StructuredCredit, _tranche_balance: f64) -> f64 {
    0.0
}

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
    pub(super) pool: &'a AssetPool,
    pub(super) tranches: &'a TrancheStructure,
    pub(super) closing_date: Date,
    pub(super) pool_balance_cleanup_threshold: f64,
    pub(super) tranche_recipient_keys: Vec<RecipientType>,
    /// Independently evolving OC/IC breach states for this simulation path.
    pub(super) tranche_triggers: Vec<super::triggers::TrancheTriggerState>,
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
                recovery_queue.add_recovery(date, amount);
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
            pool,
            tranches,
            closing_date,
            pool_balance_cleanup_threshold: template.pool_balance_cleanup_threshold,
            tranche_recipient_keys: template.tranche_recipient_keys.clone(),
            tranche_triggers: super::triggers::initial_states(tranches),
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

    pub(super) fn finalize(mut self) -> HashMap<String, TrancheCashflows> {
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

        self.results
    }
}
