//! Merton structural credit model WASM bindings.

use super::paths::JsSimulatedPaths;
use std::sync::Arc;

use crate::api::core::market_hazard::JsHazardCurve;
use crate::utils::input::{from_js_json, js_bool, js_f64, js_f64_seq, js_string, js_u64, js_uint};
use crate::utils::wire::js_parse;
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::DayCount;
use finstack_quant_models::credit::{
    self as credit, AssetDynamics, MertonBarrierType, MertonModel,
};
use wasm_bindgen::prelude::*;

/// One Merton report: the model, its term structure, grades and CDS terms.
///
/// An equity source takes the KMV default point (short-term debt plus half of
/// long-term debt) as its barrier, and grades come from the default PD master
/// scale, so every host gets the same numbers.
/// @param request - `MertonReportRequest` object or JSON text: `source` (`{ kind: "model", model }` with canonical `MertonModel` JSON, or `{ kind: "equity", equity_value, equity_vol, short_term_debt, long_term_debt, risk_free_rate, payout_rate, maturity }` in the firm's monetary unit, decimals and years), `horizons` (years, finite and positive, in display order), `maturity` (CDS and implied-equity years) and `recovery` (decimal fraction of par).
/// @returns `MertonReport` plain object; per-horizon arrays align with `horizons`, and a measure Rust rejects for this model is `null` with its message under its field name in `errors`.
///
/// # Errors
///
/// Throws with kind `validation` if the request is malformed or has an unknown
/// field, a horizon is non-finite or not positive, or the equity calibration
/// rejects its inputs.
#[wasm_bindgen(js_name = mertonReport, unchecked_return_type = "generated.models.MertonReport")]
pub fn merton_report(
    #[wasm_bindgen(unchecked_param_type = "generated.models.MertonReportRequest | string")] request: JsValue,
) -> Result<JsValue, JsValue> {
    let request: credit::MertonReportRequest = from_js_json(&request, "request")?;
    to_js_value(&credit::merton_report(&request).map_err(to_js_err)?)
}

/// Default-barrier monitoring rule of a structural model: terminal (Merton) or
/// first-passage (Black-Cox).
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const barrier = models.credit.MertonBarrierType.firstPassage(0.02);
/// console.log(barrier.toJson());
/// barrier.free();
/// ```
#[wasm_bindgen(js_name = MertonBarrierType)]
#[derive(Clone, Copy)]
pub struct JsMertonBarrierType {
    pub(crate) inner: MertonBarrierType,
}

json_round_trip!(JsMertonBarrierType, MertonBarrierType);

#[wasm_bindgen(js_class = MertonBarrierType)]
impl JsMertonBarrierType {
    /// Default is tested only at the horizon (Merton 1974).
    /// @returns The terminal-barrier rule.
    pub fn terminal() -> JsMertonBarrierType {
        Self {
            inner: MertonBarrierType::Terminal,
        }
    }

    /// Default occurs the first time assets touch a growing barrier (Black-Cox 1976).
    /// @param barrierGrowthRate - Continuously compounded growth rate of the barrier per year, as a decimal (`0.0` keeps the barrier flat).
    /// @returns The first-passage barrier rule.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `barrierGrowthRate` is not a number.
    #[wasm_bindgen(js_name = firstPassage)]
    pub fn first_passage(
        #[wasm_bindgen(js_name = "barrierGrowthRate", unchecked_param_type = "number")]
        barrier_growth_rate: JsValue,
    ) -> Result<JsMertonBarrierType, JsValue> {
        let barrier_growth_rate = js_f64(&barrier_growth_rate, "barrierGrowthRate")?;
        Ok(Self {
            inner: MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            },
        })
    }
}

