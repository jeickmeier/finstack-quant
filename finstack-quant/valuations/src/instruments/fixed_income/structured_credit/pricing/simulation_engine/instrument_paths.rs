//! Stochastic paths over instrument collateral.
//!
//! One [`InstrumentPathFlowSource`] drives one scenario path of a pool of
//! real instruments. Each period it takes the engine's period shock (the
//! pool-wide prepayment and recovery assumptions, the systematic credit
//! factor `Z` and any rate-path shift), resolves each name's default from
//! its own hazard curve or the deal model — through the per-name copula when
//! the deal has one — and advances every stochastic revolver's spread and
//! utilization before handing the period to the shared instrument engine
//! ([`run_period`]).
//!
//! # Per-name shocks
//!
//! For a revolver `i` with the deal's asset correlation `ρ` and the
//! facility's utilization–credit correlation `c`, the period draws two
//! idiosyncratic normals `η^s_i`, `η^u_i` (negated on the antithetic
//! partner) and forms
//!
//! ```text
//! ε^s_i = −(√ρ · Z + √(1 − ρ) · η^s_i)          spread shock (stress ⇒ wider)
//! ε^u_i = c · ε^s_i + √(1 − c²) · η^u_i          utilization shock
//! ```
//!
//! so wider spreads and higher utilization arrive together on stress paths,
//! the adverse selection the standalone facility already embeds. The spread
//! follows the facility's CIR process (market-anchored on its hazard curve)
//! with the QE step, and the utilization the exact OU step toward the
//! spread-linked target `clamp(θ + β(s/s₀ − 1), 0, 1)`. Zero utilization
//! volatility freezes the utilization, as in the standalone path generator.

use super::exercise::{par_forward_rate, RateView};
use super::instrument_flows::{
    hazard_period_default, run_period, MarginTerms, NamePeriod, NameState, PeriodDefaultSource,
    PeriodModel, PreparedInstrumentSchedules, SpreadTerms, FROZEN_UTILIZATION_VOL,
};
use super::pool_flow_source::{PerNameResolution, PeriodShockSource};
use super::*;
use crate::instruments::fixed_income::revolving_credit::pricing::monte_carlo_discretization::{
    ou_exact_step, spread_linked_target,
};
use crate::instruments::fixed_income::revolving_credit::MC_CLOCK_DAY_COUNT;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_models::monte_carlo::discretization::QeCir;
use finstack_quant_models::monte_carlo::traits::Discretization;

/// One funded draw of a stochastic revolver on a path, the forward loan the
/// deal made at the contractual margin.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DrawRecord {
    /// Pool row of the revolver.
    name: usize,
    /// Legal period the draw was funded in.
    period: usize,
    /// Funded amount.
    amount: f64,
    /// Path's fair spread at the draw less the contractual margin
    /// (`s_fair − s_K`, decimal).
    spread_excess: f64,
    /// Survival scale of the name when the draw was funded.
    scale_at_draw: f64,
}

/// [`PoolFlowSource`] for one scenario path over instrument collateral.
pub(crate) struct InstrumentPathFlowSource<'a, S: PeriodShockSource> {
    prepared: &'a PreparedInstrumentSchedules,
    names: Vec<NameState>,
    shocks: S,
    /// Draws funded on this path, recorded on the actual run.
    records: Vec<DrawRecord>,
    /// Draws of the actual run whose interest this counterfactual run
    /// accrues at the fair spread; `None` on the actual run.
    counterfactual: Option<Vec<DrawRecord>>,
    /// Per-name copula engine; `None` when the deal default model is not a
    /// copula, in which case names follow their hazard curve or the period's
    /// realized pool-wide default rate.
    per_name: Option<PerNameDefaultEngine>,
    /// Deal asset correlation `ρ` loading each name's spread shock on `Z`.
    asset_correlation: f64,
    /// Path-local stream for the revolver process draws.
    rng: PhiloxRng,
    /// `true` ⇒ antithetic partner: idiosyncratic process draws are negated.
    antithetic: bool,
    next_period: usize,
    inputs: Vec<NamePeriod>,
    alive: Vec<usize>,
    marginal_scratch: Vec<f64>,
    default_scratch: Vec<bool>,
    conditional_scratch: Vec<f64>,
    cir: QeCir,
}

