//! Per-name copula default simulation for finite structured-credit pools.
//!
//! The scalar-`Z` engine ([`super::CopulaBasedDefault::conditional_mdr`])
//! returns the *conditional* default probability `P(default | Z)` and applies
//! that single marginal default rate (MDR) uniformly to every asset. That is
//! the large-homogeneous-pool (LHP) limit: the realized default-count
//! distribution is a deterministic function of `Z` with **zero idiosyncratic
//! dispersion**. For a concentrated CLO (50-150 names) name-level lumpiness is
//! the dominant risk and the LHP limit materially mis-prices the
//! mezzanine/equity tranches (Glasserman §9; O'Kane Ch. 18-19).
//!
//! This module realizes per-name defaults: each name `i` draws its own
//! idiosyncratic shock `εᵢ`, forms the copula latent variable
//! `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` (Gaussian) — or the Student-t analogue with a
//! shared mixing variable `W` — and defaults in the period when `Aᵢ` crosses
//! the per-name threshold `c = Φ⁻¹(PDₜ)` (`t_ν⁻¹(PDₜ)` for Student-t).
//!
//! The latent-variable and mixing-variable maths are **not** reimplemented
//! here — they are delegated to the existing copula kernels via
//! [`Copula::latent_variable`] and [`Copula::sample_mixing`].
//!
//! # LHP fast-path
//!
//! As `N → ∞` the realized default fraction converges (law of large numbers)
//! to `E[1{Aᵢ ≤ c} | Z] = `[`Copula::conditional_default_prob`]. The LHP
//! fast-path applies exactly that conditional probability to the whole pool,
//! so it is the genuine `N → ∞` limit of this per-name model — selectable via
//! [`PoolGranularity`] for pools granular enough that the limit is an
//! acceptable, faster approximation.

use crate::instruments::common_impl::models::correlation::copula::{Copula, CopulaSpec};
use finstack_core::math::{standard_normal_inv_cdf, student_t_inv_cdf};
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::traits::RandomStream;

/// Pool-granularity policy for the structured-credit default engine.
///
/// Selects whether each scenario realizes defaults name-by-name (finite-pool
/// copula simulation) or applies the closed-form LHP conditional default
/// probability uniformly to the pool.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum PoolGranularity {
    /// Simulate each name's default independently conditional on the
    /// systematic factor (finite-pool copula). Captures name-level lumpiness;
    /// the correct default for concentrated pools. This is the default.
    #[default]
    PerName,

    /// Apply the closed-form LHP conditional default probability uniformly to
    /// every name. This is the `N → ∞` limit of [`Self::PerName`] — an
    /// acceptable, faster approximation only for genuinely granular pools.
    LargeHomogeneous,
}

/// Threshold transform for the per-name default barrier.
///
/// Gaussian/RFL/Multi-factor copulas invert the standard normal CDF;
/// the Student-t copula inverts `t_ν`.
#[derive(Debug, Clone, Copy)]
enum ThresholdKind {
    /// `c = Φ⁻¹(PD)`.
    Gaussian,
    /// `c = t_ν⁻¹(PD)`.
    StudentT { degrees_of_freedom: f64 },
}

impl ThresholdKind {
    fn from_spec(spec: &CopulaSpec) -> Self {
        match spec {
            CopulaSpec::StudentT { degrees_of_freedom } => ThresholdKind::StudentT {
                degrees_of_freedom: *degrees_of_freedom,
            },
            _ => ThresholdKind::Gaussian,
        }
    }

    /// Default barrier `c` for a per-period marginal default probability `pd`.
    fn threshold(&self, pd: f64) -> f64 {
        let p = pd.clamp(1e-10, 1.0 - 1e-10);
        match self {
            ThresholdKind::Gaussian => standard_normal_inv_cdf(p),
            ThresholdKind::StudentT { degrees_of_freedom } => {
                student_t_inv_cdf(p, *degrees_of_freedom)
            }
        }
    }
}

