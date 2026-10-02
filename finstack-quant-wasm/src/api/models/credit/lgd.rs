//! WASM bindings for loss-given-default models: seniority Beta recovery,
//! workout LGD, downturn adjustments and exposure at default.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/lgd.rs`. The data
//! types `CollateralPiece` and `WorkoutLgdResult` cross the boundary as plain
//! objects in their canonical serde form.

use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_string, js_u64, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::lgd::{
    self, BetaRecovery, CollateralPiece, CreditConversionFactor, DownturnLgd, EadCalculator,
    WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder,
};
use wasm_bindgen::prelude::*;

/// Re-validate a deserialized collateral piece through the Rust constructor.
fn checked_piece(piece: CollateralPiece) -> Result<CollateralPiece, JsValue> {
    CollateralPiece::new(piece.collateral_type, piece.book_value, piece.haircut).map_err(to_js_err)
}

/// Beta-distributed recovery rate, parameterized by its mean and standard deviation.
#[wasm_bindgen(js_name = BetaRecovery)]
pub struct JsBetaRecovery {
    pub(crate) inner: BetaRecovery,
}

json_round_trip!(JsBetaRecovery, BetaRecovery);

#[wasm_bindgen(js_class = BetaRecovery)]
impl JsBetaRecovery {
    /// Beta recovery distribution matched to a mean and standard deviation.
    /// @param mean - Mean recovery rate as a fraction strictly between 0 and 1.
    /// @param std_dev - Recovery standard deviation as a fraction; positive and below `sqrt(mean * (1 - mean))`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the moments are non-finite or do not
    /// define a Beta distribution.
    #[wasm_bindgen(constructor)]
    pub fn new(mean: JsValue, std_dev: JsValue) -> Result<JsBetaRecovery, JsValue> {
        BetaRecovery::new(js_f64(&mean, "mean")?, js_f64(&std_dev, "stdDev")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Mean recovery rate, as a fraction.
    #[wasm_bindgen(getter)]
    pub fn mean(&self) -> f64 {
        self.inner.mean()
    }

    /// Standard deviation of the recovery rate, as a fraction.
    #[wasm_bindgen(getter, js_name = stdDev)]
    pub fn std_dev(&self) -> f64 {
        self.inner.std_dev()
    }

    /// Beta shape parameter `alpha` implied by the moments.
    #[wasm_bindgen(getter)]
    pub fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    /// Beta shape parameter `beta` implied by the moments.
    #[wasm_bindgen(getter, js_name = betaParam)]
    pub fn beta_param(&self) -> f64 {
        self.inner.beta_param()
    }

    /// Variance of the recovery rate.
    #[wasm_bindgen(getter)]
    pub fn variance(&self) -> f64 {
        self.inner.variance()
    }

    /// Mode of the distribution, or `undefined` when it has no interior mode.
    #[wasm_bindgen(getter)]
    pub fn mode(&self) -> Option<f64> {
        self.inner.mode()
    }

    /// Mean loss given default, `1 - mean`.
    #[wasm_bindgen(getter, js_name = meanLgd)]
    pub fn mean_lgd(&self) -> f64 {
        self.inner.mean_lgd()
    }

    /// Recovery-rate quantile at a probability level.
    /// @param p - Probability level from 0 through 1.
    /// @returns The recovery rate below which a fraction `p` of outcomes fall.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `p` is outside `[0, 1]`.
    pub fn quantile(&self, p: JsValue) -> Result<f64, JsValue> {
        self.inner.quantile(js_f64(&p, "p")?).map_err(to_js_err)
    }

    /// Draw recovery rates with a seeded generator.
    /// @param n_samples - Number of draws; a safe non-negative integer.
    /// @param seed - Seed for reproducible draws, as a safe integer or `bigint`.
    /// @returns The sampled recovery rates.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if a count is not a safe integer, and a
    /// `validation` error if sampling fails.
    #[wasm_bindgen(js_name = sampleSeeded)]
    pub fn sample_seeded(&self, n_samples: JsValue, seed: JsValue) -> Result<Box<[f64]>, JsValue> {
        let n_samples: usize = js_uint(&n_samples, "nSamples")?;
        let seed = js_u64(&seed, "seed")?;
        self.inner
            .sample_seeded(n_samples, seed)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }
}

/// Historical Beta recovery distribution for a debt seniority class.
/// @param seniority - Seniority class label such as `"senior_secured"`, `"senior_unsecured"` or `"subordinated"`.
/// @param rating_agency - Agency whose calibration to use, such as `"moodys"` or `"sp"`; omitted uses the registry default.
/// @returns The calibrated `BetaRecovery` handle.
///
/// # Errors
///
/// Throws a `validation` error if the seniority or agency is not recognized.
#[wasm_bindgen(js_name = seniorityRecoveryStats)]
pub fn seniority_recovery_stats(
    seniority: JsValue,
    rating_agency: Option<JsValue>,
) -> Result<JsBetaRecovery, JsValue> {
    let seniority = js_string(&seniority, "seniority")?;
    match js_opt_string(rating_agency.as_ref(), "ratingAgency")? {
        Some(agency) => lgd::seniority_recovery_stats(&seniority, &agency),
        None => lgd::seniority_recovery_stats_default(&seniority),
    }
    .map(|inner| JsBetaRecovery { inner })
    .map_err(to_js_err)
}

/// Draw Beta-distributed recovery rates from a mean and standard deviation.
/// @param mean - Mean recovery rate as a fraction strictly between 0 and 1.
/// @param std - Recovery standard deviation as a fraction.
/// @param n_samples - Number of draws; a safe non-negative integer.
/// @param seed - Seed for reproducible draws, as a safe integer or `bigint`.
/// @returns The sampled recovery rates.
///
/// # Errors
///
/// Throws a `validation` error if the moments do not define a Beta
/// distribution.
#[wasm_bindgen(js_name = betaRecoverySample)]
pub fn beta_recovery_sample(
    mean: JsValue,
    std: JsValue,
    n_samples: JsValue,
    seed: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    lgd::beta_recovery_sample(
        js_f64(&mean, "mean")?,
        js_f64(&std, "std")?,
        js_uint(&n_samples, "nSamples")?,
        js_u64(&seed, "seed")?,
    )
    .map(Vec::into_boxed_slice)
    .map_err(to_js_err)
}

/// Quantile of a Beta recovery distribution given its mean and standard deviation.
/// @param mean - Mean recovery rate as a fraction strictly between 0 and 1.
/// @param std - Recovery standard deviation as a fraction.
/// @param q - Probability level from 0 through 1.
/// @returns The recovery rate at probability level `q`.
///
/// # Errors
///
/// Throws a `validation` error if the moments or `q` are out of range.
#[wasm_bindgen(js_name = betaRecoveryQuantile)]
pub fn beta_recovery_quantile(mean: JsValue, std: JsValue, q: JsValue) -> Result<f64, JsValue> {
    lgd::beta_recovery_quantile(
        js_f64(&mean, "mean")?,
        js_f64(&std, "std")?,
        js_f64(&q, "q")?,
    )
    .map_err(to_js_err)
}

/// Direct and indirect workout cost rates, each a fraction of exposure at default.
#[wasm_bindgen(js_name = WorkoutCosts)]
#[derive(Clone, Copy)]
pub struct JsWorkoutCosts {
    pub(crate) inner: WorkoutCosts,
}

json_round_trip!(JsWorkoutCosts, WorkoutCosts);

#[wasm_bindgen(js_class = WorkoutCosts)]
impl JsWorkoutCosts {
    /// Workout cost rates.
    /// @param direct_cost_rate - Direct costs (legal, administrative) as a fraction of EAD; non-negative.
    /// @param indirect_cost_rate - Indirect costs (internal overhead) as a fraction of EAD; non-negative.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a rate is negative or non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        direct_cost_rate: JsValue,
        indirect_cost_rate: JsValue,
    ) -> Result<JsWorkoutCosts, JsValue> {
        WorkoutCosts::new(
            js_f64(&direct_cost_rate, "directCostRate")?,
            js_f64(&indirect_cost_rate, "indirectCostRate")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// No workout costs.
    /// @returns Cost rates of zero.
    pub fn zero() -> JsWorkoutCosts {
        Self {
            inner: WorkoutCosts::zero(),
        }
    }

    /// Standard workout costs from the embedded credit-assumption registry.
    /// @returns The registry's default direct and indirect cost rates.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry cannot supply the defaults.
    pub fn standard() -> Result<JsWorkoutCosts, JsValue> {
        WorkoutCosts::standard()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Direct cost rate, as a fraction of EAD.
    #[wasm_bindgen(getter, js_name = directCostRate)]
    pub fn direct_cost_rate(&self) -> f64 {
        self.inner.direct_cost_rate
    }

    /// Indirect cost rate, as a fraction of EAD.
    #[wasm_bindgen(getter, js_name = indirectCostRate)]
    pub fn indirect_cost_rate(&self) -> f64 {
        self.inner.indirect_cost_rate
    }

    /// Direct plus indirect cost rate, as a fraction of EAD.
    #[wasm_bindgen(getter, js_name = totalRate)]
    pub fn total_rate(&self) -> f64 {
        self.inner.total_rate()
    }
}

/// Workout LGD model: collateral waterfall, resolution costs and time value.
#[wasm_bindgen(js_name = WorkoutLgd)]
pub struct JsWorkoutLgd {
    pub(crate) inner: WorkoutLgd,
}

json_round_trip!(JsWorkoutLgd, WorkoutLgd);

#[wasm_bindgen(js_class = WorkoutLgd)]
impl JsWorkoutLgd {
    /// Start a fluent `WorkoutLgdBuilder`.
    /// @returns An empty builder; workout years, discount rate and costs default to the registry values.
    pub fn builder() -> JsWorkoutLgdBuilder {
        JsWorkoutLgdBuilder {
            inner: WorkoutLgd::builder(),
        }
    }

    /// Net recovery, LGD and recovery rate for an exposure.
    /// @param ead - Exposure at default in monetary units; positive.
    /// @returns The `WorkoutLgdResult` object (`net_recovery`, `lgd`, `recovery_rate`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `ead` is not positive and finite.
    pub fn evaluate(&self, ead: JsValue) -> Result<JsValue, JsValue> {
        let result = self
            .inner
            .evaluate(js_f64(&ead, "ead")?)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }

    /// Loss given default for an exposure, as a fraction of EAD in `[0, 1]`.
    /// @param ead - Exposure at default in monetary units; positive.
    /// @returns The workout LGD.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `ead` is not positive and finite.
    pub fn lgd(&self, ead: JsValue) -> Result<f64, JsValue> {
        self.inner.lgd(js_f64(&ead, "ead")?).map_err(to_js_err)
    }

    /// Discounted net recovery after costs, in monetary units.
    /// @param ead - Exposure at default in monetary units; positive.
    /// @returns The net recovery amount.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `ead` is not positive and finite.
    #[wasm_bindgen(js_name = netRecovery)]
    pub fn net_recovery(&self, ead: JsValue) -> Result<f64, JsValue> {
        self.inner
            .net_recovery(js_f64(&ead, "ead")?)
            .map_err(to_js_err)
    }

    /// Net recovery as a fraction of EAD.
    /// @param ead - Exposure at default in monetary units; positive.
    /// @returns The recovery rate in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `ead` is not positive and finite.
    #[wasm_bindgen(js_name = recoveryRate)]
    pub fn recovery_rate(&self, ead: JsValue) -> Result<f64, JsValue> {
        self.inner
            .recovery_rate(js_f64(&ead, "ead")?)
            .map_err(to_js_err)
    }

    /// Collateral pieces in waterfall order, as `CollateralPiece` objects.
    #[wasm_bindgen(getter)]
    pub fn collateral(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.collateral())
    }

    /// Expected workout duration in years.
    #[wasm_bindgen(getter, js_name = workoutYears)]
    pub fn workout_years(&self) -> f64 {
        self.inner.workout_years()
    }

    /// Annual discount rate applied over the workout period, as a decimal.
    #[wasm_bindgen(getter, js_name = discountRate)]
    pub fn discount_rate(&self) -> f64 {
        self.inner.discount_rate()
    }

    /// Direct and indirect resolution cost rates.
    #[wasm_bindgen(getter)]
    pub fn costs(&self) -> JsWorkoutCosts {
        JsWorkoutCosts {
            inner: *self.inner.costs(),
        }
    }
}

/// Fluent builder for `WorkoutLgd`. Each setter consumes the builder and
/// returns the updated one, so chain the calls.
#[wasm_bindgen(js_name = WorkoutLgdBuilder)]
pub struct JsWorkoutLgdBuilder {
    inner: WorkoutLgdBuilder,
}

#[wasm_bindgen(js_class = WorkoutLgdBuilder)]
impl JsWorkoutLgdBuilder {
    /// Add one collateral piece to the waterfall.
    /// @param piece - `CollateralPiece` object or JSON: `collateral_type` (for example `"real_estate"`), `book_value` (non-negative) and `haircut` in `[0, 1]`.
    /// @returns The updated builder.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the piece is malformed, its book value
    /// is negative or its haircut is outside `[0, 1]`.
    pub fn collateral(self, piece: JsValue) -> Result<JsWorkoutLgdBuilder, JsValue> {
        let piece = checked_piece(from_js_json(&piece, "piece")?)?;
        Ok(Self {
            inner: self.inner.collateral(piece),
        })
    }

    /// Add several collateral pieces.
    /// @param pieces - Array of `CollateralPiece` objects, or its JSON text.
    /// @returns The updated builder.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if any piece is malformed or out of range.
    #[wasm_bindgen(js_name = collateralPieces)]
    pub fn collateral_pieces(self, pieces: JsValue) -> Result<JsWorkoutLgdBuilder, JsValue> {
        let pieces = from_js_json::<Vec<CollateralPiece>>(&pieces, "pieces")?
            .into_iter()
            .map(checked_piece)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inner: self.inner.collateral_pieces(pieces),
        })
    }

    /// Set the expected workout duration.
    /// @param years - Workout duration in years; non-negative (registry default when not set).
    /// @returns The updated builder.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `years` is not a number; the range is checked
    /// by `build`.
    #[wasm_bindgen(js_name = workoutYears)]
    pub fn workout_years(self, years: JsValue) -> Result<JsWorkoutLgdBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.workout_years(js_f64(&years, "years")?),
        })
    }

    /// Set the discount rate applied over the workout period.
    /// @param rate - Annual discount rate as a decimal; non-negative (registry default when not set).
    /// @returns The updated builder.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `rate` is not a number; the range is checked by
    /// `build`.
    #[wasm_bindgen(js_name = discountRate)]
    pub fn discount_rate(self, rate: JsValue) -> Result<JsWorkoutLgdBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.discount_rate(js_f64(&rate, "rate")?),
        })
    }

    /// Set the workout cost rates.
    /// @param costs - `WorkoutCosts` handle (registry default costs when not set).
    /// @returns The updated builder.
    pub fn costs(self, costs: &JsWorkoutCosts) -> JsWorkoutLgdBuilder {
        Self {
            inner: self.inner.costs(costs.inner),
        }
    }

    /// Validate the inputs and build the model.
    /// @returns The `WorkoutLgd` handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the workout years or discount rate is
    /// negative or non-finite.
    pub fn build(self) -> Result<JsWorkoutLgd, JsValue> {
        self.inner
            .build()
            .map(|inner| JsWorkoutLgd { inner })
            .map_err(to_js_err)
    }
}

