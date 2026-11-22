//! Workout-path enumeration, exercise redemption valuation, YTW and I-spread inversion.

use super::super::ExitCandidate;
use super::super::types::YieldCompounding;
use super::super::workout_ladder::WorkoutExitKind;
use super::discounting::price_from_ytm;
use crate::bond::Bond;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::pricing::tree::BondValuator;
use crate::bond::pricing::ytm_solver::{YtmPricingSpec, solve_bond_ytm};
use finstack_quant_cashflows::accrual::AccrualIndex;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Schedule-derived inputs to exercise redemption amounts: the replayed
/// outstanding-balance path and the accrual index, built once per schedule
/// and shared by every workout candidate.
pub(crate) struct RedemptionBasis<'a> {
    schedule: &'a CashFlowSchedule,
    outstanding_by_date: Vec<(Date, Money)>,
    accrual_index: AccrualIndex,
}

impl<'a> RedemptionBasis<'a> {
    /// Replay the balance path and index accruals of `schedule`.
    ///
    /// # Arguments
    ///
    /// * `schedule` - The bond's full cashflow schedule.
    /// * `accrual_config` - The bond's accrual conventions (ex-coupon,
    ///   accrual method) used for accrued interest at exercise dates.
    pub(crate) fn new(
        schedule: &'a CashFlowSchedule,
        accrual_config: &finstack_quant_cashflows::accrual::AccrualConfig,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            schedule,
            outstanding_by_date: schedule.outstanding_by_date()?,
            accrual_index: AccrualIndex::build(schedule, accrual_config)?,
        })
    }

    /// Resolve the principal struck by an exercise right and any contractual
    /// redemption that the exercise proceeds replace on the same date.
    ///
    /// Scheduled amortization and PIK are applied before exercise. At
    /// contractual maturity, however, the positive `Notional` flow is the hold
    /// alternative, so the strike is quoted on the balance immediately before
    /// that redemption and the normal redemption must be removed from the
    /// candidate cashflow path.
    pub(crate) fn exercise_principal_and_replaced_redemption(
        &self,
        exercise_date: Date,
        maturity: Date,
    ) -> (f64, f64) {
        let after_events = finstack_quant_cashflows::builder::outstanding_on(
            &self.outstanding_by_date,
            exercise_date,
            self.schedule.get_notional().initial.amount(),
        );
        let (principal_redeemed, replaced_redemption) = if exercise_date == maturity {
            self.schedule
                .get_flows()
                .iter()
                .filter(|flow| {
                    flow.get_balance_date() == Some(exercise_date)
                        && flow.get_kind() == CFKind::Notional
                        && flow.get_amount().amount() > 0.0
                })
                .fold((0.0, 0.0), |(principal, same_day_cash), flow| {
                    (
                        principal + flow.get_amount().amount(),
                        same_day_cash
                            + if flow.get_date() == exercise_date {
                                flow.get_amount().amount()
                            } else {
                                0.0
                            },
                    )
                })
        } else {
            (0.0, 0.0)
        };
        (
            (after_events + principal_redeemed).max(0.0),
            replaced_redemption,
        )
    }

    /// Principal the contractual final redemption repays, and its price as
    /// percent of the balance outstanding immediately before it.
    fn maturity_redemption(&self, maturity: Date) -> ExitRedemption {
        let (outstanding, _) = self.exercise_principal_and_replaced_redemption(maturity, maturity);
        let clean_amount: f64 = self
            .schedule
            .get_flows()
            .iter()
            .filter(|flow| {
                flow.get_balance_date() == Some(maturity)
                    && flow.get_kind() == CFKind::Notional
                    && flow.get_amount().amount() > 0.0
            })
            .map(|flow| flow.get_amount().amount())
            .sum();
        ExitRedemption {
            clean_amount,
            price_pct: if outstanding > 0.0 {
                clean_amount / outstanding * 100.0
            } else {
                100.0
            },
        }
    }
}

/// Clean cash one workout exit redeems, before any accrued interest.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExitRedemption {
    /// Clean redemption in the bond's currency, make-whole premium included.
    pub(crate) clean_amount: f64,
    /// `clean_amount` as percent of the principal redeemed.
    pub(crate) price_pct: f64,
}