/// Finite-pool per-name copula default simulator.
///
/// Holds the copula kernel and asset correlation. One instance is shared
/// (via clone of the cheap `Arc`-backed copula) across all scenario paths;
/// each path drives it with its own [`PhiloxRng`] substream.
pub(crate) struct PerNameCopulaDefault {
    /// Copula kernel — provides `latent_variable` / `sample_mixing` /
    /// `conditional_default_prob`. Never reimplemented here.
    copula: Box<dyn Copula>,
    /// Asset correlation `ρ`.
    correlation: f64,
    /// Threshold transform (`Φ⁻¹` vs `t_ν⁻¹`).
    threshold_kind: ThresholdKind,
}

impl PerNameCopulaDefault {
    /// Build a per-name simulator from a copula specification.
    pub(crate) fn new(copula_spec: &CopulaSpec, correlation: f64) -> Self {
        Self {
            copula: copula_spec.build(),
            correlation: correlation.clamp(0.0, 0.99),
            threshold_kind: ThresholdKind::from_spec(copula_spec),
        }
    }

    /// Realize per-name default indicators for one payment period.
    ///
    /// # Arguments
    ///
    /// * `systematic` — the period systematic factor `Z` (shared by all names).
    /// * `marginal_pd` — the per-name *unconditional* period marginal default
    ///   probability `PDₜ`. For a homogeneous pool every name shares it; the
    ///   slice carries one entry per still-alive name to support heterogeneous
    ///   pools.
    /// * `rng` — the path's RNG substream. Per-name idiosyncratic draws are
    ///   pulled in slice order, so the realization is deterministic and
    ///   order-stable for a fixed seed.
    /// * `out` — filled with one `bool` per name: `true` ⇒ name defaults this
    ///   period.
    ///
    /// The shared Student-t mixing variable `W` is drawn once here (one
    /// uniform), then reused for every name so tail dependence is preserved.
    pub(crate) fn simulate_period(
        &self,
        systematic: f64,
        marginal_pd: &[f64],
        rng: &mut PhiloxRng,
        out: &mut Vec<bool>,
    ) {
        out.clear();
        out.reserve(marginal_pd.len());

        // One shared mixing draw per period (1.0 for Gaussian — no mixing).
        // Drawn before any per-name εᵢ so the draw order is fixed regardless
        // of pool size.
        let mixing = self.copula.sample_mixing(rng.next_u01());

        for &pd in marginal_pd {
            let threshold = self.threshold_kind.threshold(pd);
            let idiosyncratic = rng.next_std_normal();
            let latent =
                self.copula
                    .latent_variable(systematic, idiosyncratic, mixing, self.correlation);
            out.push(latent <= threshold);
        }
    }

    /// LHP conditional default probability for one period.
    ///
    /// This is `E[1{Aᵢ ≤ c} | Z]` — the `N → ∞` limit of
    /// [`Self::simulate_period`] — used by the [`PoolGranularity::LargeHomogeneous`]
    /// fast-path. Delegates to [`Copula::conditional_default_prob`].
    pub(crate) fn conditional_default_prob(&self, systematic: f64, marginal_pd: f64) -> f64 {
        let threshold = self.threshold_kind.threshold(marginal_pd);
        self.copula
            .conditional_default_prob(threshold, &[systematic], self.correlation)
            .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-name realization, averaged over the systematic factor and a
    /// large pool, must recover the unconditional marginal PD. Without this
    /// the realized default count would be biased.
    #[test]
    fn per_name_marginal_recovers_pd() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let pd = 0.04;
        let names = vec![pd; 200];
        let mut rng = PhiloxRng::new(123);
        let mut out = Vec::new();

        let periods = 4_000usize;
        let mut total_defaults = 0usize;
        for _ in 0..periods {
            let z = rng.next_std_normal();
            sim.simulate_period(z, &names, &mut rng, &mut out);
            total_defaults += out.iter().filter(|d| **d).count();
        }
        let realized = total_defaults as f64 / (periods * names.len()) as f64;
        assert!(
            (realized - pd).abs() < 0.003,
            "per-name marginal {realized} should recover PD {pd}"
        );
    }