/// Asset-value dynamics of a structural model: geometric Brownian motion,
/// Merton jump-diffusion, or CreditGrades.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const dynamics = models.credit.AssetDynamics.jumpDiffusion(0.5, -0.05, 0.1);
/// const barrier = models.credit.MertonBarrierType.terminal();
/// const model = models.credit.MertonModel.newWithDynamics(100, 0.2, 60, 0.03, 0.0, barrier, dynamics);
/// console.log(model.defaultProbability(1));
/// ```
#[wasm_bindgen(js_name = AssetDynamics)]
#[derive(Clone, Copy)]
pub struct JsAssetDynamics {
    pub(crate) inner: AssetDynamics,
}

json_round_trip!(JsAssetDynamics, AssetDynamics);

#[wasm_bindgen(js_class = AssetDynamics)]
impl JsAssetDynamics {
    /// Lognormal diffusion without jumps (the Merton 1974 default).
    /// @returns The geometric-Brownian dynamics.
    #[wasm_bindgen(js_name = geometricBrownian)]
    pub fn geometric_brownian() -> JsAssetDynamics {
        Self {
            inner: AssetDynamics::GeometricBrownian,
        }
    }

    /// Merton (1976) jump-diffusion with lognormal jump sizes.
    /// @param jumpIntensity - Poisson jump arrival rate per year; non-negative.
    /// @param jumpMean - Mean of the log jump size (dimensionless).
    /// @param jumpVol - Standard deviation of the log jump size; non-negative.
    /// @returns The jump-diffusion dynamics.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number. Range checks run
    /// when the dynamics are attached to a model.
    #[wasm_bindgen(js_name = jumpDiffusion)]
    pub fn jump_diffusion(
        #[wasm_bindgen(js_name = "jumpIntensity", unchecked_param_type = "number")] jump_intensity: JsValue,
        #[wasm_bindgen(js_name = "jumpMean", unchecked_param_type = "number")] jump_mean: JsValue,
        #[wasm_bindgen(js_name = "jumpVol", unchecked_param_type = "number")] jump_vol: JsValue,
    ) -> Result<JsAssetDynamics, JsValue> {
        Ok(Self {
            inner: AssetDynamics::JumpDiffusion {
                jump_intensity: js_f64(&jump_intensity, "jumpIntensity")?,
                jump_mean: js_f64(&jump_mean, "jumpMean")?,
                jump_vol: js_f64(&jump_vol, "jumpVol")?,
            },
        })
    }

    /// CreditGrades dynamics with an uncertain default barrier (Finger et al. 2002).
    /// @param barrierUncertainty - Lognormal dispersion of the default barrier (dimensionless, typically about 0.3).
    /// @param meanRecovery - Mean global recovery rate as a fraction from 0 through 1.
    /// @returns The CreditGrades dynamics.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number. Range checks run
    /// when the dynamics are attached to a model.
    #[wasm_bindgen(js_name = creditGrades)]
    pub fn credit_grades(
        #[wasm_bindgen(js_name = "barrierUncertainty", unchecked_param_type = "number")]
        barrier_uncertainty: JsValue,
        #[wasm_bindgen(js_name = "meanRecovery", unchecked_param_type = "number")]
        mean_recovery: JsValue,
    ) -> Result<JsAssetDynamics, JsValue> {
        Ok(Self {
            inner: AssetDynamics::CreditGrades {
                barrier_uncertainty: js_f64(&barrier_uncertainty, "barrierUncertainty")?,
                mean_recovery: js_f64(&mean_recovery, "meanRecovery")?,
            },
        })
    }
}

/// Structural (Merton-family) credit model of one firm.
///
/// Rates and volatilities are annualized decimals, horizons are in years and
/// monetary inputs share one caller-defined unit.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const model = new models.credit.MertonModel(100, 0.25, 60, 0.03);
/// console.log(model.defaultProbability(1), model.distanceToDefault(1));
/// const curve = model.toHazardCurve("ACME", "2025-01-15", [1, 3, 5], 0.4, "act_365f");
/// console.log(curve.sp(5));
/// curve.free();
/// model.free();
/// ```
#[wasm_bindgen(js_name = MertonModel)]
pub struct JsMertonModel {
    pub(crate) inner: MertonModel,
}

json_round_trip!(JsMertonModel, MertonModel);

