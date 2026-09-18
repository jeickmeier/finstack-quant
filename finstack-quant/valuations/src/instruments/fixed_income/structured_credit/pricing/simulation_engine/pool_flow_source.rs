use super::*;

/// Source of pool-level prepayment/default/recovery assumptions for each period.
pub(crate) trait PoolFlowSource {
    /// Calculate pool cashflows for the next legal payment period.
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows>;
}

/// Inputs required to calculate pool flows for one legal payment period.
pub(crate) struct PoolFlowRequest<'a, 's> {
    pub(super) state: &'a mut SimulationState<'s>,
    pub(super) instrument: &'a StructuredCredit,
    pub(super) pay_date: Date,
    pub(super) prev_date: Date,
    pub(super) seasoning_months: u32,
    pub(super) months_per_period: f64,
    pub(super) context: &'a MarketContext,
}

/// Monthly-equivalent rate averaged over the months of one payment period.
///
/// Seasoning-ramped curves (PSA/SDA) change month to month, so for
/// non-monthly payment frequencies sampling only the END-of-period month and
/// compounding it across the whole period overstates ramp-phase speeds (the
/// end-of-period rate is the highest within the period on an upward ramp).
/// Instead, compound the per-month survival across every month the period
/// covers and convert back to a monthly-equivalent rate; the downstream
/// `powf(months_per_period)` in `calculate_pool_flows_with_rates` then
/// recovers the exact multi-month period rate.
///
/// Falls back to the end-of-period sample for monthly periods (where it is
/// exact) and for non-integer month counts (irregular stubs).
///
/// # Arguments
///
/// * `pay_date` - Payment date of the period, forwarded to `rate_at` for
///   date-based rate overrides.
/// * `seasoning_end` - Pool seasoning in months at the END of the period.
/// * `months_per_period` - Number of months the payment period spans.
/// * `rate_at` - Monthly rate (SMM or MDR, decimal) at a given payment date
///   and seasoning month.
pub(super) fn period_averaged_monthly_rate(
    pay_date: Date,
    seasoning_end: u32,
    months_per_period: f64,
    rate_at: impl Fn(Date, u32) -> Result<f64>,
) -> Result<f64> {
    let k = months_per_period.round();
    if k <= 1.0 || (months_per_period - k).abs() > 1e-9 {
        return rate_at(pay_date, seasoning_end);
    }
    let k = k as u32;
    let mut survival = 1.0_f64;
    for m in 0..k {
        let seasoning = seasoning_end.saturating_sub(k - 1 - m);
        survival *= 1.0 - rate_at(pay_date, seasoning)?.clamp(0.0, 1.0);
    }
    Ok(1.0 - survival.max(0.0).powf(1.0 / f64::from(k)))
}

/// Performing pool balance as a fraction of the ORIGINAL (cut-off) pool
/// balance, the base against which cumulative-loss and timing default curves
/// are stated. Below 1.0 for a seasoned pool, above 1.0 after par build; 1.0
/// for an empty starting pool.
fn surviving_balance_fraction(state: &SimulationState) -> f64 {
    let start = state.original_pool_balance.amount();
    if start > 0.0 {
        (state.pool_outstanding.amount() / start).max(0.0)
    } else {
        1.0
    }
}