/// Resolve what one exercise candidate pays.
///
/// Returns the clean redemption alongside the dirty cash amount that the
/// candidate's path receives on the exercise date, which is net of any
/// contractual maturity redemption already present in the truncated flow path.
fn exercise_redemption(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    basis: &RedemptionBasis<'_>,
    candidate: &ExitCandidate,
) -> finstack_quant_core::Result<(ExitRedemption, f64)> {
    let (outstanding, replaced_redemption) =
        basis.exercise_principal_and_replaced_redemption(candidate.date, bond.maturity);
    let accrued = basis.accrual_index.accrued_at(candidate.date)?.amount();
    let floor_price = outstanding * candidate.price_pct_of_par / 100.0;
    let clean_redemption = if let Some(spec) = &candidate.make_whole {
        let reference_curve = curves.get_discount(&spec.reference_curve_id)?;
        BondValuator::make_whole_call_price(
            spec,
            reference_curve.as_ref(),
            candidate.date,
            flows,
            floor_price,
            accrued,
        )?
    } else {
        floor_price
    };
    let redemption = ExitRedemption {
        clean_amount: clean_redemption,
        price_pct: if outstanding > 0.0 {
            clean_redemption / outstanding * 100.0
        } else {
            candidate.price_pct_of_par
        },
    };
    Ok((redemption, clean_redemption + accrued - replaced_redemption))
}

/// Resolve the dirty exercise-date cash amount for one workout candidate.
///
/// Issuer calls retain the deterministic make-whole term and use the same
/// reference-curve valuation as the tree pricer. Put candidates have no
/// make-whole term. The returned amount is net of any contractual maturity
/// redemption already present in the truncated flow path.
pub(crate) fn exercise_redemption_amount(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    basis: &RedemptionBasis<'_>,
    candidate: &ExitCandidate,
) -> finstack_quant_core::Result<f64> {
    exercise_redemption(bond, curves, flows, basis, candidate).map(|(_, cash)| cash)
}

/// Enumerate call/put exit candidates for yield-to-worst analysis.
///
/// For each call or put window `[start_date, end_date]` in `bond.call_put`,
/// this function produces one `ExitCandidate` per admissible exercise date:
///
/// 1. Seed with the exact contractual `start_date` and `end_date`.
/// 2. Extend with any flow dates that fall within `[start_date, end_date]`.
/// 3. Sort and de-duplicate the resulting dates.
/// 4. Retain only dates in `[as_of, bond.maturity]`.
///
/// Returns an empty `Vec` when the bond has no `call_put` schedule.
///
/// # Arguments
///
/// * `bond`  – The bond whose `call_put` schedule is enumerated.
/// * `flows` – Holder-view cashflows used to align candidates to payment dates.
/// * `as_of` – Earliest admissible exercise date (valuation/quote date).
pub(crate) fn enumerate_exit_paths(
    bond: &Bond,
    flows: &[(Date, Money)],
    as_of: Date,
) -> Vec<ExitCandidate> {
    let Some(cp) = &bond.call_put else {
        return Vec::new();
    };

    let mut call_candidates: Vec<ExitCandidate> = Vec::new();
    let mut put_candidates: Vec<ExitCandidate> = Vec::new();

    let push_period_candidates = |candidates: &mut Vec<ExitCandidate>,
                                  option: &crate::bond::CallPut,
                                  kind: WorkoutExitKind| {
        let mut exercise_dates = vec![option.start, option.end];
        exercise_dates.extend(
            flows
                .iter()
                .map(|(date, _)| *date)
                .filter(|date| *date >= option.start && *date <= option.end),
        );
        exercise_dates.sort_unstable();
        exercise_dates.dedup();

        for exercise_date in exercise_dates {
            if exercise_date >= as_of && exercise_date <= bond.maturity {
                candidates.push(ExitCandidate {
                    kind,
                    date: exercise_date,
                    price_pct_of_par: option.price_pct_of_par,
                    make_whole: if kind == WorkoutExitKind::Call {
                        option.make_whole.clone()
                    } else {
                        None
                    },
                });
            }
        }
    };

    for c in &cp.calls {
        push_period_candidates(&mut call_candidates, c, WorkoutExitKind::Call);
    }
    for p in &cp.puts {
        push_period_candidates(&mut put_candidates, p, WorkoutExitKind::Put);
    }

    // Adjacent call windows can share boundary dates while carrying different
    // make-whole terms or reference curves. Retain every contractual call
    // candidate so each consumer can evaluate the realized redemption and
    // select the issuer-cheapest path. For puts, the holder exercises the
    // highest fixed strike, so same-date candidates can be collapsed here.
    call_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.price_pct_of_par.total_cmp(&right.price_pct_of_par))
    });
    put_candidates.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| right.price_pct_of_par.total_cmp(&left.price_pct_of_par))
    });
    put_candidates.dedup_by_key(|candidate| candidate.date);
    call_candidates.extend(put_candidates);

    call_candidates
}