impl<'a, S: PeriodShockSource> InstrumentPathFlowSource<'a, S> {
    /// Create the flow source for one scenario path.
    ///
    /// # Arguments
    ///
    /// * `prepared` - Shared bucketed instrument schedules.
    /// * `shocks` - Per-period scenario inputs of this path.
    /// * `per_name` - Per-name copula engine when the deal default model is a copula.
    /// * `asset_correlation` - Deal asset correlation `ρ` in `[0, 1)` loading
    ///   revolver spread shocks on the systematic factor.
    /// * `rng` - Path-local stream for revolver process draws; antithetic
    ///   partners share their pair's stream.
    /// * `antithetic` - `true` for the second member of an antithetic pair.
    pub(crate) fn new(
        prepared: &'a PreparedInstrumentSchedules,
        shocks: S,
        per_name: Option<PerNameDefaultEngine>,
        asset_correlation: f64,
        rng: PhiloxRng,
        antithetic: bool,
    ) -> Self {
        let n = prepared.schedules.len();
        Self {
            prepared,
            names: prepared.initial_names(),
            shocks,
            records: Vec::new(),
            counterfactual: None,
            per_name,
            asset_correlation: asset_correlation.clamp(0.0, 1.0),
            rng,
            antithetic,
            next_period: 0,
            inputs: Vec::with_capacity(n),
            alive: Vec::with_capacity(n),
            marginal_scratch: Vec::with_capacity(n),
            default_scratch: Vec::with_capacity(n),
            conditional_scratch: Vec::with_capacity(n),
            cir: QeCir::new(),
        }
    }

    /// Turn this source into the counterfactual replay of a path: the same
    /// shocks and streams, with the interest of `records` (the actual run's
    /// draws) accruing at each draw's fair spread instead of the margin.
    ///
    /// # Arguments
    ///
    /// * `records` - Draws taken from the actual run of the same path.
    pub(crate) fn with_counterfactual(mut self, records: Vec<DrawRecord>) -> Self {
        self.counterfactual = Some(records);
        self
    }

    /// Draws funded so far on the actual run, in period order.
    pub(crate) fn take_draw_records(&mut self) -> Vec<DrawRecord> {
        std::mem::take(&mut self.records)
    }

