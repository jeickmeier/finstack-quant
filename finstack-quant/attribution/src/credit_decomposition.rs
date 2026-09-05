//! Credit-factor attribution detail and carry decomposition helpers.

use super::credit_cascade::{
    apply_curve_shape_residual, build_credit_factor_attribution, hierarchy_level_name,
    optional_single_issuer_adder, plan_credit_cascade, shift_credit_curves_par_spread,
    single_issuer_by_bucket, CreditStepKind,
};
use super::factors::{MarketRestoreFlags, MarketSnapshot};
use super::spec::AttributionSpec;
use super::types::PnlAttribution;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_models::factor::credit::hierarchy::CreditFactorModel;
use finstack_quant_valuations::instruments::Instrument;

impl AttributionSpec {
    /// Compute the optional `credit_factor_detail` field for a finished
    /// per-instrument attribution. The instrument's issuer (from
    /// `instrument.attributes().meta["credit::issuer_id"]`) is matched against
    /// `model.issuer_betas`; the credit-factor cascade supplies the per-factor
    /// par-spread moves (`β·ΔF` / `Δadder`), and a **real** aggregate par-spread
    /// CS01 — measured by a parallel par-spread bump — gives each factor's P&L
    /// as `CS01 × Δs_factor`.
    ///
    /// # CS01 baseline
    ///
    /// CS01 is measured at the **same market state** the `credit_curves_pnl`
    /// baseline was computed against: `market_t1` with the issuer's hazard
    /// curves restored to T0, priced at `as_of_t1`. Measuring CS01 against
    /// `market_t0` at `as_of_t0` instead would silently absorb the T0→T1 drift
    /// of forwards, discounting, and recovery rates into `curve_shape_pnl`,
    /// distorting the meaning of the `generic` / level / adder components for
    /// any portfolio whose non-hazard markets drift between dates.
    ///
    /// The non-parallel (twist / curve-shape) part is the closing residual
    /// `curve_shape_pnl = credit_curves_pnl − Σ(parallel factor steps)`, so the
    /// reconciliation invariant
    /// `generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`
    /// holds exactly. A twisted credit curve simply lands in `curve_shape_pnl`
    /// — there is no divide-by-near-zero and no twist guard.
    pub(crate) fn compute_credit_factor_detail(
        &self,
        model: &CreditFactorModel,
        instrument: &std::sync::Arc<dyn Instrument>,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
        attribution: &PnlAttribution,
        notes: &mut Vec<String>,
    ) -> Result<Option<super::CreditFactorAttribution>> {
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::IssuerId;

        let issuer_id_str = match instrument
            .attributes()
            .get_meta(finstack_quant_models::factor::matching::ISSUER_ID_META_KEY)
        {
            Some(s) => s.to_string(),
            None => return Ok(None),
        };
        let issuer_id = IssuerId::new(issuer_id_str);

        let issuer_row = model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id);

        if issuer_row.is_none() {
            notes.push(format!(
                "credit_factor_detail unavailable: issuer {} not present in \
                 CreditFactorModel.issuer_betas",
                issuer_id
            ));
            return Ok(None);
        }

        let Some(cascade) = plan_credit_cascade(model, instrument, market_t0, market_t1)? else {
            return Ok(None);
        };
        notes.extend(cascade.warnings.iter().cloned());

        let credit_snapshot = MarketSnapshot::extract(market_t0, MarketRestoreFlags::CREDIT);
        let cs01_base_market =
            MarketSnapshot::restore_market(market_t1, &credit_snapshot, MarketRestoreFlags::CREDIT);
        let cs01_bump_bp = 1.0_f64;
        let disc = cascade.discount_curve_id.as_ref();
        let recalibration_provider = CachedRecalibrationProvider::new();
        let pv_up = instrument.value(
            &shift_credit_curves_par_spread(
                market_t0,
                &cs01_base_market,
                &cascade.hazard_curve_ids,
                disc,
                cs01_bump_bp,
                &recalibration_provider,
            )?,
            self.as_of_t1,
        )?;
        let pv_down = instrument.value(
            &shift_credit_curves_par_spread(
                market_t0,
                &cs01_base_market,
                &cascade.hazard_curve_ids,
                disc,
                -cs01_bump_bp,
                &recalibration_provider,
            )?,
            self.as_of_t1,
        )?;
        let cs01_amt = (pv_up.amount() - pv_down.amount()) / (2.0 * cs01_bump_bp);

