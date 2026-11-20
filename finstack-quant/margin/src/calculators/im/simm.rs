//! ISDA Standard Initial Margin Model (SIMM) calculator.
//!
//! Computes an indicative historical v2.6 approximation for uncleared OTC
//! derivatives. USD-normalized sensitivities are required. Product-class,
//! subcurve and some non-IR factor dimensions are incomplete; every ImResult
//! is marked approximate. This is not current regulatory SIMM.
//!
//! # ISDA SIMM Methodology
//!
//! SIMM calculates IM based on sensitivities across risk classes:
//! - Interest Rate (IR): DV01-style currency sensitivities by tenor bucket
//! - Credit Qualifying (CQ): CS01-style currency sensitivities for corporate, sovereign and index credit
//! - Credit Non-Qualifying (CNQ): CS01-style currency sensitivities for securitization credit
//! - Equity: signed currency delta and vega sensitivities
//! - Commodity: signed currency delta and vega sensitivities
//! - FX: signed currency delta and vega sensitivities
//!
//! # Formula
//!
//! ```text
//! IM = sqrt(sum_i sum_j ρ_ij × K_i × K_j)
//! ```
//!
//! Where K_i is the risk-weighted sensitivity for bucket i.
//!
//! > **Implementation note:** [`SimmCalculator::calculate_from_sensitivities`] applies intra-bucket
//! > tenor correlations for IR delta, vega margin (IR, credit qualifying,
//! > credit non-qualifying, equity, commodity, FX), curvature
//! > risk, concentration add-ons, and the SIMM risk-class correlation matrix.
//!
//! # Conventions
//!
//! - Risk weights and correlations are stored as decimal quantities in the
//!   registry, not basis points.
//! - Rate and credit delta inputs are expected to be DV01 or CS01 style
//!   currency amounts per 1bp move before they reach this module.
//! - Tenor keys must match the registry-backed tenor labels exactly.
//! - The aggregation currency is chosen by the caller to
//!   [`SimmCalculator::calculate_from_sensitivities`].
//!
//! # References
//!
//! - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
//! - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`

mod curvature;

use crate::Marginable;
use crate::calculators::traits::{
    ImCalculator, ImResult, SimmBucketDetail, SimmComponentDetail, SimmDetail,
    SimmWeightedSensitivity,
};
use crate::registry::{MarginRegistry, SimmParams, embedded_registry, margin_registry_from_config};
use crate::regulatory::frtb::aggregation::{correlated_norm, inter_bucket_pairwise};
use crate::types::ImMethodology;
use crate::types::{
    SimmCreditSector, SimmRiskClass, SimmSensitivities, ordered_pair, ordered_tenor_pair,
};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use tracing::debug;

/// SIMM version identifier.
///
/// Version choice controls the registry-backed risk weights, correlations, and
/// concentration thresholds used by the calculator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(inline)
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SimmVersion {
    /// SIMM v2.6 (2023)
    #[default]
    V2_6,
}

impl SimmVersion {
    /// Stable lowercase identifier used in machine-readable APIs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V2_6 => "v2_6",
        }
    }
}

impl std::fmt::Display for SimmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimmVersion::V2_6 => write!(f, "SIMM v2.6"),
        }
    }
}

impl std::str::FromStr for SimmVersion {
    type Err = String;

    fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
        match raw {
            "v2_6" => Ok(Self::V2_6),
            _ => Err(format!(
                "unknown SIMM version '{raw}' (expected 'v2_6'). Versions are \
                 selectable only when their ISDA-published parameter tables are \
                 shipped in the margin registry."
            )),
        }
    }
}

/// Currency groups in historical v2.6 paragraphs 33 and 75.
fn ir_currency_group(currency: Currency) -> &'static str {
    match currency {
        Currency::USD | Currency::EUR | Currency::GBP => "regular_well_traded",
        Currency::AUD
        | Currency::CAD
        | Currency::CHF
        | Currency::DKK
        | Currency::HKD
        | Currency::KRW
        | Currency::NOK
        | Currency::NZD
        | Currency::SEK
        | Currency::SGD
        | Currency::TWD => "regular_less_traded",
        Currency::JPY => "low",
        _ => "high",
    }
}

fn fx_high_volatility(currency: Currency) -> bool {
    matches!(currency, Currency::BRL | Currency::RUB | Currency::TRY)
}

fn fx_category(currency: Currency) -> &'static str {
    match currency {
        Currency::USD
        | Currency::EUR
        | Currency::JPY
        | Currency::GBP
        | Currency::AUD
        | Currency::CHF
        | Currency::CAD => "1",
        Currency::BRL
        | Currency::CNY
        | Currency::HKD
        | Currency::INR
        | Currency::KRW
        | Currency::MXN
        | Currency::NOK
        | Currency::NZD
        | Currency::RUB
        | Currency::SEK
        | Currency::SGD
        | Currency::TRY
        | Currency::ZAR => "2",
        _ => "3",
    }
}

fn concentration(raw_sensitivity: f64, threshold: f64) -> f64 {
    (raw_sensitivity.abs() / threshold).sqrt().max(1.0)
}

/// Optional collector for per-bucket aggregation detail. `None` on the
/// margin-only paths, so they build no detail rows.
type BucketSink<'a> = Option<&'a mut Vec<SimmBucketDetail>>;

/// Serde label of a credit sector, used as its bucket label.
fn sector_label(sector: SimmCreditSector) -> String {
    finstack_quant_core::wire::serde_label(&sector).unwrap_or_else(|_| format!("{sector:?}"))
}

/// Accumulates [`SimmComponentDetail`] rows, breakdown entries, and risk-class margins.
struct DetailCollector {
    currency: Currency,
    /// Buckets of the component currently being aggregated.
    scratch: Vec<SimmBucketDetail>,
    components: Vec<SimmComponentDetail>,
    curvature: Vec<SimmComponentDetail>,
    breakdown: FxHashMap<String, Money>,
    risk_class_margins: FxHashMap<SimmRiskClass, f64>,
}

impl DetailCollector {
    fn new(currency: Currency) -> Self {
        Self {
            currency,
            scratch: Vec::new(),
            components: Vec::new(),
            curvature: Vec::new(),
            breakdown: FxHashMap::default(),
            risk_class_margins: FxHashMap::default(),
        }
    }

    /// Bucket sink for the next delta or vega component.
    fn sink(&mut self) -> BucketSink<'_> {
        Some(&mut self.scratch)
    }

    /// Close the component whose buckets were just written to [`Self::sink`].
    /// Components with no positive margin are dropped, matching `breakdown`.
    fn record(
        &mut self,
        component: &str,
        risk_class: SimmRiskClass,
        historical_volatility_ratio: f64,
        margin: f64,
    ) -> Result<()> {
        let buckets = std::mem::take(&mut self.scratch);
        if margin > 0.0 {
            self.components.push(SimmComponentDetail {
                component: component.to_owned(),
                risk_class,
                historical_volatility_ratio,
                buckets,
                margin,
            });
            self.breakdown
                .insert(component.to_owned(), Money::new(margin, self.currency)?);
            *self.risk_class_margins.entry(risk_class).or_insert(0.0) += margin;
        }
        Ok(())
    }

    fn finish(mut self, mpor_scale: f64) -> Result<(FxHashMap<String, Money>, SimmDetail)> {
        self.curvature.retain(|component| component.margin > 0.0);
        self.curvature.sort_by_key(|component| component.risk_class);
        self.components.append(&mut self.curvature);
        for value in self.breakdown.values_mut() {
            *value = value.checked_mul_f64(mpor_scale)?;
        }
        let risk_class_margins = self
            .risk_class_margins
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>();
        Ok((
            self.breakdown,
            SimmDetail {
                components: self.components,
                risk_class_margins,
                mpor_scale,
            },
        ))
    }
}

/// Single-bucket detail: `K_b` is the component margin itself.
fn single_bucket(
    bucket: &str,
    weighted_sensitivities: Vec<SimmWeightedSensitivity>,
    k: f64,
) -> SimmBucketDetail {
    let sum: f64 = weighted_sensitivities
        .iter()
        .map(|row| row.weighted_sensitivity)
        .sum();
    SimmBucketDetail {
        bucket: bucket.to_owned(),
        weighted_sensitivities,
        k,
        signed_sum: sum.clamp(-k, k),
    }
}

// Lookup helpers for SimmParams fields.
impl SimmParams {
    fn correlation(&self, a: SimmRiskClass, b: SimmRiskClass) -> f64 {
        if a == b {
            return 1.0;
        }
        let key = ordered_pair(a, b);
        self.risk_class_correlations
            .get(&key)
            .copied()
            .unwrap_or(1.0)
    }
}

// Lookup helpers for credit qualifying bucket parameters.
impl SimmParams {
    fn cq_bucket_weight(&self, sector: SimmCreditSector) -> f64 {
        self.cq_bucket_weights.get(&sector).copied().unwrap_or(0.0)
    }

    fn cq_inter_bucket_correlation(&self, a: SimmCreditSector, b: SimmCreditSector) -> f64 {
        if a == b {
            return 1.0;
        }
        let key = ordered_pair(a, b);
        self.cq_inter_bucket_correlations
            .get(&key)
            .copied()
            .unwrap_or(0.0)
    }

    /// Look up 1-based SIMM commodity buckets in the row-major registry matrix.
    /// Out-of-range ids contribute `0.0` correlation.
    fn commodity_inter_bucket_correlation(&self, a: u8, b: u8) -> f64 {
        let count = usize::from(crate::types::SIMM_COMMODITY_BUCKET_COUNT);
        let a = usize::from(a);
        let b = usize::from(b);
        if a == 0 || b == 0 || a > count || b > count {
            return 0.0;
        }
        let idx = (a - 1) * count + (b - 1);
        self.commodity_inter_bucket_correlations
            .get(idx)
            .copied()
            .unwrap_or(0.0)
    }
}

fn resolve_simm_params(
    version: SimmVersion,
    registry: &MarginRegistry,
) -> finstack_quant_core::Result<&SimmParams> {
    if let Some(found) = registry.simm.values().find(|p| p.version == version) {
        return Ok(found);
    }
    let available: Vec<String> = registry
        .simm
        .values()
        .map(|p| format!("{:?}", p.version))
        .collect();
    Err(finstack_quant_core::Error::Validation(format!(
        "SIMM registry does not contain version {:?}. Available versions: [{}]. \
         Update the registry overlay or pass a supported SimmVersion.",
        version,
        available.join(", ")
    )))
}

/// One-sided 99.5% standard-normal quantile `Φ⁻¹(0.995)`, used in the ISDA
/// SIMM curvature `λ` scaling. ISDA specifies this exact constant.
const SIMM_CURVATURE_Z: f64 = 2.575_829_303_548_900_4;