    /// Counterfactual interest add-on per name for period `k`: every earlier
    /// draw of a live revolver accrues `amount · (s_fair − s_K)` over the
    /// period, scaled by the survival realized since the draw.
    fn apply_counterfactual_addons(
        &mut self,
        k: usize,
        period_start: Date,
        pay_date: Date,
    ) -> Result<()> {
        let Some(records) = self.counterfactual.as_ref() else {
            return Ok(());
        };
        for record in records {
            if record.period >= k {
                continue;
            }
            let i = record.name;
            let schedule = &self.prepared.schedules[i];
            let name = &self.names[i];
            if name.retired || period_start >= schedule.maturity {
                continue;
            }
            let Some(terms) = schedule.revolver.as_ref() else {
                continue;
            };
            let end = pay_date.min(schedule.maturity);
            if end <= period_start {
                continue;
            }
            let accrual =
                terms
                    .day_count
                    .year_fraction(period_start, end, DayCountContext::default())?;
            let survival = if record.scale_at_draw > 0.0 {
                (name.scale / record.scale_at_draw).clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.inputs[i].interest_addon +=
                record.amount * record.spread_excess * accrual * survival;
        }
        Ok(())
    }

    /// Record the draws funded this period on the actual run, valued at the
    /// name's post-step spread against its contractual margin.
    fn record_draws(
        &mut self,
        k: usize,
        pay_date: Date,
        curve: Option<&dyn Discounting>,
    ) -> Result<()> {
        if self.counterfactual.is_some() {
            return Ok(());
        }
        for (i, schedule) in self.prepared.schedules.iter().enumerate() {
            let name = &self.names[i];
            if name.period_draw <= 0.0 {
                continue;
            }
            let Some(terms) = schedule
                .revolver
                .as_ref()
                .filter(|r| r.utilization.is_some())
            else {
                continue;
            };
            let margin = match terms.margin {
                MarginTerms::Spread(spread) => spread,
                MarginTerms::FixedRate(rate) => {
                    let curve = curve.ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "valuing draws of a fixed-rate revolver requires the deal discount curve"
                                .into(),
                        )
                    })?;
                    rate - par_forward_rate(curve, pay_date, schedule.maturity)?
                }
            };
            self.records.push(DrawRecord {
                name: i,
                period: k,
                amount: name.period_draw,
                spread_excess: name.spread.max(0.0) - margin,
                scale_at_draw: name.scale,
            });
        }
        Ok(())
    }

    /// One standard normal draw, negated on the antithetic partner.
    fn draw(&mut self) -> f64 {
        let raw = self.rng.next_std_normal();
        if self.antithetic {
            -raw
        } else {
            raw
        }
    }

    /// Advance every live stochastic revolver's spread and utilization over
    /// `[period_start, pay_date]` and record its end-of-period utilization.
    fn step_revolvers(
        &mut self,
        systematic_z: f64,
        period_start: Date,
        pay_date: Date,
    ) -> Result<()> {
        let dt = if pay_date > period_start {
            MC_CLOCK_DAY_COUNT.year_fraction(period_start, pay_date, DayCountContext::default())?
        } else {
            0.0
        };
        let rho = self.asset_correlation;
        for slot in 0..self.alive.len() {
            let i = self.alive[slot];
            let schedule = &self.prepared.schedules[i];
            let Some(terms) = schedule
                .revolver
                .as_ref()
                .and_then(|r| r.utilization.as_ref())
            else {
                continue;
            };
            if period_start >= schedule.maturity {
                continue;
            }
            // Two draws per live revolver per period, always consumed so the
            // stream stays order-stable whatever the states do.
            let eta_spread = self.draw();
            let eta_util = self.draw();
            let spread_shock = -(rho.sqrt() * systematic_z + (1.0 - rho).sqrt() * eta_spread);
            let c = terms.util_credit_corr;
            let util_shock = c * spread_shock + (1.0 - c * c).max(0.0).sqrt() * eta_util;

            let name = &mut self.names[i];
            let spread_bop = name.spread;
            if let SpreadTerms::Cir { process, .. } = &terms.spread {
                let mut x = [spread_bop.max(0.0)];
                let mut work = [0.0_f64; 0];
                self.cir
                    .step(process, 0.0, dt, &mut x, &[spread_shock], &mut work);
                name.spread = x[0].max(0.0);
            }
            if terms.sigma.abs() >= FROZEN_UTILIZATION_VOL {
                let theta = spread_linked_target(
                    terms.theta,
                    terms.spread_sensitivity,
                    spread_bop,
                    terms.spread.initial(),
                );
                name.utilization = ou_exact_step(
                    name.utilization,
                    theta,
                    terms.kappa,
                    terms.sigma,
                    dt,
                    util_shock,
                )
                .clamp(0.0, 1.0);
            }
            self.inputs[i].utilization = Some(if pay_date >= schedule.maturity {
                0.0
            } else {
                name.utilization
            });
        }
        Ok(())
    }
}