/// One admissible exit and the holder cashflows received if it is taken.
#[derive(Debug, Clone)]
pub(crate) struct WorkoutCandidatePath {
    /// The exit: its kind, date and contractual terms.
    pub(crate) exit: ExitCandidate,
    /// What the exit redeems.
    pub(crate) redemption: ExitRedemption,
    /// Holder cashflows up to and including the exit.
    pub(crate) flows: Vec<(Date, Money)>,
}

/// Build every exercise and rolled-maturity cashflow path from one quote-date
/// entitlement set.
pub(super) fn workout_cashflow_paths(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    quote_date: Date,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<Vec<WorkoutCandidatePath>> {
    let has_quote_date_exercise = bond.call_put.as_ref().is_some_and(|option_schedule| {
        option_schedule
            .calls
            .iter()
            .chain(&option_schedule.puts)
            .any(|option| option.start <= quote_date && quote_date <= option.end)
    });
    let inclusive_flows;
    let path_flows = if has_quote_date_exercise {
        // Contractual holder cash on a live exercise date is paid before the
        // exercise decision. Rebuild from the materialized schedule so a
        // no-lag quote does not lose a same-day coupon at the usual strict
        // settlement boundary.
        inclusive_flows =
            bond.pricing_dated_cashflows_from_schedule_inclusive(schedule, quote_date, quote_date)?;
        inclusive_flows.as_slice()
    } else {
        flows
    };
    let mut candidates = enumerate_exit_paths(bond, path_flows, quote_date);

    // Contractual maturity can precede its business-day-adjusted final payment.
    // The held path must retain that rolled cash rather than truncating it at
    // the unadjusted maturity date.
    let maturity_candidate = path_flows
        .iter()
        .map(|(date, _)| *date)
        .max()
        .map_or(bond.maturity, |last| last.max(bond.maturity));
    candidates.push(ExitCandidate {
        kind: WorkoutExitKind::Maturity,
        date: maturity_candidate,
        price_pct_of_par: 0.0,
        make_whole: None,
    });

    let basis = RedemptionBasis::new(schedule, &bond.accrual_config())?;
    let mut paths = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut path: Vec<(Date, Money)> = path_flows
            .iter()
            .copied()
            .filter(|(date, _)| {
                (*date > quote_date || (has_quote_date_exercise && *date == quote_date))
                    && *date <= candidate.date
            })
            .collect();
        let (redemption, exit_cash) = if candidate.kind == WorkoutExitKind::Maturity {
            // The contractual redemption is already in the truncated flows.
            (basis.maturity_redemption(bond.maturity), 0.0)
        } else if candidate.price_pct_of_par > 0.0 {
            exercise_redemption(bond, curves, path_flows, &basis, &candidate)?
        } else {
            let nothing = ExitRedemption {
                clean_amount: 0.0,
                price_pct: 0.0,
            };
            (nothing, 0.0)
        };
        path.push((
            candidate.date,
            Money::from_model_value(exit_cash, bond.notional.currency())?,
        ));
        paths.push(WorkoutCandidatePath {
            exit: candidate,
            redemption,
            flows: path,
        });
    }

    Ok(paths)
}

/// Price a workout path while retaining holder cash paid on the exercise date.
pub(super) fn price_workout_path_from_yield(
    bond: &Bond,
    path: &[(Date, Money)],
    quote_date: Date,
    ytw: f64,
) -> finstack_quant_core::Result<f64> {
    let immediate_cash = path
        .iter()
        .filter(|(date, _)| *date == quote_date)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    Ok(immediate_cash + price_from_ytm(bond, path, quote_date, ytw)?)
}

/// Solve one workout path after separating non-discountable quote-date cash.
fn solve_workout_path_yield(
    bond: &Bond,
    path: &[(Date, Money)],
    quote_date: Date,
    dirty_price_target: Money,
) -> finstack_quant_core::Result<f64> {
    let immediate_cash = path
        .iter()
        .filter(|(date, _)| *date == quote_date)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    let future_flows: Vec<_> = path
        .iter()
        .copied()
        .filter(|(date, _)| *date > quote_date)
        .collect();
    let target = dirty_price_target.amount();
    let residual_target = target - immediate_cash;
    let tolerance = 1e-12 * target.abs().max(immediate_cash.abs()).max(1.0);

    if future_flows.is_empty() {
        return Ok(if immediate_cash < target - tolerance {
            // A lower immediate payoff is the limiting worst path as the
            // holding-period yield tends to negative infinity.
            f64::NEG_INFINITY
        } else {
            // Immediate cash at or above the target does not lower the minimum
            // finite yield supplied by another workout path.
            f64::INFINITY
        });
    }
    if residual_target <= tolerance {
        return Ok(f64::INFINITY);
    }

    let coupon_rate = bond.cashflow_spec.plain_fixed_rate()?.unwrap_or(0.0);
    solve_bond_ytm(
        bond,
        &future_flows,
        quote_date,
        Money::from_model_value(residual_target, dirty_price_target.currency())?,
        YtmPricingSpec {
            day_count: bond.cashflow_spec.day_count(),
            notional: bond.notional,
            coupon_rate,
            compounding: YieldCompounding::Street,
            frequency: bond.cashflow_spec.frequency(),
        },
    )
}