        let ccy = attribution.credit_curves_pnl.currency();
        let mut step_pnls: Vec<Money> = cascade
            .steps
            .iter()
            .map(|step| {
                if matches!(step.kind, CreditStepKind::CurveShape) {
                    Ok(Money::from((0_i64, ccy)))
                } else {
                    Money::new(cs01_amt * step.delta_bp, ccy)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        apply_curve_shape_residual(
            &mut step_pnls,
            &cascade.steps,
            attribution.credit_curves_pnl,
        )?;

        let detail = build_credit_factor_attribution(
            model,
            &cascade,
            &self.credit_factor_detail_options,
            &step_pnls,
        )?;
        Ok(Some(detail))
    }
}

impl AttributionSpec {
    /// Split `carry_detail.coupon_income`, `carry_detail.pull_to_par` and
    /// `carry_detail.roll_down` into rates / credit parts and emit the
    /// per-factor `credit_carry_decomposition`.
    ///
    /// # Math (§7.3, §7.5)
    ///
    /// At `as_of_t0`, sample base discount rate `r` and the issuer's credit
    /// spread `s = hazard × (1 − recovery)` at the bond's tenor. With total
    /// risky yield `r + s` and credit share `w = s / (r + s)` (clamped to
    /// `[0, 1]`):
    ///
    /// - `coupon.credit_part = coupon.total × w`
    /// - `coupon.rates_part  = coupon.total − coupon.credit_part`
    /// - `pull_to_par` is split on the same `w`: it is discount-driven
    ///   convergence of PV toward par under the total risky yield, so its
    ///   credit share is well-defined by the same ratio. The wire field stays
    ///   a single `Money` (v1 schema); the split enters the two carry totals
    ///   so that `rates_carry_total + credit_carry_total ≡ carry_detail.total`
    ///   holds exactly.
    /// - `roll.credit_part   = 0` (v1: scalar level factors, no term-structure
    ///   adder → all credit roll-down lands in adder, which is 0 here)
    /// - `roll.rates_part    = roll.total`
    ///
    /// **Negative total yield**: when `r + s ≤ 0` with `s > 0`
    /// (negative-rate books where the base rate overwhelms the spread),
    /// `s / (r + s)` is negative. Curve builders enforce `hazard ≥ 0` and
    /// `recovery ∈ [0, 1]`, so the spread is the only non-negative yield
    /// component — the credit share is set to `1` and a diagnostic note is
    /// pushed.
    ///
    /// The per-factor allocation of the total credit carry uses the issuer's
    /// spread decomposition at `as_of_t0`:
    /// `S_i = β_i^PC·g + Σ_k β_i^k·L_k(g_i^k) + adder_i`.
    /// Each factor's credit-carry share is its contribution to `S_i` scaled
    /// by `credit_carry_total / S_i`, so
    /// `generic + Σ levels + adder ≡ credit_carry_total` by construction.
    ///
    /// Best-effort: returns `Ok(())` and leaves the existing CarryDetail
    /// alone if the inputs are missing (no carry detail, no issuer in model,
    /// no resolvable hazard curve). Hard-errors if validation fails.
    pub(crate) fn compute_carry_credit_split_and_decomposition(
        &self,
        model: &CreditFactorModel,
        instrument: &std::sync::Arc<dyn Instrument>,
        market_t0: &MarketContext,
        attribution: &mut PnlAttribution,
    ) -> Result<()> {
        use super::credit_factor::credit_factor_model_id;
        use super::types::{CreditCarryByLevel, CreditCarryDecomposition, LevelCarry, SourceLine};
        use finstack_quant_core::math::Compounding;
        use finstack_quant_core::money::Money;

        let Some(carry_detail) = attribution.carry_detail.as_mut() else {
            return Ok(());
        };
        let ccy = carry_detail.total.currency();

        let issuer_id_str = match instrument
            .attributes()
            .get_meta(finstack_quant_models::factor::matching::ISSUER_ID_META_KEY)
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let issuer_id = finstack_quant_core::types::IssuerId::new(issuer_id_str);
        let Some(issuer_row) = model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id) else {
            return Ok(());
        };

        let market_deps = instrument.market_dependencies()?;
        let credit_curves = &market_deps.curves.credit_curves;
        let discount_curves = &market_deps.curves.discount_curves;
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

        let fallback_tenor = finstack_quant_core::dates::Tenor::new(
            5,
            finstack_quant_core::dates::TenorUnit::Years,
        )?;
        let tenor_date = instrument.expiry().unwrap_or_else(|| {
            let cal_code = instrument
                .attributes()
                .get_meta("calendar")
                .or_else(|| instrument.attributes().get_meta("calendar_id"))
                .unwrap_or("usny");
            let calendar = finstack_quant_core::dates::calendar_by_id(cal_code)
                .or_else(|| finstack_quant_core::dates::calendar_by_id("usny"));
            let tenor = fallback_tenor;
            tenor
                .add_to_date(
                    self.as_of_t0,
                    calendar,
                    finstack_quant_core::dates::BusinessDayConvention::Following,
                )
                .unwrap_or_else(|_| {
                    let dur_days = (5.0 * 365.25) as i64;
                    self.as_of_t0
                        .checked_add(time::Duration::days(dur_days))
                        .unwrap_or(self.as_of_t0)
                })
        });
        let mut lookup_warnings: Vec<String> = Vec::new();
        let r = match disc.zero_rate_on_date(tenor_date, Compounding::Continuous) {
            Ok(v) => v,
            Err(e) => {
                lookup_warnings.push(format!(
                    "Carry credit split: discount zero-rate lookup failed ({e}); \
                     rates leg of the split treated as 0"
                ));
                0.0
            }
        };
        // Credit triangle: the spread driving the credit share of yield is the
        // hazard rate scaled by LGD = 1 − recovery (O'Kane, "Modelling
        // Single-name and Multi-name Credit Derivatives", Ch. 5; Hull Ch. 24).
        // The bare hazard rate would overstate the credit portion by 1/(1−R).
        let hazard = match haz.hazard_rate_on_date(tenor_date) {
            Ok(v) => v,
            Err(e) => {
                lookup_warnings.push(format!(
                    "Carry credit split: hazard-rate lookup failed ({e}); \
                     credit leg of the split treated as 0"
                ));
                0.0
            }
        };
        let s = hazard * (1.0 - haz.recovery_rate());

        let coupon = match carry_detail.coupon_income.as_ref() {
            Some(line) => line.total,
            None => return Ok(()),
        };
        let total_yield = r + s;
        let denominator_is_stable = total_yield.abs() > 1e-12 * r.abs().max(s).max(1e-3);
        let credit_share = if s > 0.0 && (total_yield <= 0.0 || !denominator_is_stable) {
            lookup_warnings.push(format!(
                "Carry credit split: negative total risky yield (r = {r:.6}, s = {s:.6}); \
                 the spread is the only positive-yield component, so the coupon and \
                 pull-to-par carry are attributed fully to credit (share = 1)"
            ));
            1.0
        } else if denominator_is_stable {
            (s / total_yield).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let coupon_credit_amt = coupon.amount() * credit_share;
        let (coupon_rates, coupon_credit) = (
            Money::new(coupon.amount() - coupon_credit_amt, ccy)?,
            Money::new(coupon_credit_amt, ccy)?,
        );

        let roll = carry_detail.roll_down.as_ref().map(|l| l.total);
        let (roll_rates, roll_credit) = match roll {
            Some(r) => (r, Money::from((0_i64, ccy))),
            None => (Money::from((0_i64, ccy)), Money::from((0_i64, ccy))),
        };

        let ptp_amount = carry_detail.pull_to_par.map(|m| m.amount()).unwrap_or(0.0);
        let ptp_credit_amt = ptp_amount * credit_share;
        let ptp_rates_amt = ptp_amount - ptp_credit_amt;

        if carry_detail.coupon_income.is_some() {
            carry_detail.coupon_income =
                Some(SourceLine::split(coupon, coupon_rates, coupon_credit));
        }
        if let Some(roll_total) = roll {
            carry_detail.roll_down = Some(SourceLine::split(roll_total, roll_rates, roll_credit));
        }

        let credit_total = Money::new(
            coupon_credit.amount() + roll_credit.amount() + ptp_credit_amt,
            ccy,
        )?;

        let num_levels = model.hierarchy.levels.len();
        let g_anchor = model.anchor_state.pc;
        let beta_pc = issuer_row.betas.pc;
        let pc_share_of_s = beta_pc * g_anchor;

        let mut level_share_of_s: Vec<f64> = vec![0.0; num_levels];
        for (k, share) in level_share_of_s.iter_mut().enumerate() {
            let bucket = model.hierarchy.bucket_path(&issuer_row.tags, k);
            let lk_value = match (bucket, model.anchor_state.by_level.get(k)) {
                (Some(b), Some(level_anchor)) => {
                    level_anchor.values.get(&b).copied().unwrap_or(0.0)
                }
                _ => 0.0,
            };
            let beta_k = issuer_row.betas.levels.get(k).copied().unwrap_or(0.0);
            *share = beta_k * lk_value;
        }
        let adder_of_s = issuer_row.adder_at_anchor;

        let s_model: f64 = pc_share_of_s + level_share_of_s.iter().sum::<f64>() + adder_of_s;

        // Scaling factor: credit_carry_total / S_model (scaling
        // by only the coupon credit part broke `generic + Σ levels + adder ≡
        // credit_carry_total` the moment any non-coupon credit carry, e.g.
        // the pull-to-par credit share, was nonzero). If S_model is zero, we
        // cannot allocate proportionally — route the entire credit total
        // through `adder_total` so invariant 4 still holds.
        let scale_credit = if s_model.abs() > 1e-15 {
            credit_total.amount() / s_model
        } else {
            0.0
        };
        let mut levels_out: Vec<LevelCarry> = Vec::with_capacity(num_levels);
        for (k, level_share) in level_share_of_s.iter().enumerate() {
            let dim = &model.hierarchy.levels[k];
            let level_name = hierarchy_level_name(dim);
            let share = *level_share * scale_credit;
            let total_money = Money::new(share, ccy)?;
            let by_bucket = single_issuer_by_bucket(
                model,
                issuer_row,
                k,
                total_money,
                self.credit_factor_detail_options
                    .include_per_bucket_breakdown,
            );
            levels_out.push(LevelCarry {
                level_name,
                total: total_money,
                by_bucket,
            });
        }

        let generic_money = Money::new(pc_share_of_s * scale_credit, ccy)?;
        let adder_total_money = if s_model.abs() > 1e-15 {
            Money::new(adder_of_s * scale_credit, ccy)?
        } else {
            // Degenerate: no spread observable, route the entire credit
            // total to adder so invariant 4 still holds.
            credit_total
        };

        let adder_by_issuer = optional_single_issuer_adder(
            &issuer_id,
            adder_total_money,
            self.credit_factor_detail_options.include_per_issuer_adder,
        );

        // Rates carry total: Σ rates_parts + pull_to_par rates share, minus
        // funding only when `total` is already net of financing (metrics
        // `CarryTotal`). On the reprice path funding is an overlay and
        // `coupon + ptp + roll = total`, so subtracting it here would break
        // `rates_carry_total + credit_carry_total ≡ carry_detail.total`.
        let funding_cost = carry_detail.funding_cost.map(|m| m.amount()).unwrap_or(0.0);
        let price_carry = coupon.amount() + ptp_amount + roll.map(|m| m.amount()).unwrap_or(0.0);
        let total = carry_detail.total.amount();
        let funding_netted_in_total = funding_cost.abs() > 1e-12
            && (price_carry - funding_cost - total).abs() < (price_carry - total).abs() + 1e-8;
        let funding_in_rates = if funding_netted_in_total {
            funding_cost
        } else {
            0.0
        };
        let rates_carry_total = Money::new(
            coupon_rates.amount() + roll_rates.amount() + ptp_rates_amt - funding_in_rates,
            ccy,
        )?;

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
        attribution.meta.notes.extend(lookup_warnings);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::spec::AttributionSpec;
    use crate::types::{AttributionMethod, CarryDetail, PnlAttribution, SourceLine};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{create_date, Date};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{
        DiscountCurve, HazardCurve, ValidationMode,
    };
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, IssuerId};
    use finstack_quant_models::factor::credit::hierarchy::{
        AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditFactorModelSchema,
        CreditHierarchySpec, DateRange, FactorCorrelationMatrix, GenericFactorSpec,
        HierarchyDimension, IssuerBetaMode, IssuerBetaPolicy, IssuerBetaRow, IssuerBetas,
        IssuerTags, LevelsAtAnchor, VolState,
    };
    use finstack_quant_models::factor::matching::ISSUER_ID_META_KEY;
    use finstack_quant_models::factor::{
        FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
    };
    use finstack_quant_valuations::instruments::{Attributes, Bond, Instrument, InstrumentJson};
    use std::sync::Arc;
    use time::Month;

    fn empty_factor_config() -> FactorModelConfig {
        FactorModelConfig {
            factors: vec![],
            covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: Default::default(),
            bump_size: None,
            unmatched_policy: None,
        }
    }

    /// Model with anchor pc = 100, adder_at_anchor = 20 (level anchors empty),
    /// so the model-implied spread splits 100/120 generic and 20/120 adder.
    fn make_model() -> CreditFactorModel {
        let mut tags = std::collections::BTreeMap::new();
        tags.insert("rating".to_string(), "B".to_string());
        tags.insert("region".to_string(), "US".to_string());

        CreditFactorModel {
            schema: CreditFactorModelSchema::CURRENT,
            as_of: create_date(2024, Month::March, 29).unwrap(),
            calibration_window: DateRange {
                start: create_date(2022, Month::March, 29).unwrap(),
                end: create_date(2024, Month::March, 29).unwrap(),
            },
            policy: IssuerBetaPolicy::GloballyOff,
            generic_factor: GenericFactorSpec {
                name: "CDX HY".into(),
                series_id: "cdx.hy.5y".into(),
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
            },
            panel_frequency:
                finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
            use_returns_or_levels:
                finstack_quant_models::factor::credit::calibration::PanelSpace::Returns,
            bucket_weighting:
                finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
            config: empty_factor_config(),
            issuer_betas: vec![IssuerBetaRow {
                issuer_id: IssuerId::new("ISSUER-B"),
                tags: IssuerTags(tags),
                mode: IssuerBetaMode::IssuerBeta,
                betas: IssuerBetas {
                    pc: 1.0,
                    levels: vec![1.0, 1.0],
                },
                adder_at_anchor: 20.0,
                adder_vol_annualized: 0.0,
                adder_vol_source: AdderVolSource::Default,
                fit_quality: None,
                level_fit_quality: vec![],
                spread_duration: 1.0,
            }],
            anchor_state: LevelsAtAnchor {
                pc: 100.0,
                by_level: vec![],
            },
            static_correlation: FactorCorrelationMatrix::identity(vec![]),
            vol_state: VolState {
                factors: std::collections::BTreeMap::new(),
                idiosyncratic: std::collections::BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: CalibrationDiagnostics {
                mode_counts: std::collections::BTreeMap::new(),
                bucket_sizes_per_level: vec![],
                fold_ups: vec![],
                r_squared_histogram: None,
                tag_taxonomy: std::collections::BTreeMap::new(),
            },
        }
    }

    fn credit_bond(curve_id: &CurveId) -> Bond {
        let mut bond = Bond::fixed(
            "BOND-ISSUER-B",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2030, Month::January, 1).unwrap(),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond construction");
        bond.credit_curve_id = Some(curve_id.clone());
        bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
        bond
    }

    /// Flat continuously-compounded discount curve at rate `r` (may be negative).
    fn flat_discount(base: Date, r: f64) -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (10.0, (-r * 10.0).exp())])
            .interp(InterpStyle::LogLinear)
            .validation(ValidationMode::NegativeRateFriendly {
                forward_floor: -0.10,
            })
            .build()
            .unwrap()
    }