/// Prepayment and default rates from each DATED asset's own seasoning.
///
/// A seasoning-ramped or vector curve (PSA, ABS, vector prepayment; SDA or
/// vector default) is convex in loan age, so applying it at the pool's
/// balance-weighted age misstates the speed of every row that is younger or
/// older than the average. Rows with an origination (or acquisition) date
/// therefore read the curve at their own age at the period's end; undated
/// rows and constant curves keep the pool-level rate. Cumulative-loss and
/// timing default curves are stated on the pool's life and stay pool-level.
fn asset_seasoned_rates(request: &PoolFlowRequest<'_, '_>) -> Result<AssetSeasonedRates> {
    use crate::cashflow::builder::{DefaultCurve, PrepaymentCurve};
    let model = &request.instrument.credit_model;
    let prepay_seasoned = !matches!(
        model.prepayment_spec.curve,
        None | Some(PrepaymentCurve::Constant)
    );
    let default_seasoned = matches!(
        model.default_spec.curve,
        Some(DefaultCurve::Sda { .. }) | Some(DefaultCurve::Vector { .. })
    );
    if !(prepay_seasoned || default_seasoned) {
        return Ok(AssetSeasonedRates::default());
    }
    let dates = &request.state.pool_state.origination_dates;
    let mut rates = AssetSeasonedRates {
        smm: vec![None; dates.len()],
        mdr: vec![None; dates.len()],
    };
    for (i, anchor) in dates.iter().enumerate() {
        let Some(anchor) = anchor else { continue };
        let seasoning = if *anchor < request.pay_date {
            anchor.months_until(request.pay_date)
        } else {
            0
        };
        if prepay_seasoned {
            rates.smm[i] = Some(period_averaged_monthly_rate(
                request.pay_date,
                seasoning,
                request.months_per_period,
                |_, seasoning| model.prepayment_spec.smm(seasoning),
            )?);
        }
        if default_seasoned {
            rates.mdr[i] = Some(period_averaged_monthly_rate(
                request.pay_date,
                seasoning,
                request.months_per_period,
                |_, seasoning| model.default_spec.mdr(seasoning),
            )?);
        }
    }
    Ok(rates)
}

/// Deterministic pool-flow source using the instrument's base credit model.
pub(crate) struct DeterministicPoolFlowSource;

impl PoolFlowSource for DeterministicPoolFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let smm = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| {
                request
                    .instrument
                    .credit_model
                    .prepayment_spec
                    .smm(seasoning)
            },
        )?;
        let survival = surviving_balance_fraction(request.state);
        // A cumulative-loss or timing curve states the CHARGE-OFFS by month
        // of the pool's life. With a delinquency model the default rate is
        // the entry into the first bucket, which charges off only after
        // rolling through every bucket, so the entry is sized from the
        // curve `k` months ahead (`k` = buckets) grossed up by the roll
        // probabilities, on the performing balance the entry applies to.
        // Exact when each bucket fully rolls or cures every month; a `stay`
        // share delays part of the charge-off past the curve's month.
        let curve_lookahead = request
            .instrument
            .credit_model
            .delinquency
            .as_ref()
            .filter(|_| {
                matches!(
                    request.instrument.credit_model.default_spec.curve,
                    Some(crate::cashflow::builder::DefaultCurve::CumulativeLoss { .. })
                        | Some(crate::cashflow::builder::DefaultCurve::Timing { .. })
                )
            })
            .map(|model| {
                let delinquent: f64 = request
                    .state
                    .pool_state
                    .delinquent
                    .iter()
                    .map(|buckets| super::delinquency::delinquent_balance(buckets))
                    .sum();
                let outstanding = request.state.pool_outstanding.amount();
                let performing_gross_up = if outstanding - delinquent > 0.0 {
                    outstanding / (outstanding - delinquent)
                } else {
                    1.0
                };
                let roll_through: f64 = model.roll_rates.iter().product();
                let months_to_charge_off = u32::try_from(model.buckets()).unwrap_or(u32::MAX);
                (
                    months_to_charge_off,
                    performing_gross_up / roll_through.max(f64::MIN_POSITIVE),
                )
            });
        // The curve's charge-offs in the first `k` months of the projection
        // can only come from loans already in the pipeline at the start; a
        // pool without seeded buckets has none, so the first entry also
        // carries those months and the lifetime loss is preserved (the
        // charge-offs land `k` months later than the curve states them).
        let first_month_seasoning = request
            .seasoning_months
            .saturating_sub(request.months_per_period.round().max(1.0) as u32 - 1);
        let catch_up = request.state.period_diagnostics.is_empty();
        let mdr = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| {
                let spec = &request.instrument.credit_model.default_spec;
                match curve_lookahead {
                    Some((ahead, gross_up)) => {
                        let target = seasoning.saturating_add(ahead);
                        let from = if catch_up && seasoning == first_month_seasoning {
                            seasoning
                        } else {
                            target
                        };
                        let mut rate = 0.0;
                        for month in from..=target {
                            rate += spec.mdr_with_survival(month, survival)?;
                        }
                        Ok((rate * gross_up).min(1.0))
                    }
                    None => spec.mdr_with_survival(seasoning, survival),
                }
            },
        )?;
        let asset_rates = asset_seasoned_rates(&request)?;
        calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: request.state,
            pay_date: request.pay_date,
            prev_date: request.prev_date,
            months_per_period: request.months_per_period,
            context: request.context,
            rates: PoolFlowRates {
                smm,
                mdr,
                recovery_rate: request
                    .instrument
                    .credit_model
                    .recovery_spec
                    .recovery_rate(request.seasoning_months),
            },
            asset_rates,
            copula_outcome: None,
            delinquency: request.instrument.credit_model.delinquency.as_ref(),
            special_servicing: SpecialServicingFees::from_deal(request.instrument.fees.as_ref()),
            card: request.instrument.credit_model.card.as_ref(),
        })
    }
}