/// One workout path with the yield that equates it to the target price.
#[derive(Debug, Clone)]
pub(crate) struct SolvedWorkoutExit {
    /// The exit and its cashflow path.
    pub(crate) path: WorkoutCandidatePath,
    /// Street yield of the path, in walk order `f64::INFINITY` or
    /// `f64::NEG_INFINITY` when the exit is immediate and has no future cash.
    pub(crate) yield_rate: f64,
}

/// Solve every call/put/maturity exit for one flow set and pick the worst.
///
/// This is the single walk behind yield-to-worst and the workout ladder. The
/// second value is the index of the worst exit: the first strict minimum in
/// walk order, or `None` when no exit has a yield below positive infinity.
///
/// # Arguments
///
/// * `bond` - The bond whose exits are solved, return floor already lowered.
/// * `curves` - Market context supplying any make-whole reference curves.
/// * `flows` - Holder-view cashflows (coupons + principal).
/// * `as_of` - Quote date every path is timed from.
/// * `dirty_price_target` - Dirty price each path is solved to.
/// * `schedule` - Full cashflow schedule used for outstanding principal and
///   same-day maturity-redemption replacement.
pub(crate) fn solve_workout_exits(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    as_of: Date,
    dirty_price_target: Money,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<(Vec<SolvedWorkoutExit>, Option<usize>)> {
    let mut best_yield = f64::INFINITY;
    let mut worst = None;
    let mut exits = Vec::new();
    for path in workout_cashflow_paths(bond, curves, flows, as_of, schedule)? {
        let yield_rate = solve_workout_path_yield(bond, &path.flows, as_of, dirty_price_target)?;
        if yield_rate < best_yield {
            best_yield = yield_rate;
            worst = Some(exits.len());
        }
        exits.push(SolvedWorkoutExit { path, yield_rate });
    }
    Ok((exits, worst))
}

/// Solve yield-to-worst over all call/put/maturity candidates for a given flow set.
///
/// Returns the worst (minimum) yield and the corresponding truncated cashflow path.
///
/// # Call/Put Redemption Convention
///
/// Call/put redemption prices are dirty street redemption amounts:
/// `outstanding_principal × (price_pct_of_par / 100) + accrued_interest(exercise_date)`,
/// where `outstanding_principal` is the remaining principal at the exercise date after
/// any amortization. This correctly handles amortizing callable bonds and is consistent
/// with the tree-based OAS pricing.
///
/// # Arguments
///
/// * `bond` - The bond to calculate YTW for
/// * `curves` - Market context supplying any make-whole reference curves.
/// * `flows` - Holder-view cashflows (coupons + principal)
/// * `as_of` - Valuation/quote date
/// * `dirty_price_target` - Target dirty price to match
/// * `schedule` - Full cashflow schedule used for canonical outstanding
///   principal and same-day maturity-redemption replacement.
pub(crate) fn solve_ytw_from_flows(
    bond: &Bond,
    curves: &MarketContext,
    flows: &[(Date, Money)],
    as_of: Date,
    dirty_price_target: Money,
    schedule: &CashFlowSchedule,
) -> finstack_quant_core::Result<(f64, Vec<(Date, Money)>)> {
    let (mut exits, worst) =
        solve_workout_exits(bond, curves, flows, as_of, dirty_price_target, schedule)?;
    Ok(worst.map_or((f64::INFINITY, Vec::new()), |index| {
        let exit = exits.swap_remove(index);
        (exit.yield_rate, exit.path.flows)
    }))
}

/// Convert a Street yield-to-worst quote into settlement dirty price.
///
/// Every admissible call, put, and rolled-maturity path is priced at `ytw`.
/// Because each path yield decreases monotonically as dirty price increases,
/// the inverse of `YTW(P) = min_i yield_i(P)` is the minimum path price at the
/// target yield. Return-floor protection is first lowered into its effective
/// call schedule, and make-whole calls use their deterministic reference-curve
/// redemption.
///
/// # Arguments
///
/// * `bond` - Callable or puttable bond whose cashflow schedule and exercise
///   candidates define the yield-to-worst paths.
/// * `market` - Market context supplying schedule inputs and any make-whole
///   reference curves.
/// * `as_of` - Valuation or trade date; settlement and coupon entitlement are
///   derived from the bond's quote convention.
/// * `ytw` - Annual Street-compounded yield to worst as a decimal.
///
/// # Returns
///
/// Settlement-date dirty price in the bond's notional currency.
///
/// # Errors
///
/// Returns an error when `ytw` is non-finite, required curves or schedules are
/// unavailable, return-floor lowering fails, or a path cannot be priced under
/// the bond's Street yield convention.
pub fn price_from_ytw(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    ytw: f64,
) -> finstack_quant_core::Result<f64> {
    if !ytw.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "yield to worst must be finite, got {ytw}"
        )));
    }

    let effective_bond = bond.effective_for_pricing(market, as_of)?;
    let quote_ctx = QuoteDateContext::new(&effective_bond, market, as_of)?;
    let flows = quote_ctx.entitled_flows(&effective_bond, market, as_of)?;
    let schedule = effective_bond.full_cashflow_schedule(market)?;
    let paths = workout_cashflow_paths(
        &effective_bond,
        market,
        &flows,
        quote_ctx.quote_date,
        &schedule,
    )?;

    let mut best_price: Option<f64> = None;
    for path in paths {
        let price =
            price_workout_path_from_yield(&effective_bond, &path.flows, quote_ctx.quote_date, ytw)?;
        best_price = Some(best_price.map_or(price, |current| current.min(price)));
    }
    best_price.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "yield-to-worst inversion produced no workout paths".to_string(),
        )
    })
}