impl<S: PeriodShockSource> PoolFlowSource for InstrumentPathFlowSource<'_, S> {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let k = self.next_period;
        self.next_period += 1;
        let period = self.shocks.period_shock(&request)?;
        let shock = period.shock;
        request.state.floating_rate_shift = period.rate_shift;
        let months = request.months_per_period;
        let period_smm = 1.0 - (1.0 - shock.smm.clamp(0.0, 1.0)).powf(months);
        // Deal-model names: the copula plan carries the unconditional period
        // marginal (the copula conditions it on Z); without a copula the
        // period's realized pool-wide rate already embeds the scenario.
        let pool_pd = match shock.per_name {
            Some(plan) => plan.marginal_pd,
            None => 1.0 - (1.0 - shock.mdr.clamp(0.0, 1.0)).powf(months),
        };
        let prev_date = request.prev_date;
        let pay_date = request.pay_date;

        // Unconditional per-name marginals and recoveries, in pool order.
        self.inputs.clear();
        self.alive.clear();
        self.marginal_scratch.clear();
        for (i, schedule) in self.prepared.schedules.iter().enumerate() {
            let alive = !self.names[i].retired && !request.state.pool_state.is_defaulted[i];
            let (pd, recovery_rate) = if !alive {
                (0.0, 0.0)
            } else {
                match &schedule.default_source {
                    PeriodDefaultSource::HazardCurve(id) => hazard_period_default(
                        request.context,
                        id,
                        prev_date,
                        pay_date,
                        schedule.recovery_rate,
                    )?,
                    PeriodDefaultSource::DealModel => (
                        pool_pd,
                        schedule.recovery_rate.unwrap_or(shock.recovery_rate),
                    ),
                }
            };
            if alive {
                self.alive.push(i);
                self.marginal_scratch.push(pd);
            }
            self.inputs.push(NamePeriod {
                pd,
                recovery_rate,
                utilization: None,
                interest_addon: 0.0,
            });
        }

        // Copula resolution of the marginals against the systematic factor.
        if let (Some(engine), Some(plan)) = (self.per_name.as_mut(), shock.per_name) {
            match engine.resolve(
                plan.systematic_z,
                &self.marginal_scratch,
                &mut self.default_scratch,
                &mut self.conditional_scratch,
            ) {
                PerNameResolution::Realized => {
                    for (slot, &i) in self.alive.iter().enumerate() {
                        self.inputs[i].pd = if self.default_scratch[slot] { 1.0 } else { 0.0 };
                    }
                }
                PerNameResolution::Conditional => {
                    for (slot, &i) in self.alive.iter().enumerate() {
                        self.inputs[i].pd = self.conditional_scratch[slot];
                    }
                }
            }
        }

        let period_start = prev_date.max(self.prepared.as_of);
        self.step_revolvers(shock.systematic_z, period_start, pay_date)?;
        self.apply_counterfactual_addons(k, period_start, pay_date)?;

        let discount_curve = self
            .prepared
            .discount_curve(request.instrument, request.context)?;
        let curve = discount_curve.as_deref().map(|c| c as &dyn Discounting);
        let rate_view = RateView {
            curve,
            shift: period.rate_shift,
        };
        let flows = run_period(
            self.prepared,
            &mut self.names,
            request.state,
            PeriodModel {
                k,
                prev_date,
                pay_date,
                period_smm,
                rate_view,
                names: &self.inputs,
            },
        )?;
        self.record_draws(k, pay_date, curve)?;
        Ok(flows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule, CashflowSpec};
    use crate::instruments::fixed_income::structured_credit::types::{
        AssetPool, CallExercisePolicy, DealType, DefaultModelSpec, InstrumentCollateral,
        PrepaymentModelSpec, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use rust_decimal::Decimal;
    use time::macros::date;

    const AS_OF: Date = date!(2024 - 01 - 15);

    /// Flat 3% OIS discounting, a flat 4% term index and its past fixings.
    fn market() -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(AS_OF)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (10.0, (-0.30_f64).exp()),
            ])
            .build()
            .expect("discount curve");
        let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(AS_OF)
            .day_count(DayCount::Act360)
            .reset_lag(2)
            .knots([(0.0, 0.04), (10.0, 0.04)])
            .build()
            .expect("forward curve");
        let fixings: Vec<(Date, f64)> = (0..25)
            .map(|days| (AS_OF - time::Duration::days(days), 0.04))
            .collect();
        MarketContext::new()
            .insert(discount)
            .insert(forward)
            .insert_series(
                ScalarTimeSeries::new("FIXING:USD-SOFR-3M", fixings, None).expect("fixings"),
            )
    }

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("money")
    }

    /// Pass-through deal over `bonds` with a thin equity tranche and no
    /// behavioural prepayment or default.
    fn pool_deal(
        bonds: Vec<Bond>,
        call_exercise: CallExercisePolicy,
        maturity: Date,
    ) -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.instruments = Some(InstrumentCollateral {
            bonds,
            call_exercise,
            ..Default::default()
        });
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "EQ",
                0.0,
                10.0,
                TrancheSeniority::Equity,
                usd(100_000.0),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
            Tranche::new(
                "A",
                10.0,
                100.0,
                TrancheSeniority::Senior,
                usd(900_000.0),
                TrancheCoupon::Fixed { rate: 0.03 },
                maturity,
            )
            .expect("senior"),
        ])
        .expect("tranches");
        let mut deal =
            StructuredCredit::new_clo("POOL-PATH", pool, tranches, AS_OF, maturity, "USD-OIS")
                .with_payment_calendar("nyse");
        deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        deal
    }

    /// A rate path that sits a constant `shift` above the forward curve, with
    /// no prepayment or default.
    struct ConstantShift(f64);

    impl PeriodShockSource for ConstantShift {
        fn period_shock(
            &mut self,
            _request: &PoolFlowRequest<'_, '_>,
        ) -> Result<super::super::pool_flow_source::PeriodShock> {
            Ok(super::super::pool_flow_source::PeriodShock {
                shock: PeriodPoolShock::pool_wide(0.0, 0.0, 0.4),
                rate_shift: self.0,
            })
        }
    }

    fn run_with_shift(
        deal: &StructuredCredit,
        market: &MarketContext,
        shift: f64,
    ) -> SimulationRun {
        let resolved = deal.resolved_for_pricing().expect("resolved deal");
        let prepared = prepare_deal_simulation(&resolved, AS_OF)
            .expect("prepared")
            .expect("live pool");
        let schedules = PreparedInstrumentSchedules::prepare(&resolved, market, &prepared)
            .expect("instrument schedules");
        let mut source = InstrumentPathFlowSource::new(
            &schedules,
            ConstantShift(shift),
            None,
            0.0,
            PhiloxRng::new(7),
            false,
        );
        simulate_prepared(&resolved, market, &prepared, &mut source).expect("path run")
    }

    /// Cash paid to the notes above the collateral par.
    fn carry(run: &SimulationRun) -> f64 {
        run.tranches
            .values()
            .map(|t| t.total_interest.amount() + t.total_principal.amount())
            .sum::<f64>()
            - 1_000_000.0
    }

    fn first_principal_date(run: &SimulationRun, id: &str) -> Date {
        run.tranches[id]
            .principal_flows
            .iter()
            .find(|(_, amount)| amount.amount() > 0.0)
            .map(|(date, _)| *date)
            .expect("principal paid")
    }

    /// A floating bond with a 3% all-in floor re-projected on a path 300bp
    /// below the forward curve earns the floor on every unfixed coupon.
    #[test]
    fn falling_rate_path_earns_the_floor_on_floating_collateral() {
        let mut bond = Bond::example_floating().expect("floating bond");
        if let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec {
            spec.rate_spec.all_in_floor_bp = Some(Decimal::from(300));
        }
        let deal = pool_deal(
            vec![bond],
            CallExercisePolicy::Contractual,
            date!(2029 - 01 - 15),
        );
        let market = market();

        let flat = carry(&run_with_shift(&deal, &market, 0.0));
        let low = carry(&run_with_shift(&deal, &market, -0.03));
        let high = carry(&run_with_shift(&deal, &market, 0.01));

        // Twenty quarterly coupons: the first is already fixed at 5.5% on
        // every path; the other nineteen pay 5.5% on the flat path and the
        // 3.0% floor on the falling path (index 1% + 150bp = 2.5% < floor).
        let expected_ratio = (5.5 + 19.0 * 3.0) / (20.0 * 5.5);
        assert!(
            (low / flat - expected_ratio).abs() < 0.02,
            "floored carry ratio {} vs expected {expected_ratio}",
            low / flat
        );
        assert!(high > flat, "a rising path must raise the floating carry");
    }

    /// Under the refinancing-incentive rule an 8% callable bond is called at
    /// its first window only when the path's refinancing rate is far enough
    /// below the coupon: the flat curve leaves it outstanding to maturity, a
    /// 300bp-lower path calls it.
    #[test]
    fn refinancing_incentive_calls_only_on_the_low_rate_path() {
        let mut bond = Bond::example().expect("fixed bond");
        bond.cashflow_spec =
            CashflowSpec::fixed(0.08, Tenor::semi_annual(), DayCount::Thirty360).expect("coupon");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2026 - 01 - 15),
                end_date: date!(2034 - 01 - 15),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let deal = pool_deal(
            vec![bond],
            CallExercisePolicy::RefinancingIncentive {
                threshold_bp: 600.0,
            },
            date!(2034 - 01 - 15),
        );
        let market = market();

        // Flat curve: par refinancing ≈ 3.4%, 460bp below the coupon.
        let flat = run_with_shift(&deal, &market, 0.0);
        assert!(
            first_principal_date(&flat, "A") >= date!(2033 - 12 - 01),
            "the bond must stay outstanding on the flat path, principal on {}",
            first_principal_date(&flat, "A")
        );
        // 300bp lower: ≈ 0.4%, 760bp below the coupon, called at first call.
        let low = run_with_shift(&deal, &market, -0.03);
        let called = first_principal_date(&low, "A");
        assert!(
            (date!(2026 - 01 - 15)..=date!(2026 - 05 - 01)).contains(&called),
            "the bond must be called at its first window on the low path, principal on {called}"
        );
    }
}