/// Pre-computed flat correlation matrix for IR tenor lookups.
/// Avoids per-lookup String allocations in the O(n^2) delta/vega loops.
#[derive(Debug, Clone)]
struct IrTenorCorrelationMatrix {
    tenor_to_idx: FxHashMap<String, usize>,
    matrix: Vec<f64>,
    n: usize,
}

impl IrTenorCorrelationMatrix {
    fn build(params: &SimmParams) -> Self {
        let tenors: Vec<String> = params.ir_delta_weights.keys().cloned().collect();
        let n = tenors.len();
        let mut tenor_to_idx = FxHashMap::default();
        for (i, t) in tenors.iter().enumerate() {
            tenor_to_idx.insert(t.clone(), i);
        }

        let mut matrix = vec![1.0; n * n];
        for (i, tenor_i) in tenors.iter().enumerate() {
            for (j, tenor_j) in tenors.iter().enumerate() {
                if i == j {
                    continue;
                }
                let key = ordered_tenor_pair(tenor_i, tenor_j);
                let rho = match params.ir_tenor_correlations.get(&key).copied() {
                    Some(r) => r,
                    None => {
                        tracing::error!(
                            tenor_i = %key.0,
                            tenor_j = %key.1,
                            "SIMM: missing ir_tenor_correlation post-validation; \
                             using 0.5 fallback (this indicates a registry invariant break)"
                        );
                        0.5
                    }
                };
                if let Some(cell) = matrix.get_mut(i * n + j) {
                    *cell = rho;
                }
            }
        }

        Self {
            tenor_to_idx,
            matrix,
            n,
        }
    }

    fn correlation(&self, idx_a: usize, idx_b: usize) -> f64 {
        if idx_a == idx_b {
            return 1.0;
        }
        self.matrix[idx_a * self.n + idx_b]
    }
}

/// ISDA SIMM calculator.
///
/// Calculates initial margin using the ISDA Standard Initial Margin Model for
/// bilateral OTC derivatives. The calculator is parameterized entirely from the
/// margin registry, so version changes and config overlays affect risk weights,
/// correlations, concentration thresholds, and MPOR.
///
/// # References
///
/// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
#[derive(Debug, Clone)]
pub struct SimmCalculator {
    /// SIMM parameters (risk weights, correlations, thresholds)
    params: SimmParams,
    ir_corr_matrix: IrTenorCorrelationMatrix,
}

