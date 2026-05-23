//! Credit-factor attribution detail and carry decomposition helpers.

use super::credit_cascade::{
    build_credit_factor_attribution, plan_credit_cascade, shift_credit_curves_par_spread,
    CreditStepKind,
};
use super::credit_factor::CreditFactorModelRef;
use super::spec::AttributionSpec;
use super::types::PnlAttribution;
use crate::instruments::DynInstrument;
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

impl AttributionSpec {
    /// Compute the optional `credit_factor_detail` field for a finished
    /// per-instrument attribution. The instrument's issuer (from
    /// `instrument.attributes().meta["credit::issuer_id"]`) is matched against
    /// `model.issuer_betas`; the credit-factor cascade supplies the per-factor
    /// par-spread moves (`β·ΔF` / `Δadder`), and a **real** aggregate par-spread
    /// CS01 — measured by a parallel par-spread bump — gives each factor's P&L
    /// as `−CS01 × Δs_factor`.
    ///
    /// The non-parallel (twist / curve-shape) part is the closing residual
    /// `curve_shape_pnl = credit_curves_pnl − Σ(parallel factor steps)`, so the
    /// reconciliation invariant
    /// `generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`
    /// holds exactly. A twisted credit curve simply lands in `curve_shape_pnl`
    /// — there is no divide-by-near-zero and no twist guard.
    pub(crate) fn compute_credit_factor_detail(
        &self,
        model_ref: &CreditFactorModelRef,
        instrument: &std::sync::Arc<DynInstrument>,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
        attribution: &PnlAttribution,
        notes: &mut Vec<String>,
    ) -> Result<Option<super::CreditFactorAttribution>> {
        use finstack_core::money::Money;
        use finstack_core::types::IssuerId;

        let model = model_ref.as_ref();

        // 1. Resolve issuer id from instrument attributes.
        let issuer_id_str = match instrument
            .attributes()
            .get_meta(finstack_core::factor_model::matching::ISSUER_ID_META_KEY)
        {
            Some(s) => s.to_string(),
            None => return Ok(None),
        };
        let issuer_id = IssuerId::new(issuer_id_str);

        // 2. Find issuer in model.
        let issuer_row = model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id);

        // 3. Look up tags for this issuer; if the issuer is not in the model
        //    return Ok(None) with a diagnostic note rather than silently routing
        //    the entire credit move into adder_pnl_total.
        if issuer_row.is_none() {
            notes.push(format!(
                "credit_factor_detail unavailable: issuer {} not present in \
                 CreditFactorModel.issuer_betas",
                issuer_id
            ));
            return Ok(None);
        }

        // 4. Plan the credit-factor cascade. It resolves the issuer, its hazard
        //    curves and the per-factor par-spread moves (`β·ΔF` / `Δadder`).
        //    Returns None when no cascade can be planned (unmapped issuer, no
        //    hazard exposure, …).
        let Some(cascade) = plan_credit_cascade(
            model,
            instrument,
            market_t0,
            market_t1,
            self.as_of_t0,
            self.as_of_t1,
        )?
        else {
            return Ok(None);
        };

        // 5. Real aggregate **par-spread** CS01: bump every hazard curve the
        //    instrument depends on with the *same* `shift_credit_curves_par_spread`
        //    bump the cascade applies to its steps, central-differenced.
        //    Measuring CS01 with the identical par-spread bump guarantees
        //    `cs01_amt` and the cascade's per-step `delta_bp` share units exactly
        //    (par CDS spread bp), so `cs01_amt × delta_bp` is consistent and the
        //    decomposition reconciles to the par-spread `credit_curves_pnl`.
        //    This real CS01 replaces the former synthetic `−credit_pnl / ds_i`
        //    back-solve, whose divide-by-near-zero forced a twist workaround.
        let cs01_bump_bp = 1.0_f64;
        let disc = cascade.discount_curve_id.as_ref();
        let pv_up = instrument.value(
            &shift_credit_curves_par_spread(
                market_t0,
                &cascade.hazard_curve_ids,
                disc,
                cs01_bump_bp,
            )?,
            self.as_of_t0,
        )?;
        let pv_down = instrument.value(
            &shift_credit_curves_par_spread(
                market_t0,
                &cascade.hazard_curve_ids,
                disc,
                -cs01_bump_bp,
            )?,
            self.as_of_t0,
        )?;
        let cs01_amt = (pv_up.amount() - pv_down.amount()) / (2.0 * cs01_bump_bp);

