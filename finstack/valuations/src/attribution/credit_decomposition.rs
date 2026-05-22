//! Credit-factor attribution detail and carry decomposition helpers.

use super::credit_cascade::{build_credit_factor_attribution, plan_credit_cascade};
use super::credit_factor::CreditFactorModelRef;
use super::spec::AttributionSpec;
use super::types::PnlAttribution;
use crate::instruments::DynInstrument;
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

impl AttributionSpec {
    /// Compute the optional `credit_factor_detail` field for a finished
    /// per-instrument attribution. The single instrument is treated as a
    /// one-position portfolio: its issuer id (read from
    /// `instrument.attributes().meta["credit::issuer_id"]`) is matched against
    /// `model.issuer_betas`, and a synthetic `CS01_i` is back-solved from the
    /// already-computed `credit_curves_pnl` and the observed average ΔS on the
    /// instrument's hazard curves so that
    /// `credit_curves_pnl ≡ -CS01_i × ΔS_i` holds by construction.
    ///
    /// This satisfies the reconciliation invariant
    /// `generic_pnl + Σ levels.total + adder_pnl_total ≡ credit_curves_pnl`
    /// for the single-instrument case. Multi-position wiring (true per-curve
    /// CS01 sums across a portfolio) is a portfolio-layer concern outside the
    /// PR-7 valuations scope.
    pub(crate) fn compute_credit_factor_detail(
        &self,
        model_ref: &CreditFactorModelRef,
        instrument: &std::sync::Arc<DynInstrument>,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
        attribution: &PnlAttribution,
        notes: &mut Vec<String>,
    ) -> Result<Option<super::CreditFactorAttribution>> {
        use finstack_core::market_data::diff::{measure_hazard_curve_shift, TenorSamplingMethod};
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

        // 4. Measure per-credit-curve shifts on the instrument's dependencies.
        //    Track BOTH the signed average move (`ds_i`, used for the
        //    back-solve) AND the absolute (L1) per-tenor move. A twisted curve
        //    — short tenors up, long tenors down — averages close to zero even
        //    though the curve genuinely moved; the L1 magnitude exposes that.
        let market_deps = instrument.market_dependencies()?;
        let credit_curves = &market_deps.curve_dependencies().credit_curves;
        if credit_curves.is_empty() {
            return Ok(None);
        }
        let mut total_shift_bp = 0.0;
        let mut total_abs_shift_bp = 0.0;
        let mut count = 0usize;
        for curve_id in credit_curves {
            if let Ok(shift) = measure_hazard_curve_shift(
                curve_id.as_str(),
                market_t0,
                market_t1,
                TenorSamplingMethod::Standard,
            ) {
                total_shift_bp += shift;
                total_abs_shift_bp +=
                    hazard_curve_abs_shift_bp(curve_id.as_str(), market_t0, market_t1);
                count += 1;
            }
        }
        if count == 0 {
            return Ok(None);
        }
        let avg_shift_bp = total_shift_bp / count as f64;
        let avg_abs_shift_bp = total_abs_shift_bp / count as f64;
        if avg_abs_shift_bp.abs() < 1e-12 {
            // No meaningful spread move at all; nothing to decompose.
            return Ok(None);
        }

        let mut ds_i = avg_shift_bp;

        // Guard the back-solve `CS01 = -credit_pnl / ds_i` against a
        // divide-by-near-zero. The back-solve is only well-conditioned when the
        // signed parallel move `ds_i` is a meaningful fraction of the curve's
        // total (L1) move. When the curve twisted, `|ds_i|` collapses toward
        // zero while `credit_pnl` stays large, so `-credit_pnl / ds_i` explodes
        // into a nonsensical CS01. Surface the degeneracy instead of producing
        // a meaningless decomposition.
        //
        // The threshold is `CreditFactorDetailOptions::parallel_fraction_floor`
        // (audit rec #13: previously hardcoded `1e-3`, now tunable per book —
        // books with structurally noisy hazard curves can relax it without
        // forking the code). Setting it to `0.0` disables the guard entirely.
        let parallel_fraction_floor = self.credit_factor_detail_options.parallel_fraction_floor;
        if parallel_fraction_floor > 0.0
            && avg_shift_bp.abs() < parallel_fraction_floor * avg_abs_shift_bp.abs()
        {
            let sign = if avg_shift_bp < 0.0 { -1.0 } else { 1.0 };
            ds_i = sign * avg_abs_shift_bp;
            notes.push(format!(
                "credit_factor_detail best-effort fallback: hazard curve(s) twisted \
                 (signed avg shift {:.6}bp vs absolute avg shift {:.6}bp; \
                 parallel_fraction_floor={:.3e}); using absolute avg shift for \
                 conditioning",
                avg_shift_bp, avg_abs_shift_bp, parallel_fraction_floor
            ));
        }

        // 6. Back-solve the effective CS01 from the existing credit_curves_pnl
        //    so the reconciliation `generic + Σlevels + adder ≡
        //    credit_curves_pnl` holds exactly. Here ds_i is in bp and CS01 is
        //    the dollar move per ΔS_i, so:
        //        credit_curves_pnl = -CS01 × ΔS_i  →  CS01 = -credit_pnl / ΔS_i
        let credit_pnl_amt = attribution.credit_curves_pnl.amount();
        let cs01_amt = -credit_pnl_amt / ds_i;

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
        let step_pnls: Vec<Money> = cascade
            .steps
            .iter()
            .map(|step| {
                Money::new(
                    -cs01_amt * step.delta_bp,
                    attribution.credit_curves_pnl.currency(),
                )
            })
            .collect();
        let detail = build_credit_factor_attribution(
            model,
            &cascade,
            &self.credit_factor_detail_options,
            &step_pnls,
        );
        Ok(Some(detail))
    }
}

/// Absolute (L1) hazard-curve shift in basis points, averaged over the standard
/// tenor grid.
///
/// Where [`measure_hazard_curve_shift`](finstack_core::market_data::diff::measure_hazard_curve_shift)
/// returns the *signed* mean shift (which collapses toward zero for a twisted
/// curve), this returns the mean of the per-tenor absolute shifts, so a
/// non-parallel move still registers a large magnitude. Returns `0.0` when
/// either side's curve is missing.
fn hazard_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_core::market_data::diff::STANDARD_TENORS;

    let (Ok(curve_t0), Ok(curve_t1)) = (
        market_t0.get_hazard(curve_id),
        market_t1.get_hazard(curve_id),
    ) else {
        return 0.0;
    };

    let mut total_abs = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        let h0 = curve_t0.hazard_rate(t);
        let h1 = curve_t1.hazard_rate(t);
        if h0.is_finite() && h1.is_finite() {
            total_abs += (h1 - h0).abs() * 10_000.0;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

impl AttributionSpec {
    /// Split `carry_detail.coupon_income` and `carry_detail.roll_down` into
    /// rates / credit parts and emit the per-factor
    /// `credit_carry_decomposition` (PR-8b §7).
    ///
    /// # Math (§7.3, §7.5)
    ///
    /// At `as_of_t0`, sample base discount rate `r` and the issuer's hazard
    /// rate `s` at the bond's tenor. With total yield `r + s`:
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
        let s = haz.hazard_rate_on_date(tenor_date).unwrap_or(0.0);

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