/// Pool-flow source for option-adjusted-spread (OAS) scenario pricing.
///
/// For one Monte-Carlo scenario this modulates the deal's base prepayment and
/// default rates by an optional Hull-White short-rate path (rate-dependent
/// prepayment) and/or an optional systematic credit factor `z` (correlated
/// stress on default and prepayment), then defers to the deterministic
/// pool-flow engine. Discounting — including the trial OAS spread — is applied
/// by the caller to the resulting cashflows, not here.
///
/// Computing the shock per period (from the request's `pay_date`/seasoning)
/// rather than from a pre-built vector keeps it automatically aligned to the
/// engine's payment schedule.
pub(crate) struct OasPathFlowSource {
    as_of: Date,
    /// Monthly short-rate path from `as_of` (`None` ⇒ rates not stochastic).
    rate_path: Option<Vec<f64>>,
    /// SC-M13: per-month departure of the simulated short rate from the
    /// deterministic forward curve, `rate_path[m] − forwards[m]`. Applied to
    /// FLOATING coupon projection so a floater's coupons follow the same path
    /// its discount factors do.
    rate_shift_path: Option<Vec<f64>>,
    /// Systematic credit factor for the scenario (`None` ⇒ credit not stochastic).
    credit_z: Option<f64>,
    /// Rate-dependent prepayment sensitivity (β in `exp(-β·(r-r₀))`).
    prepay_beta: f64,
    /// Base (initial) short rate `r₀`.
    base_rate: f64,
    /// Credit factor loading for the lognormal default/prepayment shocks.
    credit_loading: f64,
}

impl OasPathFlowSource {
    pub(crate) fn new(
        as_of: Date,
        rate_path: Option<Vec<f64>>,
        rate_shift_path: Option<Vec<f64>>,
        credit_z: Option<f64>,
        prepay_beta: f64,
        base_rate: f64,
        credit_loading: f64,
    ) -> Self {
        Self {
            as_of,
            rate_path,
            rate_shift_path,
            credit_z,
            prepay_beta,
            base_rate,
            credit_loading,
        }
    }
}

impl PeriodShockSource for OasPathFlowSource {
    fn period_shock(&mut self, request: &PoolFlowRequest<'_, '_>) -> Result<PeriodShock> {
        const RATE_CLAMP: f64 = 0.9999;

        // SC-M13: this period's rate shift, so FLOATING coupons — both pool
        // assets and tranches — follow the simulated path.
        //
        // Without it the OAS applied a stochastic discount factor to
        // DETERMINISTIC coupons. For a floater that is the wrong instrument
        // entirely: coupon/discount correlation is exactly what makes a floater
        // rate-insensitive, and dropping it leaves only the discounting leg. The
        // martingale correction keeps the mean PV unbiased so the OAS point
        // estimate survived, but the per-path dispersion — and therefore
        // `price_std_error` — measured a risk a CLO does not have.
        let month = self.as_of.months_until(request.pay_date) as usize;
        let rate_shift = self
            .rate_shift_path
            .as_ref()
            .and_then(|p| p.get(month).copied())
            .unwrap_or(0.0);
        let base_smm = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| {
                request
                    .instrument
                    .credit_model
                    .prepayment_spec
                    .smm(seasoning)
            },
        )?;
        let survival = surviving_balance_fraction(request.state);
        let base_mdr = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| {
                request
                    .instrument
                    .credit_model
                    .default_spec
                    .mdr_with_survival(seasoning, survival)
            },
        )?;

        let mut smm = base_smm;
        let mut mdr = base_mdr;

        // Rate-dependent prepayment: higher rates slow prepayment.
        if let Some(rate_path) = &self.rate_path {
            let month = self.as_of.months_until(request.pay_date) as usize;
            let r = rate_path.get(month).copied().unwrap_or(self.base_rate);
            let mult = (-self.prepay_beta * (r - self.base_rate)).exp();
            smm = (smm * mult).clamp(0.0, RATE_CLAMP);
        }

        // Systematic credit stress (canonical convention: low `z` is the stress
        // state). Mean-corrected lognormal multipliers keep `E[shock] ≈ 1`, so
        // the stochastic credit dimension adds dispersion without biasing the
        // mean cashflows (and hence the OAS).
        if let Some(z) = self.credit_z {
            let l = self.credit_loading;
            let mdr_mult = (-l * z - 0.5 * l * l).exp();
            let smm_mult = (l * z - 0.5 * l * l).exp();
            mdr = (mdr * mdr_mult).clamp(0.0, RATE_CLAMP);
            smm = (smm * smm_mult).clamp(0.0, RATE_CLAMP);
        }

        let mut shock = PeriodPoolShock::pool_wide(
            smm,
            mdr,
            request
                .instrument
                .credit_model
                .recovery_spec
                .recovery_rate(request.seasoning_months),
        );
        shock.systematic_z = self.credit_z.unwrap_or(0.0);
        Ok(PeriodShock { shock, rate_shift })
    }
}

