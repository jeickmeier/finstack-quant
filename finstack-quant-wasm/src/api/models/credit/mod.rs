//! WASM bindings for the structural-credit models.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/` so the
//! Rust-canonical → PyO3 → wasm-bindgen triplet keeps file parity. Every
//! handle wraps the Rust type in `inner`; the JS facade nests the exports under
//! `models.credit`.

pub mod lgd;
pub mod migration;
pub mod pd;
pub mod recovery_waterfall;
pub mod scoring;

use std::sync::Arc;

use super::serde_tag;
use crate::api::core::market_data::JsHazardCurve;
use crate::utils::input::{from_js_json, js_bool, js_f64, js_f64_seq, js_string, js_u64, js_uint};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::types::CreditRating;
use finstack_quant_models::credit::{
    self as credit, AssetDynamics, CreditState, CreditStateVariable, DynamicRecoverySpec,
    EndogenousHazardSpec, MertonBarrierType, MertonModel, RatingFactorTable, SimulatedPaths,
    ThresholdDirection, ToggleExerciseModel,
};
use wasm_bindgen::prelude::*;

pub(crate) fn js_rating(value: &JsValue, label: &str) -> Result<CreditRating, JsValue> {
    js_string(value, label)?
        .parse::<CreditRating>()
        .map_err(to_js_err)
}

/// Moody's WARF rating factor for one credit rating.
/// @param rating - Rating label such as `"Baa3"`, `"BBB-"` or `"B2"`; agency notches are normalized by the Rust parser.
/// @returns The Moody's weighted-average rating factor for that rating (for example 610 for Baa3).
///
/// # Errors
///
/// Throws a `TypeError` if `rating` is not a string, and a `validation` error
/// if it is not a recognized rating or has no factor in the Moody's table.
#[wasm_bindgen(js_name = moodysWarfFactor)]
pub fn moodys_warf_factor(rating: JsValue) -> Result<f64, JsValue> {
    credit::moodys_warf_factor(js_rating(&rating, "rating")?).map_err(to_js_err)
}

/// Rating-to-factor table used for WARF-style portfolio credit quality.
#[wasm_bindgen(js_name = RatingFactorTable)]
pub struct JsRatingFactorTable {
    pub(crate) inner: RatingFactorTable,
}

json_round_trip!(JsRatingFactorTable, RatingFactorTable);