impl SimmCalculator {
    /// Inspect the validated parameters resolved when this calculator was built.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn get_params(&self) -> &SimmParams {
        &self.params
    }

    /// Create a new SIMM calculator with the specified version.
    ///
    /// # Arguments
    ///
    /// * `version` - SIMM rule set to load from the embedded margin registry
    ///
    /// # Returns
    ///
    /// A calculator with registry-backed risk weights and correlations for `version`.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded or if
    /// `version` is absent from the registry.
    pub fn new(version: SimmVersion) -> Result<Self> {
        Self::build_from_registry(version, embedded_registry()?)
    }

    /// Create a new SIMM calculator resolved from a `FinstackConfig`.
    ///
    /// # Arguments
    ///
    /// * `version` - SIMM rule set to resolve
    /// * `cfg` - Config whose margin-registry overlay may replace embedded SIMM parameters
    ///
    /// # Returns
    ///
    /// A calculator using the merged registry derived from `cfg`.
    ///
    /// # Errors
    ///
    /// Returns an error if the margin registry cannot be loaded from `cfg` or if
    /// `version` is absent from the resolved registry.
    pub fn from_finstack_config(
        version: SimmVersion,
        cfg: &finstack_quant_core::config::FinstackConfig,
    ) -> finstack_quant_core::Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        Self::build_from_registry(version, &registry)
    }

    /// Shared construction path for [`Self::new`] and [`Self::from_finstack_config`].
    fn build_from_registry(version: SimmVersion, registry: &MarginRegistry) -> Result<Self> {
        let params = resolve_simm_params(version, registry)?.clone();
        let ir_corr_matrix = IrTenorCorrelationMatrix::build(&params);
        Ok(Self {
            params,
            ir_corr_matrix,
        })
    }

    /// SIMM version.
    #[must_use]
    pub fn version(&self) -> SimmVersion {
        self.params.version
    }

    /// Margin period of risk in business days (ISDA SIMM: 10).
    #[must_use]
    pub fn mpor_days(&self) -> u32 {
        self.params.mpor_days
    }

    /// Set margin period of risk.
    ///
    /// # Arguments
    ///
    /// * `days` - Margin period of risk in business days (ISDA SIMM standard is 10)
    ///
    /// # Returns
    ///
    /// The updated calculator.
    #[must_use]
    pub fn with_mpor(mut self, days: u32) -> Self {
        self.params.mpor_days = days;
        self
    }

    /// Calculate IR delta or vega margin with multi-currency aggregation.
    ///
    /// Per ISDA SIMM v2.6 methodology:
    /// 1. For each currency, compute the net sensitivity and per-currency concentration factor.
    /// 2. For each currency, compute `K_c` with weighted sensitivities scaled by `CR_c` using
    ///    the intra-currency tenor correlations.
    /// 3. Aggregate across currencies with `ir_inter_currency_correlation`.
    fn aggregate_ir(
        &self,
        sensitivities: &FxHashMap<(Currency, String), f64>,
        vega: bool,
        mut detail: BucketSink<'_>,
    ) -> f64 {
        let mut by_currency: std::collections::BTreeMap<Currency, Vec<(&str, f64)>> =
            std::collections::BTreeMap::new();
        for ((currency, tenor), amount) in sensitivities {
            by_currency
                .entry(*currency)
                .or_default()
                .push((tenor, *amount));
        }
        let buckets: Vec<_> = by_currency
            .into_iter()
            .map(|(currency, mut entries)| {
                entries.sort_by(|a, b| a.0.cmp(b.0));
                let group = ir_currency_group(currency);
                let thresholds = if vega {
                    &self.params.ir_vega_concentration_thresholds
                } else {
                    &self.params.ir_delta_concentration_thresholds
                };
                let cf = concentration(entries.iter().map(|(_, v)| v).sum(), thresholds[group]);
                let weights = match group {
                    "low" => &self.params.ir_delta_weights_low,
                    "high" => &self.params.ir_delta_weights_high,
                    _ => &self.params.ir_delta_weights,
                };
                let rows: Vec<(usize, &str, f64, f64)> = entries
                    .iter()
                    .filter_map(|(tenor, amount)| {
                        let index = *self.ir_corr_matrix.tenor_to_idx.get(*tenor)?;
                        let weight = if vega {
                            self.params.ir_vega_weight
                        } else {
                            weights[*tenor]
                        };
                        Some((index, *tenor, *amount, weight))
                    })
                    .collect();
                let weighted: Vec<_> = rows
                    .iter()
                    .map(|(index, _, amount, weight)| (*index, amount * weight * cf))
                    .collect();
                let k = self.ir_tenor_norm(&weighted);
                let signed = weighted.iter().map(|(_, ws)| ws).sum::<f64>().clamp(-k, k);
                if let Some(sink) = detail.as_deref_mut() {
                    sink.push(SimmBucketDetail {
                        bucket: currency.to_string(),
                        weighted_sensitivities: rows
                            .iter()
                            .zip(&weighted)
                            .map(
                                |((_, tenor, amount, weight), (_, ws))| SimmWeightedSensitivity {
                                    risk_factor: currency.to_string(),
                                    tenor: Some((*tenor).to_owned()),
                                    sensitivity: *amount,
                                    risk_weight: *weight,
                                    concentration_factor: cf,
                                    weighted_sensitivity: *ws,
                                },
                            )
                            .collect(),
                        k,
                        signed_sum: signed,
                    });
                }
                (k, signed, cf)
            })
            .collect();
        self.aggregate_ir_currencies(&buckets)
    }

    /// `sqrt(Σ ρ_ij ws_i ws_j)` over `(tenor_idx, ws)` pairs with the SIMM IR
    /// tenor correlation matrix. `indexed` must already be in canonical
    /// tenor order.
    fn ir_tenor_norm(&self, indexed: &[(usize, f64)]) -> f64 {
        let ws: Vec<f64> = indexed.iter().map(|(_, ws)| *ws).collect();
        correlated_norm(&ws, |i, j| {
            self.ir_corr_matrix.correlation(indexed[i].0, indexed[j].0)
        })
    }

    /// Combine per-currency IR margins with the SIMM inter-currency
    /// correlation `γ` (uniform off-diagonal).
    fn aggregate_ir_currencies(&self, buckets: &[(f64, f64, f64)]) -> f64 {
        let mut variance: f64 = buckets.iter().map(|(k, _, _)| k * k).sum();
        for (i, (_, s_i, cr_i)) in buckets.iter().enumerate() {
            for (_, s_j, cr_j) in &buckets[..i] {
                let g = cr_i.min(*cr_j) / cr_i.max(*cr_j);
                variance += 2.0 * self.params.ir_inter_currency_correlation * g * s_i * s_j;
            }
        }
        variance.max(0.0).sqrt()
    }

    /// Credit-qualifying delta or vega aggregation with concentration netted by issuer before weighting.
    ///
    /// Follows the ISDA SIMM v2.6 §3.B two-level aggregation: issuer-level concentration scaling,
    /// intra-bucket name correlation, capped net weighted sums, and inter-bucket sector correlation.
    fn aggregate_credit_qualifying(
        &self,
        bucketed: &FxHashMap<(SimmCreditSector, String, String), f64>,
        weight_for: impl Fn(SimmCreditSector) -> f64,
        delta: bool,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut by_sector: FxHashMap<SimmCreditSector, Vec<(&str, &str, f64)>> =
            FxHashMap::default();
        for ((sector, issuer, tenor), amount) in bucketed {
            by_sector
                .entry(*sector)
                .or_default()
                .push((issuer, tenor, *amount));
        }
        let mut buckets = Vec::new();
        let mut residual = 0.0;
        let mut bucket_details: Vec<(SimmCreditSector, SimmBucketDetail)> = Vec::new();
        for (sector, mut entries) in by_sector {
            entries.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
            let mut raw_by_name: FxHashMap<&str, f64> = FxHashMap::default();
            for (name, _, raw) in &entries {
                *raw_by_name.entry(name).or_default() += raw;
            }
            let threshold = if delta {
                self.params.cq_concentration_thresholds[&sector]
            } else {
                self.params.vega_concentration_thresholds[&SimmRiskClass::CreditQualifying]
            };
            let factors: Vec<_> = entries
                .iter()
                .map(|(name, _, _)| concentration(raw_by_name[name], threshold))
                .collect();
            let ws: Vec<_> = entries
                .iter()
                .zip(&factors)
                .map(|((_, _, raw), cf)| raw * weight_for(sector) * cf)
                .collect();
            let k = correlated_norm(&ws, |i, j| {
                let rho = if sector == SimmCreditSector::Residual {
                    self.params.credit_residual_correlation
                } else if entries[i].0 == entries[j].0 {
                    self.params.cq_same_issuer_correlation
                } else {
                    self.params.cq_intra_bucket_correlation
                };
                rho * factors[i].min(factors[j]) / factors[i].max(factors[j])
            });
            let signed = ws.iter().sum::<f64>().clamp(-k, k);
            if detail.is_some() {
                bucket_details.push((
                    sector,
                    SimmBucketDetail {
                        bucket: sector_label(sector),
                        weighted_sensitivities: entries
                            .iter()
                            .zip(factors.iter().zip(&ws))
                            .map(|((name, tenor, raw), (cf, ws))| SimmWeightedSensitivity {
                                risk_factor: (*name).to_owned(),
                                tenor: Some((*tenor).to_owned()),
                                sensitivity: *raw,
                                risk_weight: weight_for(sector),
                                concentration_factor: *cf,
                                weighted_sensitivity: *ws,
                            })
                            .collect(),
                        k,
                        signed_sum: signed,
                    },
                ));
            }
            if sector == SimmCreditSector::Residual {
                residual = k;
            } else {
                buckets.push((sector, k, signed));
            }
        }
        if let Some(sink) = detail {
            bucket_details.sort_by_key(|(sector, _)| *sector as u8);
            sink.extend(bucket_details.into_iter().map(|(_, bucket)| bucket));
        }
        buckets.sort_by_key(|(sector, _, _)| *sector as u8);
        let values: Vec<_> = buckets.iter().map(|(_, k, s)| (*k, *s)).collect();
        residual
            + inter_bucket_pairwise(&values, |i, j| {
                self.params
                    .cq_inter_bucket_correlation(buckets[i].0, buckets[j].0)
            })
    }

    /// Calculate equity delta margin.
    ///
    /// # Arguments
    ///
    /// * `equity_delta` - Signed currency equity delta sensitivity
    ///
    /// # Returns
    ///
    /// The weighted equity delta margin contribution.
    fn calculate_equity_delta(&self, equity_delta: f64) -> (f64, f64, f64) {
        let weight = self.params.equity_delta_weight;
        let cf = self.concentration_factor(SimmRiskClass::Equity, equity_delta);
        (weight, cf, equity_delta * weight * cf)
    }

    /// Equity delta or vega margin across names (zero inter-name correlation).
    ///
    /// Each name's margin is the absolute value of its weighted sensitivity;
    /// the margins are summed in ascending order so the result does not
    /// depend on map iteration order.
    fn equity_margin(
        &self,
        by_name: &FxHashMap<String, f64>,
        vega: bool,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut rows: Vec<(&str, f64, (f64, f64, f64))> = by_name
            .iter()
            .map(|(name, amount)| {
                let weighted = if vega {
                    self.calculate_equity_vega(*amount)
                } else {
                    self.calculate_equity_delta(*amount)
                };
                (name.as_str(), *amount, weighted)
            })
            .collect();
        rows.sort_by(|a, b| {
            a.2.2
                .abs()
                .total_cmp(&b.2.2.abs())
                .then_with(|| a.0.cmp(b.0))
        });
        let margins: Vec<f64> = rows.iter().map(|(_, _, (_, _, ws))| ws.abs()).collect();
        if let Some(sink) = detail {
            sink.extend(
                rows.iter()
                    .map(|(name, amount, (weight, cf, ws))| SimmBucketDetail {
                        bucket: (*name).to_owned(),
                        weighted_sensitivities: vec![SimmWeightedSensitivity {
                            risk_factor: (*name).to_owned(),
                            tenor: None,
                            sensitivity: *amount,
                            risk_weight: *weight,
                            concentration_factor: *cf,
                            weighted_sensitivity: *ws,
                        }],
                        k: ws.abs(),
                        signed_sum: *ws,
                    }),
            );
        }
        correlated_norm(&margins, |_, _| 0.0)
    }

    /// Calculate FX delta margin across currency risk factors.
    ///
    /// Each currency sensitivity is weighted and concentration-scaled
    /// independently, then aggregated with the SIMM FX intra-bucket correlation
    /// between distinct currency risk factors. This prevents opposite-signed
    /// currency deltas from receiving full rho=1 offset.
    fn calculate_fx_delta_bucketed(
        &self,
        fx_delta: &FxHashMap<Currency, f64>,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut entries: Vec<_> = fx_delta
            .iter()
            .filter(|(currency, _)| **currency != Currency::USD)
            .map(|(currency, raw)| {
                let weight = if fx_high_volatility(*currency) {
                    self.params.fx_high_delta_weight
                } else {
                    self.params.fx_delta_weight
                };
                let cf = concentration(
                    *raw,
                    self.params.fx_delta_concentration_thresholds[fx_category(*currency)],
                );
                (*currency, raw * weight * cf, cf, *raw, weight)
            })
            .collect();
        entries.sort_by_key(|(currency, ..)| *currency);
        let ws: Vec<_> = entries.iter().map(|(_, ws, ..)| *ws).collect();
        let margin = correlated_norm(&ws, |i, j| {
            let rho = match (
                fx_high_volatility(entries[i].0),
                fx_high_volatility(entries[j].0),
            ) {
                (true, true) => self.params.fx_high_high_correlation,
                (false, false) => self.params.fx_intra_bucket_correlation,
                _ => self.params.fx_regular_high_correlation,
            };
            rho * entries[i].2.min(entries[j].2) / entries[i].2.max(entries[j].2)
        });
        if let Some(sink) = detail {
            let rows = entries
                .iter()
                .map(|(currency, ws, cf, raw, weight)| SimmWeightedSensitivity {
                    risk_factor: currency.to_string(),
                    tenor: None,
                    sensitivity: *raw,
                    risk_weight: *weight,
                    concentration_factor: *cf,
                    weighted_sensitivity: *ws,
                })
                .collect();
            sink.push(single_bucket("fx", rows, margin));
        }
        margin
    }

    /// Shared commodity bucket aggregation for the delta and vega risk classes.
    ///
    /// Buckets whose label does not resolve to a SIMM commodity bucket id are
    /// dropped, matching the delta behaviour prior to this refactor.
    fn aggregate_commodity(
        &self,
        by_bucket: &FxHashMap<String, f64>,
        vega: bool,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut canonical: std::collections::BTreeMap<u8, Vec<f64>> =
            std::collections::BTreeMap::new();
        for (bucket, amount) in by_bucket {
            if let Some(id) = crate::types::commodity_bucket_id(bucket) {
                canonical.entry(id).or_default().push(*amount);
            }
        }
        let entries: Vec<_> = canonical
            .into_iter()
            .map(|(bucket, mut amounts)| {
                amounts.sort_by(f64::total_cmp);
                let key = bucket.to_string();
                let net = amounts.iter().sum::<f64>();
                let raw = net
                    * if vega {
                        self.params.commodity_historical_volatility_ratio
                    } else {
                        1.0
                    };
                let threshold = if vega {
                    self.params.commodity_vega_concentration_thresholds[&key]
                } else {
                    self.params.commodity_delta_concentration_thresholds[&key]
                };
                let weight = if vega {
                    self.params.commodity_vega_weight
                } else {
                    self.params.commodity_bucket_weights[&key]
                };
                let cf = concentration(raw, threshold);
                (bucket, raw * weight * cf, net, weight, cf)
            })
            .collect();
        let ws: Vec<_> = entries.iter().map(|(_, ws, ..)| *ws).collect();
        if let Some(sink) = detail {
            sink.extend(
                entries
                    .iter()
                    .map(|(bucket, ws, net, weight, cf)| SimmBucketDetail {
                        bucket: bucket.to_string(),
                        weighted_sensitivities: vec![SimmWeightedSensitivity {
                            risk_factor: bucket.to_string(),
                            tenor: None,
                            sensitivity: *net,
                            risk_weight: *weight,
                            concentration_factor: *cf,
                            weighted_sensitivity: *ws,
                        }],
                        k: ws.abs(),
                        signed_sum: *ws,
                    }),
            );
        }
        correlated_norm(&ws, |i, j| {
            self.params
                .commodity_inter_bucket_correlation(entries[i].0, entries[j].0)
        })
    }

    /// Weighted equity vega sensitivity from a signed currency vega amount,
    /// as `(risk weight, concentration factor, weighted sensitivity)`.
    fn calculate_equity_vega(&self, total_vega: f64) -> (f64, f64, f64) {
        let raw = total_vega * self.params.equity_historical_volatility_ratio;
        let weight = self.params.equity_vega_weight;
        let cf = concentration(
            raw,
            self.params.vega_concentration_thresholds[&SimmRiskClass::Equity],
        );
        (weight, cf, raw * weight * cf)
    }

    fn fx_vega_bucketed(
        &self,
        inputs: &FxHashMap<(Currency, Currency), f64>,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut pairs: std::collections::BTreeMap<(Currency, Currency), Vec<f64>> =
            std::collections::BTreeMap::new();
        for ((a, b), value) in inputs {
            pairs
                .entry(((*a).min(*b), (*a).max(*b)))
                .or_default()
                .push(*value);
        }
        let entries: Vec<_> = pairs
            .into_iter()
            .map(|((a, b), mut amounts)| {
                amounts.sort_by(f64::total_cmp);
                let net = amounts.iter().sum::<f64>();
                let raw = net * self.params.fx_historical_volatility_ratio;
                let (cat_a, cat_b) = (fx_category(a), fx_category(b));
                let key = format!("{}_{}", cat_a.min(cat_b), cat_a.max(cat_b));
                let cf = concentration(raw, self.params.fx_vega_concentration_thresholds[&key]);
                (raw * self.params.fx_vega_weight * cf, cf, (a, b), net)
            })
            .collect();
        let ws: Vec<_> = entries.iter().map(|(ws, ..)| *ws).collect();
        let margin = correlated_norm(&ws, |i, j| {
            0.5 * entries[i].1.min(entries[j].1) / entries[i].1.max(entries[j].1)
        });
        if let Some(sink) = detail {
            let rows = entries
                .iter()
                .map(|(ws, cf, (a, b), net)| SimmWeightedSensitivity {
                    risk_factor: format!("{a}/{b}"),
                    tenor: None,
                    sensitivity: *net,
                    risk_weight: self.params.fx_vega_weight,
                    concentration_factor: *cf,
                    weighted_sensitivity: *ws,
                })
                .collect();
            sink.push(single_bucket("fx", rows, margin));
        }
        margin
    }

    fn non_qualifying_margin(
        &self,
        inputs: &FxHashMap<(String, String), f64>,
        vega: bool,
        detail: BucketSink<'_>,
    ) -> f64 {
        let mut entries: Vec<_> = inputs.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let mut raw_by_name: FxHashMap<&str, f64> = FxHashMap::default();
        for ((name, _), amount) in &entries {
            *raw_by_name.entry(name).or_default() += **amount;
        }
        let threshold = if vega {
            self.params.vega_concentration_thresholds[&SimmRiskClass::CreditNonQualifying]
        } else {
            self.params.concentration_thresholds[&SimmRiskClass::CreditNonQualifying]
        };
        let weight = if vega {
            self.params.cnq_vega_weight
        } else {
            self.params.cnq_delta_weight
        };
        let cf: Vec<_> = entries
            .iter()
            .map(|((name, _), _)| concentration(raw_by_name[name.as_str()], threshold))
            .collect();
        let ws: Vec<_> = entries
            .iter()
            .zip(&cf)
            .map(|((_, raw), cf)| **raw * weight * cf)
            .collect();
        let margin = correlated_norm(&ws, |i, j| {
            self.params.credit_residual_correlation * cf[i].min(cf[j]) / cf[i].max(cf[j])
        });
        if let Some(sink) = detail {
            let rows = entries
                .iter()
                .zip(cf.iter().zip(&ws))
                .map(|(((name, tenor), raw), (cf, ws))| SimmWeightedSensitivity {
                    risk_factor: name.clone(),
                    tenor: Some(tenor.clone()),
                    sensitivity: **raw,
                    risk_weight: weight,
                    concentration_factor: *cf,
                    weighted_sensitivity: *ws,
                })
                .collect();
            sink.push(single_bucket("residual", rows, margin));
        }
        margin
    }

    /// Calculate concentration add-on for a risk class.
    ///
    /// If the net sensitivity exceeds the concentration threshold,
    /// apply a sqrt(|sensitivity| / threshold) multiplier.
    ///
    /// Both `net_sensitivity` and the configured threshold are interpreted in
    /// the same signed currency units.
    fn concentration_factor(&self, risk_class: SimmRiskClass, net_sensitivity: f64) -> f64 {
        self.params
            .concentration_thresholds
            .get(&risk_class)
            .copied()
            .filter(|&threshold| threshold > 0.0)
            .map_or(1.0, |threshold| concentration(net_sensitivity, threshold))
    }

    /// Calculate indicative historical SIMM v2.6 from explicit sensitivities.
    ///
    /// Results always carry `approximation = true`: the input contract omits
    /// product-class, IR subcurve and full non-IR bucket dimensions. Non-IR
    /// name aggregates use gross absolute sensitivities to prevent false offsets.
    /// This model is unsuitable for regulatory margin or current-version reconciliation.
    ///
    /// Validates the sensitivity container with [`SimmSensitivities::validate`]
    /// first, so a mistyped tenor or commodity bucket errors instead of
    /// silently pricing to zero margin, then stamps the methodology, MPOR and
    /// calculation date.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - SIMM sensitivity container using the units documented on [`SimmSensitivities`].
    /// * `currency` - Must be USD and match the sensitivity base currency. All
    ///   sensitivities must already be converted to USD before concentration tests.
    /// * `as_of` - Calculation date stamped on the result.
    ///
    /// # Errors
    ///
    /// Returns a validation error from [`SimmSensitivities::validate`] when a
    /// tenor, commodity bucket, identifier or amount is invalid.
    ///
    /// # References
    ///
    /// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
    pub fn calculate_from_sensitivities(
        &self,
        sensitivities: &SimmSensitivities,
        currency: Currency,
        as_of: Date,
    ) -> Result<ImResult> {
        sensitivities.validate()?;
        if currency != sensitivities.base_currency || currency != Currency::USD {
            return Err(finstack_quant_core::Error::Validation(
                "SIMM requires USD-normalized sensitivities and USD output because concentration thresholds are in USD; convert inputs explicitly before calculation".into(),
            ));
        }
        if self.mpor_days() == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "SIMM MPOR must be positive".into(),
            ));
        }

        let mut detail = DetailCollector::new(currency);

        if !sensitivities.ir_delta.is_empty() {
            let m = self.aggregate_ir(&sensitivities.ir_delta, false, detail.sink());
            detail.record("IR_Delta", SimmRiskClass::InterestRate, 1.0, m)?;
        }
        if !sensitivities.ir_vega.is_empty() {
            let m = self.aggregate_ir(&sensitivities.ir_vega, true, detail.sink());
            detail.record("IR_Vega", SimmRiskClass::InterestRate, 1.0, m)?;
        }
        if !sensitivities.credit_qualifying_delta.is_empty() {
            let m = self.aggregate_credit_qualifying(
                &sensitivities.credit_qualifying_delta,
                |sector| self.params.cq_bucket_weight(sector),
                true,
                detail.sink(),
            );
            detail.record(
                "Credit_Qualifying_Delta",
                SimmRiskClass::CreditQualifying,
                1.0,
                m,
            )?;
        }
        if !sensitivities.credit_qualifying_vega.is_empty() {
            let cq_vega_weight = self.params.cq_vega_weight;
            let m = self.aggregate_credit_qualifying(
                &sensitivities.credit_qualifying_vega,
                |_| cq_vega_weight,
                false,
                detail.sink(),
            );
            detail.record(
                "Credit_Qualifying_Vega",
                SimmRiskClass::CreditQualifying,
                1.0,
                m,
            )?;
        }

        // Names without classification use the official residual buckets.
        let m = self.non_qualifying_margin(
            &sensitivities.credit_non_qualifying_delta,
            false,
            detail.sink(),
        );
        detail.record(
            "Credit_NonQualifying_Delta",
            SimmRiskClass::CreditNonQualifying,
            1.0,
            m,
        )?;
        let m = self.non_qualifying_margin(
            &sensitivities.credit_non_qualifying_vega,
            true,
            detail.sink(),
        );
        detail.record(
            "Credit_NonQualifying_Vega",
            SimmRiskClass::CreditNonQualifying,
            1.0,
            m,
        )?;

        let m = self.equity_margin(&sensitivities.equity_delta, false, detail.sink());
        detail.record("Equity_Delta", SimmRiskClass::Equity, 1.0, m)?;
        let m = self.equity_margin(&sensitivities.equity_vega, true, detail.sink());
        detail.record(
            "Equity_Vega",
            SimmRiskClass::Equity,
            self.params.equity_historical_volatility_ratio,
            m,
        )?;

        let m = self.calculate_fx_delta_bucketed(&sensitivities.fx_delta, detail.sink());
        detail.record("FX_Delta", SimmRiskClass::Fx, 1.0, m)?;
        let m = self.fx_vega_bucketed(&sensitivities.fx_vega, detail.sink());
        detail.record(
            "FX_Vega",
            SimmRiskClass::Fx,
            self.params.fx_historical_volatility_ratio,
            m,
        )?;

        if !sensitivities.commodity_delta.is_empty() {
            let m = self.aggregate_commodity(&sensitivities.commodity_delta, false, detail.sink());
            detail.record("Commodity_Delta", SimmRiskClass::Commodity, 1.0, m)?;
        }
        if !sensitivities.commodity_vega.is_empty() {
            let m = self.aggregate_commodity(&sensitivities.commodity_vega, true, detail.sink());
            detail.record(
                "Commodity_Vega",
                SimmRiskClass::Commodity,
                self.params.commodity_historical_volatility_ratio,
                m,
            )?;
        }

        // Curvature contributes within each risk class before class correlation.
        for (class, margin) in
            self.curvature_by_risk_class(&sensitivities.curvature, Some(&mut detail.curvature))?
        {
            if margin > 0.0 {
                detail
                    .breakdown
                    .insert(format!("{class}_Curvature"), Money::new(margin, currency)?);
                *detail.risk_class_margins.entry(class).or_insert(0.0) += margin;
            }
        }

        let mpor_scale = (f64::from(self.mpor_days()) / 10.0).sqrt();
        let total_im = self.aggregate_risk_classes(&detail.risk_class_margins) * mpor_scale;
        let (breakdown, simm_detail) = detail.finish(mpor_scale)?;
        let mut result = ImResult::with_breakdown(
            Money::new(total_im, currency)?,
            ImMethodology::Simm,
            as_of,
            self.mpor_days(),
            breakdown,
        );
        result.approximation = true;
        result.simm_detail = Some(simm_detail);
        Ok(result)
    }

    /// Calculate model initial margin for a single explicitly selected netting set.
    ///
    /// Sensitivities are netted by their existing risk keys before applying the
    /// SIMM risk weights, concentration and correlation. The result keeps the
    /// approximation classification of the sensitivity-based calculator.
    ///
    /// # Arguments
    /// * `instruments` - Trades in the same netting set, each with its own
    ///   contractual notional and direction, counted once in input order.
    /// * `market` - Valuation market supplying each trade's risk factors.
    /// * `as_of` - Valuation date used for every trade and stamped on the result.
    /// * `currency` - USD calculation currency. Trade sensitivity amounts must
    ///   already use this currency; the method applies no FX conversion.
    ///
    /// # Errors
    /// Rejects an empty selection, unsupported currency or sensitivity inputs
    /// in another currency, and propagates failures from trade repricing and
    /// the sensitivity-based IM calculation.
    pub fn calculate_netting_set<'a>(
        &self,
        instruments: impl IntoIterator<Item = &'a dyn crate::traits::Marginable>,
        market: &MarketContext,
        as_of: Date,
        currency: Currency,
    ) -> Result<ImResult> {
        if currency != Currency::USD {
            return Err(finstack_quant_core::Error::Validation(
                "SIMM calculation currency must be USD".into(),
            ));
        }
        let mut instruments = instruments.into_iter();
        let first = instruments.next().ok_or_else(|| {
            finstack_quant_core::Error::Validation("SIMM netting set is empty".into())
        })?;
        let mut sensitivities = SimmSensitivities::new(currency);
        for instrument in std::iter::once(first).chain(instruments) {
            sensitivities.merge(&instrument.simm_sensitivities(market, as_of)?)?;
        }
        self.calculate_from_sensitivities(&sensitivities, currency, as_of)
    }

    /// Aggregate risk class margins with the SIMM inter-risk-class correlation matrix.
    ///
    /// `Total = sqrt(sum_i sum_j rho(i,j) * K_i * K_j)`
    fn aggregate_risk_classes(&self, risk_class_margins: &FxHashMap<SimmRiskClass, f64>) -> f64 {
        // Reduce in a canonical risk-class order so the f64 quadratic form is
        // bit-reproducible across runs, independent of `FxHashMap` iteration order
        // (mirrors `calculate_curvature`).
        let mut margins: Vec<(SimmRiskClass, f64)> =
            risk_class_margins.iter().map(|(rc, m)| (*rc, *m)).collect();
        margins.sort_by_key(|(rc, _)| *rc as u8);
        let ks: Vec<f64> = margins.iter().map(|(_, m)| *m).collect();
        correlated_norm(&ks, |i, j| {
            self.params.correlation(margins[i].0, margins[j].0)
        })
    }
}