    fn flat_hazard(id: &str, base: Date, rate: f64, recovery: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(base)
            .recovery_rate(recovery)
            .knots([(1.0, rate), (10.0, rate)])
            .build()
            .unwrap()
    }

    fn spec(t0: Date, t1: Date, bond: Bond) -> AttributionSpec {
        AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: (&MarketContext::new()).into(),
            market_t1: (&MarketContext::new()).into(),
            as_of_t0: t0,
            as_of_t1: t1,
            method: AttributionMethod::MetricsBased,
            model_params_t0: None,
            config: None,
            credit_factor_model: None,
            credit_factor_detail_options: Default::default(),
            full_cross_attribution: false,
        }
    }

    /// CarryDetail with all four components nonzero:
    /// total = 1000 (coupon) + 300 (pull_to_par) + 200 (roll) − 50 (funding).
    fn four_component_carry_detail(ccy: Currency) -> CarryDetail {
        CarryDetail {
            total: Money::from((1450_i64, ccy)),
            coupon_income: Some(SourceLine::scalar(Money::from((1000_i64, ccy)))),
            pull_to_par: Some(Money::from((300_i64, ccy))),
            roll_down: Some(SourceLine::scalar(Money::from((200_i64, ccy)))),
            funding_cost: Some(Money::from((50_i64, ccy))),
        }
    }

    fn run_split(r: f64, hazard_rate: f64, recovery: f64) -> PnlAttribution {
        let t0 = create_date(2025, Month::January, 1).unwrap();
        let t1 = create_date(2025, Month::January, 2).unwrap();
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let bond = credit_bond(&curve_id);
        let instrument: Arc<dyn Instrument> = Arc::new(bond.clone());
        let model = make_model();
        let market_t0 = MarketContext::new()
            .insert(flat_discount(t0, r))
            .insert(flat_hazard(curve_id.as_str(), t0, hazard_rate, recovery));

        let mut attribution = PnlAttribution::new(
            Money::from((0_i64, Currency::USD)),
            "BOND-ISSUER-B",
            t0,
            t1,
            AttributionMethod::MetricsBased,
        );
        attribution.carry_detail = Some(four_component_carry_detail(Currency::USD));

        spec(t0, t1, bond)
            .compute_carry_credit_split_and_decomposition(
                &model,
                &instrument,
                &market_t0,
                &mut attribution,
            )
            .expect("carry credit split");
        attribution
    }

    /// M2: `rates_carry_total + credit_carry_total` must equal
    /// `carry_detail.total` exactly — pull_to_par must enter the partition
    /// (split on the same s/(r+s) ratio as the coupon), not vanish.
    #[test]
    fn carry_partition_includes_pull_to_par() {
        // r = 2% continuous, hazard 1% at recovery 40% → s = 0.6%.
        let attribution = run_split(0.02, 0.01, 0.4);
        let detail = attribution.carry_detail.as_ref().expect("carry detail");
        let decomp = attribution
            .credit_carry_decomposition
            .as_ref()
            .expect("decomposition");

        let partition_sum = decomp.rates_carry_total.amount() + decomp.credit_carry_total.amount();
        assert!(
            (partition_sum - detail.total.amount()).abs() < 1e-10,
            "rates ({}) + credit ({}) must equal carry_detail.total ({}); gap = {}",
            decomp.rates_carry_total.amount(),
            decomp.credit_carry_total.amount(),
            detail.total.amount(),
            partition_sum - detail.total.amount()
        );
        // The credit leg must carry the pull-to-par credit share, not just the
        // coupon share: credit_total > coupon_credit alone.
        let coupon_credit = detail
            .coupon_income
            .as_ref()
            .and_then(|l| l.credit_part)
            .expect("coupon credit part")
            .amount();
        assert!(
            decomp.credit_carry_total.amount() > coupon_credit + 1e-12,
            "credit_carry_total ({}) must include the pull_to_par credit share \
             beyond coupon_credit ({})",
            decomp.credit_carry_total.amount(),
            coupon_credit
        );
    }

    /// The per-factor allocation must sum to `credit_carry_total`
    /// (generic + Σ levels + adder ≡ credit_carry_total), not merely to the
    /// coupon credit share.
    #[test]
    fn per_factor_allocation_sums_to_credit_carry_total() {
        let attribution = run_split(0.02, 0.01, 0.4);
        let decomp = attribution
            .credit_carry_decomposition
            .as_ref()
            .expect("decomposition");

        let factor_sum = decomp.credit_by_level.generic.amount()
            + decomp
                .credit_by_level
                .levels
                .iter()
                .map(|l| l.total.amount())
                .sum::<f64>()
            + decomp.credit_by_level.adder_total.amount();
        assert!(
            (factor_sum - decomp.credit_carry_total.amount()).abs() < 1e-10,
            "generic + Σ levels + adder ({}) must equal credit_carry_total ({})",
            factor_sum,
            decomp.credit_carry_total.amount()
        );
        assert!(
            decomp.credit_carry_total.amount() > 0.0,
            "fixture must exercise a nonzero credit carry"
        );
    }

    /// With r = −2% and s = +1% the naive share s/(r+s) = −1 used to
    /// clamp to 0, routing the whole coupon to rates despite a genuine 100bp
    /// spread. The spread is the only positive-yield component, so the coupon
    /// must go to credit (share = 1) with a diagnostic note.
    #[test]
    fn negative_total_yield_routes_coupon_to_credit() {
        // r = −2% continuous; hazard 1% at recovery 0 → s = 1%; r + s = −1%.
        let attribution = run_split(-0.02, 0.01, 0.0);
        let detail = attribution.carry_detail.as_ref().expect("carry detail");
        let coupon = detail.coupon_income.as_ref().expect("coupon line");
        let coupon_credit = coupon.credit_part.expect("credit part").amount();
        let coupon_rates = coupon.rates_part.expect("rates part").amount();

        assert!(
            (coupon_credit - 1000.0).abs() < 1e-9,
            "with negative total yield and s > 0 the full coupon must be \
             credit carry, got credit = {coupon_credit}, rates = {coupon_rates}"
        );
        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|n| n.contains("negative total risky yield")),
            "a diagnostic note must flag the negative-yield credit share, \
             notes = {:?}",
            attribution.meta.notes
        );
    }
}
