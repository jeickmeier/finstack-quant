//! Systematic-factor generation, AR(1) evolution, and period shock aggregation.

use super::{PreparedDefault, PreparedPrepayment, PreparedRun, StochasticPricer};
use crate::structured_credit::pricing::simulation_engine::{PerNamePeriodInput, PeriodPoolShock};
use crate::structured_credit::types::StructuredCredit;
use finstack_quant_core::Result;
use finstack_quant_models::correlation::LatentFactorSpec;
use finstack_quant_models::credit::pool::PrepaymentState;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::RandomStream;

/// Salt for the INDEPENDENT component of the prepayment factor.
///
/// XOR-salting keeps this stream disjoint from the credit-factor and
/// per-name streams, so adding it leaves every existing draw sequence
/// bit-identical — the credit factors, and therefore all existing default
/// results, are unchanged.
pub(super) const PREPAY_FACTOR_SEED_SALT: u64 = 0x5052_4550_4159_5A32; // "PREPAYZ2"

/// Independent stream for initial prepayment regimes and monthly transitions.
const PREPAY_REGIME_SEED_SALT: u64 = 0x5052_4550_4159_5247; // "PREPAYRG"

/// Mutable prepayment state isolated to one simulation path.
pub(super) struct PrepaymentPath {
    pub(super) burnout: f64,
    model_state: PrepaymentState,
    transition_rng: PhiloxRng,
    mirror_draws: bool,
}

impl PrepaymentPath {
    pub(super) fn next_uniform(&mut self) -> f64 {
        let uniform = self.transition_rng.next_u01();
        if self.mirror_draws {
            1.0 - uniform
        } else {
            uniform
        }
    }
}

impl StochasticPricer {
    /// Monthly systematic factors of Monte Carlo path `path_index`.
    ///
    /// One base Philox RNG seeded from `config.seed`; each path draws from a
    /// counter-based substream, so a path's factors do not depend on which
    /// other paths run or in what order. With `antithetic`, path `2k + 1` is
    /// the negation of path `2k`: both members share `substream(k)`, so the
    /// pair is perfectly correlated while pairs stay independent. Every
    /// non-antithetic path draws from `substream(path_index)`.
    pub(super) fn monte_carlo_path_factors(
        &self,
        instrument: &StructuredCredit,
        path_index: usize,
        antithetic: bool,
    ) -> Result<Vec<f64>> {
        let base_rng = PhiloxRng::new(self.config.tree_config.seed);
        if antithetic {
            let mut rng = base_rng.substream((path_index / 2) as u64);
            let mut factors = self.random_factors(instrument, &mut rng)?;
            if path_index % 2 == 1 {
                factors.iter_mut().for_each(|z| *z = -*z);
            }
            Ok(factors)
        } else {
            let mut rng = base_rng.substream(path_index as u64);
            self.random_factors(instrument, &mut rng)
        }
    }

    fn random_factors(
        &self,
        instrument: &StructuredCredit,
        rng: &mut PhiloxRng,
    ) -> Result<Vec<f64>> {
        let month_count = self.month_count(instrument);
        if !self.has_stochastic_rates() {
            return Ok(vec![0.0; month_count]);
        }
        (0..month_count).map(|_| rng.next_std_normal()).collect()
    }