impl PoolFlowSource for OasPathFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let period = self.period_shock(&request)?;
        request.state.floating_rate_shift = period.rate_shift;
        calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: request.state,
            pay_date: request.pay_date,
            prev_date: request.prev_date,
            months_per_period: request.months_per_period,
            context: request.context,
            rates: PoolFlowRates {
                smm: period.shock.smm,
                mdr: period.shock.mdr,
                recovery_rate: period.shock.recovery_rate,
            },
            asset_rates: AssetSeasonedRates::default(),
            copula_outcome: None,
            delinquency: request.instrument.credit_model.delinquency.as_ref(),
            special_servicing: SpecialServicingFees::from_deal(request.instrument.fees.as_ref()),
            card: request.instrument.credit_model.card.as_ref(),
        })
    }
}

/// Per-period systematic inputs for finite-pool per-name copula default
/// simulation.
///
/// When present on a [`PeriodPoolShock`], the engine realizes each pool
/// asset's default individually (latent variable `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ`)
/// instead of applying the pool-wide MDR uniformly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PerNamePeriodInput {
    /// Systematic factor `Z` for the payment period, shared by every name.
    pub(crate) systematic_z: f64,
    /// Per-name *unconditional* marginal default probability for the period.
    /// Homogeneous pools share one value; the threshold `Φ⁻¹(PDₜ)` is
    /// recomputed per name to support heterogeneous pools.
    pub(crate) marginal_pd: f64,
}

/// Aggregated scenario assumptions for a legal payment period.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PeriodPoolShock {
    /// Equivalent monthly SMM for the payment period.
    pub(crate) smm: f64,
    /// Equivalent monthly MDR for the payment period.
    ///
    /// Used as the pool-wide default rate when `per_name` is `None` (the LHP
    /// fast-path), and ignored for assets when per-name simulation is active.
    pub(crate) mdr: f64,
    /// Recovery rate applied to defaults in the payment period.
    pub(crate) recovery_rate: f64,
    /// Per-name copula inputs. `Some` ⇒ realize defaults name-by-name;
    /// `None` ⇒ apply the pool-wide LHP MDR.
    pub(crate) per_name: Option<PerNamePeriodInput>,
    /// Period systematic credit factor `Z` (standard normal, low is stress),
    /// available to instrument collateral whether or not the default model
    /// is a copula; `0.0` when the scenario has no stochastic credit.
    pub(crate) systematic_z: f64,
}

impl PeriodPoolShock {
    /// Construct a pool-wide (LHP / non-copula) shock with no per-name plan.
    pub(crate) fn pool_wide(smm: f64, mdr: f64, recovery_rate: f64) -> Self {
        Self {
            smm,
            mdr,
            recovery_rate,
            per_name: None,
            systematic_z: 0.0,
        }
    }
}

