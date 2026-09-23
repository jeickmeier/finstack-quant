use super::*;
use crate::instruments::fixed_income::structured_credit::utils::amortization::level_payment_per_unit;

/// AssetPool flow results for a single period.
pub(crate) struct PoolFlows {
    pub(super) interest: Money,
    pub(super) scheduled_principal: Money,
    pub(super) prepayment: Money,
    pub(super) default: Money,
    pub(super) recovery: Money,
    /// Collateral draws funded from this period's principal collections;
    /// the engine keeps this amount out of the waterfall. Draws funded from
    /// the reserve and unfunded draws are accumulated on the simulation state
    /// by `reserve::fund_collateral_draws`.
    pub(super) draw_from_principal: Money,
    /// Revolver repayments diverted to replenish the reserve toward its
    /// target; the engine keeps this amount out of the waterfall.
    pub(super) reserve_replenished: Money,
    /// Call/put redemption premium above par, treated as interest proceeds.
    pub(super) call_premium: Money,
    /// Special-servicing workout and liquidation fees taken inside the pool
    /// flows; `interest` / `recovery` are already net of them.
    pub(super) special_servicing_fees: Money,
    /// Balloon workouts booked this period, as `(effective default date,
    /// recovery, defaulted par)`: the date is shifted so the deal's recovery
    /// lag releases the recovery `workout_months` after maturity. Included
    /// in `default` / `recovery`; the engine queues them separately.
    pub(super) workout_claims: Vec<(Date, Money, Money)>,
}

impl PoolFlows {
    /// Zero flows in `currency`.
    pub(super) fn zero(currency: Currency) -> Self {
        let zero = Money::from((0_i64, currency));
        Self {
            interest: zero,
            scheduled_principal: zero,
            prepayment: zero,
            default: zero,
            recovery: zero,
            draw_from_principal: zero,
            reserve_replenished: zero,
            workout_claims: Vec::new(),
            call_premium: zero,
            special_servicing_fees: zero,
        }
    }
}

/// Calculate all pool flows for the period.
///
/// Implements:
/// - M1: Scheduled amortization for amortizing assets (mortgages, auto, etc.)
/// - M3: Maturity/balloon payment when an asset reaches maturity
/// - m2: Sequential default → scheduled principal → prepay application
///   (Intex/Moody's Analytics & SIFMA convention: MDR on the BOP balance,
///   scheduled principal on the survivor, SMM on the remainder)
#[derive(Debug, Clone, Copy)]
pub(super) struct PoolFlowRates {
    pub(super) smm: f64,
    pub(super) mdr: f64,
    pub(super) recovery_rate: f64,
}

/// Per-asset monthly prepayment and default rates resolved from each dated
/// asset's own seasoning; empty (or `None` per asset) when the pool-level
/// rate applies.
#[derive(Debug, Clone, Default)]
pub(super) struct AssetSeasonedRates {
    /// Single-month mortality per asset.
    pub(super) smm: Vec<Option<f64>>,
    /// Monthly default rate per asset.
    pub(super) mdr: Vec<Option<f64>>,
}

impl AssetSeasonedRates {
    /// Apply scenario multipliers to every per-asset rate, clamping each
    /// scaled rate to `[0, cap]`. A `None` multiplier drops that channel's
    /// per-asset rates so the pool-level rate applies to every row.
    ///
    /// # Arguments
    ///
    /// * `smm_mult` - Factor on each asset's monthly prepayment rate.
    /// * `mdr_mult` - Factor on each asset's monthly default rate.
    /// * `cap` - Upper bound on a scaled monthly rate (decimal, below 1).
    pub(super) fn scaled(mut self, smm_mult: Option<f64>, mdr_mult: Option<f64>, cap: f64) -> Self {
        fn scale(rates: &mut Vec<Option<f64>>, mult: Option<f64>, cap: f64) {
            match mult {
                Some(mult) => rates
                    .iter_mut()
                    .flatten()
                    .for_each(|rate| *rate = (*rate * mult).clamp(0.0, cap)),
                None => rates.clear(),
            }
        }
        scale(&mut self.smm, smm_mult, cap);
        scale(&mut self.mdr, mdr_mult, cap);
        self
    }

    fn smm(&self, i: usize) -> Option<f64> {
        self.smm.get(i).copied().flatten()
    }

    fn mdr(&self, i: usize) -> Option<f64> {
        self.mdr.get(i).copied().flatten()
    }
}

/// Copula-resolved default outcome for one payment period.
///
/// Present only when the scenario default model is a copula; otherwise the
/// engine uses the legacy monthly-equivalent `PoolFlowRates::mdr`.
pub(super) enum PeriodDefaultOutcome<'a> {
    /// Per-name finite-pool simulation. Entry `k` of each slice describes the
    /// `k`-th still-performing asset (`!is_defaulted && balance > 0`) in the
    /// pool's intrinsic asset order.
    PerName {
        /// `true` ⇒ the asset defaults in full this period.
        defaults: &'a [bool],
        /// The recovery rate the asset realizes if it defaults this period,
        /// scattered idiosyncratically around the period systematic recovery.
        recoveries: &'a [f64],
    },

    /// LHP fast-path: a single **period-level** default rate (already
    /// aggregated over the period — *not* a monthly-equivalent rate) applied
    /// uniformly to every performing asset.
    PoolWidePeriodRate(f64),
}