    /// Configured systematic-factor mean-reversion speed κ (per year).
    ///
    /// For [`LatentFactorSpec::SingleFactor`], κ is taken from the factor spec
    /// (clamped to non-negative). Other factor specs default to `0.0`.
    ///
    /// Semantics match the OU / random-walk convention used elsewhere in this
    /// module and in `factor_model.rs`:
    ///
    /// * `κ = 0` → φ = 1 → one systematic draw held across the horizon
    ///   (canonical single-factor copula; Li 2000; Vasicek 2002)
    /// * `κ → ∞` → φ → 0 → independent monthly factors
    ///
    /// Pair with [`Self::period_factor_scale`]: a persistent factor aggregated
    /// without renormalization would leave the period factor with variance
    /// other than 1 and de-calibrate the copula barrier `Φ⁻¹(PD)`.
    pub(super) fn factor_mean_reversion(&self) -> f64 {
        match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { mean_reversion, .. } => mean_reversion.max(0.0),
            _ => 0.0,
        }
    }

    /// Configured prepay/default factor correlation.
    ///
    /// `Some(rho)` for a two-factor spec, **including `rho == 0`**: zero means
    /// the prepayment factor is independent of credit, which is a different
    /// model from sharing one factor. `None` is returned only for
    /// [`LatentFactorSpec::SingleFactor`], where prepayment and credit are
    /// driven by the same factor (implied correlation +1).
    ///
    /// # Errors
    ///
    /// [`LatentFactorSpec::MultiFactor`] is not supported by this lattice.
    pub(super) fn factor_correlation(&self) -> finstack_quant_core::Result<Option<f64>> {
        match &self.config.tree_config.factor_spec {
            LatentFactorSpec::SingleFactor { .. } => Ok(None),
            LatentFactorSpec::TwoFactor { correlation, .. } => {
                Ok(Some(correlation.clamp(-1.0, 1.0)))
            }
            other => Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit stochastic pricer supports SingleFactor and TwoFactor \
                 latent specs; got {other:?}"
            ))),
        }
    }

    /// Monthly factor autocorrelation `φ = e^{−κ/12}`.
    #[cfg(test)]
    pub(super) fn factor_phi(&self) -> f64 {
        (-self.factor_mean_reversion() / 12.0).exp()
    }

    /// Normalizing divisor that keeps the aggregated period factor `N(0,1)`.
    ///
    /// The period-representative systematic factor is `Z_period = (Σ Zₘ)/s`.
    /// The copula barrier `c = Φ⁻¹(PDₜ)` assumes a standard-normal factor, so
    /// `s` must equal `sqrt(Var(Σ Zₘ))`.
    ///
    /// For the stationary AR(1) path from [`Self::evolved_factors`], marginal
    /// variance is 1 and autocorrelation at lag `h` is `φ^h`:
    ///
    /// ```text
    /// Var(Σ_{m=1..M} Zₘ) = Σ_i Σ_j φ^{|i−j|} = M + 2·Σ_{k=1..M−1} (M−k)·φ^k
    /// ```
    ///
    /// Limits: `φ = 0` ⇒ `s = √M` (independent months); `φ = 1` ⇒ `s = M`
    /// (`Z_period = Z₁`, single-draw canonical copula).
    pub(super) fn period_factor_scale(months: usize, phi: f64) -> f64 {
        let m = months as f64;
        if months <= 1 {
            return 1.0;
        }
        let mut variance = m;
        let mut phi_k = 1.0;
        for k in 1..months {
            phi_k *= phi;
            variance += 2.0 * (m - k as f64) * phi_k;
        }
        variance.max(f64::MIN_POSITIVE).sqrt()
    }

    /// Evolve monthly factor innovations into a stationary AR(1)/OU path.
    ///
    /// The raw draws `ε_m ~ N(0,1)` are treated as innovations of the exact
    /// OU discretization with monthly step `Δt = 1/12`:
    ///
    /// ```text
    /// Z_1 = ε_1,    Z_m = φ·Z_{m−1} + √(1−φ²)·ε_m,    φ = e^{−κΔt}
    /// ```
    ///
    /// Each `Z_m` keeps the stationary `N(0,1)` marginal, so the conditional
    /// MDR/SMM models and the copula barriers `Φ⁻¹(PD)` stay correctly
    /// calibrated. The factor autocorrelation at lag `h` months is
    /// `φ^h = e^{−κh/12}`; the effective correlation half-life is
    /// `12·ln 2 / κ` months (e.g. κ = 0.5 → ≈ 16.6 months). κ → ∞ recovers
    /// the previous i.i.d.-per-month behavior.
    ///
    /// The transform is linear in the innovations, so antithetic negation of
    /// the raw draws negates the whole evolved path and the variance
    /// reduction is preserved.
    pub(super) fn evolved_factors(innovations: &[f64], kappa: f64) -> Vec<f64> {
        let phi = (-kappa / 12.0).exp();
        let innovation_scale = (1.0 - phi * phi).max(0.0).sqrt();
        let mut evolved = Vec::with_capacity(innovations.len());
        let mut state = 0.0;
        for (month, eps) in innovations.iter().enumerate() {
            state = if month == 0 {
                *eps
            } else {
                phi * state + innovation_scale * eps
            };
            evolved.push(state);
        }
        evolved
    }

    /// Evolved prepayment factor series of one path, correlated with the
    /// evolved `factor_path` at the configured level; `None` when the
    /// factor spec is single-factor (prepayment then shares the credit
    /// factor).
    ///
    /// A SECOND factor for prepayment. Handing ONE scalar to both
    /// `conditional_smm` and `conditional_mdr` would drive prepayment and
    /// default off the same realization, forcing their implied correlation to
    /// +1 (or -1 through a negative loading) regardless of the -0.30
    /// configured in the shipped RMBS/CLO calibrations. Construction is the
    /// standard two-factor decomposition
    ///     Z_prepay = rho * Z_credit + sqrt(1 - rho^2) * Z_indep
    /// which gives `Z_prepay` a unit-variance standard-normal marginal and
    /// exactly `rho` correlation with the credit factor.
    ///
    /// `Z_indep` comes from a SALTED stream so the credit draws are
    /// untouched. Antithetic pairs `(2k, 2k+1)` share `substream(k)` and the
    /// odd member negates its draws, mirroring the credit factors (negated by
    /// `monte_carlo_path_factors`) and the per-name streams, so the pair's
    /// prepayment factors are exact negatives; independent paths use
    /// `substream(path_index)`.
    ///
    /// # Arguments
    ///
    /// * `factor_path` - The path's evolved monthly credit factors.
    /// * `path_index` - Path number within the run.
    /// * `antithetic` - Whether paths are drawn as antithetic pairs.
    /// * `prepared` - Run state carrying the factor correlation and κ.
    pub(super) fn prepay_factors(
        &self,
        factor_path: &[f64],
        (path_index, antithetic): (usize, bool),
        prepared: &PreparedRun,
    ) -> Result<Option<Vec<f64>>> {
        let Some(rho) = prepared.factor_correlation else {
            return Ok(None);
        };
        let base = PhiloxRng::new(self.config.tree_config.seed ^ PREPAY_FACTOR_SEED_SALT);
        let (mut rng, negate) = if antithetic {
            (base.substream((path_index / 2) as u64), path_index % 2 == 1)
        } else {
            (base.substream(path_index as u64), false)
        };
        let mut independent = vec![0.0; factor_path.len()];
        rng.fill_std_normals(&mut independent)?;
        if negate {
            for draw in &mut independent {
                *draw = -*draw;
            }
        }
        let evolved_independent = Self::evolved_factors(&independent, prepared.factor_kappa);
        let scale = (1.0 - rho * rho).max(0.0).sqrt();
        Ok(Some(
            factor_path
                .iter()
                .zip(evolved_independent.iter())
                .map(|(zc, zi)| rho * zc + scale * zi)
                .collect(),
        ))
    }

    pub(super) fn path_shocks_from_factors(
        &self,
        instrument: &StructuredCredit,
        factors: &[f64],
        path: (usize, bool),
        prepared: &PreparedRun,
    ) -> Result<Vec<PeriodPoolShock>> {
        // AR(1)/OU persistence: monthly draws are innovations. Applied
        // unconditionally — persistence is a property of the factor, not of
        // which channels are simulated.
        let evolved_storage = Self::evolved_factors(factors, prepared.factor_kappa);
        let factor_path: &[f64] = &evolved_storage;
        let prepay_storage = self.prepay_factors(factor_path, path, prepared)?;
        // SingleFactor: prepayment shares the credit factor, i.e. implied
        // correlation +1.
        let prepay_factors: &[f64] = prepay_storage.as_deref().unwrap_or(factor_path);
        let factors: &[f64] = factor_path;
        let months_per_period = instrument.frequency.months().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Structured credit stochastic pricing requires month-based payment frequencies"
                    .to_string(),
            )
        })? as usize;
        let months_per_period = months_per_period.max(1);
        let payment_periods = self.payment_period_count(instrument);
        // Burnout and the latent regime persist across payment periods.
        let mut prepayment = self.prepayment_path(prepared, path);
        let mut shocks = Vec::with_capacity(payment_periods);

        for period in 0..payment_periods {
            let start = period * months_per_period;
            let end = (start + months_per_period).min(factors.len());
            let month_slice = if start < end {
                &factors[start..end]
            } else {
                &[][..]
            };
            // Prepayment reads its OWN factor slice, correlated with
            // the credit one at the configured rho.
            let prepay_slice = if start < end {
                &prepay_factors[start..end]
            } else {
                &[][..]
            };
            let mut shock = self.aggregate_monthly_shocks(
                prepared,
                start as u32,
                month_slice,
                prepay_slice,
                &mut prepayment,
            )?;
            shock.systematic_z = self.period_systematic_z(prepared, month_slice);
            shock.per_name = self.copula_period_input(prepared, start as u32, month_slice)?;
            shocks.push(shock);
        }

        Ok(shocks)
    }

    pub(super) fn prepayment_path(
        &self,
        prepared: &PreparedRun,
        path: (usize, bool),
    ) -> PrepaymentPath {
        let (path_index, antithetic) = path;
        let stream_index = if antithetic {
            path_index / 2
        } else {
            path_index
        };
        let mut prepayment = PrepaymentPath {
            burnout: 1.0,
            model_state: PrepaymentState::default(),
            transition_rng: PhiloxRng::new(self.config.tree_config.seed ^ PREPAY_REGIME_SEED_SALT)
                .substream(stream_index as u64),
            mirror_draws: antithetic && path_index % 2 == 1,
        };
        if let PreparedPrepayment::Model(model) = &prepared.prepay_model {
            let uniform = prepayment.next_uniform();
            prepayment.model_state =
                model.initial_state(self.config.tree_config.seasoning_months, uniform);
        }
        prepayment
    }

    /// Build the per-name copula plan for one payment period.
    ///
    /// Returns the period systematic factor `Z` and the *unconditional*
    /// period marginal default probability `PDₜ`. The period systematic
    /// factor aggregates all `M` of the period's monthly `N(0,1)` factors as
    /// `(Σ Zₘ)/√M`, the period-representative systematic shock — itself
    /// `N(0,1)` — so the copula channel conditions on the same months the
    /// LHP/MDR channel integrates (item 10). A monthly-pay deal recovers a
    /// single fresh `N(0,1)` per period. The marginal PD compounds the
    /// model's unconditional monthly MDR (`expected_mdr`) over the period's
    /// months.
    pub(super) fn copula_period_input(
        &self,
        prepared: &PreparedRun,
        start_month: u32,
        factors: &[f64],
    ) -> Result<Option<PerNamePeriodInput>> {
        // Per-name simulation applies only to copula default models; other
        // stochastic specs keep the pool-wide MDR path even though they also
        // carry a built default model.
        let (Some(_), PreparedDefault::Model(model)) =
            (prepared.copula_rho, &prepared.default_model)
        else {
            return Ok(None);
        };
        let months = factors.len().max(1) as u32;

        // Unconditional period marginal PD: 1 − ∏(1 − monthly_unconditional_MDR).
        let mut survival = 1.0;
        for offset in 0..months {
            let seasoning = self
                .config
                .tree_config
                .seasoning_months
                .saturating_add(start_month)
                .saturating_add(offset + 1);
            survival *= 1.0 - model.expected_mdr(seasoning)?.clamp(0.0, 1.0);
        }
        let marginal_pd = (1.0 - survival).clamp(0.0, 1.0);

        Ok(Some(PerNamePeriodInput {
            systematic_z: self.period_systematic_z(prepared, factors),
            marginal_pd,
        }))
    }

    /// Period systematic factor (item 10).
    ///
    /// Multi-month periods use `Z_period = (Σ Zₘ)/s` so the copula
    /// conditions on the whole period (matching LHP/MDR aggregation).
    /// `s = period_factor_scale` keeps `Z_period ~ N(0,1)` under the AR(1)
    /// autocorrelation `φ`; `√M` is correct only for independent months.
    /// Zero without stochastic credit.
    pub(super) fn period_systematic_z(&self, prepared: &PreparedRun, factors: &[f64]) -> f64 {
        if self.has_stochastic_rates() && !factors.is_empty() {
            let sum: f64 = factors.iter().sum();
            sum / Self::period_factor_scale(factors.len(), prepared.factor_phi)
        } else {
            0.0
        }
    }

    pub(super) fn aggregate_monthly_shocks(
        &self,
        prepared: &PreparedRun,
        start_month: u32,
        factors: &[f64],
        prepay_factors: &[f64],
        prepayment: &mut PrepaymentPath,
    ) -> Result<PeriodPoolShock> {
        if factors.is_empty() {
            return self.monthly_shock(
                prepared,
                start_month.saturating_add(1),
                0.0,
                0.0,
                prepayment,
            );
        }

        let mut prepay_survival = 1.0;
        let mut default_survival = 1.0;
        let mut recovery_factor_sum = 0.0;
        for (offset, factor) in factors.iter().enumerate() {
            let prepay_factor = prepay_factors.get(offset).copied().unwrap_or(*factor);
            let (smm, mdr, factor) = self.monthly_rates(
                prepared,
                start_month.saturating_add(offset as u32 + 1),
                *factor,
                prepay_factor,
                prepayment,
            )?;
            prepay_survival *= 1.0 - smm;
            default_survival *= 1.0 - mdr;
            recovery_factor_sum += factor;
        }

        let months = factors.len() as f64;
        let recovery_factor = recovery_factor_sum / months;
        let mut shock = PeriodPoolShock::pool_wide(
            1.0 - prepay_survival.powf(1.0 / months),
            1.0 - default_survival.powf(1.0 / months),
            prepared
                .recovery_model
                .conditional_recovery(recovery_factor)?,
        );
        shock.recovery_factor = recovery_factor;
        Ok(shock)
    }

    pub(super) fn monthly_shock(
        &self,
        prepared: &PreparedRun,
        month_offset: u32,
        z: f64,
        z_prepay: f64,
        prepayment: &mut PrepaymentPath,
    ) -> Result<PeriodPoolShock> {
        let (smm, mdr, factor) =
            self.monthly_rates(prepared, month_offset, z, z_prepay, prepayment)?;
        let mut shock = PeriodPoolShock::pool_wide(
            smm,
            mdr,
            prepared.recovery_model.conditional_recovery(factor)?,
        );
        shock.recovery_factor = factor;
        Ok(shock)
    }

    fn monthly_rates(
        &self,
        prepared: &PreparedRun,
        month_offset: u32,
        z: f64,
        z_prepay: f64,
        prepayment: &mut PrepaymentPath,
    ) -> Result<(f64, f64, f64)> {
        let stochastic = self.has_stochastic_rates();
        let factor = if stochastic { z } else { 0.0 };
        // Prepayment reads its own factor. With no configured
        // correlation the caller passes the credit factor through, so this is
        // exact identity with the previous single-factor behaviour.
        let prepay_factor = if stochastic { z_prepay } else { 0.0 };
        let seasoning = self
            .config
            .tree_config
            .seasoning_months
            .saturating_add(month_offset);
        let factor_path = [factor];
        let prepay_factors = [prepay_factor];

        Ok((
            self.conditional_smm(prepared, seasoning, &prepay_factors, prepayment)?,
            self.conditional_mdr(prepared, seasoning, &factor_path)?,
            factor,
        ))
    }

    /// Conditional SMM for one month, advancing the path's burnout state.
    ///
    /// Hard-coding `burnout = 1.0` here (or never calling
    /// `update_burnout`) leaves the Richard-Roll burnout channel inert:
    /// seasoned pools that have already refinanced heavily get modelled with
    /// full prepayment propensity while `has_burnout()` still returns true.
    ///
    /// Burnout is path-state: it accumulates as realized prepayment runs above
    /// or below expectation, so it must be threaded through the month loop
    /// rather than recomputed. `expected_smm` is the unconditional speed at
    /// this seasoning, and the realized/expected ratio is what drives the
    /// update (fast prepayers leave the pool; slow ones rejuvenate it).
    pub(super) fn conditional_smm(
        &self,
        prepared: &PreparedRun,
        seasoning: u32,
        factors: &[f64],
        prepayment: &mut PrepaymentPath,
    ) -> Result<f64> {
        match &prepared.prepay_model {
            PreparedPrepayment::Model(model) => {
                let uniform = prepayment.next_uniform();
                let realized = model
                    .sample_smm(
                        seasoning,
                        factors,
                        self.config.tree_config.market_refi_rate,
                        prepayment.burnout,
                        &mut prepayment.model_state,
                        uniform,
                    )
                    .clamp(0.0, 0.50);
                if model.has_burnout() {
                    let expected = model.expected_smm(seasoning)?;
                    prepayment.burnout =
                        model.update_burnout(prepayment.burnout, realized, expected);
                }
                Ok(realized)
            }
            PreparedPrepayment::Deterministic(spec) => {
                spec.smm(seasoning).map(|rate| rate.clamp(0.0, 0.50))
            }
        }
    }

    fn conditional_mdr(
        &self,
        prepared: &PreparedRun,
        seasoning: u32,
        factors: &[f64],
    ) -> Result<f64> {
        let rate = match &prepared.default_model {
            PreparedDefault::Model(model) => model.conditional_mdr(seasoning, factors),
            PreparedDefault::Deterministic(spec) => spec.mdr(seasoning)?,
        };
        Ok(rate.clamp(0.0, 0.50))
    }
}