/// One period's scenario inputs for instrument collateral.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PeriodShock {
    /// Pool-level prepayment, default and recovery assumptions.
    pub(super) shock: PeriodPoolShock,
    /// Additive shift of the simulated rate path over the forward curve,
    /// applied to floating coupons; `0.0` without a rate path.
    pub(super) rate_shift: f64,
}

/// Per-period scenario inputs consumed by the instrument path source.
///
/// Implemented by the pre-generated shock vector of the Monte Carlo engine
/// and by the OAS scenario modulation, so both engines drive the same
/// instrument path source.
pub(crate) trait PeriodShockSource {
    /// Scenario inputs for the period described by `request`.
    fn period_shock(&mut self, request: &PoolFlowRequest<'_, '_>) -> Result<PeriodShock>;
}

/// Pre-generated period shocks of one Monte Carlo path.
pub(crate) struct PathShocks {
    shocks: Vec<PeriodPoolShock>,
    next_period: usize,
}

impl PathShocks {
    /// Wrap one path's period shocks, consumed in order.
    pub(crate) fn new(shocks: Vec<PeriodPoolShock>) -> Self {
        Self {
            shocks,
            next_period: 0,
        }
    }
}

impl PeriodShockSource for PathShocks {
    fn period_shock(&mut self, _request: &PoolFlowRequest<'_, '_>) -> Result<PeriodShock> {
        let shock = self.shocks.get(self.next_period).copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "stochastic path has no pool shock for payment period {}",
                self.next_period + 1
            ))
        })?;
        self.next_period += 1;
        Ok(PeriodShock {
            shock,
            rate_shift: 0.0,
        })
    }
}

/// Per-path per-name copula default engine carried by a scenario flow source.
///
/// Owns the path's idiosyncratic-draw RNG substream so that per-name `εᵢ`
/// draws are deterministic and order-stable (period → name index). The
/// `simulator` is shared (cheap `Arc` clone of the copula kernel) across
/// paths; only the RNG is per-path.
///
/// # Antithetic pairing
///
/// When `antithetic` is `true` this engine is the *second member* of an
/// antithetic pair: it shares its RNG substream with the first member and
/// **negates** every idiosyncratic `εᵢ` draw. Combined with the systematic
/// factor `Z` being negated by `monte_carlo_factor_sets`, the copula latent
/// variable `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` becomes `−Aᵢ` for the paired path — the
/// genuine antithetic variate. Without this the per-name idiosyncratic
/// channel of paired paths would be independent, defeating the variance
/// reduction and making the reported confidence interval too narrow.
///
/// The Student-t mixing variable `W` is drawn from the same shared substream
/// and is *not* negated: the χ²-based mixing is asymmetric, and standard
/// antithetic treatment for the Student-t copula negates only the Gaussian
/// components while keeping the mixing common to the pair.
///
/// Cloning yields an engine at the same stream position, which the
/// instrument-collateral counterfactual run uses to replay a path.
#[derive(Clone)]
pub(crate) struct PerNameDefaultEngine {
    simulator: Arc<PerNameCopulaDefault>,
    granularity: PoolGranularity,
    rng: PhiloxRng,
    /// `true` ⇒ second member of an antithetic pair; negate idiosyncratic draws.
    antithetic: bool,
    /// Idiosyncratic (name-specific) recovery volatility. When `> 0`, each
    /// defaulted name recovers at its own rate scattered around the period
    /// systematic recovery; `0` ⇒ every default recovers at the period rate
    /// (no per-name dispersion, e.g. constant recovery).
    idiosyncratic_recovery_vol: f64,
}

impl PerNameDefaultEngine {
    /// Create a per-name engine for one scenario path (independent draws).
    pub(crate) fn new(
        simulator: Arc<PerNameCopulaDefault>,
        granularity: PoolGranularity,
        rng: PhiloxRng,
        idiosyncratic_recovery_vol: f64,
    ) -> Self {
        Self {
            simulator,
            granularity,
            rng,
            antithetic: false,
            idiosyncratic_recovery_vol,
        }
    }