/// Convert an I-spread quote into settlement dirty price.
///
/// The inverse of `ISpreadCalculator`: for a bullet the price is the Street
/// YTM price at `i_spread + par(quote_date → maturity)`. For a bond with
/// exercise rights every workout path `k` is priced at
/// `i_spread + par(quote_date → T_k)`; the answer is the lowest such price
/// whose own yield-to-worst path is `k` (self-consistent), so the metric
/// recovers `i_spread` at that price. When no path is self-consistent (the
/// quote falls in the gap where the workout date flips), the lowest path
/// price is returned.
///
/// # Arguments
///
/// * `bond` - Bond whose schedule, exercise rights and conventions define the
///   yield paths and the proxy swap leg.
/// * `curves` - Market context with the bond's discount curve.
/// * `as_of` - Valuation date; settlement and coupon entitlement follow the
///   bond's quote convention.
/// * `i_spread` - I-spread as a decimal (e.g. `0.015` for 150 bp).
pub(crate) fn price_from_i_spread(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    i_spread: f64,
) -> finstack_quant_core::Result<f64> {
    use super::super::curve_par_rate;

    let disc = curves.get_discount(&bond.discount_curve_id)?;
    let effective_bond = bond.effective_for_pricing(curves, as_of)?;
    let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
    let flows = quote_ctx.entitled_flows(bond, curves, as_of)?;
    let quote_date = quote_ctx.quote_date;
    if !effective_bond
        .call_put
        .as_ref()
        .is_some_and(|cp| cp.has_options())
    {
        let par = curve_par_rate(bond, disc.as_ref(), quote_date, bond.maturity)?;
        return price_from_ytm(bond, &flows, quote_date, i_spread + par);
    }

    let schedule = effective_bond.full_cashflow_schedule(curves)?;
    let paths = workout_cashflow_paths(&effective_bond, curves, &flows, quote_date, &schedule)?;
    let mut candidates = Vec::with_capacity(paths.len());
    for path in paths {
        let horizon = path.flows.last().map_or(bond.maturity, |(date, _)| *date);
        let par = curve_par_rate(bond, disc.as_ref(), quote_date, horizon)?;
        let price = price_workout_path_from_yield(
            &effective_bond,
            &path.flows,
            quote_date,
            i_spread + par,
        )?;
        candidates.push((price, horizon));
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(price, horizon) in &candidates {
        let (_, workout) = solve_ytw_from_flows(
            &effective_bond,
            curves,
            &flows,
            quote_date,
            Money::from_model_value(price, bond.notional.currency())?,
            &schedule,
        )?;
        if workout.last().map(|(date, _)| *date) == Some(horizon) {
            return Ok(price);
        }
    }
    candidates.first().map(|&(price, _)| price).ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "I-spread inversion produced no workout paths".to_string(),
        )
    })
}