/// Deal-level special-servicing fee terms applied inside the pool flows.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SpecialServicingFees {
    /// Workout fee as a decimal fraction of the P&I collected on specially
    /// serviced loans.
    pub(super) workout: f64,
    /// Liquidation fee as a decimal fraction of liquidation proceeds.
    pub(super) liquidation: f64,
}

impl SpecialServicingFees {
    /// Read the deal's fee schedule (`None` for a fee-free deal).
    pub(super) fn from_deal(fees: Option<&DealFees>) -> Self {
        Self {
            workout: fees.and_then(|f| f.workout_fee_pct).unwrap_or(0.0) / 100.0,
            liquidation: fees.and_then(|f| f.liquidation_fee_pct).unwrap_or(0.0) / 100.0,
        }
    }

    /// Split liquidation `proceeds` into `(net proceeds, fee)`.
    fn liquidation_split(self, proceeds: f64) -> (f64, f64) {
        let fee = proceeds.max(0.0) * self.liquidation.clamp(0.0, 1.0);
        (proceeds - fee, fee)
    }
}

/// Write off `share` of asset `i`'s arrears and advances against a
/// charge-off. The written-off advances are reimbursed from the charge-off's
/// `proceeds` first; the shortfall is non-recoverable and is added to
/// `non_recoverable` when the policy reimburses it from pool collections
/// (otherwise the servicer absorbs it). Returns the proceeds net of the
/// reimbursement.
fn settle_charged_off_advances(
    pool_state: &mut PoolState,
    i: usize,
    share: f64,
    proceeds: f64,
    reimburse_from_collections: bool,
    non_recoverable: &mut f64,
) -> f64 {
    let share = share.clamp(0.0, 1.0);
    if share <= 0.0 {
        return proceeds;
    }
    pool_state.arrears_interest[i] *= 1.0 - share;
    pool_state.arrears_principal[i] *= 1.0 - share;
    let written_off = pool_state.advances[i] * share;
    pool_state.advances[i] -= written_off;
    let reimbursed = proceeds.max(0.0).min(written_off);
    if reimburse_from_collections {
        *non_recoverable += written_off - reimbursed;
    }
    proceeds - reimbursed
}

pub(super) struct RatedPoolFlowRequest<'a, 's> {
    pub(super) state: &'a mut SimulationState<'s>,
    pub(super) pay_date: Date,
    pub(super) prev_date: Date,
    pub(super) months_per_period: f64,
    pub(super) context: &'a MarketContext,
    pub(super) rates: PoolFlowRates,
    /// Per-asset monthly rates from each dated asset's own seasoning
    /// (`None` entries and an empty vector fall back to `rates`).
    pub(super) asset_rates: AssetSeasonedRates,
    /// `Some` when the scenario default model is a copula (per-name or LHP);
    /// `None` for the legacy pool-wide MDR / deterministic path.
    pub(super) copula_outcome: Option<PeriodDefaultOutcome<'a>>,
    /// Roll-rate delinquency model, when the deal carries one.
    pub(super) delinquency: Option<&'a DelinquencyModel>,
    /// Card master-trust portfolio model, when the deal carries one.
    pub(super) card: Option<&'a CardPortfolioSpec>,
    /// Deal-level special-servicing fee terms.
    pub(super) special_servicing: SpecialServicingFees,
}