    /// Create the *antithetic partner* per-name engine for a scenario path.
    ///
    /// `rng` must be the SAME substream the paired path uses; this engine
    /// negates every idiosyncratic `εᵢ` draw so the copula latent variable is
    /// the antithetic variate of its partner.
    pub(crate) fn new_antithetic(
        simulator: Arc<PerNameCopulaDefault>,
        granularity: PoolGranularity,
        rng: PhiloxRng,
        idiosyncratic_recovery_vol: f64,
    ) -> Self {
        Self {
            simulator,
            granularity,
            rng,
            antithetic: true,
            idiosyncratic_recovery_vol,
        }
    }
}

/// Which buffer [`PerNameDefaultEngine::resolve`] filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PerNameResolution {
    /// `defaults` holds one realized indicator per name.
    Realized,
    /// `conditional` holds one LHP conditional probability per name.
    Conditional,
}

impl PerNameDefaultEngine {
    /// Resolve one period's defaults for names with their own marginals.
    ///
    /// `PerName` granularity realizes each name (antithetic partners negate
    /// their idiosyncratic draws); `LargeHomogeneous` evaluates each name's
    /// conditional default probability under one shared mixing draw.
    ///
    /// # Arguments
    ///
    /// * `systematic_z` - Period systematic factor shared by every name.
    /// * `marginal_pd` - Unconditional period default probability per live name.
    /// * `defaults` - Filled with realized indicators under `PerName`.
    /// * `conditional` - Filled with conditional probabilities under
    ///   `LargeHomogeneous`.
    pub(super) fn resolve(
        &mut self,
        systematic_z: f64,
        marginal_pd: &[f64],
        defaults: &mut Vec<bool>,
        conditional: &mut Vec<f64>,
    ) -> PerNameResolution {
        match self.granularity {
            PoolGranularity::PerName => {
                if self.antithetic {
                    self.simulator.simulate_period_antithetic(
                        systematic_z,
                        marginal_pd,
                        &mut self.rng,
                        defaults,
                    );
                } else {
                    self.simulator.simulate_period(
                        systematic_z,
                        marginal_pd,
                        &mut self.rng,
                        defaults,
                    );
                }
                PerNameResolution::Realized
            }
            PoolGranularity::LargeHomogeneous => {
                self.simulator.conditional_default_probs(
                    systematic_z,
                    marginal_pd,
                    &mut self.rng,
                    conditional,
                );
                PerNameResolution::Conditional
            }
        }
    }
}

/// Stochastic path pool-flow source using pre-generated period shocks.
pub(crate) struct StochasticPathFlowSource {
    shocks: Vec<PeriodPoolShock>,
    next_period: usize,
    /// Per-name copula engine. `Some` when the scenario uses finite-pool
    /// per-name default simulation.
    per_name: Option<PerNameDefaultEngine>,
    /// Scratch buffer for per-name default indicators, reused each period to
    /// avoid per-period allocation.
    default_scratch: Vec<bool>,
    /// Scratch buffer for per-name recovery rates, index-aligned with
    /// `default_scratch`. Entry `k` is the recovery the `k`-th performing
    /// asset realizes if it defaults this period.
    recovery_scratch: Vec<f64>,
    /// Scratch buffer holding one marginal-PD entry per still-performing asset,
    /// reused each period to avoid allocating a fresh vector for the per-name
    /// copula simulation.
    marginal_scratch: Vec<f64>,
}

impl StochasticPathFlowSource {
    /// Create a flow source for one scenario path (pool-wide / LHP shocks).
    pub(crate) fn new(shocks: Vec<PeriodPoolShock>) -> Self {
        Self {
            shocks,
            next_period: 0,
            per_name: None,
            default_scratch: Vec::new(),
            recovery_scratch: Vec::new(),
            marginal_scratch: Vec::new(),
        }
    }

    /// Create a flow source that realizes per-name copula defaults.
    pub(crate) fn with_per_name(
        shocks: Vec<PeriodPoolShock>,
        per_name: PerNameDefaultEngine,
    ) -> Self {
        Self {
            shocks,
            next_period: 0,
            per_name: Some(per_name),
            default_scratch: Vec::new(),
            recovery_scratch: Vec::new(),
            marginal_scratch: Vec::new(),
        }
    }
}

