//! SIMM version enum, parameter resolution, bucket/tenor helpers, and detail collector.

use crate::calculators::traits::{
    SimmBucketDetail, SimmComponentDetail, SimmDetail, SimmWeightedSensitivity,
};
use crate::registry::{MarginRegistry, SimmParams};
use crate::types::{SimmCreditSector, SimmRiskClass, ordered_pair, ordered_tenor_pair};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

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
pub(super) fn ir_currency_group(currency: Currency) -> &'static str {
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

pub(super) fn fx_high_volatility(currency: Currency) -> bool {
    matches!(currency, Currency::BRL | Currency::RUB | Currency::TRY)
}

pub(super) fn fx_category(currency: Currency) -> &'static str {
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

pub(super) fn concentration(raw_sensitivity: f64, threshold: f64) -> f64 {
    (raw_sensitivity.abs() / threshold).sqrt().max(1.0)
}

/// Optional collector for per-bucket aggregation detail. `None` on the
/// margin-only paths, so they build no detail rows.
pub(super) type BucketSink<'a> = Option<&'a mut Vec<SimmBucketDetail>>;

/// Serde label of a credit sector, used as its bucket label.
pub(super) fn sector_label(sector: SimmCreditSector) -> String {
    finstack_quant_core::wire::serde_label(&sector).unwrap_or_else(|_| format!("{sector:?}"))
}

/// Accumulates [`SimmComponentDetail`] rows, breakdown entries, and risk-class margins.
pub(super) struct DetailCollector {
    pub(super) currency: Currency,
    /// Buckets of the component currently being aggregated.
    pub(super) scratch: Vec<SimmBucketDetail>,
    pub(super) components: Vec<SimmComponentDetail>,
    pub(super) curvature: Vec<SimmComponentDetail>,
    pub(super) breakdown: FxHashMap<String, Money>,
    pub(super) risk_class_margins: FxHashMap<SimmRiskClass, f64>,
}

impl DetailCollector {
    pub(super) fn new(currency: Currency) -> Self {
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
    pub(super) fn sink(&mut self) -> BucketSink<'_> {
        Some(&mut self.scratch)
    }

    /// Close the component whose buckets were just written to [`Self::sink`].
    /// Components with no positive margin are dropped, matching `breakdown`.
    pub(super) fn record(
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

    pub(super) fn finish(
        mut self,
        mpor_scale: f64,
    ) -> Result<(FxHashMap<String, Money>, SimmDetail)> {
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
pub(super) fn single_bucket(
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
    pub(super) fn correlation(&self, a: SimmRiskClass, b: SimmRiskClass) -> f64 {
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
    pub(super) fn cq_bucket_weight(&self, sector: SimmCreditSector) -> f64 {
        self.cq_bucket_weights.get(&sector).copied().unwrap_or(0.0)
    }

    pub(super) fn cq_inter_bucket_correlation(
        &self,
        a: SimmCreditSector,
        b: SimmCreditSector,
    ) -> f64 {
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
    pub(super) fn commodity_inter_bucket_correlation(&self, a: u8, b: u8) -> f64 {
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

pub(super) fn resolve_simm_params(
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
pub(super) const SIMM_CURVATURE_Z: f64 = 2.575_829_303_548_900_4;

/// Pre-computed flat correlation matrix for IR tenor lookups.
/// Avoids per-lookup String allocations in the O(n^2) delta/vega loops.
#[derive(Debug, Clone)]
pub(super) struct IrTenorCorrelationMatrix {
    pub(super) tenor_to_idx: FxHashMap<String, usize>,
    pub(super) matrix: Vec<f64>,
    pub(super) n: usize,
}

impl IrTenorCorrelationMatrix {
    pub(super) fn build(params: &SimmParams) -> Self {
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

    pub(super) fn correlation(&self, idx_a: usize, idx_b: usize) -> f64 {
        if idx_a == idx_b {
            return 1.0;
        }
        self.matrix[idx_a * self.n + idx_b]
    }
}