        // 6. Each parallel factor step's P&L is its own contribution
        //    `−CS01 × Δs_factor`; the `CurveShape` step absorbs the non-parallel
        //    residual so `generic + Σ levels + adder + curve_shape ≡
        //    credit_curves_pnl` closes exactly. A twisted credit curve simply
        //    lands in `curve_shape` — no twist guard needed.
        let ccy = attribution.credit_curves_pnl.currency();
        let mut step_pnls: Vec<Money> = cascade
            .steps
            .iter()
            .map(|step| {
                if matches!(step.kind, CreditStepKind::CurveShape) {
                    Money::new(0.0, ccy)
                } else {
                    // P&L = ∂PV/∂s × Δs_factor. `cs01_amt` is already the signed
                    // PV sensitivity to an up-bump, so no extra negation.
                    Money::new(cs01_amt * step.delta_bp, ccy)
                }
            })
            .collect();
        let parallel_sum: f64 = step_pnls
            .iter()
            .zip(cascade.steps.iter())
            .filter(|(_, s)| !matches!(s.kind, CreditStepKind::CurveShape))
            .map(|(pnl, _)| pnl.amount())
            .sum();
        let curve_shape_amt = attribution.credit_curves_pnl.amount() - parallel_sum;
        for (pnl, step) in step_pnls.iter_mut().zip(cascade.steps.iter()) {
            if matches!(step.kind, CreditStepKind::CurveShape) {
                *pnl = Money::new(curve_shape_amt, ccy);
            }
        }

        let detail = build_credit_factor_attribution(
            model,
            &cascade,
            &self.credit_factor_detail_options,
            &step_pnls,
        );
        Ok(Some(detail))
    }
}

impl AttributionSpec {
    /// Split `carry_detail.coupon_income` and `carry_detail.roll_down` into
    /// rates / credit parts and emit the per-factor
    /// `credit_carry_decomposition` (PR-8b §7).
    ///
    /// # Math (§7.3, §7.5)
    ///
    /// At `as_of_t0`, sample base discount rate `r` and the issuer's credit
    /// spread `s = hazard × (1 − recovery)` at the bond's tenor. With total
    /// risky yield `r + s`:
    ///
    /// - `coupon.credit_part = coupon.total × s / (r + s)`
    /// - `coupon.rates_part  = coupon.total − coupon.credit_part`
    /// - `roll.credit_part   = 0` (v1: scalar level factors, no term-structure
    ///   adder → all credit roll-down lands in adder, which is 0 here)
    /// - `roll.rates_part    = roll.total`
    ///
    /// The per-factor allocation of `coupon.credit_part` uses the issuer's
    /// spread decomposition at `as_of_t0`:
    /// `S_i = β_i^PC·g + Σ_k β_i^k·L_k(g_i^k) + adder_i`.
    /// Each factor's credit-carry share is its contribution to `S_i` scaled
    /// by `coupon.credit_part / S_i`. Σ shares ≡ coupon.credit_part by
    /// construction.
    ///
    /// Best-effort: returns `Ok(())` and leaves the existing CarryDetail
    /// alone if the inputs are missing (no carry detail, no issuer in model,
    /// no resolvable hazard curve). Hard-errors if validation fails.
    pub(crate) fn compute_carry_credit_split_and_decomposition(
        &self,
        model_ref: &super::CreditFactorModelRef,
        instrument: &std::sync::Arc<DynInstrument>,
        market_t0: &MarketContext,
        attribution: &mut PnlAttribution,
    ) -> Result<()> {
        use super::credit_factor::credit_factor_model_id;
        use super::types::{CreditCarryByLevel, CreditCarryDecomposition, LevelCarry, SourceLine};
        use finstack_core::factor_model::credit_hierarchy::{dimension_key, HierarchyDimension};
        use finstack_core::math::Compounding;
        use finstack_core::money::Money;
        use std::collections::BTreeMap;

        // 0. Need a populated carry_detail to split.
        let carry_detail = match attribution.carry_detail.as_mut() {
            Some(d) => d,
            None => return Ok(()),
        };
        let ccy = carry_detail.total.currency();

        // 1. Resolve issuer.
        let issuer_id_str = match instrument
            .attributes()
            .get_meta(finstack_core::factor_model::matching::ISSUER_ID_META_KEY)
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let issuer_id = finstack_core::types::IssuerId::new(issuer_id_str);
        let model = model_ref.as_ref();
        let issuer_row = match model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id) {
            Some(r) => r,
            None => return Ok(()),
        };