wasm_getters!(JsMertonModel, MertonModel |s| {
    /// Current fair value of the firm's assets, in monetary units.
    asset_value as assetValue -> f64 = s.inner.asset_value();
    /// Annualized asset-return volatility, as a decimal.
    asset_vol as assetVol -> f64 = s.inner.asset_vol();
    /// Debt face value that defines the default barrier, in monetary units.
    debt_barrier as debtBarrier -> f64 = s.inner.debt_barrier();
    /// Continuously compounded risk-free rate, as a decimal.
    risk_free_rate as riskFreeRate -> f64 = s.inner.risk_free_rate();
    /// Continuous payout rate on assets, as a decimal.
    payout_rate as payoutRate -> f64 = s.inner.payout_rate();
    /// Default-barrier monitoring rule of this model.
    barrier_type as barrierType -> JsMertonBarrierType = JsMertonBarrierType {
        inner: *s.inner.barrier_type(),
    };
    /// Asset-value dynamics of this model.
    dynamics -> JsAssetDynamics = JsAssetDynamics {
        inner: *s.inner.dynamics(),
    };
});

#[wasm_bindgen(js_class = MertonModel)]
impl JsMertonModel {
    /// Terminal-barrier Merton model with geometric-Brownian assets and no payout.
    /// @param assetValue - Current fair value of the firm's assets in monetary units; positive.
    /// @param assetVol - Annualized volatility of firm-asset returns, as a decimal; positive.
    /// @param debtBarrier - Positive debt face value defining the default barrier.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal, such as 0.05 for 5%.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, and a `validation`
    /// error if `assetValue`, `assetVol` or `debtBarrier` is not positive and
    /// finite or `riskFreeRate` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "assetValue", unchecked_param_type = "number")]
        asset_value: JsValue,
        #[wasm_bindgen(js_name = "assetVol", unchecked_param_type = "number")] asset_vol: JsValue,
        #[wasm_bindgen(js_name = "debtBarrier", unchecked_param_type = "number")]
        debt_barrier: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::new(
            js_f64(&asset_value, "assetValue")?,
            js_f64(&asset_vol, "assetVol")?,
            js_f64(&debt_barrier, "debtBarrier")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Calibrate asset value and volatility from observable equity (KMV iteration).
    /// @param equityValue - Current market value of equity in the firm's monetary units.
    /// @param equityVol - Annualized equity-return volatility as a decimal.
    /// @param totalDebt - Total debt face value used as the default barrier.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal.
    /// @param payoutRate - Continuous dividend or payout yield on assets, as a decimal.
    /// @param maturity - Calibration horizon in years; positive and finite.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range and a
    /// `computation` error if the calibration does not converge.
    #[wasm_bindgen(js_name = fromEquity)]
    pub fn from_equity(
        #[wasm_bindgen(js_name = "equityValue", unchecked_param_type = "number")]
        equity_value: JsValue,
        #[wasm_bindgen(js_name = "equityVol", unchecked_param_type = "number")] equity_vol: JsValue,
        #[wasm_bindgen(js_name = "totalDebt", unchecked_param_type = "number")] total_debt: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(js_name = "payoutRate", unchecked_param_type = "number")]
        payout_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] maturity: JsValue,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::from_equity(
            js_f64(&equity_value, "equityValue")?,
            js_f64(&equity_vol, "equityVol")?,
            js_f64(&total_debt, "totalDebt")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&payout_rate, "payoutRate")?,
            js_f64(&maturity, "maturity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Calibrate asset volatility to a target CDS par spread.
    ///
    /// The objective is a full ISDA-style par spread built from the model's
    /// survival curve. A quote that no volatility in `[0.01, 2.0]` reproduces,
    /// or one consistent with several volatilities, is rejected rather than
    /// resolved arbitrarily.
    /// @param cdsSpreadBp - Target CDS par spread in basis points.
    /// @param recovery - Recovery rate at default as a fraction from 0 through 1.
    /// @param totalDebt - Total debt face value in the firm's monetary units.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal.
    /// @param maturity - CDS maturity in years; positive and finite.
    /// @param assetValue - Assumed current firm asset value in monetary units.
    /// @param payoutRate - Continuous payout rate on assets, as a decimal.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range or the quote is
    /// unattainable or ambiguous.
    #[wasm_bindgen(js_name = fromCdsSpread)]
    pub fn from_cds_spread(
        #[wasm_bindgen(js_name = "cdsSpreadBp", unchecked_param_type = "number")]
        cds_spread_bp: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] recovery: JsValue,
        #[wasm_bindgen(js_name = "totalDebt", unchecked_param_type = "number")] total_debt: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] maturity: JsValue,
        #[wasm_bindgen(js_name = "assetValue", unchecked_param_type = "number")]
        asset_value: JsValue,
        #[wasm_bindgen(js_name = "payoutRate", unchecked_param_type = "number")]
        payout_rate: JsValue,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::from_cds_spread(
            js_f64(&cds_spread_bp, "cdsSpreadBp")?,
            js_f64(&recovery, "recovery")?,
            js_f64(&total_debt, "totalDebt")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&maturity, "maturity")?,
            js_f64(&asset_value, "assetValue")?,
            js_f64(&payout_rate, "payoutRate")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Calibrate the default barrier to a target cumulative default probability.
    /// @param assetValue - Current fair value of the firm's assets in monetary units.
    /// @param assetVol - Annualized volatility of firm-asset returns as a decimal; positive.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal; pass the physical asset return to calibrate against a real-world default rate.
    /// @param payoutRate - Continuous payout rate on assets as a decimal; it enters the calibration drift and is carried on the model.
    /// @param targetPd - Target cumulative default probability strictly between 0 and 1.
    /// @param maturity - Calibration horizon in years; positive and finite.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range.
    #[wasm_bindgen(js_name = fromTargetPd)]
    pub fn from_target_pd(
        #[wasm_bindgen(js_name = "assetValue", unchecked_param_type = "number")]
        asset_value: JsValue,
        #[wasm_bindgen(js_name = "assetVol", unchecked_param_type = "number")] asset_vol: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(js_name = "payoutRate", unchecked_param_type = "number")]
        payout_rate: JsValue,
        #[wasm_bindgen(js_name = "targetPd", unchecked_param_type = "number")] target_pd: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] maturity: JsValue,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::from_target_pd(
            js_f64(&asset_value, "assetValue")?,
            js_f64(&asset_vol, "assetVol")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&payout_rate, "payoutRate")?,
            js_f64(&target_pd, "targetPd")?,
            js_f64(&maturity, "maturity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Moody's KMV default point: short-term debt plus half of long-term debt.
    /// @param shortTermDebt - Liabilities due within one year, in the firm's monetary units.
    /// @param longTermDebt - Liabilities maturing beyond one year, in the same units; half of it enters the default point.
    /// @returns The default point in the same monetary units.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if either input is negative or non-finite,
    /// or the resulting default point is zero.
    #[wasm_bindgen(js_name = kmvDefaultPoint)]
    pub fn kmv_default_point(
        #[wasm_bindgen(js_name = "shortTermDebt", unchecked_param_type = "number")] short_term_debt: JsValue,
        #[wasm_bindgen(js_name = "longTermDebt", unchecked_param_type = "number")]
        long_term_debt: JsValue,
    ) -> Result<f64, JsValue> {
        MertonModel::kmv_default_point(
            js_f64(&short_term_debt, "shortTermDebt")?,
            js_f64(&long_term_debt, "longTermDebt")?,
        )
        .map_err(to_js_err)
    }

    /// Model with an explicit barrier rule and asset dynamics.
    /// @param assetValue - Current fair value of the firm's assets in monetary units; positive.
    /// @param assetVol - Annualized volatility of firm-asset returns as a decimal; positive.
    /// @param debtBarrier - Positive debt face value defining the default barrier.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal.
    /// @param payoutRate - Continuous payout rate on assets, as a decimal.
    /// @param barrierType - `MertonBarrierType` handle selecting terminal or first-passage default.
    /// @param dynamics - `AssetDynamics` handle selecting GBM, jump-diffusion or CreditGrades assets.
    /// @returns The validated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range or the barrier
    /// rule and dynamics are incompatible (for example jump-diffusion with a
    /// first-passage barrier).
    #[wasm_bindgen(js_name = newWithDynamics)]
    pub fn new_with_dynamics(
        #[wasm_bindgen(js_name = "assetValue", unchecked_param_type = "number")]
        asset_value: JsValue,
        #[wasm_bindgen(js_name = "assetVol", unchecked_param_type = "number")] asset_vol: JsValue,
        #[wasm_bindgen(js_name = "debtBarrier", unchecked_param_type = "number")]
        debt_barrier: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(js_name = "payoutRate", unchecked_param_type = "number")]
        payout_rate: JsValue,
        #[wasm_bindgen(js_name = "barrierType")] barrier_type: &JsMertonBarrierType,
        dynamics: &JsAssetDynamics,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::new_with_dynamics(
            js_f64(&asset_value, "assetValue")?,
            js_f64(&asset_vol, "assetVol")?,
            js_f64(&debt_barrier, "debtBarrier")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&payout_rate, "payoutRate")?,
            barrier_type.inner,
            dynamics.inner,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// CreditGrades model calibrated from equity observables.
    /// @param equityValue - Current market value of equity in the firm's monetary units.
    /// @param equityVol - Annualized equity-return volatility as a decimal.
    /// @param totalDebt - Total debt face value in the firm's monetary units.
    /// @param riskFreeRate - Continuously compounded risk-free rate as a decimal.
    /// @param barrierUncertainty - Lognormal dispersion of the CreditGrades default barrier.
    /// @param meanRecovery - Mean recovery rate at default as a fraction from 0 through 1.
    /// @returns The CreditGrades model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range.
    #[wasm_bindgen(js_name = creditGrades)]
    pub fn credit_grades(
        #[wasm_bindgen(js_name = "equityValue", unchecked_param_type = "number")]
        equity_value: JsValue,
        #[wasm_bindgen(js_name = "equityVol", unchecked_param_type = "number")] equity_vol: JsValue,
        #[wasm_bindgen(js_name = "totalDebt", unchecked_param_type = "number")] total_debt: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(js_name = "barrierUncertainty", unchecked_param_type = "number")]
        barrier_uncertainty: JsValue,
        #[wasm_bindgen(js_name = "meanRecovery", unchecked_param_type = "number")]
        mean_recovery: JsValue,
    ) -> Result<JsMertonModel, JsValue> {
        MertonModel::credit_grades(
            js_f64(&equity_value, "equityValue")?,
            js_f64(&equity_vol, "equityVol")?,
            js_f64(&total_debt, "totalDebt")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&barrier_uncertainty, "barrierUncertainty")?,
            js_f64(&mean_recovery, "meanRecovery")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Risk-neutral distance to default `d2` over a horizon.
    ///
    /// Lower values indicate higher default risk. This is not the Moody's KMV
    /// distance to default; use `distanceToDefaultWithDrift` for that.
    /// @param horizon - Forward-looking horizon in years.
    /// @returns Distance to default in standard deviations.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `horizon` is not a number.
    #[wasm_bindgen(js_name = distanceToDefault)]
    pub fn distance_to_default(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self.inner.distance_to_default(js_f64(&horizon, "horizon")?))
    }

    /// Physical-measure (Moody's KMV) distance to default.
    /// @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
    /// @param horizon - Forward-looking horizon in years.
    /// @returns Distance to default in standard deviations under the physical drift.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if drift or horizon is non-finite, a Gaussian
    /// variate is undefined, or the model uses driftless CreditGrades dynamics.
    /// Numerically unresolved structural probabilities raise a computation error.
    #[wasm_bindgen(js_name = distanceToDefaultWithDrift)]
    pub fn distance_to_default_with_drift(
        &self,
        #[wasm_bindgen(js_name = "assetDrift", unchecked_param_type = "number")]
        asset_drift: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .distance_to_default_with_drift(
                js_f64(&asset_drift, "assetDrift")?,
                js_f64(&horizon, "horizon")?,
            )
            .map_err(to_js_err)
    }

    /// Risk-neutral cumulative default probability over a horizon.
    /// @param horizon - Forward-looking horizon in years; a non-positive horizon returns 0.
    /// @returns Default probability in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `horizon` is not a number. A non-finite horizon or undefined variate raises
    /// a validation error. Numerically unresolved structural probabilities raise a computation error.
    #[wasm_bindgen(js_name = defaultProbability)]
    pub fn default_probability(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .default_probability(js_f64(&horizon, "horizon")?)
            .map_err(to_js_err)
    }

    /// Risk-neutral default probabilities over a grid of horizons.
    ///
    /// Python returns the same values as a pandas Series labelled by horizon.
    /// @param horizons - Horizons in years, as a `number[]` or `Float64Array`.
    /// @returns One default probability per horizon, in input order.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `horizons` is not an array of numbers. A non-finite horizon or undefined variate raises
    /// a validation error. Numerically unresolved structural probabilities raise a computation error.
    #[wasm_bindgen(js_name = defaultProbabilities)]
    pub fn default_probabilities(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] horizons: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        let horizons = js_f64_seq(&horizons, "horizons")?;
        Ok(self
            .inner
            .default_probabilities(&horizons)
            .map_err(to_js_err)?
            .into_boxed_slice())
    }

    /// Physical-measure (Moody's KMV) default probability, the theoretical EDF.
    /// @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
    /// @param horizon - Forward-looking horizon in years.
    /// @returns Default probability in `[0, 1]` under the physical drift.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `assetDrift` is not finite or the model
    /// uses driftless CreditGrades dynamics.
    #[wasm_bindgen(js_name = defaultProbabilityWithDrift)]
    pub fn default_probability_with_drift(
        &self,
        #[wasm_bindgen(js_name = "assetDrift", unchecked_param_type = "number")]
        asset_drift: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .default_probability_with_drift(
                js_f64(&asset_drift, "assetDrift")?,
                js_f64(&horizon, "horizon")?,
            )
            .map_err(to_js_err)
    }

    /// Zero-coupon credit spread given an exogenous recovery paid at maturity.
    /// @param horizon - Maturity in years; positive and finite.
    /// @param recovery - Recovery rate at default as a fraction of par from 0 through 1.
    /// @returns Credit spread per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `horizon` is not positive and finite or
    /// `recovery` is outside `[0, 1]`.
    #[wasm_bindgen(js_name = impliedSpread)]
    pub fn implied_spread(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] recovery: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .implied_spread(js_f64(&horizon, "horizon")?, js_f64(&recovery, "recovery")?)
            .map_err(to_js_err)
    }

    /// Merton (1974) endogenous debt spread, where recovery is the firm's own
    /// terminal asset value.
    /// @param horizon - Maturity of the firm's debt in years; positive.
    /// @returns Debt spread per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `horizon` is not positive, the barrier is
    /// not terminal, or the implied debt value is not positive.
    #[wasm_bindgen(js_name = debtSpread)]
    pub fn debt_spread(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .debt_spread(js_f64(&horizon, "horizon")?)
            .map_err(to_js_err)
    }

    /// ISDA-style CDS par spread implied by the model's survival curve.
    /// @param maturity - CDS maturity in years; positive and finite.
    /// @param recovery - Recovery rate at default as a fraction of par from 0 through 1.
    /// @returns CDS par spread per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `maturity` is not positive, `recovery`
    /// is outside `[0, 1]` or contradicts the CreditGrades `mean_recovery`, or
    /// the implied survival curve cannot be bootstrapped.
    #[wasm_bindgen(js_name = cdsParSpread)]
    pub fn cds_par_spread(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] maturity: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] recovery: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .cds_par_spread(
                js_f64(&maturity, "maturity")?,
                js_f64(&recovery, "recovery")?,
            )
            .map_err(to_js_err)
    }

    /// Equity value and equity volatility implied by the model.
    /// @param horizon - Debt maturity in years; positive and finite.
    /// @returns A `Float64Array` of length 2: `[equityValue, equityVolatility]`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `horizon` is not positive and finite, the
    /// firm is economically in default, or the inversion is ill-conditioned.
    #[wasm_bindgen(js_name = impliedEquity)]
    pub fn implied_equity(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        let (equity, equity_vol) = self
            .inner
            .implied_equity(js_f64(&horizon, "horizon")?)
            .map_err(to_js_err)?;
        Ok(Box::new([equity, equity_vol]))
    }

    /// Bootstrap a piecewise-constant hazard curve from the structural default
    /// probabilities.
    /// @param id - Identifier assigned to the hazard curve.
    /// @param baseDate - Valuation date in ISO-8601 form, such as `"2025-01-15"`.
    /// @param tenors - Tenor grid in years as a `number[]` or `Float64Array`; non-empty, positive and distinct.
    /// @param recovery - Recovery rate as a fraction from 0 through 1; must equal the model's `mean_recovery` under CreditGrades dynamics.
    /// @param dayCount - Day-count convention the curve uses for year fractions, such as `"act_365f"` or `"act_360"`.
    /// @returns The bootstrapped `core.HazardCurve` handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the date or day count does not parse,
    /// `tenors` is empty or non-positive, `recovery` is out of range, or the
    /// implied survival curve is not monotone.
    #[wasm_bindgen(js_name = toHazardCurve)]
    pub fn to_hazard_curve(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] tenors: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] recovery: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
    ) -> Result<JsHazardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = parse_iso_date(&js_string(&base_date, "baseDate")?)?;
        let tenors = js_f64_seq(&tenors, "tenors")?;
        let recovery = js_f64(&recovery, "recovery")?;
        // `FromStr` applies the canonical parser's outer whitespace and
        // ASCII case normalization, unlike strict serde labels.
        let day_count: DayCount = js_parse(&day_count, "dayCount")?;
        self.inner
            .to_hazard_curve(&id, base_date, &tenors, recovery, day_count)
            .map(|curve| JsHazardCurve {
                inner: Arc::new(curve),
            })
            .map_err(to_js_err)
    }

    /// Simulate firm-asset paths by Monte Carlo.
    /// @param numPaths - Number of paths to simulate; a safe non-negative integer.
    /// @param numSteps - Number of time steps per path; at least 1.
    /// @param horizon - Simulation horizon in years; positive and finite.
    /// @param seed - Seed for reproducible draws, as a safe integer or `bigint`; the Rust generator (PCG64) gives equal paths for equal seeds in every host.
    /// @param antithetic - When `true`, use antithetic variates for variance reduction.
    /// @returns The simulated paths.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if a count is not a safe integer, and a
    /// `validation` error if `numSteps` is zero, `horizon` is not positive and
    /// finite, or the path storage size overflows.
    #[wasm_bindgen(js_name = simulatePaths)]
    pub fn simulate_paths(
        &self,
        #[wasm_bindgen(js_name = "numPaths", unchecked_param_type = "number")] num_paths: JsValue,
        #[wasm_bindgen(js_name = "numSteps", unchecked_param_type = "number")] num_steps: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number | bigint")] seed: JsValue,
        #[wasm_bindgen(unchecked_param_type = "boolean")] antithetic: JsValue,
    ) -> Result<JsSimulatedPaths, JsValue> {
        let num_paths: usize = js_uint(&num_paths, "numPaths")?;
        let num_steps: usize = js_uint(&num_steps, "numSteps")?;
        let horizon = js_f64(&horizon, "horizon")?;
        let seed = js_u64(&seed, "seed")?;
        let antithetic = js_bool(&antithetic, "antithetic")?;
        self.inner
            .simulate_paths(num_paths, num_steps, horizon, seed, antithetic)
            .map(|inner| JsSimulatedPaths { inner })
            .map_err(to_js_err)
    }
}