#[wasm_bindgen(js_class = RatingFactorTable)]
impl JsRatingFactorTable {
    /// Moody's standard WARF table from the embedded credit-assumption registry.
    /// @returns The Moody's standard rating-factor table.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the embedded registry cannot supply the
    /// table.
    #[wasm_bindgen(js_name = moodysStandard)]
    pub fn moodys_standard() -> Result<JsRatingFactorTable, JsValue> {
        RatingFactorTable::moodys_standard()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Rating-factor table registered under an explicit registry identifier.
    /// @param id - Registry identifier of the table, such as `"moodys_standard"`.
    /// @returns The rating-factor table registered under `id`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `id` is not a string, and a `validation` error
    /// if no table is registered under it.
    #[wasm_bindgen(js_name = fromRegistryId)]
    pub fn from_registry_id(id: JsValue) -> Result<JsRatingFactorTable, JsValue> {
        let id = js_string(&id, "id")?;
        RatingFactorTable::from_registry_id(&id)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Factor assigned to one rating; unlisted ratings fall back to `defaultFactor`.
    /// @param rating - Rating label such as `"Baa3"`, `"BBB-"` or `"B2"`.
    /// @returns The dimensionless rating factor for that rating.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `rating` is not a string, and a `validation`
    /// error if it is not a recognized rating.
    #[wasm_bindgen(js_name = getFactor)]
    pub fn get_factor(&self, rating: JsValue) -> Result<f64, JsValue> {
        self.inner
            .get_factor(js_rating(&rating, "rating")?)
            .map_err(to_js_err)
    }

    /// Rating agency the table belongs to, such as `"Moodys"`.
    #[wasm_bindgen(getter)]
    pub fn agency(&self) -> String {
        self.inner.agency().to_string()
    }

    /// Methodology label of the table, such as the CLO methodology it was taken from.
    #[wasm_bindgen(getter)]
    pub fn methodology(&self) -> String {
        self.inner.methodology().to_string()
    }

    /// Factor applied to ratings the table does not list.
    #[wasm_bindgen(getter, js_name = defaultFactor)]
    pub fn default_factor(&self) -> f64 {
        self.inner.default_factor()
    }
}

/// Default-barrier monitoring rule of a structural model: terminal (Merton) or
/// first-passage (Black-Cox).
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
    /// @param barrier_growth_rate - Continuously compounded growth rate of the barrier per year, as a decimal (`0.0` keeps the barrier flat).
    /// @returns The first-passage barrier rule.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `barrierGrowthRate` is not a number.
    #[wasm_bindgen(js_name = firstPassage)]
    pub fn first_passage(barrier_growth_rate: JsValue) -> Result<JsMertonBarrierType, JsValue> {
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
    /// @param jump_intensity - Poisson jump arrival rate per year; non-negative.
    /// @param jump_mean - Mean of the log jump size (dimensionless).
    /// @param jump_vol - Standard deviation of the log jump size; non-negative.
    /// @returns The jump-diffusion dynamics.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number. Range checks run
    /// when the dynamics are attached to a model.
    #[wasm_bindgen(js_name = jumpDiffusion)]
    pub fn jump_diffusion(
        jump_intensity: JsValue,
        jump_mean: JsValue,
        jump_vol: JsValue,
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
    /// @param barrier_uncertainty - Lognormal dispersion of the default barrier (dimensionless, typically about 0.3).
    /// @param mean_recovery - Mean global recovery rate as a fraction from 0 through 1.
    /// @returns The CreditGrades dynamics.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number. Range checks run
    /// when the dynamics are attached to a model.
    #[wasm_bindgen(js_name = creditGrades)]
    pub fn credit_grades(
        barrier_uncertainty: JsValue,
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
#[wasm_bindgen(js_name = MertonModel)]
pub struct JsMertonModel {
    pub(crate) inner: MertonModel,
}

json_round_trip!(JsMertonModel, MertonModel);

#[wasm_bindgen(js_class = MertonModel)]
impl JsMertonModel {
    /// Terminal-barrier Merton model with geometric-Brownian assets and no payout.
    /// @param asset_value - Current fair value of the firm's assets in monetary units; positive.
    /// @param asset_vol - Annualized volatility of firm-asset returns, as a decimal; positive.
    /// @param debt_barrier - Positive debt face value defining the default barrier.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal, such as 0.05 for 5%.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number, and a `validation`
    /// error if `assetValue`, `assetVol` or `debtBarrier` is not positive and
    /// finite or `riskFreeRate` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        asset_value: JsValue,
        asset_vol: JsValue,
        debt_barrier: JsValue,
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
    /// @param equity_value - Current market value of equity in the firm's monetary units.
    /// @param equity_vol - Annualized equity-return volatility as a decimal.
    /// @param total_debt - Total debt face value used as the default barrier.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal.
    /// @param payout_rate - Continuous dividend or payout yield on assets, as a decimal.
    /// @param maturity - Calibration horizon in years; positive and finite.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range and a
    /// `computation` error if the calibration does not converge.
    #[wasm_bindgen(js_name = fromEquity)]
    pub fn from_equity(
        equity_value: JsValue,
        equity_vol: JsValue,
        total_debt: JsValue,
        risk_free_rate: JsValue,
        payout_rate: JsValue,
        maturity: JsValue,
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
    /// @param cds_spread_bp - Target CDS par spread in basis points.
    /// @param recovery - Recovery rate at default as a fraction from 0 through 1.
    /// @param total_debt - Total debt face value in the firm's monetary units.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal.
    /// @param maturity - CDS maturity in years; positive and finite.
    /// @param asset_value - Assumed current firm asset value in monetary units.
    /// @param payout_rate - Continuous payout rate on assets, as a decimal.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range or the quote is
    /// unattainable or ambiguous.
    #[wasm_bindgen(js_name = fromCdsSpread)]
    pub fn from_cds_spread(
        cds_spread_bp: JsValue,
        recovery: JsValue,
        total_debt: JsValue,
        risk_free_rate: JsValue,
        maturity: JsValue,
        asset_value: JsValue,
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
    /// @param asset_value - Current fair value of the firm's assets in monetary units.
    /// @param asset_vol - Annualized volatility of firm-asset returns as a decimal; positive.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal; pass the physical asset return to calibrate against a real-world default rate.
    /// @param payout_rate - Continuous payout rate on assets as a decimal; it enters the calibration drift and is carried on the model.
    /// @param target_pd - Target cumulative default probability strictly between 0 and 1.
    /// @param maturity - Calibration horizon in years; positive and finite.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range.
    #[wasm_bindgen(js_name = fromTargetPd)]
    pub fn from_target_pd(
        asset_value: JsValue,
        asset_vol: JsValue,
        risk_free_rate: JsValue,
        payout_rate: JsValue,
        target_pd: JsValue,
        maturity: JsValue,
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
    /// @param short_term_debt - Liabilities due within one year, in the firm's monetary units.
    /// @param long_term_debt - Liabilities maturing beyond one year, in the same units; half of it enters the default point.
    /// @returns The default point in the same monetary units.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if either input is negative or non-finite,
    /// or the resulting default point is zero.
    #[wasm_bindgen(js_name = kmvDefaultPoint)]
    pub fn kmv_default_point(
        short_term_debt: JsValue,
        long_term_debt: JsValue,
    ) -> Result<f64, JsValue> {
        MertonModel::kmv_default_point(
            js_f64(&short_term_debt, "shortTermDebt")?,
            js_f64(&long_term_debt, "longTermDebt")?,
        )
        .map_err(to_js_err)
    }

    /// Model with an explicit barrier rule and asset dynamics.
    /// @param asset_value - Current fair value of the firm's assets in monetary units; positive.
    /// @param asset_vol - Annualized volatility of firm-asset returns as a decimal; positive.
    /// @param debt_barrier - Positive debt face value defining the default barrier.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal.
    /// @param payout_rate - Continuous payout rate on assets, as a decimal.
    /// @param barrier_type - `MertonBarrierType` handle selecting terminal or first-passage default.
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
        asset_value: JsValue,
        asset_vol: JsValue,
        debt_barrier: JsValue,
        risk_free_rate: JsValue,
        payout_rate: JsValue,
        barrier_type: &JsMertonBarrierType,
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
    /// @param equity_value - Current market value of equity in the firm's monetary units.
    /// @param equity_vol - Annualized equity-return volatility as a decimal.
    /// @param total_debt - Total debt face value in the firm's monetary units.
    /// @param risk_free_rate - Continuously compounded risk-free rate as a decimal.
    /// @param barrier_uncertainty - Lognormal dispersion of the CreditGrades default barrier.
    /// @param mean_recovery - Mean recovery rate at default as a fraction from 0 through 1.
    /// @returns The CreditGrades model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range.
    #[wasm_bindgen(js_name = creditGrades)]
    pub fn credit_grades(
        equity_value: JsValue,
        equity_vol: JsValue,
        total_debt: JsValue,
        risk_free_rate: JsValue,
        barrier_uncertainty: JsValue,
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

    /// Current fair value of the firm's assets, in monetary units.
    #[wasm_bindgen(getter, js_name = assetValue)]
    pub fn asset_value(&self) -> f64 {
        self.inner.asset_value()
    }

    /// Annualized asset-return volatility, as a decimal.
    #[wasm_bindgen(getter, js_name = assetVol)]
    pub fn asset_vol(&self) -> f64 {
        self.inner.asset_vol()
    }

    /// Debt face value that defines the default barrier, in monetary units.
    #[wasm_bindgen(getter, js_name = debtBarrier)]
    pub fn debt_barrier(&self) -> f64 {
        self.inner.debt_barrier()
    }

    /// Continuously compounded risk-free rate, as a decimal.
    #[wasm_bindgen(getter, js_name = riskFreeRate)]
    pub fn risk_free_rate(&self) -> f64 {
        self.inner.risk_free_rate()
    }

    /// Continuous payout rate on assets, as a decimal.
    #[wasm_bindgen(getter, js_name = payoutRate)]
    pub fn payout_rate(&self) -> f64 {
        self.inner.payout_rate()
    }

    /// Default-barrier monitoring rule of this model.
    #[wasm_bindgen(getter, js_name = barrierType)]
    pub fn barrier_type(&self) -> JsMertonBarrierType {
        JsMertonBarrierType {
            inner: *self.inner.barrier_type(),
        }
    }

    /// Asset-value dynamics of this model.
    #[wasm_bindgen(getter)]
    pub fn dynamics(&self) -> JsAssetDynamics {
        JsAssetDynamics {
            inner: *self.inner.dynamics(),
        }
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
    pub fn distance_to_default(&self, horizon: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.distance_to_default(js_f64(&horizon, "horizon")?))
    }

    /// Physical-measure (Moody's KMV) distance to default.
    /// @param asset_drift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
    /// @param horizon - Forward-looking horizon in years.
    /// @returns Distance to default in standard deviations under the physical drift.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `assetDrift` is not finite or the model
    /// uses driftless CreditGrades dynamics.
    #[wasm_bindgen(js_name = distanceToDefaultWithDrift)]
    pub fn distance_to_default_with_drift(
        &self,
        asset_drift: JsValue,
        horizon: JsValue,
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
    /// Throws a `TypeError` if `horizon` is not a number.
    #[wasm_bindgen(js_name = defaultProbability)]
    pub fn default_probability(&self, horizon: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.default_probability(js_f64(&horizon, "horizon")?))
    }

    /// Risk-neutral default probabilities over a grid of horizons.
    ///
    /// Python returns the same values as a pandas Series labelled by horizon.
    /// @param horizons - Horizons in years, as a `number[]` or `Float64Array`.
    /// @returns One default probability per horizon, in input order.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `horizons` is not an array of numbers.
    #[wasm_bindgen(js_name = defaultProbabilities)]
    pub fn default_probabilities(&self, horizons: JsValue) -> Result<Box<[f64]>, JsValue> {
        let horizons = js_f64_seq(&horizons, "horizons")?;
        Ok(self
            .inner
            .default_probabilities(&horizons)
            .into_boxed_slice())
    }

    /// Physical-measure (Moody's KMV) default probability, the theoretical EDF.
    /// @param asset_drift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
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
        asset_drift: JsValue,
        horizon: JsValue,
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
    pub fn implied_spread(&self, horizon: JsValue, recovery: JsValue) -> Result<f64, JsValue> {
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
    pub fn debt_spread(&self, horizon: JsValue) -> Result<f64, JsValue> {
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
    pub fn cds_par_spread(&self, maturity: JsValue, recovery: JsValue) -> Result<f64, JsValue> {
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
    #[wasm_bindgen(js_name = tryImpliedEquity)]
    pub fn try_implied_equity(&self, horizon: JsValue) -> Result<Box<[f64]>, JsValue> {
        let (equity, equity_vol) = self
            .inner
            .try_implied_equity(js_f64(&horizon, "horizon")?)
            .map_err(to_js_err)?;
        Ok(Box::new([equity, equity_vol]))
    }

    /// Bootstrap a piecewise-constant hazard curve from the structural default
    /// probabilities.
    /// @param id - Identifier assigned to the hazard curve.
    /// @param base_date - Valuation date in ISO-8601 form, such as `"2025-01-15"`.
    /// @param tenors - Tenor grid in years as a `number[]` or `Float64Array`; non-empty, positive and distinct.
    /// @param recovery - Recovery rate as a fraction from 0 through 1; must equal the model's `mean_recovery` under CreditGrades dynamics.
    /// @param day_count - Day-count convention the curve uses for year fractions, such as `"act_365f"` or `"act_360"`.
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
        id: JsValue,
        base_date: JsValue,
        tenors: JsValue,
        recovery: JsValue,
        day_count: JsValue,
    ) -> Result<JsHazardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = parse_iso_date(&js_string(&base_date, "baseDate")?)?;
        let tenors = js_f64_seq(&tenors, "tenors")?;
        let recovery = js_f64(&recovery, "recovery")?;
        let day_count: DayCount = js_string(&day_count, "dayCount")?
            .parse()
            .map_err(to_js_err)?;
        self.inner
            .to_hazard_curve(&id, base_date, &tenors, recovery, day_count)
            .map(|curve| JsHazardCurve {
                inner: Arc::new(curve),
            })
            .map_err(to_js_err)
    }

    /// Simulate firm-asset paths by Monte Carlo.
    /// @param num_paths - Number of paths to simulate; a safe non-negative integer.
    /// @param num_steps - Number of time steps per path; at least 1.
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
        num_paths: JsValue,
        num_steps: JsValue,
        horizon: JsValue,
        seed: JsValue,
        antithetic: JsValue,
    ) -> Result<JsSimulatedPaths, JsValue> {
        let num_paths: usize = js_uint(&num_paths, "numPaths")?;
        let num_steps: usize = js_uint(&num_steps, "numSteps")?;
        let horizon = js_f64(&horizon, "horizon")?;
        let seed = js_u64(&seed, "seed")?;
        let antithetic = js_bool(&antithetic, "antithetic")?;
        self.inner
            .simulate_paths_seeded(num_paths, num_steps, horizon, seed, antithetic)
            .map(|inner| JsSimulatedPaths { inner })
            .map_err(to_js_err)
    }
}

/// Monte Carlo asset-value paths from `MertonModel.simulatePaths`.
///
/// `assetValues` is row-major: path `p` occupies indices
/// `p * valuesPerPath .. (p + 1) * valuesPerPath`, where
/// `valuesPerPath == numSteps + 1` (the grid includes `t = 0`).
#[wasm_bindgen(js_name = SimulatedPaths)]
pub struct JsSimulatedPaths {
    pub(crate) inner: SimulatedPaths,
}

json_round_trip!(JsSimulatedPaths, SimulatedPaths);

#[wasm_bindgen(js_class = SimulatedPaths)]
impl JsSimulatedPaths {
    /// Time grid in years from 0 to the simulation horizon.
    #[wasm_bindgen(getter)]
    pub fn times(&self) -> Box<[f64]> {
        self.inner.times.clone().into_boxed_slice()
    }

    /// Asset values in row-major order (path by path).
    #[wasm_bindgen(getter, js_name = assetValues)]
    pub fn asset_values(&self) -> Box<[f64]> {
        self.inner.asset_values.clone().into_boxed_slice()
    }

    /// Number of simulated paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Number of time steps between grid points.
    #[wasm_bindgen(getter, js_name = numSteps)]
    pub fn num_steps(&self) -> usize {
        self.inner.num_steps
    }

    /// Number of stored values per path (`numSteps + 1`, including `t = 0`).
    #[wasm_bindgen(getter, js_name = valuesPerPath)]
    pub fn values_per_path(&self) -> usize {
        self.inner.values_per_path()
    }

    /// One asset value by path and time-grid index.
    /// @param path_idx - Zero-based path index.
    /// @param time_idx - Zero-based time-grid index (index 0 is `t = 0`).
    /// @returns The asset value, or `undefined` when either index is out of range.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an index is not a safe non-negative integer.
    pub fn get(&self, path_idx: JsValue, time_idx: JsValue) -> Result<Option<f64>, JsValue> {
        let path_idx: usize = js_uint(&path_idx, "pathIdx")?;
        let time_idx: usize = js_uint(&time_idx, "timeIdx")?;
        Ok(self.inner.get(path_idx, time_idx))
    }

    /// The contiguous asset-value row of one path.
    /// @param path_idx - Zero-based path index.
    /// @returns The path's values on the time grid, or `undefined` when the index is out of range.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `pathIdx` is not a safe non-negative integer.
    pub fn path(&self, path_idx: JsValue) -> Result<Option<Box<[f64]>>, JsValue> {
        let path_idx: usize = js_uint(&path_idx, "pathIdx")?;
        Ok(self.inner.path(path_idx).map(Box::from))
    }

    /// Materialize the paths as one row of values per path.
    /// @returns An array with one `number[]` per path, each of length `valuesPerPath`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = toNested)]
    pub fn to_nested(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_nested())
    }
}

/// Notional-dependent recovery specification for PIK-accreting instruments.
#[wasm_bindgen(js_name = DynamicRecoverySpec)]
pub struct JsDynamicRecoverySpec {
    pub(crate) inner: DynamicRecoverySpec,
}

json_round_trip!(JsDynamicRecoverySpec, DynamicRecoverySpec);

#[wasm_bindgen(js_class = DynamicRecoverySpec)]
impl JsDynamicRecoverySpec {
    /// Recovery that does not depend on notional.
    /// @param recovery - Recovery rate at default as a fraction of par from 0 through 1.
    /// @returns The constant recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `recovery` is outside `[0, 1]`.
    pub fn constant(recovery: JsValue) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::constant(js_f64(&recovery, "recovery")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Recovery `R0 * N0 / N`: fixed recoverable value spread over a larger claim.
    /// @param base_recovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param base_notional - Positive reference notional `N0` in the instrument's currency.
    /// @returns The inverse-linear recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = inverseLinear)]
    pub fn inverse_linear(
        base_recovery: JsValue,
        base_notional: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::inverse_linear(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Recovery `R0 * (N0 / N)^exponent`.
    /// @param base_recovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param base_notional - Positive reference notional `N0` in the instrument's currency.
    /// @param exponent - Non-negative power applied to the notional ratio.
    /// @returns The inverse-power recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = inversePower)]
    pub fn inverse_power(
        base_recovery: JsValue,
        base_notional: JsValue,
        exponent: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::inverse_power(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&exponent, "exponent")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Inverse-linear recovery that never falls below a floor.
    /// @param base_recovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param base_notional - Positive reference notional `N0` in the instrument's currency.
    /// @param floor - Minimum recovery as a fraction in `[0, baseRecovery]`.
    /// @returns The floored-inverse recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = flooredInverse)]
    pub fn floored_inverse(
        base_recovery: JsValue,
        base_notional: JsValue,
        floor: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::floored_inverse(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&floor, "floor")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Recovery that declines linearly in the notional ratio down to a floor.
    /// @param base_recovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param base_notional - Positive reference notional `N0` in the instrument's currency.
    /// @param slope - Non-negative recovery lost per unit increase of `N / N0 - 1`.
    /// @param floor - Minimum recovery as a fraction in `[0, baseRecovery]`.
    /// @returns The linear-decline recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = linearDecline)]
    pub fn linear_decline(
        base_recovery: JsValue,
        base_notional: JsValue,
        slope: JsValue,
        floor: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::linear_decline(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&slope, "slope")?,
            js_f64(&floor, "floor")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Canonical name of the active recovery model: `"constant"`,
    /// `"inverse_linear"`, `"inverse_power"`, `"floored_inverse"` or
    /// `"linear_decline"`.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> Result<String, JsValue> {
        serde_tag(self.inner.model())
    }

    /// Base (reference) recovery rate `R0`, as a fraction from 0 through 1.
    #[wasm_bindgen(getter, js_name = baseRecovery)]
    pub fn base_recovery(&self) -> f64 {
        self.inner.base_recovery()
    }

    /// Base (reference) notional `N0` the recovery mapping is anchored to.
    #[wasm_bindgen(getter, js_name = baseNotional)]
    pub fn base_notional(&self) -> f64 {
        self.inner.base_notional()
    }

    /// Notional-to-recovery mapping in canonical JSON form: a tag string for
    /// the parameterless models, or a single-key object carrying the parameters.
    #[wasm_bindgen(getter)]
    pub fn model(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.model())
    }

    /// Recovery rate at a given current notional, clamped to `[0, baseRecovery]`.
    /// @param notional - Current (accreted) notional in the instrument's currency.
    /// @returns Recovery rate as a fraction of par.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `notional` is not a number.
    #[wasm_bindgen(js_name = recoveryAtNotional)]
    pub fn recovery_at_notional(&self, notional: JsValue) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .recovery_at_notional(js_f64(&notional, "notional")?))
    }
}

/// Leverage-dependent hazard-rate feedback for PIK-accreting instruments.
#[wasm_bindgen(js_name = EndogenousHazardSpec)]
pub struct JsEndogenousHazardSpec {
    pub(crate) inner: EndogenousHazardSpec,
}

json_round_trip!(JsEndogenousHazardSpec, EndogenousHazardSpec);

#[wasm_bindgen(js_class = EndogenousHazardSpec)]
impl JsEndogenousHazardSpec {
    /// Power-law mapping `lambda(L) = baseHazard * (L / baseLeverage)^exponent`.
    /// @param base_hazard - Annualized hazard rate at the base leverage, as a decimal; non-negative.
    /// @param base_leverage - Positive reference debt-to-assets leverage ratio.
    /// @param exponent - Power-law exponent applied to the leverage ratio.
    /// @returns The power-law hazard specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = powerLaw)]
    pub fn power_law(
        base_hazard: JsValue,
        base_leverage: JsValue,
        exponent: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::power_law(
            js_f64(&base_hazard, "baseHazard")?,
            js_f64(&base_leverage, "baseLeverage")?,
            js_f64(&exponent, "exponent")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Exponential mapping `lambda(L) = baseHazard * exp(sensitivity * (L - baseLeverage))`.
    /// @param base_hazard - Annualized hazard rate at the base leverage, as a decimal; non-negative.
    /// @param base_leverage - Positive reference debt-to-assets leverage ratio.
    /// @param sensitivity - Log-hazard change per unit of leverage above the base.
    /// @returns The exponential hazard specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    pub fn exponential(
        base_hazard: JsValue,
        base_leverage: JsValue,
        sensitivity: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::exponential(
            js_f64(&base_hazard, "baseHazard")?,
            js_f64(&base_leverage, "baseLeverage")?,
            js_f64(&sensitivity, "sensitivity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Piecewise-linear leverage-to-hazard table.
    /// @param leverage_points - Strictly increasing leverage ratios, at least two, as a `number[]` or `Float64Array`.
    /// @param hazard_points - Annualized hazard rates at those leverage points; same length, non-negative.
    /// @returns The tabular hazard specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the arrays differ in length, hold fewer
    /// than two points, are not strictly increasing in leverage, or contain
    /// non-finite or negative values.
    pub fn tabular(
        leverage_points: JsValue,
        hazard_points: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::tabular(
            js_f64_seq(&leverage_points, "leveragePoints")?,
            js_f64_seq(&hazard_points, "hazardPoints")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Canonical name of the active mapping: `"power_law"`, `"exponential"` or
    /// `"tabular"`.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> Result<String, JsValue> {
        serde_tag(self.inner.leverage_hazard_map())
    }

    /// Base (reference) hazard rate, annualized, as a decimal.
    #[wasm_bindgen(getter, js_name = baseHazardRate)]
    pub fn base_hazard_rate(&self) -> f64 {
        self.inner.base_hazard_rate()
    }

    /// Base (reference) leverage the hazard mapping is anchored to.
    #[wasm_bindgen(getter, js_name = baseLeverage)]
    pub fn base_leverage(&self) -> f64 {
        self.inner.base_leverage()
    }

    /// Leverage-to-hazard mapping in canonical JSON form: a single-key object
    /// (`power_law`, `exponential`, `tabular`) carrying that model's parameters.
    #[wasm_bindgen(getter, js_name = leverageHazardMap)]
    pub fn leverage_hazard_map(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.leverage_hazard_map())
    }

    /// Annualized hazard rate at a leverage ratio, floored at zero.
    /// @param leverage - Debt-to-assets leverage ratio; non-negative.
    /// @returns Hazard rate per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `leverage` is not a number.
    #[wasm_bindgen(js_name = hazardAtLeverage)]
    pub fn hazard_at_leverage(&self, leverage: JsValue) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .hazard_at_leverage(js_f64(&leverage, "leverage")?))
    }

    /// Hazard rate after PIK accretion, with leverage `accretedNotional / assetValue`.
    /// @param accreted_notional - Notional outstanding after PIK accrual, in the instrument's currency.
    /// @param asset_value - Firm asset value in the same currency; strictly positive.
    /// @returns Hazard rate per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(js_name = hazardAfterPikAccrual)]
    pub fn hazard_after_pik_accrual(
        &self,
        accreted_notional: JsValue,
        asset_value: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self.inner.hazard_after_pik_accrual(
            js_f64(&accreted_notional, "accretedNotional")?,
            js_f64(&asset_value, "assetValue")?,
        ))
    }
}

/// PIK/cash toggle exercise rule: threshold, stochastic (logistic) or nested
/// optimal exercise.
#[wasm_bindgen(js_name = ToggleExerciseModel)]
pub struct JsToggleExerciseModel {
    pub(crate) inner: ToggleExerciseModel,
}

json_round_trip!(JsToggleExerciseModel, ToggleExerciseModel);

#[wasm_bindgen(js_class = ToggleExerciseModel)]
impl JsToggleExerciseModel {
    /// Deterministic rule: PIK when `variable` is `direction` the threshold.
    /// @param variable - Credit-state variable observed: `"hazard_rate"`, `"distance_to_default"` or `"leverage"`.
    /// @param threshold - Trigger level in the variable's own units (annualized decimal hazard, standard deviations, or leverage ratio).
    /// @param direction - `"above"` to PIK when the variable exceeds the threshold, `"below"` when it falls under it.
    /// @returns The threshold rule.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown variable or direction, or a
    /// non-finite threshold.
    pub fn threshold(
        variable: JsValue,
        threshold: JsValue,
        direction: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        let variable = js_string(&variable, "variable")?
            .parse::<CreditStateVariable>()
            .map_err(to_js_err)?;
        let threshold = js_f64(&threshold, "threshold")?;
        let direction = js_string(&direction, "direction")?
            .parse::<ThresholdDirection>()
            .map_err(to_js_err)?;
        ToggleExerciseModel::threshold(variable, threshold, direction)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Stochastic rule: PIK with probability `logistic(intercept + sensitivity * x)`.
    /// @param variable - Credit-state variable `x` observed: `"hazard_rate"`, `"distance_to_default"` or `"leverage"`.
    /// @param intercept - Logit intercept (dimensionless).
    /// @param sensitivity - Logit slope per unit of the variable.
    /// @returns The stochastic rule.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown variable or a non-finite
    /// intercept or sensitivity.
    pub fn stochastic(
        variable: JsValue,
        intercept: JsValue,
        sensitivity: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        let variable = js_string(&variable, "variable")?
            .parse::<CreditStateVariable>()
            .map_err(to_js_err)?;
        ToggleExerciseModel::stochastic(
            variable,
            js_f64(&intercept, "intercept")?,
            js_f64(&sensitivity, "sensitivity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Nested-Monte-Carlo optimal exercise rule.
    /// @param nested_paths - Inner simulation paths per decision date; a positive safe integer.
    /// @param equity_discount_rate - Continuously compounded equity discount rate, as a decimal.
    /// @param asset_vol - Annualized asset volatility as a decimal; non-negative.
    /// @param risk_free_rate - Continuously compounded risk-free rate, as a decimal.
    /// @param horizon - Inner simulation horizon in years; positive and finite.
    /// @returns The optimal-exercise rule.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `nestedPaths` is not a safe non-negative integer,
    /// and a `validation` error if it is zero, a rate is non-finite, `assetVol`
    /// is negative, or `horizon` is not positive and finite.
    pub fn optimal(
        nested_paths: JsValue,
        equity_discount_rate: JsValue,
        asset_vol: JsValue,
        risk_free_rate: JsValue,
        horizon: JsValue,
    ) -> Result<JsToggleExerciseModel, JsValue> {
        ToggleExerciseModel::optimal(
            js_uint(&nested_paths, "nestedPaths")?,
            js_f64(&equity_discount_rate, "equityDiscountRate")?,
            js_f64(&asset_vol, "assetVol")?,
            js_f64(&risk_free_rate, "riskFreeRate")?,
            js_f64(&horizon, "horizon")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Whether the rule elects PIK for a credit state given one uniform draw.
    ///
    /// Threshold rules ignore `u`; stochastic rules elect PIK when `u` is
    /// below the logistic probability; optimal-exercise rules run their nested
    /// simulation with a seed derived from `u`, so equal draws give equal
    /// decisions.
    /// @param state - `CreditState` JSON or plain object (`hazard_rate`, `distance_to_default`, `leverage`, `accreted_notional`, `coupon_due`, `asset_value`).
    /// @param u - Uniform draw in `[0, 1)`.
    /// @returns `true` when the rule elects to pay in kind.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `state` is neither a string nor a plain object
    /// or `u` is not a number, and a `validation` error if `state` is malformed.
    #[wasm_bindgen(js_name = shouldPikWithUniform)]
    pub fn should_pik_with_uniform(&self, state: JsValue, u: JsValue) -> Result<bool, JsValue> {
        let state: CreditState = from_js_json(&state, "state")?;
        Ok(self.inner.should_pik_with_uniform(&state, js_f64(&u, "u")?))
    }

    /// Which rule this model carries: `"threshold"`, `"stochastic"` or
    /// `"optimal_exercise"` (the canonical serde tag).
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> Result<String, JsValue> {
        serde_tag(&self.inner)
    }

    /// Parameters of the active rule as a plain object in canonical JSON form.
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> Result<JsValue, JsValue> {
        match &self.inner {
            ToggleExerciseModel::Threshold(spec) => to_js_value(spec),
            ToggleExerciseModel::Stochastic(spec) => to_js_value(spec),
            ToggleExerciseModel::OptimalExercise(spec) => to_js_value(spec),
        }
    }
}