impl ImCalculator for SimmCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let mtm = instrument.mtm_for_vm(context, as_of)?;
        let currency = mtm.currency();
        let sensitivities = instrument.simm_sensitivities(context, as_of)?;
        let result = self.calculate_from_sensitivities(&sensitivities, currency, as_of)?;
        debug!(
            instrument = instrument.id(),
            total_im = result.amount.amount(),
            risk_classes = result.breakdown.len(),
            "SIMM IM calculated"
        );
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::validate_simm_params;
    use crate::types::SimmCurvatureSensitivity;
    fn test_date() -> Date {
        Date::from_calendar_date(2025, time::Month::January, 15).expect("date")
    }
    fn usd_tenors(values: &FxHashMap<String, f64>) -> FxHashMap<(Currency, String), f64> {
        values
            .iter()
            .map(|(tenor, amount)| ((Currency::USD, tenor.clone()), *amount))
            .collect()
    }
    fn single_credit(amount: f64) -> FxHashMap<(String, String), f64> {
        [(("NAME".to_string(), "5Y".to_string()), amount)]
            .into_iter()
            .collect()
    }
    use super::*;
    use crate::Marginable;

    #[test]
    fn simm_version_display() {
        assert_eq!(SimmVersion::V2_6.to_string(), "SIMM v2.6");
        assert_eq!(SimmVersion::V2_6.as_str(), "v2_6");
        assert!(
            "v2_5".parse::<SimmVersion>().is_err(),
            "v2.5 tables are not shipped, so the version is not selectable"
        );
        for noncanonical in ["SIMM 2.6", "2_6", "v2.6", " V2_6"] {
            assert!(noncanonical.parse::<SimmVersion>().is_err());
        }
    }

    #[test]
    fn embedded_simm_registries_pass_validation() {
        SimmCalculator::new(SimmVersion::V2_6)
            .expect("embedded SIMM v2.6 registry should validate");
    }

    #[test]
    fn validate_rejects_missing_ir_tenor_pair() {
        let mut params = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .params;
        let mut tenors = params.ir_delta_weights.keys();
        let a = tenors
            .next()
            .expect("embedded SIMM registry should define at least two IR tenors");
        let b = tenors
            .next()
            .expect("embedded SIMM registry should define at least two IR tenors");
        let pair = ordered_tenor_pair(a, b);
        params.ir_tenor_correlations.remove(&pair);
        let err =
            validate_simm_params(&params).expect_err("should reject missing ir_tenor_correlations");
        let msg = err.to_string();
        assert!(
            msg.contains("ir_tenor_correlations"),
            "error should name the map: {msg}"
        );
    }

    #[test]
    fn ir_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        // Single-tenor: correlation matrix is 1.0 on diagonal so
        // result = sqrt((dv01 * weight)^2) = |dv01 * weight|
        let dv01_by_tenor: FxHashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0), // $100K DV01 at 5y
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.aggregate_ir(&usd_tenors(&dv01_by_tenor), false, None);

        // ISDA SIMM v2.6 D.1 p14: regular-currency 5Y weight is 60bp.
        assert!((ir_margin - 100_000.0 * 60.0).abs() < 1.0);
    }

    #[test]
    fn credit_non_qualifying_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0;
        let cnq_margin = calc.non_qualifying_margin(&single_credit(cs01), false, None);

        // ISDA SIMM v2.6 F.1 p18: unclassified non-qualifying credit
        // uses the 1300bp residual weight, below the USD500K/bp threshold.
        assert!((cnq_margin - 50_000.0 * 1300.0).abs() < 1.0);
    }

    #[test]
    fn fx_delta_bucketed_preserves_partial_offset() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut fx_delta = FxHashMap::default();
        fx_delta.insert(Currency::EUR, 100.0);
        fx_delta.insert(Currency::JPY, -100.0);

        let actual = calc.calculate_fx_delta_bucketed(&fx_delta, None);
        let expected = 100.0 * calc.params.fx_delta_weight;

        assert!(
            (actual - expected).abs() < 1e-9,
            "opposite FX deltas should aggregate with rho=0.5: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn params_loaded() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        assert_eq!(calc.version(), SimmVersion::V2_6);
        assert!(calc.params.ir_delta_weights.contains_key("5Y"));
        assert!(
            calc.params
                .cq_bucket_weights
                .contains_key(&SimmCreditSector::Financial)
        );
    }

    #[test]
    fn aggregation() {
        let calc = SimmCalculator::new(SimmVersion::default()).expect("embedded registry");

        let risk_class_margins: FxHashMap<SimmRiskClass, f64> = [
            (SimmRiskClass::InterestRate, 1_000_000.0),
            (SimmRiskClass::CreditQualifying, 500_000.0),
        ]
        .into_iter()
        .collect();

        let total = calc.aggregate_risk_classes(&risk_class_margins);

        // ISDA v2.6 K: IR/CQ correlation is 4%.
        let expected = (1e12_f64 + 0.25e12 + 2.0 * 0.04 * 1e6 * 0.5e6).sqrt();
        assert!((total - expected).abs() < 1e-6);
    }

    #[test]
    fn calculate_from_sensitivities_uses_risk_class_correlation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);

        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let total_im = result.amount.amount();
        let breakdown = &result.breakdown;

        let ir_margin = breakdown
            .get("IR_Delta")
            .expect("IR margin present")
            .amount();
        let eq_margin = breakdown
            .get("Equity_Delta")
            .expect("Equity margin present")
            .amount();

        let expected =
            (ir_margin * ir_margin + eq_margin * eq_margin + 2.0 * 0.07 * ir_margin * eq_margin)
                .sqrt();
        assert!((total_im - expected).abs() < 1.0);
    }

    #[test]
    fn ir_delta_multi_tenor_with_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let dv01_by_tenor: FxHashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0),
            ("10Y".to_string(), -80_000.0), // Partially hedged
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.aggregate_ir(&usd_tenors(&dv01_by_tenor), false, None);

        // ws_5y = 100K*51 = 5.1M, ws_10y = -80K*51 = -4.08M
        // With high tenor correlation (~0.96), the hedge offsets most of the risk
        // so margin should be much less than the uncorrelated sqrt(5.1^2 + 4.08^2) ≈ 6.53M
        assert!(ir_margin > 1_000_000.0);
        assert!(ir_margin < 3_000_000.0);
    }

    #[test]
    fn ir_vega_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let vega_by_tenor: FxHashMap<String, f64> =
            [("5Y".to_string(), 500_000.0)].into_iter().collect();

        let ir_vega_margin = calc.aggregate_ir(&usd_tenors(&vega_by_tenor), true, None);
        // D.1 paragraph 35: single tenor uses VRW 0.23; no IR vega HVR.
        assert!((ir_vega_margin - 115_000.0).abs() < 1.0);
    }

    #[test]
    fn m15_ir_vega_multi_currency_preserves_same_tenor_exposures() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_vega(Currency::USD, "5Y", 500_000.0);
        sensitivities.add_ir_vega(Currency::EUR, "5Y", 500_000.0);

        let result = calc
            .calculate_from_sensitivities(&sensitivities, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let ir_vega_margin = result
            .breakdown
            .get("IR_Vega")
            .expect("M-15: IR vega margin should be present")
            .amount();

        let single_currency_margin = calc.aggregate_ir(
            &usd_tenors(&[("5Y".to_string(), 500_000.0)].into_iter().collect()),
            true,
            None,
        );
        assert!(
            ir_vega_margin > single_currency_margin,
            "M-15: same-tenor multi-currency IR vega must not collapse to one exposure"
        );
    }

    #[test]
    fn curvature_is_computed_within_each_class_before_class_correlation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
        let inputs = [
            SimmCurvatureSensitivity {
                risk_class: SimmRiskClass::InterestRate,
                bucket: "USD".into(),
                factor: "OIS".into(),
                risk_tenor: Some("5Y".into()),
                expiry_tenor: "2W".into(),
                volatility_weighted_vega: 1000.0,
            },
            SimmCurvatureSensitivity {
                risk_class: SimmRiskClass::Equity,
                bucket: "residual".into(),
                factor: "ACME".into(),
                risk_tenor: None,
                expiry_tenor: "2W".into(),
                volatility_weighted_vega: 500.0,
            },
        ];
        let z2 = 2.575_829_303_548_900_4_f64.powi(2);
        let ir = 500.0 * z2 / 0.47_f64.powi(2);
        let equity = 250.0 * z2;
        let expected = (ir * ir + equity * equity + 2.0 * 0.07 * ir * equity).sqrt();
        let actual = calc.aggregate_risk_classes(
            &calc
                .curvature_by_risk_class(&inputs, None)
                .expect("curvature"),
        );
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn commodity_delta_uses_bucket_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let delta_by_bucket: FxHashMap<String, f64> =
            [("2".to_string(), 100_000.0), ("3".to_string(), -100_000.0)]
                .into_iter()
                .collect();

        let actual = calc.aggregate_commodity(&delta_by_bucket, false, None);
        let bucket_2 = 100_000.0 * calc.params.commodity_bucket_weights["2"];
        let bucket_3 = -100_000.0 * calc.params.commodity_bucket_weights["3"];
        let rho_23 = 0.92_f64;
        let expected =
            (bucket_2 * bucket_2 + bucket_3 * bucket_3 + 2.0 * rho_23 * bucket_2 * bucket_3).sqrt();

        assert!(
            (actual - expected).abs() < 1.0,
            "expected correlated commodity margin {}, got {}",
            expected,
            actual
        );
    }

    #[derive(Clone)]
    struct MarginableTestInstrument {
        id: String,
        value: Money,
        sensitivities: SimmSensitivities,
    }

    impl MarginableTestInstrument {
        fn new(value: Money, sensitivities: SimmSensitivities) -> Self {
            Self {
                id: "SIMM-TEST".to_string(),
                value,
                sensitivities,
            }
        }
    }

    impl Marginable for MarginableTestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&crate::types::OtcMarginSpec> {
            None
        }

        fn netting_set_id(&self) -> Option<crate::types::NettingSetId> {
            None
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<SimmSensitivities> {
            Ok(self.sensitivities.clone())
        }

        fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }
    }

    #[test]
    fn public_calculate_matches_full_simm_sensitivities() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        sensitivities.add_equity_delta("AAPL", 100_000.0);
        sensitivities.add_fx_delta(Currency::EUR, 80_000.0);

        let instrument = MarginableTestInstrument::new(
            Money::from((1_000_000_i64, Currency::USD)),
            sensitivities.clone(),
        );
        let market = MarketContext::new();

        let expected = calc
            .calculate_from_sensitivities(&sensitivities, Currency::USD, as_of)
            .expect("valid sensitivity fixture");
        let actual = calc
            .calculate(&instrument, &market, as_of)
            .expect("SIMM calculation should succeed");

        assert!(
            (actual.amount.amount() - expected.amount.amount()).abs() < 1e-2,
            "expected total {}, got {} with breakdown {:?}",
            expected.amount.amount(),
            actual.amount.amount(),
            actual.breakdown
        );
        for (key, expected_amount) in &expected.breakdown {
            let actual_amount = actual
                .breakdown
                .get(key)
                .expect("expected breakdown entry should be present");
            assert!(
                (actual_amount.amount() - expected_amount.amount()).abs() < 1e-2,
                "breakdown mismatch for {key}: expected {}, got {}",
                expected_amount.amount(),
                actual_amount.amount()
            );
        }
        assert!(actual.breakdown.contains_key("Equity_Delta"));
        assert!(actual.breakdown.contains_key("FX_Delta"));
    }

    // Bucketed credit qualifying delta tests

    #[test]
    fn bucketed_single_bucket_uses_sector_weight_and_concentration() {
        // When all sensitivities are in one bucket with one name, the bucketed
        // aggregation should produce K = |cs01 * sector_weight| multiplied by
        // the bucket concentration factor.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0;
        let sector = SimmCreditSector::BasicMaterials;
        let weight = calc.params.cq_bucket_weight(sector);
        let raw_ws = cs01 * weight;
        let cf = concentration(cs01, calc.params.cq_concentration_thresholds[&sector]);

        // Bucketed path: single name in one bucket.
        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector, "ISSUER_A".to_string(), "5Y".to_string()), cs01);
        let bucketed_margin = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        let expected = (raw_ws * cf).abs();
        assert!(
            (bucketed_margin - expected).abs() < 1.0,
            "bucketed mismatch: expected {expected}, got {bucketed_margin}"
        );
    }

    #[test]
    fn bucketed_diversification_reduces_margin() {
        // A diversified portfolio across multiple sectors should produce LOWER
        // margin than the equivalent scalar approach (which sums everything
        // into one bucket with no diversification benefit).
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_per_name = 1_000.0;

        // Bucketed path: spread across four different sectors.
        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert(
            (
                SimmCreditSector::Sovereign,
                "GOVT_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::BasicMaterials,
                "MINING_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        bucketed.insert(
            (
                SimmCreditSector::TechnologyMedia,
                "TECH_A".to_string(),
                "5Y".to_string(),
            ),
            cs01_per_name,
        );
        let bucketed_margin = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );
        let gross_weighted: f64 = bucketed
            .iter()
            .map(|((sector, _, _), cs01)| (cs01 * calc.params.cq_bucket_weight(*sector)).abs())
            .sum();

        assert!(
            bucketed_margin < gross_weighted,
            "diversified bucketed margin ({bucketed_margin}) should be less \
             than gross weighted margin ({gross_weighted})"
        );
        // The bucketed margin should still be positive.
        assert!(bucketed_margin > 0.0, "bucketed margin should be positive");
    }

    #[test]
    fn bucketed_inter_bucket_correlation_formula() {
        // Verify the inter-bucket aggregation formula directly.
        // Two buckets with known K values and inter-bucket correlation gamma.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_a = 100_000.0;
        let cs01_b = 80_000.0;
        let sector_a = SimmCreditSector::Sovereign;
        let sector_b = SimmCreditSector::Financial;

        let weight_a = calc.params.cq_bucket_weight(sector_a);
        let weight_b = calc.params.cq_bucket_weight(sector_b);

        let ws_a = cs01_a * weight_a;
        let ws_b = cs01_b * weight_b;
        let cf_a = concentration(cs01_a, calc.params.cq_concentration_thresholds[&sector_a]);
        let cf_b = concentration(cs01_b, calc.params.cq_concentration_thresholds[&sector_b]);

        // Single-name per bucket: K_b = |cs01 * weight * concentration_factor|
        let k_a = (ws_a * cf_a).abs();
        let k_b = (ws_b * cf_b).abs();
        let s_a = (ws_a * cf_a).clamp(-k_a, k_a);
        let s_b = (ws_b * cf_b).clamp(-k_b, k_b);

        let gamma = calc.params.cq_inter_bucket_correlation(sector_a, sector_b);

        // Expected: sqrt(K_a^2 + K_b^2 + 2*gamma*S_a*S_b)
        let expected = (k_a * k_a + k_b * k_b + 2.0 * gamma * s_a * s_b).sqrt();

        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector_a, "GOVT_A".to_string(), "5Y".to_string()), cs01_a);
        bucketed.insert((sector_b, "BANK_A".to_string(), "5Y".to_string()), cs01_b);
        let actual = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        assert!(
            (actual - expected).abs() < 1.0,
            "inter-bucket formula: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn bucketed_intra_bucket_two_names() {
        // Verify intra-bucket aggregation with two names in the same sector.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01_1 = 60_000.0;
        let cs01_2 = 40_000.0;
        let sector = SimmCreditSector::Financial;
        let weight = calc.params.cq_bucket_weight(sector);
        let rho = calc.params.cq_intra_bucket_correlation;

        let ws_1 = cs01_1 * weight;
        let ws_2 = cs01_2 * weight;
        let cf_1 = concentration(cs01_1, calc.params.cq_concentration_thresholds[&sector]);
        let cf_2 = concentration(cs01_2, calc.params.cq_concentration_thresholds[&sector]);

        // K_b = sqrt((cf*ws_1)^2 + (cf*ws_2)^2 + 2*rho*(cf*ws_1)*(cf*ws_2))
        let scaled_1 = cf_1 * ws_1;
        let scaled_2 = cf_2 * ws_2;
        let expected = (scaled_1 * scaled_1
            + scaled_2 * scaled_2
            + 2.0 * rho * (cf_1.min(cf_2) / cf_1.max(cf_2)) * scaled_1 * scaled_2)
            .sqrt();

        let mut bucketed: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        bucketed.insert((sector, "BANK_A".to_string(), "5Y".to_string()), cs01_1);
        bucketed.insert((sector, "BANK_B".to_string(), "5Y".to_string()), cs01_2);
        let actual = calc.aggregate_credit_qualifying(
            &bucketed,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );

        assert!(
            (actual - expected).abs() < 1.0,
            "intra-bucket two names: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn calculate_from_sensitivities_uses_sector_bucketed_credit_qualifying_delta() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_qualifying_delta(SimmCreditSector::Sovereign, "GOVT_A", "5Y", 50_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_A", "5Y", 50_000.0);

        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        let total_im = result.amount.amount();
        let breakdown = &result.breakdown;
        assert!(total_im > 0.0, "total IM should be positive");
        assert!(
            breakdown.contains_key("Credit_Qualifying_Delta"),
            "breakdown should contain Credit_Qualifying_Delta"
        );

        // The bucketed margin should match the direct bucketed calculation.
        let expected = calc.aggregate_credit_qualifying(
            &sens.credit_qualifying_delta,
            |s| calc.params.cq_bucket_weight(s),
            true,
            None,
        );
        let actual = breakdown
            .get("Credit_Qualifying_Delta")
            .expect("CQ delta breakdown entry")
            .amount();
        assert!(
            (actual - expected).abs() < 1.0,
            "calculate_from_sensitivities should delegate to bucketed: \
             expected {expected}, got {actual}"
        );
    }

    /// Netting set exercising every newly wired vega risk class.
    ///
    /// An IR delta anchor is included so the correlated risk-class
    /// aggregation is exercised end to end rather than degenerating to a
    /// single-class square root.
    fn credit_commodity_vega_sensitivities() -> SimmSensitivities {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 10_000.0);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 25_000.0);
        sens.add_credit_non_qualifying_vega("RMBS_A", "5Y", 15_000.0);
        sens.add_commodity_vega("Crude", 20_000.0);
        sens
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn cq_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.cq_vega_weight = base.params.cq_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "cq_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn cnq_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.cnq_vega_weight = base.params.cnq_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "cnq_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Guard against the registry weight going silently inert: doubling the
    /// weight MUST move the margin number.
    #[test]
    fn commodity_vega_weight_is_read_by_the_calculation() {
        let base = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut bumped = base.clone();
        bumped.params.commodity_vega_weight = base.params.commodity_vega_weight * 2.0;

        let sens = credit_commodity_vega_sensitivities();
        let baseline = base
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_bump = bumped
            .calculate_from_sensitivities(&sens, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        assert!(baseline > 0.0, "baseline margin must be positive");
        assert_ne!(
            baseline, with_bump,
            "commodity_vega_weight must change the SIMM margin"
        );
        assert!(with_bump > baseline, "a larger vega weight must raise IM");
    }

    /// Each vega sensitivity family must move the aggregate margin, proving the
    /// input path reaches the calculation rather than being dropped.
    #[test]
    fn credit_and_commodity_vega_sensitivities_change_total_margin() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut anchor = SimmSensitivities::new(Currency::USD);
        anchor.add_ir_delta(Currency::USD, "5Y", 10_000.0);
        let baseline = calc
            .calculate_from_sensitivities(&anchor, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();

        let mut with_cq = anchor.clone();
        with_cq.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 25_000.0);
        let cq = calc
            .calculate_from_sensitivities(&with_cq, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(baseline, cq.amount.amount(), "CQ vega must change total IM");
        assert!(cq.breakdown.contains_key("Credit_Qualifying_Vega"));

        let mut with_cnq = anchor.clone();
        with_cnq.add_credit_non_qualifying_vega("RMBS_A", "5Y", 15_000.0);
        let cnq = calc
            .calculate_from_sensitivities(&with_cnq, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(
            baseline,
            cnq.amount.amount(),
            "CNQ vega must change total IM"
        );
        assert!(cnq.breakdown.contains_key("Credit_NonQualifying_Vega"));

        let mut with_commodity = anchor.clone();
        with_commodity.add_commodity_vega("Crude", 20_000.0);
        let commodity = calc
            .calculate_from_sensitivities(&with_commodity, Currency::USD, test_date())
            .expect("valid sensitivity fixture");
        assert_ne!(
            baseline,
            commodity.amount.amount(),
            "commodity vega must change total IM"
        );
        assert!(commodity.breakdown.contains_key("Commodity_Vega"));
    }

    #[test]
    fn zero_credit_and_commodity_vega_contributes_nothing() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut anchor = SimmSensitivities::new(Currency::USD);
        anchor.add_ir_delta(Currency::USD, "5Y", 10_000.0);

        let mut zeroed = anchor.clone();
        zeroed.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 0.0);
        zeroed.add_credit_non_qualifying_vega("RMBS_A", "5Y", 0.0);
        zeroed.add_commodity_vega("Crude", 0.0);

        let baseline = calc
            .calculate_from_sensitivities(&anchor, Currency::USD, test_date())
            .expect("valid sensitivity fixture")
            .amount
            .amount();
        let with_zero = calc
            .calculate_from_sensitivities(&zeroed, Currency::USD, test_date())
            .expect("valid sensitivity fixture");

        assert_eq!(
            baseline,
            with_zero.amount.amount(),
            "zero vega must not move the margin"
        );
        assert!(!with_zero.breakdown.contains_key("Credit_Qualifying_Vega"));
        assert!(
            !with_zero
                .breakdown
                .contains_key("Credit_NonQualifying_Vega")
        );
        assert!(!with_zero.breakdown.contains_key("Commodity_Vega"));

        assert_eq!(
            calc.aggregate_credit_qualifying(
                &FxHashMap::default(),
                |_| calc.params.cq_vega_weight,
                false,
                None,
            ),
            0.0
        );
        assert_eq!(
            calc.non_qualifying_margin(&single_credit(0.0), true, None),
            0.0
        );
        assert_eq!(
            calc.aggregate_commodity(&FxHashMap::default(), true, None),
            0.0
        );
    }

    /// A single name / single bucket reduces to `|vega * VRW|` for each of the
    /// three risk classes, mirroring the equivalent single-factor delta cases.
    #[test]
    fn single_factor_credit_and_commodity_vega_match_closed_form() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut cq: FxHashMap<(SimmCreditSector, String, String), f64> = FxHashMap::default();
        cq.insert(
            (
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            ),
            25_000.0,
        );
        let expected_cq = 25_000.0 * calc.params.cq_vega_weight;
        assert!(
            (calc.aggregate_credit_qualifying(&cq, |_| calc.params.cq_vega_weight, false, None)
                - expected_cq)
                .abs()
                < 1e-6
        );

        let expected_cnq = 15_000.0 * calc.params.cnq_vega_weight;
        assert!(
            (calc.non_qualifying_margin(&single_credit(15_000.0), true, None) - expected_cnq).abs()
                < 1e-6
        );
        assert!(
            (calc.non_qualifying_margin(&single_credit(-15_000.0), true, None) - expected_cnq)
                .abs()
                < 1e-6,
            "vega margin is sign-insensitive, matching the delta path"
        );

        let mut commodity: FxHashMap<String, f64> = FxHashMap::default();
        commodity.insert("Crude".to_string(), 20_000.0);
        let expected_commodity = 20_000.0 * 0.74 * calc.params.commodity_vega_weight;
        assert!(
            (calc.aggregate_commodity(&commodity, true, None) - expected_commodity).abs() < 1e-6
        );
    }

    #[test]
    fn registry_ir_tenors_match_the_published_simm_tenor_set() {
        let calc = SimmCalculator::new(SimmVersion::default()).expect("registry should load");
        let mut registry: Vec<&str> = calc
            .params
            .ir_delta_weights
            .keys()
            .map(String::as_str)
            .collect();
        registry.sort_unstable();
        let mut published: Vec<&str> = crate::types::SIMM_TENORS.to_vec();
        published.sort_unstable();
        assert_eq!(registry, published, "registry tenors drifted");
    }

    #[test]
    fn typed_calculation_rejects_unknown_tenor_instead_of_zero_margin() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let as_of = test_date();
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "7Y", 50_000.0);
        let err = calc
            .calculate_from_sensitivities(&sens, Currency::USD, as_of)
            .expect_err("7Y must be rejected");
        assert!(err.to_string().contains("7Y"), "{err}");

        let mut good = SimmSensitivities::new(Currency::USD);
        good.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        let result = calc
            .calculate_from_sensitivities(&good, Currency::USD, as_of)
            .expect("valid tenor prices");
        assert!(result.amount.amount() > 0.0);
        assert_eq!(result.mpor_days, calc.mpor_days());
        assert!(result.breakdown.contains_key("IR_Delta"));
    }

    fn detail_fixture() -> SimmSensitivities {
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "2Y", 4.0e9); // breaches the concentration threshold
        sens.add_ir_delta(Currency::USD, "10Y", -2.5e9);
        sens.add_ir_delta(Currency::EUR, "5Y", 60_000.0);
        sens.add_ir_delta(Currency::JPY, "5Y", -45_000.0);
        sens.add_ir_vega(Currency::USD, "5Y", 500_000.0);
        sens.add_ir_vega(Currency::EUR, "10Y", -200_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_A", "5Y", 30_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Financial, "BANK_B", "5Y", -12_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Sovereign, "SOV_A", "10Y", 9_000.0);
        sens.add_credit_qualifying_delta(SimmCreditSector::Residual, "UNRATED", "5Y", 2_000.0);
        sens.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 7_000.0);
        sens.add_credit_non_qualifying_delta("RMBS_1", "5Y", 15_000.0);
        sens.add_credit_non_qualifying_delta("RMBS_2", "5Y", -4_000.0);
        sens.add_credit_non_qualifying_vega("RMBS_1", "5Y", 3_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);
        sens.add_equity_delta("MSFT", -70_000.0);
        sens.add_equity_vega("AAPL", 40_000.0);
        sens.add_fx_delta(Currency::EUR, 400_000.0);
        sens.add_fx_delta(Currency::BRL, -150_000.0);
        sens.add_fx_vega(Currency::EUR, Currency::USD, 90_000.0);
        sens.add_fx_vega(Currency::USD, Currency::JPY, -35_000.0);
        sens.add_commodity_delta("2", 80_000.0);
        sens.add_commodity_delta("12", -30_000.0);
        sens.add_commodity_vega("2", 20_000.0);
        sens.add_curvature(SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::InterestRate,
            bucket: "USD".into(),
            factor: "OIS".into(),
            risk_tenor: Some("5Y".into()),
            expiry_tenor: "2W".into(),
            volatility_weighted_vega: 1000.0,
        });
        sens.add_curvature(SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::Equity,
            bucket: "residual".into(),
            factor: "ACME".into(),
            risk_tenor: None,
            expiry_tenor: "2W".into(),
            volatility_weighted_vega: 500.0,
        });
        sens
    }

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
    }

    #[test]
    fn simm_detail_rebuilds_breakdown_and_total() {
        let calc = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .with_mpor(20);
        let sens = detail_fixture();
        let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("date");
        let result = calc
            .calculate_from_sensitivities(&sens, Currency::USD, as_of)
            .expect("SIMM");
        let detail = result.simm_detail.as_ref().expect("SIMM detail");

        // One component per breakdown key; margin × MPOR scale is the breakdown amount.
        assert!(close(detail.mpor_scale, 2.0_f64.sqrt()));
        let mut labels: Vec<&str> = detail
            .components
            .iter()
            .map(|c| c.component.as_str())
            .collect();
        labels.sort_unstable();
        assert_eq!(
            labels,
            result
                .breakdown
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            labels.len(),
            14,
            "every delta, vega and curvature component"
        );
        let mut by_class: std::collections::BTreeMap<SimmRiskClass, f64> =
            std::collections::BTreeMap::new();
        for component in &detail.components {
            assert!(close(
                component.margin * detail.mpor_scale,
                result.breakdown[&component.component].amount()
            ));
            *by_class.entry(component.risk_class).or_insert(0.0) += component.margin;

            // Weighted sensitivities replay from their recorded inputs.
            for bucket in &component.buckets {
                let mut sum = 0.0;
                for row in &bucket.weighted_sensitivities {
                    let hvr = if component.component.ends_with("_Curvature") {
                        1.0
                    } else {
                        component.historical_volatility_ratio
                    };
                    assert!(close(
                        row.weighted_sensitivity,
                        row.sensitivity * hvr * row.risk_weight * row.concentration_factor
                    ));
                    assert!(row.concentration_factor >= 1.0);
                    sum += row.weighted_sensitivity;
                }
                assert!(close(bucket.signed_sum, sum.clamp(-bucket.k, bucket.k)));
            }
        }

        // Risk-class margins are the component sums; the total is their correlated norm.
        assert_eq!(
            by_class.keys().collect::<Vec<_>>(),
            detail.risk_class_margins.keys().collect::<Vec<_>>()
        );
        for (class, margin) in &detail.risk_class_margins {
            assert!(close(*margin, by_class[class]));
        }
        let classes: Vec<(SimmRiskClass, f64)> = detail
            .risk_class_margins
            .iter()
            .map(|(class, margin)| (*class, *margin))
            .collect();
        let mut variance = 0.0;
        for (a, k_a) in &classes {
            for (b, k_b) in &classes {
                variance += calc.params.correlation(*a, *b) * k_a * k_b;
            }
        }
        assert!(close(
            variance.sqrt() * detail.mpor_scale,
            result.amount.amount()
        ));

        let component = |label: &str| {
            detail
                .components
                .iter()
                .find(|c| c.component == label)
                .unwrap_or_else(|| panic!("{label} detail"))
        };

        // IR delta: bucket K from the tenor correlations, then the currency aggregation.
        let ir = component("IR_Delta");
        let mut currencies: Vec<&str> = ir.buckets.iter().map(|b| b.bucket.as_str()).collect();
        currencies.sort_unstable();
        assert_eq!(currencies, ["EUR", "JPY", "USD"]);
        let mut ir_buckets = Vec::new();
        for bucket in &ir.buckets {
            let rows = &bucket.weighted_sensitivities;
            let mut k2 = 0.0;
            for a in rows {
                for b in rows {
                    let rho = match (&a.tenor, &b.tenor) {
                        (Some(x), Some(y)) if x == y => 1.0,
                        (Some(x), Some(y)) => {
                            calc.params.ir_tenor_correlations[&ordered_tenor_pair(x, y)]
                        }
                        _ => unreachable!("IR rows carry tenors"),
                    };
                    k2 += rho * a.weighted_sensitivity * b.weighted_sensitivity;
                }
            }
            assert!(close(bucket.k, k2.sqrt()));
            ir_buckets.push((bucket.k, bucket.signed_sum, rows[0].concentration_factor));
        }
        assert!(close(ir.margin, calc.aggregate_ir_currencies(&ir_buckets)));
        let usd = ir
            .buckets
            .iter()
            .find(|b| b.bucket == "USD")
            .expect("USD bucket");
        assert!(
            usd.weighted_sensitivities[0].concentration_factor > 1.0,
            "the USD position breaches its concentration threshold"
        );

        // Equity delta: one bucket per name, no correlation between names.
        let equity = component("Equity_Delta");
        assert_eq!(equity.buckets.len(), 2);
        let equity_k2: f64 = equity.buckets.iter().map(|b| b.k * b.k).sum();
        assert!(close(equity.margin, equity_k2.sqrt()));

        // Commodity delta: one net factor per bucket, correlated across buckets.
        let commodity = component("Commodity_Delta");
        let mut commodity_k2 = 0.0;
        for a in &commodity.buckets {
            for b in &commodity.buckets {
                let rho = if a.bucket == b.bucket {
                    1.0
                } else {
                    calc.params.commodity_inter_bucket_correlation(
                        a.bucket.parse().expect("bucket id"),
                        b.bucket.parse().expect("bucket id"),
                    )
                };
                commodity_k2 += rho * a.signed_sum * b.signed_sum;
            }
        }
        assert!(close(commodity.margin, commodity_k2.sqrt()));

        // Credit qualifying: sector buckets plus the residual bucket added outside the norm.
        let cq = component("Credit_Qualifying_Delta");
        assert_eq!(
            cq.buckets
                .iter()
                .map(|b| b.bucket.as_str())
                .collect::<Vec<_>>(),
            ["sovereign", "financial", "residual"]
        );
        let (sov, fin, residual) = (&cq.buckets[0], &cq.buckets[1], &cq.buckets[2]);
        let gamma = calc
            .params
            .cq_inter_bucket_correlation(SimmCreditSector::Sovereign, SimmCreditSector::Financial);
        let cq_expected = residual.k
            + (sov.k * sov.k + fin.k * fin.k + 2.0 * gamma * sov.signed_sum * fin.signed_sum)
                .sqrt();
        assert!(close(cq.margin, cq_expected));

        // Single-bucket components: the bucket K is the component margin.
        for label in [
            "Credit_NonQualifying_Delta",
            "Credit_NonQualifying_Vega",
            "FX_Delta",
            "FX_Vega",
        ] {
            let single = component(label);
            assert_eq!(single.buckets.len(), 1);
            assert_eq!(single.buckets[0].k, single.margin);
        }
        assert_eq!(
            component("FX_Vega").historical_volatility_ratio,
            calc.params.fx_historical_volatility_ratio
        );

        // The detail survives the wire and is absent for other methodologies.
        let json = serde_json::to_string(&result).expect("serialize");
        let back: ImResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.simm_detail.map(|d| d.components.len()),
            Some(detail.components.len())
        );
        let schedule = ImResult::simple(result.amount, ImMethodology::Schedule, as_of, 10);
        assert!(schedule.simm_detail.is_none());
        assert!(
            !serde_json::to_string(&schedule)
                .expect("serialize")
                .contains("simm_detail")
        );
    }
}

