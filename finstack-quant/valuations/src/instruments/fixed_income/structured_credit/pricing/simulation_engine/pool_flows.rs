use super::*;

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
            call_premium: zero,
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

pub(super) struct RatedPoolFlowRequest<'a, 's> {
    pub(super) state: &'a mut SimulationState<'s>,
    pub(super) pay_date: Date,
    pub(super) prev_date: Date,
    pub(super) months_per_period: f64,
    pub(super) context: &'a MarketContext,
    pub(super) rates: PoolFlowRates,
    /// `Some` when the scenario default model is a copula (per-name or LHP);
    /// `None` for the legacy pool-wide MDR / deterministic path.
    pub(super) copula_outcome: Option<PeriodDefaultOutcome<'a>>,
    /// Roll-rate delinquency model, when the deal carries one.
    pub(super) delinquency: Option<&'a DelinquencyModel>,
    /// Card master-trust portfolio model, when the deal carries one.
    pub(super) card: Option<&'a CardPortfolioSpec>,
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

        // ── NPL/RPL resolution ────────────────────────────────────────
        // A non-performing loan pays nothing until its resolution date.
        // There the liquidated share is booked as a default whose recovery
        // is the net proceeds (released through the recovery lag queue like
        // any other), and the re-performing share becomes a performing loan
        // at the modified coupon from the next period on, its level payment
        // recast on the re-performing balance.
        if let Some(spec) = state.pool_state.liquidation[i] {
            let months = i32::try_from(spec.months_to_resolution).unwrap_or(i32::MAX);
            if request.pay_date < state.closing_date.add_months(months) {
                continue;
            }
            state.pool_state.liquidation[i] = None;
            let liquidated = balance * (1.0 - spec.reperformance_prob.clamp(0.0, 1.0));
            let proceeds = liquidated * spec.net_proceeds_fraction();
            total_default = total_default.checked_add(Money::new(liquidated, base_currency)?)?;
            total_recovery = total_recovery.checked_add(Money::new(proceeds, base_currency)?)?;
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
        //   4. Legacy pool-wide MDR (`global_period_mdr`).
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

        // 1. Interest -- computed first so matured assets still pay their final coupon
        let rate = if let Some(curve_idx) = state.pool_state.curve_indices[i] {
            collateral_asset_rate_for_period(
                resolved_curves[curve_idx].as_ref(),
                request.context,
                request.prev_date,
                state.pool_state.rates[i],
                state.pool_state.spread_bp[i],
                state.floating_rate_shift,
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
        let interest = Money::new(
            performing_bop * rate * accrual_factor * default_accrual_haircut * appraisal_haircut,
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
            None => balance * period_mdr,
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
        let recovery_amt = default_amt * asset_recovery_rate;
        // Servicer advances are reimbursed from recovery proceeds before the
        // cash reaches the waterfall.
        let recovery_amt = if delinquency.is_some() && state.servicer_advances_outstanding > 0.0 {
            let reimbursed = recovery_amt.min(state.servicer_advances_outstanding);
            state.servicer_advances_outstanding -= reimbursed;
            recovery_amt - reimbursed
        } else {
            recovery_amt
        };
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
                let matured_recovery = matured_delinquent * asset_recovery_rate;
                total_default =
                    total_default.checked_add(Money::new(matured_delinquent, base_currency)?)?;
                total_recovery =
                    total_recovery.checked_add(Money::new(matured_recovery, base_currency)?)?;
            }
            // Balloon: the share that cannot refinance is extended instead of
            // paid, at the extension coupon, and pays at the extended maturity.
            let performing_at_maturity = balance_after_default - matured_delinquent;
            let extended = match state.pool_state.balloon[i].take() {
                Some(spec) if spec.default_prob > 0.0 => {
                    state.pool_state.maturities[i] = state.pool_state.maturities[i]
                        .add_months(i32::try_from(spec.extension_months).unwrap_or(i32::MAX));
                    if let Some(extension_rate) = spec.extension_rate {
                        state.pool_state.rates[i] = extension_rate;
                    }
                    performing_at_maturity * spec.default_prob.clamp(0.0, 1.0)
                }
                _ => 0.0,
            };
            let balloon = Money::new(performing_at_maturity - extended, base_currency)?;
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
            let remaining_months = request
                .pay_date
                .months_until(state.pool_state.maturities[i]);
            let remaining_periods_f64 =
                f64::from(remaining_months.div_ceil(months_per_period_u32) + 1);
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
                    surviving_payment +=
                        outcome.modified * level_payment_per_unit(reduced_rate, extended_periods);
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
        } else {
            0.0
        };

        total_scheduled =
            total_scheduled.checked_add(Money::new(scheduled_principal, base_currency)?)?;

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
            // Servicer advances of the P&I the delinquent balance missed,
            // capped by what the servicer deems recoverable.
            if let AdvancingPolicy::PrincipalAndInterest {
                recoverability_cap_pct,
            } = model.advancing
            {
                let delinquent_after =
                    super::delinquency::delinquent_balance(&state.pool_state.delinquent[i]);
                let missed_interest = delinquent_bop * rate * accrual_factor;
                let missed_principal = if performing_after > 0.0 {
                    scheduled_principal * delinquent_bop / performing_after
                } else {
                    0.0
                };
                let missed = missed_interest + missed_principal;
                let room = (recoverability_cap_pct / 100.0 * delinquent_after
                    - state.servicer_advances_outstanding)
                    .max(0.0);
                let advanced = missed.min(room);
                if advanced > 0.0 && missed > 0.0 {
                    let advanced_interest = advanced * missed_interest / missed;
                    total_interest = total_interest
                        .checked_add(Money::new(advanced_interest, base_currency)?)?;
                    total_scheduled = total_scheduled
                        .checked_add(Money::new(advanced - advanced_interest, base_currency)?)?;
                    state.servicer_advances_outstanding += advanced;
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
        let prepay_amt = balance_after_sched * period_smm;
        total_prepay = total_prepay.checked_add(Money::new(prepay_amt, base_currency)?)?;
        // Prepayment penalties reach the trust as interest.
        if let Some(penalty) = state.pool_state.prepayment_penalty[i] {
            let premium = penalty.premium(
                prepay_amt,
                rate,
                request.pay_date,
                state.pool_state.maturities[i],
            );
            if premium > 0.0 {
                total_interest = total_interest.checked_add(Money::new(premium, base_currency)?)?;
            }
        }

        let new_balance = balance_after_default - scheduled_principal - prepay_amt;
        state.pool_state.balances[i] = new_balance.max(0.0);
    }

    Ok(PoolFlows {
        interest: total_interest,
        scheduled_principal: total_scheduled,
        prepayment: total_prepay,
        default: total_default,
        recovery: total_recovery,
        ..PoolFlows::zero(base_currency)
    })
}

/// Level payment per unit of balance over `periods` payment periods at
/// `period_rate` (nominal per-period rate): `r / (1 − (1 + r)^−n)`, or
/// `1 / n` at a zero rate. Pays the whole balance when the term is empty.
fn level_payment_per_unit(period_rate: f64, periods: f64) -> f64 {
    if !periods.is_finite() || periods <= 0.0 {
        return 1.0;
    }
    if period_rate.abs() < 1e-12 {
        return 1.0 / periods;
    }
    let denom = 1.0 - (1.0 + period_rate).powf(-periods);
    if denom.is_finite() && denom.abs() > 1e-12 {
        period_rate / denom
    } else {
        1.0
    }
}