        // 2. Find a credit (hazard) curve and discount curve on the instrument.
        let market_deps = instrument.market_dependencies()?;
        let credit_curves = &market_deps.curve_dependencies().credit_curves;
        let discount_curves = &market_deps.curve_dependencies().discount_curves;
        let credit_curve_id = match credit_curves.first() {
            Some(c) => c.clone(),
            None => return Ok(()),
        };
        let discount_curve_id = match discount_curves.first() {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let haz = market_t0.get_hazard(credit_curve_id.as_str())?;
        let disc = market_t0.get_discount(discount_curve_id.as_str())?;

        // 3. Sample base rate r and spread s at the bond's tenor (or 5y
        //    fallback). Use the instrument's expiry when available.
        let tenor_date = instrument.expiry().unwrap_or_else(|| {
            let cal_code = instrument
                .attributes()
                .get_meta("calendar")
                .or_else(|| instrument.attributes().get_meta("calendar_id"))
                .unwrap_or("usny");
            let calendar = finstack_core::dates::CalendarRegistry::global()
                .resolve_str(cal_code)
                .or_else(|| finstack_core::dates::CalendarRegistry::global().resolve_str("usny"));
            let _day_count = instrument
                .attributes()
                .get_meta("day_count")
                .or_else(|| instrument.attributes().get_meta("daycount"))
                .and_then(|dc| dc.parse::<finstack_core::dates::DayCount>().ok())
                .unwrap_or(finstack_core::dates::DayCount::Act365F);
            let tenor = finstack_core::dates::Tenor::new(5, finstack_core::dates::TenorUnit::Years);
            tenor
                .add_to_date(
                    self.as_of_t0,
                    calendar,
                    finstack_core::dates::BusinessDayConvention::Following,
                )
                .unwrap_or_else(|_| {
                    let dur_days = (5.0 * 365.25) as i64;
                    self.as_of_t0
                        .checked_add(time::Duration::days(dur_days))
                        .unwrap_or(self.as_of_t0)
                })
        });
        let r = disc
            .zero_rate_on_date(tenor_date, Compounding::Continuous)
            .unwrap_or(0.0);
        // Credit triangle: the spread driving the credit share of yield is the
        // hazard rate scaled by LGD = 1 − recovery (O'Kane, "Modelling
        // Single-name and Multi-name Credit Derivatives", Ch. 5; Hull Ch. 24).
        // The bare hazard rate would overstate the credit portion by 1/(1−R).
        let hazard = haz.hazard_rate_on_date(tenor_date).unwrap_or(0.0);
        let s = hazard * (1.0 - haz.recovery_rate());

        // 4. Split coupon_income proportionally to r and s.
        // coupon_income must be present; if not, skip the decomposition entirely.
        // Emitting zeros would be indistinguishable from a genuinely zero-spread
        // issuer, so we return Ok(()) to match the existing early-return pattern
        // used above for missing issuer_id, credit curve, etc.
        // Note: "credit_carry_decomposition skipped: coupon_income not present".
        let coupon = match carry_detail.coupon_income.as_ref() {
            Some(line) => line.total,
            None => return Ok(()),
        };
        let total_yield = r + s;
        let (coupon_rates, coupon_credit) = if total_yield.abs() > 1e-15 {
            let credit_amt = coupon.amount() * (s / total_yield);
            let rates_amt = coupon.amount() - credit_amt;
            (Money::new(rates_amt, ccy), Money::new(credit_amt, ccy))
        } else {
            // Degenerate: zero total yield. Push everything to rates.
            (coupon, Money::new(0.0, ccy))
        };

        // 5. Split roll_down. v1: scalar level factors → all credit roll
        //    flows to adder, and the model carries no adder term structure
        //    (only a scalar `adder_at_anchor`), so credit roll = 0 over the
        //    period. All roll_down lands in rates_part.
        let roll = carry_detail.roll_down.as_ref().map(|l| l.total);
        let (roll_rates, roll_credit) = match roll {
            Some(r) => (r, Money::new(0.0, ccy)),
            None => (Money::new(0.0, ccy), Money::new(0.0, ccy)),
        };

        // 6. Update CarryDetail's source lines with the split. If the field
        //    was None we don't synthesize (keeps no-model behavior tight).
        if carry_detail.coupon_income.is_some() {
            carry_detail.coupon_income =
                Some(SourceLine::split(coupon, coupon_rates, coupon_credit));
        }
        if let Some(roll_total) = roll {
            carry_detail.roll_down = Some(SourceLine::split(roll_total, roll_rates, roll_credit));
        }

        // 7. Per-factor allocation of credit_carry_total. Use the issuer's
        //    spread decomposition at as_of_t0 to partition `coupon_credit`
        //    across generic / each level / adder. The issuer's spread
        //    satisfies the linear identity
        //    `S = β_PC·g + Σ_k β_k · L_k(g_i^k) + adder_i`.
        //    We compute each piece, then scale by `coupon_credit / S` so
        //    pieces sum to `coupon_credit`. (When `coupon_credit` is zero we
        //    short-circuit and emit zeros.)
        let credit_total = Money::new(coupon_credit.amount() + roll_credit.amount(), ccy);

        let num_levels = model.hierarchy.levels.len();

        // Compute each piece of the model-implied spread:
        //   S_model = β_PC·g_anchor + Σ_k β_k · L_k(g_i^k, anchor) + adder_at_anchor.
        // We allocate `coupon_credit` proportionally to these pieces so that
        // generic + Σ levels + adder == credit_carry_total exactly (§7.4 inv 4).
        // Using the model-implied S (rather than the observed hazard rate)
        // keeps the reconciliation tight by construction even when the
        // calibrated decomposition does not exactly match the market curve.
        let g_anchor = model.anchor_state.pc;
        let beta_pc = issuer_row.betas.pc;
        let pc_share_of_s = beta_pc * g_anchor;

        let mut level_share_of_s: Vec<f64> = vec![0.0; num_levels];
        let mut level_bucket: Vec<(String, f64)> = Vec::with_capacity(num_levels);
        for (k, share) in level_share_of_s.iter_mut().enumerate() {
            let bucket = model.hierarchy.bucket_path(&issuer_row.tags, k);
            let (lk_value, bucket_label) = match (bucket, model.anchor_state.by_level.get(k)) {
                (Some(b), Some(level_anchor)) => {
                    let v = level_anchor.values.get(&b).copied().unwrap_or(0.0);
                    (v, b)
                }
                _ => (0.0, String::new()),
            };
            let beta_k = issuer_row.betas.levels.get(k).copied().unwrap_or(0.0);
            *share = beta_k * lk_value;
            level_bucket.push((bucket_label, lk_value));
        }
        let adder_of_s = issuer_row.adder_at_anchor;

        let s_model: f64 = pc_share_of_s + level_share_of_s.iter().sum::<f64>() + adder_of_s;

        // Scaling factor: coupon_credit / S_model. If S_model is zero,
        // we cannot allocate proportionally — route the entire credit total
        // through `adder_total` so invariant 4 still holds.
        let scale_coupon = if s_model.abs() > 1e-15 {
            coupon_credit.amount() / s_model
        } else {
            0.0
        };
        // Build the LevelCarry vector.
        let mut levels_out: Vec<LevelCarry> = Vec::with_capacity(num_levels);
        for k in 0..num_levels {
            let dim = &model.hierarchy.levels[k];
            let level_name = match dim {
                HierarchyDimension::Custom(s) => s.clone(),
                _ => dimension_key(dim),
            };
            let share = level_share_of_s[k] * scale_coupon;
            let total_money = Money::new(share, ccy);
            let mut by_bucket = BTreeMap::new();
            if self
                .credit_factor_detail_options
                .include_per_bucket_breakdown
            {
                let (bucket_path, _l_value) = &level_bucket[k];
                if !bucket_path.is_empty() {
                    by_bucket.insert(bucket_path.clone(), total_money);
                }
            }
            levels_out.push(LevelCarry {
                level_name,
                total: total_money,
                by_bucket,
            });
        }

        let generic_money = Money::new(pc_share_of_s * scale_coupon, ccy);
        let adder_total_money = if s_model.abs() > 1e-15 {
            Money::new(adder_of_s * scale_coupon, ccy)
        } else {
            // Degenerate: no spread observable, route the entire credit
            // total to adder so invariant 4 still holds.
            credit_total
        };

        let adder_by_issuer = if self.credit_factor_detail_options.include_per_issuer_adder {
            let mut m = BTreeMap::new();
            m.insert(issuer_id.clone(), adder_total_money);
            Some(m)
        } else {
            None
        };

        // Rates carry total: Σ rates_parts − funding_cost.
        let funding_cost = carry_detail.funding_cost.map(|m| m.amount()).unwrap_or(0.0);
        let rates_carry_total = Money::new(
            coupon_rates.amount() + roll_rates.amount() - funding_cost,
            ccy,
        );

        attribution.credit_carry_decomposition = Some(CreditCarryDecomposition {
            model_id: credit_factor_model_id(model),
            rates_carry_total,
            credit_carry_total: credit_total,
            credit_by_level: CreditCarryByLevel {
                generic: generic_money,
                levels: levels_out,
                adder_total: adder_total_money,
                adder_by_issuer,
            },
        });

        Ok(())
    }
}