pub(super) fn calculate_pool_flows_with_rates(
    request: RatedPoolFlowRequest<'_, '_>,
) -> Result<PoolFlows> {
    let state = request.state;
    let base_currency = state.base_currency;
    let mut total_interest = Money::from((0_i64, base_currency));
    let mut total_scheduled = Money::from((0_i64, base_currency));
    let mut total_prepay = Money::from((0_i64, base_currency));
    let mut total_default = Money::from((0_i64, base_currency));
    let mut total_recovery = Money::from((0_i64, base_currency));
    let mut workout_claims: Vec<(Date, Money, Money)> = Vec::new();
    let mut special_servicing_fees = 0.0_f64;
    // Non-recoverable servicer advances to take off the top of this period's
    // collections (only under `reimburse_from_collections`).
    let mut non_recoverable_advances = 0.0_f64;
    let reimburse_from_collections = matches!(
        request.delinquency.map(|model| model.advancing),
        Some(AdvancingPolicy::PrincipalAndInterest {
            reimburse_from_collections: true,
            ..
        })
    );

    // Compound the monthly-equivalent SMM/MDR across the payment period.
    // For seasoning-ramped curves (PSA/SDA) on non-monthly frequencies, the
    // deterministic/OAS sources pre-average the per-month rates within the
    // period (`period_averaged_monthly_rate`), so this compounding recovers
    // the exact multi-month period rate rather than overstating ramp-phase
    // speeds from an end-of-period sample.
    let global_period_smm = 1.0 - (1.0 - request.rates.smm).powf(request.months_per_period);
    let global_period_mdr = 1.0 - (1.0 - request.rates.mdr).powf(request.months_per_period);

    // Pre-resolve all curves
    let mut resolved_curves = Vec::with_capacity(state.pool_state.unique_curves.len());
    for idx_str in &state.pool_state.unique_curves {
        resolved_curves.push(request.context.get_forward(idx_str)?);
    }

    // Copula default resolution. For `PerName`, `per_name_mask[k]` is the
    // realized default outcome of the k-th still-performing asset (in pool
    // order); `alive_idx` advances for every asset that passes the
    // performing-asset gate below, so the indicator slice stays
    // index-aligned with the simulator's draw order.
    // For the LHP fast-path, `lhp_period_rate` is a single period-level rate
    // applied to every performing asset.
    let (per_name_outcome, lhp_period_rate) = match &request.copula_outcome {
        Some(PeriodDefaultOutcome::PerName {
            defaults,
            recoveries,
        }) => (Some((*defaults, *recoveries)), None),
        Some(PeriodDefaultOutcome::PoolWidePeriodRate(rate)) => (None, Some(*rate)),
        None => (None, None),
    };
    let mut alive_idx = 0usize;

    let n = state.pool_state.len();

    // The per-name mask and recovery slice are ordered over assets performing
    // at period start. Validate their lengths before mutating asset state so
    // each idiosyncratic draw remains aligned with its pool index.
    if let Some((mask, recoveries)) = per_name_outcome {
        let performing = (0..n)
            .filter(|&i| state.pool_state.balances[i] > 0.0 && !state.pool_state.is_defaulted[i])
            .count();
        if mask.len() != performing {
            return Err(finstack_quant_core::Error::Validation(format!(
                "per-name copula default mask is misaligned with the asset \
                 loop: mask carries {} entries but {} assets are performing \
                 at period start (pay_date {})",
                mask.len(),
                performing,
                request.pay_date,
            )));
        }
        // The recovery slice is built name-aligned with the default mask in
        // the same period; guard the invariant so a future regression cannot
        // silently mis-pair recoveries with defaults.
        if recoveries.len() != mask.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "per-name recovery slice ({} entries) is misaligned with the \
                 default mask ({} entries) at pay_date {}",
                recoveries.len(),
                mask.len(),
                request.pay_date,
            )));
        }
    }

    for i in 0..n {
        let balance = state.pool_state.balances[i];
        if balance <= 0.0 {
            continue;
        }
        // Card master trust after the revolving period: the investor's
        // collections are its fixed allocation of the flows on the level
        // trust receivables, not a share of its own amortizing interest.
        let card_base = request
            .card
            .and(state.card_flow_base.as_ref())
            .and_then(|base| base.get(i).copied());

        // ── NPL/RPL resolution ────────────────────────────────────────
        // A non-performing loan pays nothing until its resolution date,
        // counted from its origination (acquisition date, else closing).
        // There the liquidated share is booked as a default whose recovery
        // is the net proceeds, released on the resolution date itself (the
        // workout already ran through the timeline), and the re-performing
        // share becomes a performing loan at the modified coupon from the
        // next period on, its level payment recast on the re-performing
        // balance.
        if let Some(spec) = state.pool_state.liquidation[i] {
            let months = i32::try_from(spec.months_to_resolution).unwrap_or(i32::MAX);
            let anchor = state.pool_state.acquisition_dates[i].unwrap_or(state.closing_date);
            if request.pay_date < anchor.add_months(months) {
                continue;
            }
            state.pool_state.liquidation[i] = None;
            let liquidated = balance * (1.0 - spec.reperformance_prob.clamp(0.0, 1.0));
            let proceeds = liquidated * spec.net_proceeds_fraction();
            total_default = total_default.checked_add(Money::new(liquidated, base_currency)?)?;
            total_recovery = total_recovery.checked_add(Money::new(proceeds, base_currency)?)?;
            if liquidated > 0.0 {
                // Dated so the deal's recovery lag lands the release today.
                let claim_date = request
                    .pay_date
                    .add_months(-i32::try_from(state.recovery_lag_months).unwrap_or(i32::MAX));
                workout_claims.push((
                    claim_date,
                    Money::new(proceeds, base_currency)?,
                    Money::new(liquidated, base_currency)?,
                ));
            }
            let reperforming = balance - liquidated;
            if reperforming <= balance * 1e-10 {
                state.pool_state.is_defaulted[i] = true;
                state.pool_state.balances[i] = 0.0;
                continue;
            }
            state.pool_state.balances[i] = reperforming;
            state.pool_state.level_payments[i] = None;
            if let Some(rate) = spec.modified_rate {
                state.pool_state.rates[i] = rate;
                state.pool_state.spread_bp[i] = None;
                state.pool_state.curve_indices[i] = None;
            }
            continue;
        }

        // Skip already-defaulted assets: prevents pre-existing defaulted assets
        // (e.g. assets that entered the pool in workout) from accruing interest,
        // defaulting again, or prepaying. Also guards against assets marked as
        // fully defaulted during simulation.
        if state.pool_state.is_defaulted[i] {
            continue;
        }

        // This asset is performing at period start; claim its per-name
        // default indicator and idiosyncratic recovery. The pre-loop length
        // guard proves `alive_idx` is always in bounds here, so the claim is
        // exact and order-stable.
        let per_name_claim = per_name_outcome.map(|(mask, recoveries)| {
            let defaulted = mask.get(alive_idx).copied().unwrap_or(false);
            let recovery = recoveries
                .get(alive_idx)
                .copied()
                .unwrap_or(request.rates.recovery_rate);
            alive_idx += 1;
            (defaulted, recovery)
        });

        // Resolve this period's default rate up-front — it is needed both for
        // the mid-period interest-accrual haircut below and for the principal
        // default amount further down. The rate depends only on the asset's
        // MDR override, the per-name copula realization, the LHP period rate,
        // or the legacy pool-wide MDR — none of which depend on the scheduled
        // amortization computed later.
        //
        // Default-rate precedence:
        //   1. Per-asset `mdr_override` (explicit user input) — always wins.
        //   2. Per-name copula realization — full default (1.0) or none (0.0).
        //   3. LHP fast-path period rate — the closed-form `N → ∞` limit.
        //   4. The asset's own seasoning on the deal's default curve (dated
        //      rows under SDA/vector curves).
        //   5. Pool-wide MDR (`global_period_mdr`).
        let period_mdr = if let Some(mdr) = state.pool_state.mdr_overrides[i] {
            1.0 - (1.0 - mdr).powf(request.months_per_period)
        } else if let Some((defaulted, _)) = per_name_claim {
            if defaulted {
                1.0
            } else {
                0.0
            }
        } else if let Some(rate) = lhp_period_rate {
            rate.clamp(0.0, 1.0)
        } else if let Some(mdr) = request.asset_rates.mdr(i) {
            1.0 - (1.0 - mdr).powf(request.months_per_period)
        } else {
            global_period_mdr
        };
        // Card receivables charge off at the portfolio's annual rate; an
        // explicit per-asset MDR override still wins.
        let period_mdr = match (request.card, state.pool_state.mdr_overrides[i]) {
            (Some(card), None) => {
                1.0 - (1.0 - card.charge_off_rate.clamp(0.0, 1.0))
                    .powf(request.months_per_period / 12.0)
            }
            _ => period_mdr,
        };

        // Special-servicing status at the period's opening: the workout fee
        // applies to this period's collections on loans already in special
        // servicing; a loan entering it this period pays from the next.
        let serviced_bop = state.pool_state.special_serviced[i];

        // 1. Interest -- computed first so matured assets still pay their final coupon
        let rate = if let Some(curve_idx) = state.pool_state.curve_indices[i] {
            collateral_asset_rate_for_period(
                resolved_curves[curve_idx].as_ref(),
                request.context,
                request.prev_date,
                state.pool_state.rates[i],
                state.pool_state.spread_bp[i],
                state.floating_rate_shift,
                state.pool_state.index_floors[i],
            )?
        } else {
            state.pool_state.rates[i]
        };
        let rate = state.pool_state.rate_floors[i].map_or(rate, |floor| rate.max(floor));
        // Card receivables earn the portfolio yield, not a contractual coupon.
        let rate = request.card.map_or(rate, |card| card.portfolio_yield);

        // Mid-period maturities accrue interest only through maturity.
        let interest_end = state.pool_state.maturities[i].min(request.pay_date);

        let accrual_factor = state.pool_state.day_counts[i].year_fraction(
            request.prev_date,
            interest_end,
            DayCountContext::default(),
        )?;

        // Defaults in a period are modeled as a rate `period_mdr` (a fraction
        // of the balance), with no explicit intra-period default date. Under
        // the standard market convention defaults are assumed uniformly
        // distributed over the period, so the defaulting fraction accrues, on
        // average, HALF the period's interest. The non-defaulting fraction
        // accrues the full period. Net interest is therefore scaled by
        // `(1 − 0.5·period_mdr)` rather than accruing the full pre-default
        // balance for the whole period.
        // Delinquency buckets: with a model, the default rate is the entry
        // into 30 days delinquent, balances roll or cure month by month, and
        // only the roll out of the last bucket is a charge-off. Delinquent
        // balances stay in the asset's par but pay no interest or scheduled
        // principal until they cure.
        let mut delinquent_bop = 0.0_f64;
        let mut performing_bop = balance;
        let delinquency = request.delinquency.map(|model| {
            let buckets = &mut state.pool_state.delinquent[i];
            if buckets.len() != model.buckets() {
                buckets.resize(model.buckets(), 0.0);
            }
            delinquent_bop = super::delinquency::delinquent_balance(buckets);
            performing_bop = (balance - delinquent_bop).max(0.0);
            let mut performing = performing_bop;
            let months = request.months_per_period.round().max(1.0) as u32;
            let monthly_entry =
                1.0 - (1.0 - period_mdr.clamp(0.0, 1.0)).powf(1.0 / f64::from(months));
            let outcome = super::delinquency::roll_period(
                buckets,
                &mut performing,
                monthly_entry,
                months,
                model,
            );
            (outcome, performing)
        });
        // Share of the performing balance that stopped paying this period:
        // the default rate without a model, the bucket entry with one.
        let attrition_frac = match &delinquency {
            Some((outcome, _)) if performing_bop > 0.0 => {
                (outcome.entered / performing_bop).clamp(0.0, 1.0)
            }
            Some(_) => 0.0,
            None => period_mdr,
        };
        let default_accrual_haircut = 1.0 - 0.5 * attrition_frac.clamp(0.0, 1.0);
        // Appraisal reduction (ASER): interest is advanced only on the
        // appraised share of a specially serviced loan.
        let appraisal_haircut = 1.0 - state.pool_state.appraisal_reduction[i].clamp(0.0, 1.0);
        let interest_base = card_base.unwrap_or(performing_bop);
        let interest = Money::new(
            interest_base * rate * accrual_factor * default_accrual_haircut * appraisal_haircut,
            base_currency,
        )?;
        total_interest = total_interest.checked_add(interest)?;

        // ── Default FIRST, on the beginning-of-period balance ────────────
        //
        // Market convention (Intex/Moody's Analytics; SIFMA standard MBS
        // cashflow methodology): the period default rate (MDR) is applied to
        // the BEGINNING-of-period balance, scheduled principal is then
        // computed on the surviving (post-default) balance, and the SMM is
        // applied to the survivor after scheduled principal. This also
        // reconciles with the mid-period interest-accrual haircut above,
        // which already assumes the defaulting fraction comes out of the
        // pre-scheduled (BOP) balance.
        let default_amt = match &delinquency {
            Some((outcome, _)) => outcome.charged_off,
            None => (card_base.unwrap_or(balance) * period_mdr).min(balance),
        };
        let balance_after_default = balance - default_amt;
        // Performing balance after this period's entries, cures and
        // modifications; the whole survivor without a delinquency model.
        let performing_after = match &delinquency {
            Some((_, performing)) => performing.clamp(0.0, balance_after_default),
            None => balance_after_default,
        };

        // Per-name defaults recover at their own idiosyncratically-dispersed
        // rate; the LHP and legacy paths use the period systematic recovery.
        let asset_recovery_rate =
            state.pool_state.recovery_rates[i].unwrap_or(match per_name_claim {
                Some((_, recovery)) => recovery,
                None => request.rates.recovery_rate,
            });
        if default_amt > 0.0 {
            // A defaulting loan enters special servicing.
            state.pool_state.special_serviced[i] = true;
        }
        // The special servicer's liquidation fee comes out of the proceeds.
        let (recovery_amt, liquidation_fee) = request
            .special_servicing
            .liquidation_split(default_amt * asset_recovery_rate);
        special_servicing_fees += liquidation_fee;
        // Charged-off delinquent balances take their arrears with them; the
        // servicer's advances on them are reimbursed from their own
        // liquidation proceeds and anything left uncovered is
        // non-recoverable.
        let charged_share = match &delinquency {
            Some((outcome, _)) => {
                let before = delinquent_bop + outcome.entered;
                if before > 0.0 {
                    (outcome.charged_off / before).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        let recovery_amt = settle_charged_off_advances(
            &mut state.pool_state,
            i,
            charged_share,
            recovery_amt,
            reimburse_from_collections,
            &mut non_recoverable_advances,
        );
        total_default = total_default.checked_add(Money::new(default_amt, base_currency)?)?;
        total_recovery = total_recovery.checked_add(Money::new(recovery_amt, base_currency)?)?;

        // Mark asset as fully defaulted if default consumed (nearly) all the
        // BOP balance. Relative tolerance 1 - 1e-10 catches floating-point
        // imprecision when the MDR is effectively 100% (e.g. a per-name
        // copula full default) without false positives from small balances.
        if default_amt >= balance * (1.0 - 1e-10) {
            state.pool_state.is_defaulted[i] = true;
            state.pool_state.balances[i] = 0.0;
            continue;
        }

        // Check maturity -- if asset has matured, return the surviving
        // (post-default) balance as a balloon payment and zero out the asset.
        // Interest was already computed above (capped at maturity date, with
        // the default haircut applied).
        if request.pay_date >= state.pool_state.maturities[i] {
            // Balances still delinquent at maturity never pay: charge them off
            // (with recovery) and pay the performing balance as the balloon.
            let matured_delinquent = if delinquency.is_some() {
                let buckets = &mut state.pool_state.delinquent[i];
                let amount = super::delinquency::delinquent_balance(buckets);
                buckets.iter_mut().for_each(|bucket| *bucket = 0.0);
                amount.min(balance_after_default)
            } else {
                0.0
            };
            if matured_delinquent > 0.0 {
                state.pool_state.special_serviced[i] = true;
                let (matured_recovery, liquidation_fee) = request
                    .special_servicing
                    .liquidation_split(matured_delinquent * asset_recovery_rate);
                special_servicing_fees += liquidation_fee;
                // Everything still delinquent charges off: its arrears and
                // the advances on them are written off against the proceeds.
                let matured_recovery = settle_charged_off_advances(
                    &mut state.pool_state,
                    i,
                    1.0,
                    matured_recovery,
                    reimburse_from_collections,
                    &mut non_recoverable_advances,
                );
                total_default =
                    total_default.checked_add(Money::new(matured_delinquent, base_currency)?)?;
                total_recovery =
                    total_recovery.checked_add(Money::new(matured_recovery, base_currency)?)?;
            }
            // Balloon: the loss share defaults into a workout, the extension
            // share is extended at the extension coupon and pays at the
            // extended maturity, and the rest pays as the balloon.
            let performing_at_maturity = balance_after_default - matured_delinquent;
            let (extended, workout) = match state.pool_state.balloon[i].take() {
                Some(spec) if spec.extension_prob > 0.0 || spec.loss_prob > 0.0 => {
                    let extension_prob = spec.extension_prob.clamp(0.0, 1.0);
                    let workout = performing_at_maturity * spec.loss_prob.clamp(0.0, 1.0);
                    if workout > 0.0 {
                        // A balloon loss enters special servicing; the
                        // liquidation fee comes out of the workout proceeds.
                        state.pool_state.special_serviced[i] = true;
                        let (recovery, liquidation_fee) =
                            request.special_servicing.liquidation_split(
                                workout * (1.0 - spec.severity_pct.clamp(0.0, 100.0) / 100.0),
                            );
                        special_servicing_fees += liquidation_fee;
                        // The recovery arrives `workout_months` after maturity:
                        // the claim is dated so the deal's recovery lag lands
                        // it there (the deal lag itself when 0).
                        let shift = if spec.workout_months == 0 {
                            0
                        } else {
                            i32::try_from(spec.workout_months).unwrap_or(i32::MAX)
                                - i32::try_from(state.recovery_lag_months).unwrap_or(i32::MAX)
                        };
                        let claim_date = request.pay_date.add_months(shift);
                        total_default =
                            total_default.checked_add(Money::new(workout, base_currency)?)?;
                        total_recovery =
                            total_recovery.checked_add(Money::new(recovery, base_currency)?)?;
                        workout_claims.push((
                            claim_date,
                            Money::new(recovery, base_currency)?,
                            Money::new(workout, base_currency)?,
                        ));
                    }
                    let extended = performing_at_maturity * extension_prob;
                    if extended > 0.0 {
                        state.pool_state.maturities[i] = state.pool_state.maturities[i]
                            .add_months(i32::try_from(spec.extension_months).unwrap_or(i32::MAX));
                        if let Some(extension_rate) = spec.extension_rate {
                            // An all-in modification rate: the extended
                            // balance no longer floats.
                            state.pool_state.rates[i] = extension_rate;
                            state.pool_state.spread_bp[i] = None;
                            state.pool_state.curve_indices[i] = None;
                        }
                        // An interest-only loan stays interest-only through the
                        // extension; a level-pay loan keeps its schedule scaled
                        // to the extended slice.
                        if let Some(io) = state.pool_state.io_months[i].as_mut() {
                            *io = io.saturating_add(spec.extension_months);
                        }
                        state.pool_state.level_payments[i] = state.pool_state.level_payments[i]
                            .map(|payment| payment * extension_prob);
                    }
                    (extended, workout)
                }
                _ => (0.0, 0.0),
            };
            let balloon = Money::new(performing_at_maturity - extended - workout, base_currency)?;
            total_scheduled = total_scheduled.checked_add(balloon)?;
            state.pool_state.balances[i] = extended;
            continue;
        }

        // This period's prepayment rate, resolved up-front alongside
        // `period_mdr`: both are attrition channels that retire whole loans
        // from the (rep-line) asset, and the contractual level payment below
        // must scale by BOTH survival fractions.
        let period_smm = if let Some(smm) = state.pool_state.smm_overrides[i] {
            1.0 - (1.0 - smm).powf(request.months_per_period)
        } else if let Some(smm) = request.asset_rates.smm(i) {
            1.0 - (1.0 - smm).powf(request.months_per_period)
        } else {
            global_period_smm
        };
        // Cardholder payments retire principal at the monthly payment rate;
        // the revolving period recycles them into new receivables.
        let period_smm = request.card.map_or(period_smm, |card| {
            1.0 - (1.0 - card.monthly_payment_rate.clamp(0.0, 1.0)).powf(request.months_per_period)
        });

        // Level-pay loans retain their contractual payment after prepayment;
        // prepayment shortens the term rather than recasting the payment.
        // Defaulted loans stop paying, so scale the frozen aggregate payment by
        // the period survival fraction `(1 − period_mdr)`.
        let scheduled_principal = if state.pool_state.is_amortizing[i] && rate > 0.0 {
            if !rate.is_finite() || rate <= -1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "invalid amortization rate for pool asset '{}': {rate}",
                    state.pool_state.ids[i]
                )));
            }
            // Nominal periodic rate `rate × months/12` (US mortgage convention;
            // matches mbs_passthrough/pricer.rs `wac / 12.0`).
            let period_rate = rate * request.months_per_period / 12.0;
            if !period_rate.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "invalid amortization math for pool asset '{}': rate={rate}, period_rate={period_rate}",
                    state.pool_state.ids[i]
                )));
            }

            // Resolve the frozen contractual level payment, computing it once
            // on the first period the asset amortizes (period-native math:
            // level_payment = P * r_p / (1 − (1+r_p)^−n_p)).
            let months_per_period_u32 = request.months_per_period.round().max(1.0) as u32;
            // Loan age at the period's opening, from origination (else the
            // acquisition date, else the deal closing).
            let origination = state.pool_state.origination_dates[i].unwrap_or(state.closing_date);
            let age_months = if origination < request.prev_date {
                origination.months_until(request.prev_date)
            } else {
                0
            };
            // The schedule runs over the amortization term from origination
            // (the unamortized balance pays as the balloon at maturity), or
            // fully amortizes by maturity when no term is given.
            let remaining_periods_f64 = f64::from(
                crate::instruments::fixed_income::structured_credit::utils::amortization::remaining_schedule_periods(
                    age_months,
                    state.pool_state.amortization_term_months[i],
                    request
                        .pay_date
                        .months_until(state.pool_state.maturities[i]),
                    months_per_period_u32,
                ),
            );
            // No scheduled principal inside the interest-only window; the
            // level payment is frozen when amortization starts.
            let in_io_window = state.pool_state.io_months[i].is_some_and(|io| age_months < io);
            if in_io_window {
                0.0
            } else {
                let level_payment = match state.pool_state.level_payments[i] {
                    Some(lp) => lp,
                    None => {
                        let denom = 1.0 - (1.0 + period_rate).powf(-remaining_periods_f64);
                        if !remaining_periods_f64.is_finite() || !denom.is_finite() {
                            return Err(finstack_quant_core::Error::Validation(format!(
                            "invalid amortization math for pool asset '{}': rate={rate}, period_rate={period_rate}",
                            state.pool_state.ids[i]
                        )));
                        }
                        let lp = if denom.abs() > 1e-12 && remaining_periods_f64 > 0.0 {
                            balance * period_rate / denom
                        } else {
                            // Denominator ~0 (very short term): pay the full balance.
                            balance
                        };
                        if !lp.is_finite() {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "invalid level payment for pool asset '{}': {lp}",
                                state.pool_state.ids[i]
                            )));
                        }
                        state.pool_state.level_payments[i] = Some(lp);
                        lp
                    }
                };

                // Defaulted loans' contractual payments terminate. Defaults are
                // applied pro-rata across the (rep-line) asset, so the surviving
                // pool's aggregate level payment scales by this period's survival
                // fraction.
                let mut surviving_payment = level_payment * (1.0 - attrition_frac);
                if let (Some(model), Some((outcome, _))) = (request.delinquency, &delinquency) {
                    // Cured loans resume their contractual payment; modified loans
                    // return at the reduced coupon over the extended term.
                    let per_unit = if performing_bop > 0.0 {
                        level_payment / performing_bop
                    } else {
                        level_payment_per_unit(period_rate, remaining_periods_f64)
                    };
                    surviving_payment += outcome.cured * per_unit;
                    if let Some(spec) = model.modification {
                        let reduced_rate = (rate - spec.rate_reduction_bp / 10_000.0).max(0.0)
                            * request.months_per_period
                            / 12.0;
                        let extended_periods = remaining_periods_f64
                            + f64::from(spec.term_extension_months)
                                / request.months_per_period.max(1.0);
                        surviving_payment += outcome.modified
                            * level_payment_per_unit(reduced_rate, extended_periods);
                    }
                }

                // Prepaid loans' contractual payments terminate too. SMM is Single
                // Monthly *Mortality* — the fraction of the rep-line that pays off
                // in FULL and leaves the pool — so the aggregate level payment must
                // scale by the prepayment survival fraction exactly as it does by
                // the default survival fraction. This is the SIFMA/BMA pool
                // convention (Fabozzi, "Handbook of Mortgage-Backed Securities"):
                // scheduled principal is the underlying loan's amortization *rate*
                // applied to the CURRENT pool balance, which is what scaling the
                // payment alongside the balance reproduces. It also matches the
                // reference implementation in `mbs_passthrough/pricer.rs`, which
                // re-derives the payment from the current balance each period.
                //
                // Scaling only by default survival treats the rep-line as a SINGLE
                // loan receiving perpetual curtailments: the payment stays flat
                // while the balance shrinks, so the interest component falls faster
                // than it should and scheduled principal is progressively
                // overstated. Measured impact before this fix, on a 30y 6% pool at
                // ~5.8% CPR: WAL 7.007y and full payoff at month 168, versus the
                // correct 10.750y running to month 360 — a 35% WAL error. The two
                // channels are applied multiplicatively because they attrit
                // disjoint slices of the line (defaults off the BOP balance,
                // prepayments off the post-scheduled remainder).
                //
                // `period_smm` scales the payment PERSISTED for future periods, not
                // the one used for this period's scheduled principal: prepayments
                // occur after scheduled principal in the period ordering, so the
                // loans prepaying this period do make their scheduled payment.
                state.pool_state.level_payments[i] =
                    Some(surviving_payment * (1.0 - period_smm).clamp(0.0, 1.0));

                // Scheduled principal = survivors' level payment − this period's
                // interest on the surviving balance (interest + scheduled
                // principal = level payment under the same nominal-rate
                // convention). As the balance amortizes the interest portion
                // shrinks and the principal portion grows — the correct level-pay
                // profile. Bounded by the surviving balance so the loan never
                // over-amortizes.
                (surviving_payment - performing_after * period_rate)
                    .max(0.0)
                    .min(performing_after)
            }
        } else {
            0.0
        };

        total_scheduled =
            total_scheduled.checked_add(Money::new(scheduled_principal, base_currency)?)?;

        // Scheduled principal arrears a curing balance repays this period.
        let mut cured_arrears_principal = 0.0_f64;
        if let (Some(model), Some((outcome, _))) = (request.delinquency, &delinquency) {
            // Modified loans blend their coupon concession into the line.
            if let Some(spec) = model.modification {
                if outcome.modified > 0.0 && performing_after > 0.0 {
                    let weight = (outcome.modified / performing_after).min(1.0);
                    if state.pool_state.curve_indices[i].is_some() {
                        if let Some(spread) = state.pool_state.spread_bp[i].as_mut() {
                            *spread -= spec.rate_reduction_bp * weight;
                        }
                    } else {
                        state.pool_state.rates[i] = (state.pool_state.rates[i]
                            - spec.rate_reduction_bp / 10_000.0 * weight)
                            .max(0.0);
                    }
                }
            }
            // Arrears: the P&I the delinquent balance missed, owed by the
            // borrower. The share that cures repays its arrears — the
            // servicer's advances are reimbursed first and the rest reaches
            // the trust — and the modified share has them forgiven. The
            // charged-off share was written off above.
            let missed_interest = delinquent_bop * rate * accrual_factor;
            let missed_principal = if performing_after > 0.0 {
                scheduled_principal * delinquent_bop / performing_after
            } else {
                0.0
            };
            let before = delinquent_bop + outcome.entered;
            let survivor = 1.0 - charged_share;
            let share_of_survivor = |amount: f64| {
                if before > 0.0 && survivor > 0.0 {
                    (amount / before / survivor).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            };
            let cured_share = share_of_survivor(outcome.cured);
            let modified_share = share_of_survivor(outcome.modified);
            let pool_state = &mut state.pool_state;
            // A cure brings the loan current: this period's missed P&I is
            // part of what it repays.
            pool_state.arrears_interest[i] += missed_interest;
            pool_state.arrears_principal[i] += missed_principal;
            let repaid_interest = pool_state.arrears_interest[i] * cured_share;
            let repaid_principal = pool_state.arrears_principal[i] * cured_share;
            let kept = (1.0 - cured_share - modified_share).max(0.0);
            pool_state.arrears_interest[i] *= kept;
            pool_state.arrears_principal[i] *= kept;
            // Repaid scheduled principal amortizes the loan whether the cash
            // reimburses the servicer or reaches the trust.
            cured_arrears_principal = repaid_principal;
            let repaid = repaid_interest + repaid_principal;
            let reimbursed = repaid.min(pool_state.advances[i]);
            pool_state.advances[i] -= reimbursed;
            let to_trust = repaid - reimbursed;
            if to_trust > 0.0 {
                let interest_part = to_trust * repaid_interest / repaid;
                total_interest =
                    total_interest.checked_add(Money::new(interest_part, base_currency)?)?;
                total_scheduled = total_scheduled
                    .checked_add(Money::new(to_trust - interest_part, base_currency)?)?;
            }
            // Servicer advances of the P&I missed this period, capped by
            // what the servicer deems recoverable on this loan.
            if let AdvancingPolicy::PrincipalAndInterest {
                recoverability_cap_pct,
                ..
            } = model.advancing
            {
                let delinquent_after =
                    super::delinquency::delinquent_balance(&pool_state.delinquent[i]);
                let missed = missed_interest + missed_principal;
                let room = (recoverability_cap_pct / 100.0 * delinquent_after
                    - pool_state.advances[i])
                    .max(0.0);
                let advanced = missed.min(room);
                if advanced > 0.0 && missed > 0.0 {
                    let advanced_interest = advanced * missed_interest / missed;
                    total_interest = total_interest
                        .checked_add(Money::new(advanced_interest, base_currency)?)?;
                    total_scheduled = total_scheduled
                        .checked_add(Money::new(advanced - advanced_interest, base_currency)?)?;
                    pool_state.advances[i] += advanced;
                }
            }
        }

        // Balance after default and scheduled amortization (performing part)
        let balance_after_sched = performing_after - scheduled_principal;

        // Prepayment LAST: SMM applies to the survivor balance after
        // scheduled principal (Intex/Moody's Analytics & SIFMA standard
        // ordering: default on BOP balance → scheduled principal on the
        // survivor → prepayment on the remainder). `period_smm` was resolved
        // above so the contractual level payment could scale by it.
        // A lockout blocks voluntary prepayment on the loan.
        let locked_out = state.pool_state.prepayment_penalty[i]
            .as_ref()
            .is_some_and(|penalty| penalty.locks_out(request.pay_date));
        let prepay_amt = if locked_out {
            0.0
        } else {
            (card_base.unwrap_or(balance_after_sched) * period_smm).min(balance_after_sched)
        };
        total_prepay = total_prepay.checked_add(Money::new(prepay_amt, base_currency)?)?;
        // Prepayment penalties reach the trust as interest.
        if let Some(penalty) = &state.pool_state.prepayment_penalty[i] {
            let curve = match penalty.discount_curve_id() {
                Some(id) => Some(request.context.get_discount(id.as_str())?),
                None => None,
            };
            let premium = penalty.premium(
                prepay_amt,
                rate,
                request.pay_date,
                state.pool_state.maturities[i],
                curve.as_deref(),
            )?;
            if premium > 0.0 {
                total_interest = total_interest.checked_add(Money::new(premium, base_currency)?)?;
            }
        }

        // Workout fee: the special servicer's cut of the P&I collected on
        // a loan in special servicing at the period's opening, taken from
        // its interest first, then its principal.
        if serviced_bop && request.special_servicing.workout > 0.0 {
            let collected = interest.amount() + scheduled_principal + prepay_amt;
            let fee = collected.max(0.0) * request.special_servicing.workout.clamp(0.0, 1.0);
            let from_interest = fee.min(interest.amount().max(0.0));
            total_interest =
                total_interest.checked_sub(Money::new(from_interest, base_currency)?)?;
            total_scheduled =
                total_scheduled.checked_sub(Money::new(fee - from_interest, base_currency)?)?;
            special_servicing_fees += fee;
        }

        let new_balance =
            balance_after_default - scheduled_principal - prepay_amt - cured_arrears_principal;
        state.pool_state.balances[i] = new_balance.max(0.0);
    }

    // Non-recoverable advances come off the top of the period's collections:
    // interest first, then scheduled principal, then prepayments.
    if non_recoverable_advances > 0.0 {
        let mut owed = non_recoverable_advances;
        for total in [&mut total_interest, &mut total_scheduled, &mut total_prepay] {
            let take = owed.min(total.amount().max(0.0));
            *total = total.checked_sub(Money::new(take, base_currency)?)?;
            owed -= take;
        }
    }

    Ok(PoolFlows {
        interest: total_interest,
        scheduled_principal: total_scheduled,
        prepayment: total_prepay,
        default: total_default,
        recovery: total_recovery,
        workout_claims,
        special_servicing_fees: Money::new(special_servicing_fees, base_currency)?,
        ..PoolFlows::zero(base_currency)
    })
}
