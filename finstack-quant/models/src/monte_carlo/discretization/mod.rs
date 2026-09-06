//! Time-stepping schemes for stochastic differential equations.
//!
//! Start with [`exact`] whenever an analytical transition is available because
//! it avoids discretization bias. This module includes general-purpose Euler /
//! Milstein schemes and model-specific QE schemes for Heston and CIR dynamics.
//!
//! Each discretization module documents the assumptions it makes about the
//! process state, convergence behavior, and positivity / stability guarantees.

pub mod cheyette_rough;
pub mod euler;
pub mod exact;
pub mod exact_gbm_dividends;
pub mod exact_hw1f;
pub mod lmm_predictor_corrector;
pub mod milstein;
pub mod qe_cir;
pub(crate) mod qe_common;
pub mod qe_heston;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;

pub use cheyette_rough::CheyetteRoughEuler;
pub use euler::{EulerMaruyama, LogEuler};
pub use exact::{ExactGbm, ExactMultiGbm, ExactMultiGbmCorrelated};
pub use exact_gbm_dividends::ExactGbmWithDividends;
pub use exact_hw1f::ExactHullWhite1F;
pub use lmm_predictor_corrector::LmmPredictorCorrector;
pub use milstein::Milstein;
pub use qe_cir::QeCir;
pub use qe_heston::QeHeston;
pub use rough_bergomi::RoughBergomiEuler;
pub use rough_heston::RoughHestonHybrid;
pub use schwartz_smith::ExactSchwartzSmith;

#[cfg(test)]
mod work_size_contract {
    use super::lmm_predictor_corrector::LmmPredictorCorrector;
    use super::{CheyetteRoughEuler, ExactGbm, RoughBergomiEuler, RoughHestonHybrid};
    use crate::monte_carlo::process::cheyette_rough::{
        CheyetteRoughVolParams, CheyetteRoughVolProcess,
    };
    use crate::monte_carlo::process::gbm::{GbmParams, GbmProcess};
    use crate::monte_carlo::process::lmm::{LmmParams, LmmProcess};
    use crate::monte_carlo::process::rough_bergomi::{RoughBergomiParams, RoughBergomiProcess};
    use crate::monte_carlo::process::rough_heston::{RoughHestonParams, RoughHestonProcess};
    use crate::monte_carlo::traits::Discretization;
    use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
    use finstack_quant_core::math::fractional::HurstExponent;

    #[test]
    fn work_size_matches_scheme_layout() {
        let hurst = HurstExponent::new(0.1).expect("valid hurst");

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).expect("valid gbm"));
        assert_eq!(ExactGbm::new().work_size(&gbm), 0);

        let cheyette = CheyetteRoughVolProcess::new(
            CheyetteRoughVolParams::new(
                0.03,
                ForwardVarianceCurve::flat(0.005).expect("valid flat curve"),
                hurst,
                1.5,
                -0.3,
                &[(0.0, 0.02), (10.0, 0.03)],
            )
            .expect("valid cheyette"),
        );
        assert_eq!(CheyetteRoughEuler::new(hurst).work_size(&cheyette), 1);

        let bergomi = RoughBergomiProcess::new(
            RoughBergomiParams::new(
                0.05,
                0.02,
                hurst,
                1.9,
                -0.9,
                ForwardVarianceCurve::flat(0.04).expect("valid flat curve"),
            )
            .expect("valid bergomi"),
        );
        assert_eq!(RoughBergomiEuler::new(hurst).work_size(&bergomi), 1);

        let lmm = LmmProcess::new(
            LmmParams {
                num_forwards: 3,
                num_factors: 2,
                tenors: vec![0.0, 1.0, 2.0, 3.0],
                accrual_factors: vec![1.0, 1.0, 1.0],
                displacements: vec![0.005, 0.005, 0.005],
                vol_times: vec![],
                vol_values: vec![vec![
                    [0.15, 0.05, 0.0],
                    [0.12, 0.08, 0.0],
                    [0.10, 0.10, 0.0],
                ]],
                initial_forwards: vec![0.03, 0.03, 0.03],
            }
            .validate()
            .expect("valid lmm"),
        );
        assert_eq!(LmmPredictorCorrector::new().work_size(&lmm), 9);

        let heston = RoughHestonProcess::new(
            RoughHestonParams {
                r: 0.05,
                q: 0.02,
                hurst,
                kappa: 2.0,
                theta: 0.04,
                sigma_v: 0.3,
                rho: -0.7,
                v0: 0.04,
            }
            .validate()
            .expect("valid heston"),
        );
        let times: Vec<f64> = (0..=50).map(|i| i as f64 / 50.0).collect();
        let hybrid = RoughHestonHybrid::new(&times, 0.1).expect("valid hybrid");
        // 50 drift-rate slots + 50 noise slots + 1 counter
        assert_eq!(hybrid.work_size(&heston), 101);
    }
}