impl PoolFlowSource for StochasticPathFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let shock = self.shocks.get(self.next_period).copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "stochastic path has no pool shock for payment period {}",
                self.next_period + 1
            ))
        })?;
        self.next_period += 1;

        // Copula default resolution: when the per-name engine and the
        // period's per-name plan are both present, the copula owns the
        // period's default rate. `PerName` granularity realizes each asset
        // individually (latent variable `Aᵢ`); `LargeHomogeneous` applies the
        // closed-form LHP conditional default probability uniformly — the
        // `N → ∞` limit of the per-name model.
        let copula_outcome = match (self.per_name.as_mut(), shock.per_name) {
            (Some(engine), Some(plan)) => match engine.granularity {
                PoolGranularity::PerName => {
                    // One marginal-PD entry per still-performing asset, in
                    // the pool's intrinsic asset order, so the per-name εᵢ
                    // draws are order-stable.
                    let alive = request
                        .state
                        .pool_state
                        .is_defaulted
                        .iter()
                        .zip(request.state.pool_state.balances.iter())
                        .filter(|(defaulted, balance)| !**defaulted && **balance > 0.0)
                        .count();
                    // Reuse a per-source scratch buffer for the marginal-PD
                    // vector instead of allocating one per period.
                    self.marginal_scratch.clear();
                    self.marginal_scratch.resize(alive, plan.marginal_pd);
                    // Antithetic partners negate their idiosyncratic εᵢ draws
                    // so the copula latent variable is the antithetic variate
                    // of the paired path (the systematic Z is already negated
                    // by `monte_carlo_factor_sets`).
                    if engine.antithetic {
                        engine.simulator.simulate_period_antithetic(
                            plan.systematic_z,
                            &self.marginal_scratch,
                            &mut engine.rng,
                            &mut self.default_scratch,
                        );
                    } else {
                        engine.simulator.simulate_period(
                            plan.systematic_z,
                            &self.marginal_scratch,
                            &mut engine.rng,
                            &mut self.default_scratch,
                        );
                    }

                    // Per-name idiosyncratic recovery dispersion: each name
                    // recovers at its own rate, scattered around the period
                    // systematic recovery `shock.recovery_rate`. A draw is
                    // taken for every name (not only defaulters) so the RNG
                    // stream stays order-stable; the antithetic partner negates
                    // the recovery shock, mirroring the default-shock negation.
                    // When the recovery model has no idiosyncratic volatility
                    // no draw is consumed, so a constant-recovery scenario is
                    // bit-identical to the pre-dispersion engine.
                    self.recovery_scratch.clear();
                    self.recovery_scratch.reserve(self.default_scratch.len());
                    let sigma = engine.idiosyncratic_recovery_vol;
                    for _ in 0..self.default_scratch.len() {
                        let recovery = if sigma > 0.0 {
                            let raw = engine.rng.next_std_normal();
                            let eps = if engine.antithetic { -raw } else { raw };
                            (shock.recovery_rate + sigma * eps).clamp(0.0, 1.0)
                        } else {
                            shock.recovery_rate
                        };
                        self.recovery_scratch.push(recovery);
                    }

                    Some(PeriodDefaultOutcome::PerName {
                        defaults: &self.default_scratch,
                        recoveries: &self.recovery_scratch,
                    })
                }
                PoolGranularity::LargeHomogeneous => {
                    // Closed-form LHP limit: apply E[1{Aᵢ ≤ c} | Z, W] to the
                    // whole pool as a period-level default rate. The simulator
                    // draws the same shared mixing `W` per period as the
                    // per-name path, so this is the genuine `N → ∞` limit.
                    let rate = engine.simulator.conditional_default_prob(
                        plan.systematic_z,
                        plan.marginal_pd,
                        &mut engine.rng,
                    );
                    Some(PeriodDefaultOutcome::PoolWidePeriodRate(rate))
                }
            },
            _ => None,
        };

        calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: request.state,
            pay_date: request.pay_date,
            prev_date: request.prev_date,
            months_per_period: request.months_per_period,
            context: request.context,
            rates: PoolFlowRates {
                smm: shock.smm,
                mdr: shock.mdr,
                recovery_rate: shock.recovery_rate,
            },
            asset_rates: AssetSeasonedRates::default(),
            copula_outcome,
            delinquency: request.instrument.credit_model.delinquency.as_ref(),
            special_servicing: SpecialServicingFees::from_deal(request.instrument.fees.as_ref()),
            card: request.instrument.credit_model.card.as_ref(),
        })
    }
}