/// One-call workout LGD from collateral tuples and cost assumptions.
/// @param ead - Exposure at default in monetary units; positive.
/// @param collateral - Array of `[collateralType, bookValue, haircut]` tuples, such as `[["real_estate", 800000, 0.3]]`.
/// @param direct_cost_pct - Direct workout costs as a fraction of EAD.
/// @param indirect_cost_pct - Indirect workout costs as a fraction of EAD.
/// @param time_to_resolution_years - Expected workout duration in years.
/// @param discount_rate - Annual discount rate over the workout period, as a decimal.
/// @returns The `WorkoutLgdResult` object (`net_recovery`, `lgd`, `recovery_rate`).
///
/// # Errors
///
/// Throws a `validation` error if any input is out of range or a collateral
/// type is not recognized.
#[wasm_bindgen(js_name = workoutLgd)]
pub fn workout_lgd(
    ead: JsValue,
    collateral: JsValue,
    direct_cost_pct: JsValue,
    indirect_cost_pct: JsValue,
    time_to_resolution_years: JsValue,
    discount_rate: JsValue,
) -> Result<JsValue, JsValue> {
    let collateral: Vec<(String, f64, f64)> = from_js_json(&collateral, "collateral")?;
    let result = lgd::workout_lgd(
        js_f64(&ead, "ead")?,
        collateral,
        js_f64(&direct_cost_pct, "directCostPct")?,
        js_f64(&indirect_cost_pct, "indirectCostPct")?,
        js_f64(&time_to_resolution_years, "timeToResolutionYears")?,
        js_f64(&discount_rate, "discountRate")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Downturn LGD adjustment: Frye-Jacobs style stress or a regulatory add-on with a floor.
#[wasm_bindgen(js_name = DownturnLgd)]
pub struct JsDownturnLgd {
    pub(crate) inner: DownturnLgd,
}

#[wasm_bindgen(js_class = DownturnLgd)]
impl JsDownturnLgd {
    /// Stressed-factor downturn adjustment.
    /// @param asset_correlation - Asset correlation with the systematic factor, from 0 to 1.
    /// @param lgd_sensitivity - Sensitivity of LGD to the systematic factor; non-negative.
    /// @param stress_quantile - Stress quantile of the systematic factor, strictly between 0 and 1 (for example 0.999).
    /// @returns The stressed downturn adjustment.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on out-of-range inputs.
    pub fn stressed(
        asset_correlation: JsValue,
        lgd_sensitivity: JsValue,
        stress_quantile: JsValue,
    ) -> Result<JsDownturnLgd, JsValue> {
        DownturnLgd::stressed(
            js_f64(&asset_correlation, "assetCorrelation")?,
            js_f64(&lgd_sensitivity, "lgdSensitivity")?,
            js_f64(&stress_quantile, "stressQuantile")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Regulatory downturn adjustment: `max(baseLgd + addOn, floor)`.
    /// @param add_on - Additive LGD add-on as a fraction; non-negative.
    /// @param floor - Minimum downturn LGD as a fraction from 0 through 1.
    /// @returns The regulatory-floor downturn adjustment.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on out-of-range inputs.
    #[wasm_bindgen(js_name = regulatoryFloor)]
    pub fn regulatory_floor(add_on: JsValue, floor: JsValue) -> Result<JsDownturnLgd, JsValue> {
        DownturnLgd::regulatory_floor(js_f64(&add_on, "addOn")?, js_f64(&floor, "floor")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Downturn adjustment registered under an explicit registry identifier.
    /// @param id - Registry identifier of the downturn calibration.
    /// @returns The registered downturn adjustment.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if no calibration is registered under `id`.
    #[wasm_bindgen(js_name = fromRegistryId)]
    pub fn from_registry_id(id: JsValue) -> Result<JsDownturnLgd, JsValue> {
        DownturnLgd::from_registry_id(&js_string(&id, "id")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Basel downturn calibration for secured exposures.
    /// @returns The registry's Basel secured downturn adjustment.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry cannot supply the
    /// calibration.
    #[wasm_bindgen(js_name = baselSecured)]
    pub fn basel_secured() -> Result<JsDownturnLgd, JsValue> {
        DownturnLgd::basel_secured()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Basel downturn calibration for unsecured exposures.
    /// @returns The registry's Basel unsecured downturn adjustment.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry cannot supply the
    /// calibration.
    #[wasm_bindgen(js_name = baselUnsecured)]
    pub fn basel_unsecured() -> Result<JsDownturnLgd, JsValue> {
        DownturnLgd::basel_unsecured()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Apply the downturn adjustment to a through-the-cycle LGD.
    /// @param base_lgd - Through-the-cycle LGD as a fraction from 0 through 1.
    /// @returns The downturn LGD as a fraction in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `baseLgd` is outside `[0, 1]`.
    pub fn adjust(&self, base_lgd: JsValue) -> Result<f64, JsValue> {
        self.inner
            .adjust(js_f64(&base_lgd, "baseLgd")?)
            .map_err(to_js_err)
    }

    /// Canonical name of the adjustment method: `"stressed_approximation"` or `"regulatory_floor"`.
    #[wasm_bindgen(getter)]
    pub fn method(&self) -> Result<String, JsValue> {
        super::super::serde_tag(self.inner.method())
    }

    /// Method parameters as a plain object in canonical JSON form.
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.method())
    }

    /// Load a downturn adjustment from canonical JSON and validate its parameters.
    /// @param json - `DownturnLgd` JSON text or plain object; unknown fields are rejected.
    /// @returns The validated handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the payload is malformed or its
    /// parameters are out of range.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsDownturnLgd, JsValue> {
        let inner: DownturnLgd = from_js_json(&json, "json")?;
        inner.validate().map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    /// @returns Compact canonical JSON text.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }
}

/// Exposure-at-default calculator for drawn and undrawn commitments.
#[wasm_bindgen(js_name = EadCalculator)]
pub struct JsEadCalculator {
    pub(crate) inner: EadCalculator,
}

json_round_trip!(JsEadCalculator, EadCalculator);

#[wasm_bindgen(js_class = EadCalculator)]
impl JsEadCalculator {
    /// EAD inputs with an explicit credit conversion factor.
    /// @param drawn - Drawn balance in monetary units; non-negative.
    /// @param undrawn - Undrawn commitment in monetary units; non-negative.
    /// @param ccf - Credit conversion factor applied to the undrawn amount, from 0 through 1.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on negative, non-finite or out-of-range inputs.
    #[wasm_bindgen(constructor)]
    pub fn new(drawn: JsValue, undrawn: JsValue, ccf: JsValue) -> Result<JsEadCalculator, JsValue> {
        let ccf = CreditConversionFactor::new(js_f64(&ccf, "ccf")?).map_err(to_js_err)?;
        EadCalculator::new(js_f64(&drawn, "drawn")?, js_f64(&undrawn, "undrawn")?, ccf)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Fully drawn term loan: EAD equals the drawn balance.
    /// @param drawn - Drawn balance in monetary units; non-negative.
    /// @returns The term-loan EAD calculator.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `drawn` is negative or non-finite.
    #[wasm_bindgen(js_name = termLoan)]
    pub fn term_loan(drawn: JsValue) -> Result<JsEadCalculator, JsValue> {
        EadCalculator::term_loan(js_f64(&drawn, "drawn")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Revolver with the Basel credit conversion factor on the undrawn amount.
    /// @param drawn - Drawn balance in monetary units; non-negative.
    /// @param undrawn - Undrawn commitment in monetary units; non-negative.
    /// @returns The revolver EAD calculator.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on negative or non-finite inputs.
    pub fn revolver(drawn: JsValue, undrawn: JsValue) -> Result<JsEadCalculator, JsValue> {
        EadCalculator::revolver(js_f64(&drawn, "drawn")?, js_f64(&undrawn, "undrawn")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Exposure at default: `drawn + ccf * undrawn`, in monetary units.
    #[wasm_bindgen(getter)]
    pub fn ead(&self) -> f64 {
        self.inner.ead()
    }

    /// Drawn balance as a fraction of the total commitment.
    #[wasm_bindgen(getter)]
    pub fn utilization(&self) -> f64 {
        self.inner.utilization()
    }

    /// Drawn plus undrawn commitment, in monetary units.
    #[wasm_bindgen(getter, js_name = totalCommitment)]
    pub fn total_commitment(&self) -> f64 {
        self.inner.total_commitment()
    }

    /// Loan-equivalent factor implied by an observed exposure at default.
    /// @param observed_ead - Realized exposure at default in monetary units.
    /// @returns `(observedEad - drawn) / undrawn`, or `undefined` when nothing is undrawn.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `observedEad` is not a number.
    #[wasm_bindgen(js_name = leqFromObservedEad)]
    pub fn leq_from_observed_ead(&self, observed_ead: JsValue) -> Result<Option<f64>, JsValue> {
        Ok(self
            .inner
            .leq_from_observed_ead(js_f64(&observed_ead, "observedEad")?))
    }
}

/// Stressed-factor downturn LGD for a base LGD.
/// @param base_lgd - Through-the-cycle LGD as a fraction from 0 through 1.
/// @param asset_correlation - Asset correlation with the systematic factor, from 0 to 1.
/// @param lgd_sensitivity - Sensitivity of LGD to the systematic factor; non-negative.
/// @param stress_quantile - Stress quantile of the systematic factor, strictly between 0 and 1.
/// @returns The downturn LGD as a fraction in `[0, 1]`.
///
/// # Errors
///
/// Throws a `validation` error on out-of-range inputs.
#[wasm_bindgen(js_name = downturnLgdStressed)]
pub fn downturn_lgd_stressed(
    base_lgd: JsValue,
    asset_correlation: JsValue,
    lgd_sensitivity: JsValue,
    stress_quantile: JsValue,
) -> Result<f64, JsValue> {
    lgd::downturn_lgd_stressed(
        js_f64(&base_lgd, "baseLgd")?,
        js_f64(&asset_correlation, "assetCorrelation")?,
        js_f64(&lgd_sensitivity, "lgdSensitivity")?,
        js_f64(&stress_quantile, "stressQuantile")?,
    )
    .map_err(to_js_err)
}

/// Regulatory downturn LGD: `max(baseLgd + addOn, floor)`.
/// @param base_lgd - Through-the-cycle LGD as a fraction from 0 through 1.
/// @param add_on - Additive LGD add-on as a fraction; non-negative.
/// @param floor - Minimum downturn LGD as a fraction from 0 through 1.
/// @returns The downturn LGD as a fraction in `[0, 1]`.
///
/// # Errors
///
/// Throws a `validation` error on out-of-range inputs.
#[wasm_bindgen(js_name = downturnLgdRegulatoryFloor)]
pub fn downturn_lgd_regulatory_floor(
    base_lgd: JsValue,
    add_on: JsValue,
    floor: JsValue,
) -> Result<f64, JsValue> {
    lgd::downturn_lgd_regulatory_floor(
        js_f64(&base_lgd, "baseLgd")?,
        js_f64(&add_on, "addOn")?,
        js_f64(&floor, "floor")?,
    )
    .map_err(to_js_err)
}

/// Exposure at default of a fully drawn term loan.
/// @param principal - Outstanding principal in monetary units; non-negative.
/// @returns The EAD, equal to the principal.
///
/// # Errors
///
/// Throws a `validation` error if `principal` is negative or non-finite.
#[wasm_bindgen(js_name = eadTermLoan)]
pub fn ead_term_loan(principal: JsValue) -> Result<f64, JsValue> {
    lgd::ead_term_loan(js_f64(&principal, "principal")?).map_err(to_js_err)
}

/// Exposure at default of a revolver: `drawn + ccf * undrawn`.
/// @param drawn - Drawn balance in monetary units; non-negative.
/// @param undrawn - Undrawn commitment in monetary units; non-negative.
/// @param ccf - Credit conversion factor applied to the undrawn amount, from 0 through 1.
/// @returns The EAD in monetary units.
///
/// # Errors
///
/// Throws a `validation` error on negative, non-finite or out-of-range inputs.
#[wasm_bindgen(js_name = eadRevolver)]
pub fn ead_revolver(drawn: JsValue, undrawn: JsValue, ccf: JsValue) -> Result<f64, JsValue> {
    lgd::ead_revolver(
        js_f64(&drawn, "drawn")?,
        js_f64(&undrawn, "undrawn")?,
        js_f64(&ccf, "ccf")?,
    )
    .map_err(to_js_err)
}