/// Independent transcription of historical ISDA SIMM v2.6 tables.
///
/// Source: https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf
/// PDF SHA256 57e9d2e9e080e27abc92197923b019e40eb2fe60d9c82ce2a0f8238b7c4bf8e0.
/// Reread 2026-09-09. Provenance: tests/fixtures/simm_v26_provenance.md.
#[cfg(test)]
mod simm_schedule_parity_tests {
    use super::{SimmCalculator, SimmVersion};
    use crate::types::{SIMM_TENORS, SimmRiskClass};

    #[test]
    fn published_ir_weights_and_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
        let p = calc.get_params();
        // D.1 tables 1-3, printed page 14.
        for (table, expected) in [
            (
                &p.ir_delta_weights,
                [109., 105., 90., 71., 66., 66., 64., 60., 60., 61., 61., 67.],
            ),
            (
                &p.ir_delta_weights_low,
                [15., 18., 9., 11., 13., 15., 19., 23., 23., 22., 22., 23.],
            ),
            (
                &p.ir_delta_weights_high,
                [
                    163., 109., 87., 89., 102., 96., 101., 97., 97., 102., 106., 101.,
                ],
            ),
        ] {
            assert_eq!(table.len(), 12);
            for (tenor, weight) in SIMM_TENORS.iter().zip(expected) {
                assert_eq!(table[*tenor], weight);
            }
        }
        // D.2 paragraph 36, printed pages 14-15, strict upper triangle in tenor order.
        let rows: &[&[f64]] = &[
            &[77., 67., 59., 48., 39., 34., 30., 25., 23., 21., 20.],
            &[84., 74., 56., 43., 36., 31., 26., 21., 19., 19.],
            &[88., 69., 55., 47., 40., 34., 27., 25., 25.],
            &[86., 73., 65., 57., 49., 40., 38., 37.],
            &[94., 87., 79., 68., 60., 57., 55.],
            &[96., 91., 80., 74., 70., 69.],
            &[97., 88., 81., 77., 76.],
            &[95., 90., 86., 85.],
            &[97., 94., 94.],
            &[98., 97.],
            &[99.],
        ];
        assert_eq!(p.ir_tenor_correlations.len(), 66);
        for (i, row) in rows.iter().enumerate() {
            for (offset, percent) in row.iter().enumerate() {
                let a = SIMM_TENORS[i].to_owned();
                let b = SIMM_TENORS[i + offset + 1].to_owned();
                let actual = p
                    .ir_tenor_correlations
                    .get(&(a.clone(), b.clone()))
                    .or_else(|| p.ir_tenor_correlations.get(&(b, a)))
                    .expect("pair");
                assert_eq!(*actual, percent / 100.0);
            }
        }
        assert_eq!(p.ir_inter_currency_correlation, 0.32);
        assert_eq!(p.ir_subcurve_correlation, 0.993);
        assert_eq!(p.ir_historical_volatility_ratio, 0.47);
        assert_eq!(p.ir_vega_weight, 0.23);
    }

    #[test]
    fn published_non_ir_weights_and_historical_ratios() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        // E through I: selected supported residual classes and every commodity bucket.
        assert_eq!(p.cnq_delta_weight, 1300.0);
        assert_eq!(p.equity_delta_weight, 50.0);
        assert_eq!(p.fx_delta_weight, 7.4);
        assert_eq!(p.fx_high_delta_weight, 14.7);
        assert_eq!(p.cq_vega_weight, 0.76);
        assert_eq!(p.cnq_vega_weight, 0.76);
        assert_eq!(p.equity_vega_weight, 0.45);
        assert_eq!(p.fx_vega_weight, 0.48);
        assert_eq!(p.commodity_vega_weight, 0.55);
        assert_eq!(p.equity_historical_volatility_ratio, 0.60);
        assert_eq!(p.fx_historical_volatility_ratio, 0.57);
        assert_eq!(p.commodity_historical_volatility_ratio, 0.74);
        assert_eq!(p.cq_same_issuer_correlation, 0.93);
        assert_eq!(p.cq_intra_bucket_correlation, 0.46);
        assert_eq!(p.credit_residual_correlation, 0.50);
        assert_eq!(p.fx_intra_bucket_correlation, 0.50);
        assert_eq!(p.fx_regular_high_correlation, 0.25);
        assert_eq!(p.fx_high_high_correlation, -0.05);
        let weights = [
            48., 29., 33., 25., 35., 30., 60., 52., 68., 63., 21., 21., 15., 16., 13., 68., 17.,
        ];
        let correlations = [
            83., 97., 93., 97., 98., 90., 98., 49., 80., 46., 58., 53., 62., 16., 18., 0., 38.,
        ];
        for i in 0..17 {
            let key = (i + 1).to_string();
            assert_eq!(p.commodity_bucket_weights[&key], weights[i]);
            assert_eq!(
                p.commodity_intra_bucket_correlations[&key],
                correlations[i] / 100.0
            );
        }
    }

    #[test]
    fn published_raw_concentration_thresholds() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        // J, printed pages 26-28. Table units USD millions, per bp/% for delta.
        for (key, delta, vega) in [
            ("high", 30., 74.),
            ("regular_well_traded", 330., 4900.),
            ("regular_less_traded", 130., 520.),
            ("low", 61., 970.),
        ] {
            assert_eq!(p.ir_delta_concentration_thresholds[key], delta * 1e6);
            assert_eq!(p.ir_vega_concentration_thresholds[key], vega * 1e6);
        }
        for (key, threshold) in [("1", 3300.), ("2", 880.), ("3", 170.)] {
            assert_eq!(p.fx_delta_concentration_thresholds[key], threshold * 1e6);
        }
        for (key, threshold) in [
            ("1_1", 2800.),
            ("1_2", 1400.),
            ("1_3", 590.),
            ("2_2", 520.),
            ("2_3", 340.),
            ("3_3", 210.),
        ] {
            assert_eq!(p.fx_vega_concentration_thresholds[key], threshold * 1e6);
        }
        let delta = [
            310., 2100., 1700., 1700., 1700., 2800., 2800., 2700., 2700., 52., 530., 1300., 100.,
            100., 100., 52., 4000.,
        ];
        let vega = [
            390., 2900., 310., 310., 310., 6300., 6300., 1200., 1200., 120., 390., 1300., 590.,
            590., 590., 69., 69.,
        ];
        for i in 0..17 {
            let key = (i + 1).to_string();
            assert_eq!(
                p.commodity_delta_concentration_thresholds[&key],
                delta[i] * 1e6
            );
            assert_eq!(
                p.commodity_vega_concentration_thresholds[&key],
                vega[i] * 1e6
            );
        }
        for (class, delta, vega) in [
            (SimmRiskClass::Equity, 0.37, 39.),
            (SimmRiskClass::CreditNonQualifying, 0.50, 70.),
        ] {
            assert_eq!(p.concentration_thresholds[&class], delta * 1e6);
            assert_eq!(p.vega_concentration_thresholds[&class], vega * 1e6);
        }
        assert_eq!(
            p.vega_concentration_thresholds[&SimmRiskClass::CreditQualifying],
            360e6
        );
    }

    #[test]
    fn published_cross_risk_class_correlations() {
        use SimmRiskClass::*;
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
        let p = calc.get_params();
        let classes = [
            InterestRate,
            CreditQualifying,
            CreditNonQualifying,
            Equity,
            Commodity,
            Fx,
        ];
        let rows: &[&[f64]] = &[
            &[4., 4., 7., 37., 14.],
            &[54., 70., 27., 37.],
            &[46., 24., 15.],
            &[35., 39.],
            &[35.],
        ];
        for (i, row) in rows.iter().enumerate() {
            for (j, percent) in row.iter().enumerate() {
                let a = classes[i];
                let b = classes[i + j + 1];
                assert_eq!(
                    *p.risk_class_correlations
                        .get(&(a, b))
                        .or_else(|| p.risk_class_correlations.get(&(b, a)))
                        .expect("pair"),
                    percent / 100.0
                );
            }
        }
    }
}