    /// Conditional on `Z`, the realized default fraction of a large pool must
    /// converge to the LHP conditional default probability. This is the
    /// finite-pool → LHP correctness anchor.
    #[test]
    fn large_pool_converges_to_lhp_conditional() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.25);
        let pd = 0.05;
        let n = 20_000usize;
        let names = vec![pd; n];
        let mut rng = PhiloxRng::new(99);
        let mut out = Vec::new();

        for &z in &[-1.5_f64, 0.0, 1.5] {
            sim.simulate_period(z, &names, &mut rng, &mut out);
            let realized = out.iter().filter(|d| **d).count() as f64 / n as f64;
            let lhp = sim.conditional_default_prob(z, pd);
            assert!(
                (realized - lhp).abs() < 0.01,
                "z={z}: realized fraction {realized} should converge to LHP {lhp}"
            );
        }
    }

    /// A negative systematic factor (market stress) must produce more
    /// defaults than a positive one — the copula's systematic channel.
    #[test]
    fn stress_factor_increases_defaults() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let pd = 0.05;
        let names = vec![pd; 5_000];
        let mut rng = PhiloxRng::new(7);
        let mut out = Vec::new();

        sim.simulate_period(-2.0, &names, &mut rng, &mut out);
        let stressed = out.iter().filter(|d| **d).count();
        sim.simulate_period(2.0, &names, &mut rng, &mut out);
        let benign = out.iter().filter(|d| **d).count();

        assert!(
            stressed > benign,
            "stress (Z=-2) defaults {stressed} should exceed benign (Z=+2) {benign}"
        );
    }

    /// A finite concentrated pool must show genuine idiosyncratic dispersion:
    /// repeated draws at a *fixed* `Z` produce a spread of default counts,
    /// unlike the LHP limit which is a deterministic function of `Z`.
    #[test]
    fn finite_pool_has_idiosyncratic_dispersion() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.20);
        let pd = 0.06;
        let n = 80usize;
        let names = vec![pd; n];
        let mut rng = PhiloxRng::new(2024);
        let mut out = Vec::new();

        let z = 0.0; // fixed systematic factor
        let trials = 600usize;
        let mut counts = Vec::with_capacity(trials);
        for _ in 0..trials {
            sim.simulate_period(z, &names, &mut rng, &mut out);
            counts.push(out.iter().filter(|d| **d).count());
        }
        let min = counts.iter().copied().min().unwrap_or(0);
        let max = counts.iter().copied().max().unwrap_or(0);
        // The LHP limit would give an identical count every trial. Per-name
        // simulation must spread it.
        assert!(
            max > min,
            "finite pool must show dispersion in default counts (min={min}, max={max})"
        );
    }

    /// Student-t per-name simulation recovers the marginal PD and is
    /// order-stable.
    #[test]
    fn student_t_per_name_marginal_recovers_pd() {
        let sim = PerNameCopulaDefault::new(
            &CopulaSpec::StudentT {
                degrees_of_freedom: 6.0,
            },
            0.30,
        );
        let pd = 0.05;
        let names = vec![pd; 128];
        let mut rng = PhiloxRng::new(555);
        let mut out = Vec::new();

        let periods = 5_000usize;
        let mut total = 0usize;
        for _ in 0..periods {
            let z = rng.next_std_normal();
            sim.simulate_period(z, &names, &mut rng, &mut out);
            total += out.iter().filter(|d| **d).count();
        }
        let realized = total as f64 / (periods * names.len()) as f64;
        assert!(
            (realized - pd).abs() < 0.004,
            "Student-t per-name marginal {realized} should recover PD {pd}"
        );
    }

    /// Determinism: the same seed and inputs reproduce the per-name default
    /// mask bit-for-bit.
    #[test]
    fn per_name_simulation_is_deterministic() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let names = vec![0.05; 100];

        let run = |seed: u64| {
            let mut rng = PhiloxRng::new(seed);
            let mut out = Vec::new();
            let mut all = Vec::new();
            for _ in 0..10 {
                let z = rng.next_std_normal();
                sim.simulate_period(z, &names, &mut rng, &mut out);
                all.extend_from_slice(&out);
            }
            all
        };

        assert_eq!(run(42), run(42), "fixed seed must reproduce default mask");
    }
}
