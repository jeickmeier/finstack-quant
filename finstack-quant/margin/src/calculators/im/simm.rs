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
//! > **Implementation note:** `calculate_from_sensitivities_parts` applies intra-bucket
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
//!   [`SimmCalculator::calculate_from_sensitivities_parts`].
//!
//! # References
//!
//! - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
//! - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::registry::{
    embedded_registry, margin_registry_from_config, validate_simm_params, MarginRegistry,
    SimmParams,
};
use crate::regulatory::frtb::aggregation::{correlated_norm, inter_bucket_pairwise};
use crate::traits::Marginable;
use crate::types::ImMethodology;
use crate::types::{
    ordered_credit_sector_pair, ordered_risk_class_pair, ordered_tenor_pair, SimmCreditSector,
    SimmRiskClass, SimmSensitivities,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use tracing::debug;

/// SIMM version identifier.
///
/// Version choice controls the registry-backed risk weights, correlations, and
/// concentration thresholds used by the calculator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

// Lookup helpers for SimmParams fields.
impl SimmParams {
    fn correlation(&self, a: SimmRiskClass, b: SimmRiskClass) -> f64 {
        if a == b {
            return 1.0;
        }
        let key = ordered_risk_class_pair(a, b);
        self.risk_class_correlations
            .get(&key)
            .copied()
            .unwrap_or(1.0)
    }

    fn commodity_bucket_weight(&self, bucket: &str) -> f64 {
        let key = crate::types::commodity_bucket_id(bucket)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "other".to_string());
        self.commodity_bucket_weights
            .get(&key)
            .or_else(|| self.commodity_bucket_weights.get("other"))
            .copied()
            .unwrap_or(64.0)
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
        let key = ordered_credit_sector_pair(a, b);
        self.cq_inter_bucket_correlations
            .get(&key)
            .copied()
            .unwrap_or(0.0)
    }

    fn cq_concentration_factor(&self, sector: SimmCreditSector, net_ws: f64) -> f64 {
        if let Some(&threshold) = self.cq_concentration_thresholds.get(&sector) {
            if threshold > 0.0 && net_ws.abs() > threshold {
                (net_ws.abs() / threshold).sqrt()
            } else {
                1.0
            }
        } else {
            1.0
        }
    }

    /// Look up 1-based SIMM commodity buckets in the row-major registry matrix.
    /// Out-of-range ids contribute `0.0` correlation.
    fn commodity_inter_bucket_correlation(&self, a: u8, b: u8) -> f64 {
        let a = usize::from(a);
        let b = usize::from(b);
        if a == 0 || b == 0 || a > COMMODITY_BUCKET_COUNT || b > COMMODITY_BUCKET_COUNT {
            return 0.0;
        }
        let idx = (a - 1) * COMMODITY_BUCKET_COUNT + (b - 1);
        self.commodity_inter_bucket_correlations
            .get(idx)
            .copied()
            .unwrap_or(0.0)
    }
}

/// Number of SIMM commodity buckets used to validate registry correlation data.
pub(crate) const COMMODITY_BUCKET_COUNT: usize = 17;

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
    tenor_to_idx: HashMap<String, usize>,
    matrix: Vec<f64>,
    n: usize,
}

impl IrTenorCorrelationMatrix {
    fn build(params: &SimmParams) -> Self {
        let tenors: Vec<String> = params.ir_delta_weights.keys().cloned().collect();
        let n = tenors.len();
        let mut tenor_to_idx = HashMap::default();
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
    pub params: SimmParams,
    ir_corr_matrix: IrTenorCorrelationMatrix,
}

impl Default for SimmCalculator {
    #[allow(clippy::expect_used)] // Embedded margin registry is a compile-time asset.
    fn default() -> Self {
        Self::new(SimmVersion::V2_6).expect("embedded margin registry is a compile-time asset")
    }
}

impl SimmCalculator {
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
    /// the resolved SIMM parameters fail the completeness invariants checked by
    /// `validate_simm_params`.
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
    /// the merged SIMM parameters fail the completeness invariants checked by
    /// `validate_simm_params` — catches broken config overlays at load time
    /// rather than as silent miscalculations during margin runs.
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
        validate_simm_params(&params)?;
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

    /// Calculate IR delta margin with multi-currency aggregation.
    ///
    /// Per ISDA SIMM v2.6 methodology:
    /// 1. For each currency, compute the net weighted sensitivity
    ///    `net_c = sum_t WS_{c,t}` and the per-currency concentration
    ///    factor `CR_c = concentration_factor(InterestRate, net_c)`.
    /// 2. For each currency, compute `K_c` with `WS_{c,t}` scaled by
    ///    `CR_c` (uniform-by-currency convention), using the intra-
    ///    currency tenor correlations.
    /// 3. Aggregate across currencies: `sqrt(sum_c sum_d gamma_cd * K_c * K_d)`
    ///    where `gamma_cd = 1` on the diagonal and
    ///    `ir_inter_currency_correlation` off-diagonal.
    ///
    /// Applying the concentration factor at the currency level rather
    /// than pool-wide matches the SIMM specification: a large net USD
    /// position should not have its concentration penalty diluted by an
    /// offsetting JPY position in the pooled sum.
    ///
    /// # Arguments
    ///
    /// * `ir_delta` - Map of (currency, tenor) to DV01 sensitivity
    pub fn calculate_ir_delta_multi_currency(
        &self,
        ir_delta: &HashMap<(Currency, String), f64>,
    ) -> f64 {
        let mut by_currency: HashMap<Currency, HashMap<String, f64>> = HashMap::default();
        for ((ccy, tenor), delta) in ir_delta {
            *by_currency
                .entry(*ccy)
                .or_default()
                .entry(tenor.clone())
                .or_insert(0.0) += delta;
        }

        // Canonical currency and tenor order so the f64 quadratic-form
        // reductions are bit-reproducible regardless of HashMap iteration.
        let mut currencies: Vec<(&Currency, &HashMap<String, f64>)> = by_currency.iter().collect();
        currencies.sort_by_key(|(ccy, _)| **ccy);
        let k_values: Vec<(f64, f64, f64)> = currencies
            .into_iter()
            .map(|(_, tenor_map)| {
                let mut weighted: Vec<(usize, f64)> = tenor_map
                    .iter()
                    .filter_map(|(tenor, dv01)| {
                        let w = self.params.ir_delta_weights.get(tenor)?;
                        let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor)?;
                        Some((*idx, dv01 * w))
                    })
                    .collect();
                weighted.sort_by_key(|(idx, _)| *idx);
                let raw_net: f64 = tenor_map.values().sum();
                let cf = self.concentration_factor(SimmRiskClass::InterestRate, raw_net);
                for (_, ws) in &mut weighted {
                    *ws *= cf;
                }
                let k = self.ir_tenor_norm(&weighted);
                let signed = weighted.iter().map(|(_, ws)| *ws).sum::<f64>().clamp(-k, k);
                (k, signed, cf)
            })
            .collect();

        self.aggregate_ir_currencies(&k_values)
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

    /// Calculate IR vega margin with multi-currency aggregation.
    ///
    /// This mirrors [`Self::calculate_ir_delta_multi_currency`] for the IR vega
    /// risk class: same-tenor sensitivities are first grouped by currency so
    /// one currency cannot overwrite another, then per-currency vega margins
    /// are combined using the SIMM IR inter-currency correlation.
    ///
    /// # Arguments
    ///
    /// * `ir_vega` - Map of (currency, tenor) to signed IR vega sensitivity.
    pub fn calculate_ir_vega_multi_currency(
        &self,
        ir_vega: &HashMap<(Currency, String), f64>,
    ) -> f64 {
        let mut by_currency: HashMap<Currency, HashMap<String, f64>> = HashMap::default();
        for ((ccy, tenor), vega) in ir_vega {
            *by_currency
                .entry(*ccy)
                .or_default()
                .entry(tenor.clone())
                .or_insert(0.0) += vega;
        }

        let mut currencies: Vec<(&Currency, &HashMap<String, f64>)> = by_currency.iter().collect();
        currencies.sort_by_key(|(ccy, _)| **ccy);
        let k_values: Vec<(f64, f64, f64)> = currencies
            .into_iter()
            .map(|(_, tenor_map)| {
                let k = self.calculate_ir_vega(tenor_map);
                let signed =
                    (tenor_map.values().sum::<f64>() * self.params.ir_vega_weight).clamp(-k, k);
                (k, signed, 1.0)
            })
            .collect();

        self.aggregate_ir_currencies(&k_values)
    }

    /// Calculate IR delta margin for a single currency from DV01-style sensitivities.
    ///
    /// Uses intra-bucket tenor correlations per ISDA SIMM methodology:
    /// `K = sqrt(sum_i sum_j rho(i,j) * WS_i * WS_j)`
    ///
    /// # Arguments
    ///
    /// * `dv01_by_tenor` - Map of tenor bucket to signed currency DV01 per 1bp move
    ///
    /// # Returns
    ///
    /// The interest-rate delta margin contribution in the caller's implicit currency units.
    pub fn calculate_ir_delta(&self, dv01_by_tenor: &HashMap<String, f64>) -> f64 {
        let mut weighted: Vec<(usize, f64)> = dv01_by_tenor
            .iter()
            .filter_map(|(tenor, dv01)| {
                let weight = self.params.ir_delta_weights.get(tenor)?;
                let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor)?;
                Some((*idx, dv01 * weight))
            })
            .collect();
        // Canonical tenor order so the f64 quadratic form is bit-reproducible.
        weighted.sort_by_key(|(idx, _)| *idx);
        self.ir_tenor_norm(&weighted)
    }

    /// Calculate credit non-qualifying delta margin from aggregate CS01.
    ///
    /// # Arguments
    ///
    /// * `cs01` - Signed currency CS01 per 1bp par-spread move for explicitly
    ///   non-qualifying exposures.
    ///
    /// # Returns
    ///
    /// The non-qualifying credit delta margin after the registry risk weight.
    pub fn calculate_credit_non_qualifying_delta(&self, cs01: f64) -> f64 {
        (cs01 * self.params.cnq_delta_weight).abs()
    }

    /// Calculate credit non-qualifying vega margin from aggregate vega.
    ///
    /// Mirrors [`Self::calculate_credit_non_qualifying_delta`]: the pooled
    /// signed vega is weighted by the credit-non-qualifying vega risk weight
    /// and reported as a magnitude.
    ///
    /// # Arguments
    ///
    /// * `vega` - Signed currency vega for explicitly non-qualifying exposures.
    ///
    /// # Returns
    ///
    /// The non-qualifying credit vega margin after the registry risk weight.
    pub fn calculate_credit_non_qualifying_vega(&self, vega: f64) -> f64 {
        (vega * self.params.cnq_vega_weight).abs()
    }

    /// Calculate credit qualifying delta margin with bucket-level aggregation.
    ///
    /// Follows the ISDA SIMM v2.6 §3.B two-level aggregation for credit
    /// qualifying:
    ///
    /// 1. **Weighting + concentration**: For each bucket `b`, compute the
    ///    bucket-level concentration factor `CR_b` from the net weighted
    ///    sensitivity. Each WS is then scaled by `CR_b` (uniform within
    ///    the bucket, matching the simplified SIMM convention of a single
    ///    concentration factor per bucket).
    /// 2. **Intra-bucket**:
    ///    `K_b = sqrt(sum_i sum_j rho * (CR_b * WS_i) * (CR_b * WS_j))`.
    /// 3. **Net weighted sum (capped)**:
    ///    `S_b = max(-K_b, min(K_b, sum_i CR_b * WS_i))`.
    /// 4. **Inter-bucket**:
    ///    `K = sqrt(sum_b K_b^2 + sum_{b != c} gamma_bc * S_b * S_c)`.
    ///
    /// The diagonal of the inter-bucket sum contributes `K_b²` (not
    /// `S_b²`), consistent with the SIMM formula.
    ///
    /// # Arguments
    ///
    /// * `bucketed_delta` - Map of `(sector, issuer, tenor)` to signed CS01 sensitivity
    ///
    /// # Returns
    ///
    /// The credit qualifying delta margin after bucket diversification.
    pub fn calculate_credit_qualifying_delta(
        &self,
        bucketed_delta: &HashMap<(SimmCreditSector, String, String), f64>,
    ) -> f64 {
        self.aggregate_credit_qualifying(
            bucketed_delta,
            |sector| self.params.cq_bucket_weight(sector),
            true,
        )
    }

    /// Calculate credit-qualifying vega margin with bucket-level aggregation.
    ///
    /// Shares the ISDA SIMM v2.6 §3.B two-level aggregation used by
    /// [`Self::calculate_credit_qualifying_delta`] — same sector buckets, same
    /// intra-bucket name correlation, same inter-bucket correlation matrix —
    /// but weights every sensitivity by the single credit-qualifying vega risk
    /// weight `VRW_CreditQ` instead of the per-bucket delta weights.
    ///
    /// As with the IR, equity, and FX vega paths, no concentration factor is
    /// applied: the registry ships delta concentration thresholds only.
    ///
    /// # Arguments
    ///
    /// * `bucketed_vega` - Map of `(sector, issuer, tenor)` to signed currency vega
    ///
    /// # Returns
    ///
    /// The credit-qualifying vega margin after bucket diversification.
    pub fn calculate_credit_qualifying_vega(
        &self,
        bucketed_vega: &HashMap<(SimmCreditSector, String, String), f64>,
    ) -> f64 {
        let weight = self.params.cq_vega_weight;
        self.aggregate_credit_qualifying(bucketed_vega, |_| weight, false)
    }

    /// Shared two-level credit-qualifying aggregation for the delta and vega
    /// risk classes.
    ///
    /// `weight_for` supplies the risk weight per sector bucket (per-bucket for
    /// delta, a single flat weight for vega). `apply_concentration` selects
    /// whether the registry's per-bucket delta concentration thresholds are
    /// applied; the vega path passes `false` to match the IR/equity/FX vega
    /// treatment elsewhere in this calculator.
    fn aggregate_credit_qualifying(
        &self,
        bucketed: &HashMap<(SimmCreditSector, String, String), f64>,
        weight_for: impl Fn(SimmCreditSector) -> f64,
        apply_concentration: bool,
    ) -> f64 {
        let mut by_sector: HashMap<SimmCreditSector, Vec<f64>> = HashMap::default();
        for ((sector, _issuer, _tenor), amount) in bucketed {
            let weight = weight_for(*sector);
            let ws = *amount * weight;
            by_sector.entry(*sector).or_default().push(ws);
        }

        let rho = self.params.cq_intra_bucket_correlation;

        let mut bucket_results: Vec<(SimmCreditSector, f64, f64)> = Vec::new();
        for (sector, weighted_sensitivities) in &by_sector {
            let raw_net: f64 = weighted_sensitivities.iter().sum();
            let cf = if apply_concentration {
                self.params.cq_concentration_factor(*sector, raw_net)
            } else {
                1.0
            };

            // K_b = sqrt(sum_i sum_j rho_ij * (CR*WS_i) * (CR*WS_j))
            //     = |CR| * sqrt(sum_i sum_j rho_ij * WS_i * WS_j)
            let mut scaled: Vec<f64> = weighted_sensitivities.iter().map(|ws| ws * cf).collect();
            scaled.sort_by(f64::total_cmp);
            let k_b = correlated_norm(&scaled, |_, _| rho);

            // S_b = max(-K_b, min(K_b, sum CR*WS))
            let net_scaled: f64 = scaled.iter().sum();
            let s_b = net_scaled.clamp(-k_b, k_b);

            bucket_results.push((*sector, k_b, s_b));
        }

        bucket_results.sort_by_key(|(sector, _, _)| *sector as u8);

        // Inter-bucket: K = sqrt(sum_b K_b^2 + sum_{b != c} gamma_bc * S_b * S_c)
        let ks: Vec<(f64, f64)> = bucket_results.iter().map(|&(_, k, s)| (k, s)).collect();
        inter_bucket_pairwise(&ks, |i, j| {
            self.params
                .cq_inter_bucket_correlation(bucket_results[i].0, bucket_results[j].0)
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
    pub fn calculate_equity_delta(&self, equity_delta: f64) -> f64 {
        (equity_delta * self.params.equity_delta_weight).abs()
    }

    /// Calculate FX delta margin across currency risk factors.
    ///
    /// Each currency sensitivity is weighted and concentration-scaled
    /// independently, then aggregated with the SIMM FX intra-bucket correlation
    /// between distinct currency risk factors. This prevents opposite-signed
    /// currency deltas from receiving full rho=1 offset.
    pub fn calculate_fx_delta_bucketed(&self, fx_delta: &HashMap<Currency, f64>) -> f64 {
        let mut weighted: Vec<f64> = fx_delta
            .values()
            .map(|delta| {
                let ws = delta * self.params.fx_delta_weight;
                let cf = self.concentration_factor(SimmRiskClass::Fx, ws);
                ws * cf
            })
            .collect();
        // Canonical order (by value) so the f64 quadratic form is reproducible
        // regardless of `HashMap` iteration order.
        weighted.sort_by(f64::total_cmp);

        let rho = self.params.fx_intra_bucket_correlation;
        correlated_norm(&weighted, |_, _| rho)
    }

    /// Calculate commodity delta margin using SIMM bucket risk weights.
    ///
    /// # Arguments
    ///
    /// * `delta_by_bucket` - Signed currency delta by SIMM commodity bucket label
    ///
    /// # Returns
    ///
    /// The commodity delta margin contribution after bucket weighting and inter-bucket correlation.
    pub fn calculate_commodity_delta(&self, delta_by_bucket: &HashMap<String, f64>) -> f64 {
        self.aggregate_commodity(delta_by_bucket, |bucket| {
            self.params.commodity_bucket_weight(bucket)
        })
    }

    /// Calculate commodity vega margin using the SIMM commodity bucket structure.
    ///
    /// Mirrors [`Self::calculate_commodity_delta`] — same bucket labels, same
    /// 17x17 inter-bucket correlation matrix — but weights every bucket by the
    /// single commodity vega risk weight `VRW_Commodity` instead of the
    /// per-bucket delta weights.
    ///
    /// # Arguments
    ///
    /// * `vega_by_bucket` - Signed currency vega by SIMM commodity bucket label
    ///
    /// # Returns
    ///
    /// The commodity vega margin contribution after inter-bucket correlation.
    pub fn calculate_commodity_vega(&self, vega_by_bucket: &HashMap<String, f64>) -> f64 {
        let weight = self.params.commodity_vega_weight;
        self.aggregate_commodity(vega_by_bucket, |_| weight)
    }

    /// Shared commodity bucket aggregation for the delta and vega risk classes.
    ///
    /// Buckets whose label does not resolve to a SIMM commodity bucket id are
    /// dropped, matching the delta behaviour prior to this refactor.
    fn aggregate_commodity(
        &self,
        by_bucket: &HashMap<String, f64>,
        weight_for: impl Fn(&str) -> f64,
    ) -> f64 {
        let mut weighted_buckets: Vec<(u8, f64)> = by_bucket
            .iter()
            .filter_map(|(bucket, amount)| {
                let bucket_id = crate::types::commodity_bucket_id(bucket)?;
                let weight = weight_for(bucket);
                Some((bucket_id, amount * weight))
            })
            .collect();
        // Canonical order so the f64 quadratic form is reproducible.
        weighted_buckets.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));

        let ws: Vec<f64> = weighted_buckets.iter().map(|(_, w)| *w).collect();
        correlated_norm(&ws, |i, j| {
            let (bucket_i, bucket_j) = (weighted_buckets[i].0, weighted_buckets[j].0);
            if bucket_i == bucket_j {
                1.0
            } else {
                self.params
                    .commodity_inter_bucket_correlation(bucket_i, bucket_j)
            }
        })
    }

    /// Calculate IR vega margin from tenor-bucketed vega sensitivities.
    ///
    /// # Arguments
    ///
    /// * `vega_by_tenor` - Signed currency vega amounts keyed by SIMM tenor label
    ///
    /// # Returns
    ///
    /// The interest-rate vega margin contribution.
    pub fn calculate_ir_vega(&self, vega_by_tenor: &HashMap<String, f64>) -> f64 {
        let weight = self.params.ir_vega_weight;
        let mut indexed: Vec<(usize, f64)> = vega_by_tenor
            .iter()
            .filter_map(|(tenor, vega)| {
                let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor)?;
                Some((*idx, *vega * weight))
            })
            .collect();
        // Canonical tenor order so the f64 quadratic form is bit-reproducible.
        indexed.sort_by_key(|(idx, _)| *idx);
        self.ir_tenor_norm(&indexed)
    }

    /// Calculate equity vega margin from a signed currency vega amount.
    pub fn calculate_equity_vega(&self, total_vega: f64) -> f64 {
        (total_vega * self.params.equity_vega_weight).abs()
    }

    /// Calculate FX vega margin from a signed currency vega amount.
    pub fn calculate_fx_vega(&self, total_vega: f64) -> f64 {
        (total_vega * self.params.fx_vega_weight).abs()
    }

    /// Calculate the curvature margin add-on across risk classes per the ISDA
    /// SIMM curvature aggregation formula.
    ///
    /// Given per-risk-class curvature contributions `CVR_i` (signed currency
    /// amounts, before the flat `curvature_scale_factor`), this applies the
    /// scale factor and then aggregates with the ISDA SIMM combination
    ///
    /// ```text
    /// θ = min( ΣCVR_i / Σ|CVR_i| , 0 )
    /// λ = (Φ⁻¹(0.995)² − 1)·(1 + θ) − θ
    /// K = sqrt( max(0, Σ_i Σ_j ρ_ij² · CVR_i · CVR_j) )
    /// curvature = max( 0, ΣCVR_i + λ·K )
    /// ```
    ///
    /// using **squared** cross-risk-class correlations `ρ_ij²` (diagonal 1),
    /// the `λ(θ)` scaling with `Φ⁻¹(0.995) ≈ 2.5758`, and the `max(0, ·)` floor —
    /// matching ISDA SIMM §8–9.
    ///
    /// # Remaining approximation
    ///
    /// The cross-risk-class correlations are reused from the delta matrix, and a
    /// single flat `curvature_scale_factor` stands in for the per-tenor SIMM
    /// scale `SF(t) = 0.5·min(1, 14/t_days)` that ISDA applies upstream when
    /// forming `CVR` from vega (the inputs here are taken as already-formed
    /// `CVR`). It has not been tied out against ISDA golden vectors, so it may
    /// differ at the margins for option-heavy books; the aggregation *shape*
    /// (ρ², λ, θ, max-floor) now follows the spec.
    ///
    /// `curvature_by_risk_class` should contain signed currency curvature
    /// contributions before the SIMM scale factor is applied.
    pub fn calculate_curvature(
        &self,
        curvature_by_risk_class: &HashMap<SimmRiskClass, f64>,
    ) -> f64 {
        let scale = self.params.curvature_scale_factor;
        // Scaled per-risk-class curvature contributions. Sort into a canonical
        // order (independent of `HashMap` iteration) so the f64 quadratic-form
        // reduction below is bit-reproducible across runs and toolchains.
        let mut cvr: Vec<(SimmRiskClass, f64)> = curvature_by_risk_class
            .iter()
            .map(|(rc, v)| (*rc, v * scale))
            .collect();
        cvr.sort_by_key(|(rc, _)| *rc as u8);

        let sum_cvr: f64 = cvr.iter().map(|(_, v)| *v).sum();
        let sum_abs: f64 = cvr.iter().map(|(_, v)| v.abs()).sum();
        if sum_abs == 0.0 {
            return 0.0;
        }

        // Gate: the curvature add-on uses a flat `curvature_scale_factor` in place
        // of ISDA's per-tenor SF(t) and has not been tied out against ISDA golden
        // vectors. Warn once per process so a desk consciously accepts the
        // approximation rather than relying on an unvalidated regulatory number.
        static CURVATURE_WARNED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !CURVATURE_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!(
                "SIMM curvature add-on uses an unvalidated flat scale factor \
                 (no per-tenor SF(t), not tied out vs ISDA golden vectors); \
                 treat the curvature component as approximate"
            );
        }

        // θ ∈ [-1, 0]; λ scales the diversified term per ISDA SIMM.
        let theta = (sum_cvr / sum_abs).min(0.0);
        let lambda = (SIMM_CURVATURE_Z * SIMM_CURVATURE_Z - 1.0) * (1.0 + theta) - theta;

        // Diversified term using squared correlations (diagonal = 1).
        let cvr_values: Vec<f64> = cvr.iter().map(|(_, v)| *v).collect();
        let k = correlated_norm(&cvr_values, |i, j| {
            let rho = self.params.correlation(cvr[i].0, cvr[j].0);
            rho * rho
        });

        (sum_cvr + lambda * k).max(0.0)
    }

    /// Calculate concentration add-on for a risk class.
    ///
    /// If the net sensitivity exceeds the concentration threshold,
    /// apply a sqrt(|sensitivity| / threshold) multiplier.
    ///
    /// Both `net_sensitivity` and the configured threshold are interpreted in
    /// the same signed currency units.
    pub fn concentration_factor(&self, risk_class: SimmRiskClass, net_sensitivity: f64) -> f64 {
        if let Some(&threshold) = self.params.concentration_thresholds.get(&risk_class) {
            if threshold > 0.0 && net_sensitivity.abs() > threshold {
                (net_sensitivity.abs() / threshold).sqrt()
            } else {
                1.0
            }
        } else {
            1.0
        }
    }

    /// Calculate indicative historical SIMM v2.6 from sensitivities as a raw
    /// `(total, breakdown)` tuple. This omits the approximation flag from the
    /// result envelope; all amounts remain approximate for the limitations
    /// documented on [`Self::calculate_from_sensitivities`].
    ///
    /// Specialised variant of [`Self::calculate_from_sensitivities`] for Rust
    /// callers that aggregate many netting sets and do not want an
    /// [`ImResult`] envelope. Validates the sensitivity container before
    /// aggregation, so invalid buckets cannot silently contribute zero margin.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - SIMM sensitivities by risk class using the units documented on [`SimmSensitivities`]
    /// * `currency` - USD, matching the sensitivity base currency; concentration thresholds are USD-denominated and no FX conversion is performed
    ///
    /// # Returns
    ///
    /// A tuple of `(total_margin, breakdown_by_risk_class)` where `total_margin`
    /// is a scalar amount in `currency` units and the breakdown labels the
    /// major SIMM components included in the aggregate.
    ///
    /// # Notes
    ///
    /// Currency labels from `sensitivities.ir_delta` and similar fields are
    /// preserved for bucketing but the returned margin amounts are all reported
    /// in `currency`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_margin::{SimmCalculator, SimmSensitivities, SimmVersion};
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let calc = SimmCalculator::new(SimmVersion::V2_6)?;
    /// let mut sensitivities = SimmSensitivities::new(Currency::USD);
    /// sensitivities.add_ir_delta(Currency::USD, "5Y", 50_000.0);
    ///
    /// let (total, breakdown) =
    ///     calc.calculate_from_sensitivities_parts(&sensitivities, Currency::USD).expect("valid sensitivity fixture");
    ///
    /// assert!(total >= 0.0);
    /// assert!(breakdown.contains_key("IR_Delta"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
    pub fn calculate_from_sensitivities_parts(
        &self,
        sensitivities: &SimmSensitivities,
        currency: Currency,
    ) -> finstack_quant_core::Result<(f64, HashMap<String, Money>)> {
        sensitivities.validate()?;
        if currency != sensitivities.base_currency || currency != Currency::USD {
            return Err(finstack_quant_core::Error::Validation(
                "SIMM requires USD-normalized sensitivities and USD output because concentration thresholds are in USD; convert inputs explicitly before calculation".into()
            ));
        }
        let mut breakdown = HashMap::default();
        let mut risk_class_margins = HashMap::default();

        // IR Delta — per-currency calculation with inter-currency aggregation
        if !sensitivities.ir_delta.is_empty() {
            let ir_margin = self.calculate_ir_delta_multi_currency(&sensitivities.ir_delta);
            if ir_margin > 0.0 {
                breakdown.insert("IR_Delta".to_string(), Money::new(ir_margin, currency)?);
                risk_class_margins.insert(SimmRiskClass::InterestRate, ir_margin);
            }
        }

        // IR Vega
        if !sensitivities.ir_vega.is_empty() {
            let ir_vega_margin = self.calculate_ir_vega_multi_currency(&sensitivities.ir_vega);
            if ir_vega_margin > 0.0 {
                breakdown.insert("IR_Vega".to_string(), Money::new(ir_vega_margin, currency)?);
                *risk_class_margins
                    .entry(SimmRiskClass::InterestRate)
                    .or_insert(0.0) += ir_vega_margin;
            }
        }

        // Credit Delta (Qualifying): mandatory ISDA SIMM sector aggregation.
        if !sensitivities.credit_qualifying_delta.is_empty() {
            let credit_margin =
                self.calculate_credit_qualifying_delta(&sensitivities.credit_qualifying_delta);
            if credit_margin > 0.0 {
                breakdown.insert(
                    "Credit_Qualifying_Delta".to_string(),
                    Money::new(credit_margin, currency)?,
                );
                risk_class_margins.insert(SimmRiskClass::CreditQualifying, credit_margin);
            }
        }

        // Credit Vega (Qualifying): same sector buckets as the delta path.
        if !sensitivities.credit_qualifying_vega.is_empty() {
            let credit_vega_margin =
                self.calculate_credit_qualifying_vega(&sensitivities.credit_qualifying_vega);
            if credit_vega_margin > 0.0 {
                breakdown.insert(
                    "Credit_Qualifying_Vega".to_string(),
                    Money::new(credit_vega_margin, currency)?,
                );
                *risk_class_margins
                    .entry(SimmRiskClass::CreditQualifying)
                    .or_insert(0.0) += credit_vega_margin;
            }
        }

        // Credit Delta (Non-Qualifying)
        let non_qual_total = sensitivities
            .credit_non_qualifying_delta
            .values()
            .map(|s| s.abs())
            .sum::<f64>();
        if non_qual_total.abs() > 0.0 {
            let credit_margin = self.calculate_credit_non_qualifying_delta(non_qual_total);
            if credit_margin > 0.0 {
                breakdown.insert(
                    "Credit_NonQualifying_Delta".to_string(),
                    Money::new(credit_margin, currency)?,
                );
                risk_class_margins.insert(SimmRiskClass::CreditNonQualifying, credit_margin);
            }
        }

        // Credit Vega (Non-Qualifying)
        let non_qual_vega_total = sensitivities
            .credit_non_qualifying_vega
            .values()
            .map(|s| s.abs())
            .sum::<f64>();
        if non_qual_vega_total.abs() > 0.0 {
            let credit_vega_margin = self.calculate_credit_non_qualifying_vega(non_qual_vega_total);
            if credit_vega_margin > 0.0 {
                breakdown.insert(
                    "Credit_NonQualifying_Vega".to_string(),
                    Money::new(credit_vega_margin, currency)?,
                );
                *risk_class_margins
                    .entry(SimmRiskClass::CreditNonQualifying)
                    .or_insert(0.0) += credit_vega_margin;
            }
        }

        // Equity Delta
        let total_equity: f64 = sensitivities.equity_delta.values().map(|s| s.abs()).sum();
        if total_equity.abs() > 0.0 {
            let equity_margin = self.calculate_equity_delta(total_equity);
            if equity_margin > 0.0 {
                breakdown.insert(
                    "Equity_Delta".to_string(),
                    Money::new(equity_margin, currency)?,
                );
                risk_class_margins.insert(SimmRiskClass::Equity, equity_margin);
            }
        }

        // Equity Vega
        let total_equity_vega: f64 = sensitivities.equity_vega.values().map(|s| s.abs()).sum();
        if total_equity_vega.abs() > 0.0 {
            let equity_vega_margin = self.calculate_equity_vega(total_equity_vega);
            if equity_vega_margin > 0.0 {
                breakdown.insert(
                    "Equity_Vega".to_string(),
                    Money::new(equity_vega_margin, currency)?,
                );
                *risk_class_margins
                    .entry(SimmRiskClass::Equity)
                    .or_insert(0.0) += equity_vega_margin;
            }
        }

        // FX Delta. Apply the FX concentration factor per currency, then
        // aggregate currency factors with the SIMM FX intra-bucket correlation.
        if !sensitivities.fx_delta.is_empty() {
            let fx_margin = self.calculate_fx_delta_bucketed(&sensitivities.fx_delta);
            if fx_margin > 0.0 {
                breakdown.insert("FX_Delta".to_string(), Money::new(fx_margin, currency)?);
                risk_class_margins.insert(SimmRiskClass::Fx, fx_margin);
            }
        }

        // FX Vega
        let total_fx_vega: f64 = sensitivities.fx_vega.values().map(|s| s.abs()).sum();
        if total_fx_vega.abs() > 0.0 {
            let fx_vega_margin = self.calculate_fx_vega(total_fx_vega);
            if fx_vega_margin > 0.0 {
                breakdown.insert("FX_Vega".to_string(), Money::new(fx_vega_margin, currency)?);
                *risk_class_margins.entry(SimmRiskClass::Fx).or_insert(0.0) += fx_vega_margin;
            }
        }

        // Commodity Delta
        if !sensitivities.commodity_delta.is_empty() {
            let commodity_margin = self.calculate_commodity_delta(&sensitivities.commodity_delta);
            if commodity_margin > 0.0 {
                breakdown.insert(
                    "Commodity_Delta".to_string(),
                    Money::new(commodity_margin, currency)?,
                );
                risk_class_margins.insert(SimmRiskClass::Commodity, commodity_margin);
            }
        }

        // Commodity Vega
        if !sensitivities.commodity_vega.is_empty() {
            let commodity_vega_margin =
                self.calculate_commodity_vega(&sensitivities.commodity_vega);
            if commodity_vega_margin > 0.0 {
                breakdown.insert(
                    "Commodity_Vega".to_string(),
                    Money::new(commodity_vega_margin, currency)?,
                );
                *risk_class_margins
                    .entry(SimmRiskClass::Commodity)
                    .or_insert(0.0) += commodity_vega_margin;
            }
        }

        // Apply concentration factors for the remaining risk classes.
        //
        // - InterestRate: per-currency CF already applied inside
        //   `calculate_ir_delta_multi_currency`.
        // - Fx: per-currency CF already applied in the FX block above.
        // - CreditQualifying: per-bucket CF already applied inside
        //   `calculate_credit_qualifying_delta`.
        //
        // For CreditNonQualifying, Equity, and Commodity (where the inputs are
        // pooled by construction), the pool-level CF is the available model.
        let net_sensitivities: HashMap<SimmRiskClass, f64> = [
            (
                SimmRiskClass::CreditNonQualifying,
                sensitivities
                    .credit_non_qualifying_delta
                    .values()
                    .sum::<f64>(),
            ),
            (SimmRiskClass::Equity, sensitivities.total_equity_delta()),
            (
                SimmRiskClass::Commodity,
                sensitivities.commodity_delta.values().sum::<f64>(),
            ),
        ]
        .into_iter()
        .collect();

        for (rc, margin) in risk_class_margins.iter_mut() {
            match *rc {
                SimmRiskClass::InterestRate
                | SimmRiskClass::CreditQualifying
                | SimmRiskClass::Fx => continue,
                _ => {}
            }
            let Some(&net) = net_sensitivities.get(rc) else {
                continue;
            };
            let cf = self.concentration_factor(*rc, net);
            if cf > 1.0 {
                *margin *= cf;
            }
        }

        // Curvature -- added on top of the correlated risk-class total
        let curvature_addon = if !sensitivities.curvature.is_empty() {
            let cm = self.calculate_curvature(&sensitivities.curvature);
            if cm > 0.0 {
                breakdown.insert("Curvature".to_string(), Money::new(cm, currency)?);
            }
            cm
        } else {
            0.0
        };

        let correlated_total = if risk_class_margins.is_empty() {
            0.0
        } else {
            self.aggregate_risk_classes(&risk_class_margins)
        };
        let total_im = correlated_total + curvature_addon;

        Ok((total_im, breakdown))
    }

    /// Calculate indicative historical SIMM v2.6 from explicit sensitivities.
    ///
    /// Results always carry `approximation = true`: the input contract omits
    /// product-class, IR subcurve and full non-IR bucket dimensions. Non-IR
    /// name aggregates use gross absolute sensitivities to prevent false offsets.
    /// This model is unsuitable for regulatory margin or current-version reconciliation.
    ///
    /// This is the canonical entry point: it validates the container with
    /// [`SimmSensitivities::validate`] first, so a mistyped tenor or commodity
    /// bucket errors instead of silently pricing to zero margin, then stamps
    /// the methodology, MPOR and calculation date. Use
    /// [`Self::calculate_from_sensitivities_parts`] for the raw
    /// `(total, breakdown)` tuple with the same validation and no result stamping.
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
    pub fn calculate_from_sensitivities(
        &self,
        sensitivities: &SimmSensitivities,
        currency: Currency,
        as_of: Date,
    ) -> Result<ImResult> {
        let (amount, breakdown) =
            self.calculate_from_sensitivities_parts(sensitivities, currency)?;
        let mut result = ImResult::with_breakdown(
            Money::new(amount, currency)?,
            ImMethodology::Simm,
            as_of,
            self.mpor_days(),
            breakdown,
        );
        result.approximation = true;
        Ok(result)
    }

    /// Aggregate risk class margins with the SIMM inter-risk-class correlation matrix.
    ///
    /// `Total = sqrt(sum_i sum_j rho(i,j) * K_i * K_j)`
    pub fn aggregate_risk_classes(&self, risk_class_margins: &HashMap<SimmRiskClass, f64>) -> f64 {
        // Reduce in a canonical risk-class order so the f64 quadratic form is
        // bit-reproducible across runs, independent of `HashMap` iteration order
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

    fn methodology(&self) -> ImMethodology {
        ImMethodology::Simm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Marginable;

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
        let dv01_by_tenor: HashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0), // $100K DV01 at 5y
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.calculate_ir_delta(&dv01_by_tenor);

        // Risk weight for 5y is 51, so margin = 100K * 51 = 5.1M
        assert!((ir_margin - 5_100_000.0).abs() < 1.0);
    }

    #[test]
    fn ir_delta_tenor_lookup_uses_canonical_simm_case() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let canonical: HashMap<String, f64> = [("5Y".to_string(), 100_000.0)].into_iter().collect();
        let noncanonical: HashMap<String, f64> =
            [("5y".to_string(), 100_000.0)].into_iter().collect();

        let canonical_margin = calc.calculate_ir_delta(&canonical);
        let noncanonical_margin = calc.calculate_ir_delta(&noncanonical);

        assert!(canonical_margin > 0.0, "canonical tenor must be recognized");
        assert_eq!(noncanonical_margin, 0.0);

        // Vega path shares the same tenor index lookup.
        let canonical_vega: HashMap<String, f64> =
            [("5Y".to_string(), 50_000.0)].into_iter().collect();
        let noncanonical_vega: HashMap<String, f64> =
            [("5y".to_string(), 50_000.0)].into_iter().collect();
        assert!(calc.calculate_ir_vega(&canonical_vega) > 0.0);
        assert_eq!(calc.calculate_ir_vega(&noncanonical_vega), 0.0);
    }

    #[test]
    fn credit_non_qualifying_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0;
        let cnq_margin = calc.calculate_credit_non_qualifying_delta(cs01);

        assert!((cnq_margin - 25_000_000.0).abs() < 1.0); // 50K * 500
    }

    #[test]
    fn fx_delta_bucketed_preserves_partial_offset() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut fx_delta = HashMap::default();
        fx_delta.insert(Currency::EUR, 100.0);
        fx_delta.insert(Currency::JPY, -100.0);

        let actual = calc.calculate_fx_delta_bucketed(&fx_delta);
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
        assert!(calc
            .params
            .cq_bucket_weights
            .contains_key(&SimmCreditSector::Financial));
    }

    #[test]
    fn aggregation() {
        let calc = SimmCalculator::default();

        let risk_class_margins: HashMap<SimmRiskClass, f64> = [
            (SimmRiskClass::InterestRate, 1_000_000.0),
            (SimmRiskClass::CreditQualifying, 500_000.0),
        ]
        .into_iter()
        .collect();

        let total = calc.aggregate_risk_classes(&risk_class_margins);

        // sqrt(1M^2 + 0.5M^2 + 2*0.10*1M*0.5M) ≈ 1.162M
        assert!((total - 1_161_895.0).abs() < 1.0);
    }

    #[test]
    fn calculate_from_sensitivities_uses_risk_class_correlation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);

        let (total_im, breakdown) = calc
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");

        let ir_margin = breakdown
            .get("IR_Delta")
            .expect("IR margin present")
            .amount();
        let eq_margin = breakdown
            .get("Equity_Delta")
            .expect("Equity margin present")
            .amount();

        let expected =
            (ir_margin * ir_margin + eq_margin * eq_margin + 2.0 * 0.12 * ir_margin * eq_margin)
                .sqrt();
        assert!((total_im - expected).abs() < 1.0);
    }

    #[test]
    fn ir_delta_multi_tenor_with_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let dv01_by_tenor: HashMap<String, f64> = [
            ("5Y".to_string(), 100_000.0),
            ("10Y".to_string(), -80_000.0), // Partially hedged
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.calculate_ir_delta(&dv01_by_tenor);

        // ws_5y = 100K*51 = 5.1M, ws_10y = -80K*51 = -4.08M
        // With high tenor correlation (~0.96), the hedge offsets most of the risk
        // so margin should be much less than the uncorrelated sqrt(5.1^2 + 4.08^2) ≈ 6.53M
        assert!(ir_margin > 1_000_000.0);
        assert!(ir_margin < 3_000_000.0);
    }

    #[test]
    fn ir_vega_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let vega_by_tenor: HashMap<String, f64> =
            [("5Y".to_string(), 500_000.0)].into_iter().collect();

        let ir_vega_margin = calc.calculate_ir_vega(&vega_by_tenor);
        // Single tenor: sqrt((500K * 0.21)^2) = 500K * 0.21 = 105K
        assert!((ir_vega_margin - 105_000.0).abs() < 1.0);
    }

    #[test]
    fn m15_ir_vega_multi_currency_preserves_same_tenor_exposures() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_vega(Currency::USD, "5Y", 500_000.0);
        sensitivities.add_ir_vega(Currency::EUR, "5Y", 500_000.0);

        let (_, breakdown) = calc
            .calculate_from_sensitivities_parts(&sensitivities, Currency::USD)
            .expect("valid sensitivity fixture");
        let ir_vega_margin = breakdown
            .get("IR_Vega")
            .expect("M-15: IR vega margin should be present")
            .amount();

        let single_currency_margin =
            calc.calculate_ir_vega(&[("5Y".to_string(), 500_000.0)].into_iter().collect());
        assert!(
            ir_vega_margin > single_currency_margin,
            "M-15: same-tenor multi-currency IR vega must not collapse to one exposure"
        );
    }

    #[test]
    fn curvature_uses_isda_lambda_and_squared_correlation_aggregation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let curvature_by_risk_class: HashMap<SimmRiskClass, f64> = [
            (SimmRiskClass::InterestRate, 1_000_000.0),
            (SimmRiskClass::Equity, -600_000.0),
        ]
        .into_iter()
        .collect();

        let actual = calc.calculate_curvature(&curvature_by_risk_class);

        // ISDA SIMM curvature aggregation: max(0, ΣCVR + λ·sqrt(Σ ρ²·CVR_i·CVR_j)).
        let scale = calc.params.curvature_scale_factor;
        let rho = calc
            .params
            .correlation(SimmRiskClass::InterestRate, SimmRiskClass::Equity);
        let ir = 1_000_000.0 * scale;
        let eq = -600_000.0 * scale;
        let sum_cvr = ir + eq;
        let theta = (sum_cvr / (ir.abs() + eq.abs())).min(0.0);
        let z = 2.575_829_303_548_900_4_f64;
        let lambda = (z * z - 1.0) * (1.0 + theta) - theta;
        // Squared correlation on the off-diagonal (diagonal = 1).
        let quad = ir * ir + eq * eq + 2.0 * rho * rho * ir * eq;
        let expected = (sum_cvr + lambda * quad.max(0.0).sqrt()).max(0.0);

        assert!(
            (actual - expected).abs() < 1.0,
            "expected ISDA curvature {}, got {}",
            expected,
            actual
        );

        // Discriminator: λ ≈ 5.63 inflates the diversified term well beyond the
        // plain correlated sqrt the old approximation produced.
        let old_approx = (ir * ir + eq * eq + 2.0 * rho * ir * eq).sqrt();
        assert!(
            actual > old_approx * 1.5,
            "λ scaling must materially exceed the old plain-sqrt charge (old={old_approx}, new={actual})"
        );
    }

    #[test]
    fn commodity_delta_uses_bucket_correlations() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let delta_by_bucket: HashMap<String, f64> =
            [("2".to_string(), 100_000.0), ("3".to_string(), -100_000.0)]
                .into_iter()
                .collect();

        let actual = calc.calculate_commodity_delta(&delta_by_bucket);
        let bucket_2 = 100_000.0 * calc.params.commodity_bucket_weight("2");
        let bucket_3 = -100_000.0 * calc.params.commodity_bucket_weight("3");
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

        fn netting_set_id(&self) -> Option<crate::NettingSetId> {
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
            .calculate_from_sensitivities_parts(&sensitivities, Currency::USD)
            .expect("valid sensitivity fixture");
        let actual = calc
            .calculate(&instrument, &market, as_of)
            .expect("SIMM calculation should succeed");

        assert!(
            (actual.amount.amount() - expected.0).abs() < 1e-2,
            "expected total {}, got {} with breakdown {:?}",
            expected.0,
            actual.amount.amount(),
            actual.breakdown
        );
        for (key, expected_amount) in &expected.1 {
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
        let cf = calc.params.cq_concentration_factor(sector, raw_ws);

        // Bucketed path: single name in one bucket.
        let mut bucketed: HashMap<(SimmCreditSector, String, String), f64> = HashMap::default();
        bucketed.insert((sector, "ISSUER_A".to_string(), "5Y".to_string()), cs01);
        let bucketed_margin = calc.calculate_credit_qualifying_delta(&bucketed);

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
        let mut bucketed: HashMap<(SimmCreditSector, String, String), f64> = HashMap::default();
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
        let bucketed_margin = calc.calculate_credit_qualifying_delta(&bucketed);
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
        let cf_a = calc.params.cq_concentration_factor(sector_a, ws_a);
        let cf_b = calc.params.cq_concentration_factor(sector_b, ws_b);

        // Single-name per bucket: K_b = |cs01 * weight * concentration_factor|
        let k_a = (ws_a * cf_a).abs();
        let k_b = (ws_b * cf_b).abs();
        let s_a = (ws_a * cf_a).clamp(-k_a, k_a);
        let s_b = (ws_b * cf_b).clamp(-k_b, k_b);

        let gamma = calc.params.cq_inter_bucket_correlation(sector_a, sector_b);

        // Expected: sqrt(K_a^2 + K_b^2 + 2*gamma*S_a*S_b)
        let expected = (k_a * k_a + k_b * k_b + 2.0 * gamma * s_a * s_b).sqrt();

        let mut bucketed: HashMap<(SimmCreditSector, String, String), f64> = HashMap::default();
        bucketed.insert((sector_a, "GOVT_A".to_string(), "5Y".to_string()), cs01_a);
        bucketed.insert((sector_b, "BANK_A".to_string(), "5Y".to_string()), cs01_b);
        let actual = calc.calculate_credit_qualifying_delta(&bucketed);

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
        let cf = calc.params.cq_concentration_factor(sector, ws_1 + ws_2);

        // K_b = sqrt((cf*ws_1)^2 + (cf*ws_2)^2 + 2*rho*(cf*ws_1)*(cf*ws_2))
        let scaled_1 = cf * ws_1;
        let scaled_2 = cf * ws_2;
        let expected =
            (scaled_1 * scaled_1 + scaled_2 * scaled_2 + 2.0 * rho * scaled_1 * scaled_2).sqrt();

        let mut bucketed: HashMap<(SimmCreditSector, String, String), f64> = HashMap::default();
        bucketed.insert((sector, "BANK_A".to_string(), "5Y".to_string()), cs01_1);
        bucketed.insert((sector, "BANK_B".to_string(), "5Y".to_string()), cs01_2);
        let actual = calc.calculate_credit_qualifying_delta(&bucketed);

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

        let (total_im, breakdown) = calc
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");
        assert!(total_im > 0.0, "total IM should be positive");
        assert!(
            breakdown.contains_key("Credit_Qualifying_Delta"),
            "breakdown should contain Credit_Qualifying_Delta"
        );

        // The bucketed margin should match the direct bucketed calculation.
        let expected = calc.calculate_credit_qualifying_delta(&sens.credit_qualifying_delta);
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
        let (baseline, _) = base
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");
        let (with_bump, _) = bumped
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");

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
        let (baseline, _) = base
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");
        let (with_bump, _) = bumped
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");

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
        let (baseline, _) = base
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");
        let (with_bump, _) = bumped
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");

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
        let (baseline, _) = calc
            .calculate_from_sensitivities_parts(&anchor, Currency::USD)
            .expect("valid sensitivity fixture");

        let mut with_cq = anchor.clone();
        with_cq.add_credit_qualifying_vega(SimmCreditSector::Financial, "BANK_A", "5Y", 25_000.0);
        let (cq_total, cq_breakdown) = calc
            .calculate_from_sensitivities_parts(&with_cq, Currency::USD)
            .expect("valid sensitivity fixture");
        assert_ne!(baseline, cq_total, "CQ vega must change total IM");
        assert!(cq_breakdown.contains_key("Credit_Qualifying_Vega"));

        let mut with_cnq = anchor.clone();
        with_cnq.add_credit_non_qualifying_vega("RMBS_A", "5Y", 15_000.0);
        let (cnq_total, cnq_breakdown) = calc
            .calculate_from_sensitivities_parts(&with_cnq, Currency::USD)
            .expect("valid sensitivity fixture");
        assert_ne!(baseline, cnq_total, "CNQ vega must change total IM");
        assert!(cnq_breakdown.contains_key("Credit_NonQualifying_Vega"));

        let mut with_commodity = anchor.clone();
        with_commodity.add_commodity_vega("Crude", 20_000.0);
        let (commodity_total, commodity_breakdown) = calc
            .calculate_from_sensitivities_parts(&with_commodity, Currency::USD)
            .expect("valid sensitivity fixture");
        assert_ne!(
            baseline, commodity_total,
            "commodity vega must change total IM"
        );
        assert!(commodity_breakdown.contains_key("Commodity_Vega"));
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

        let (baseline, _) = calc
            .calculate_from_sensitivities_parts(&anchor, Currency::USD)
            .expect("valid sensitivity fixture");
        let (with_zero, breakdown) = calc
            .calculate_from_sensitivities_parts(&zeroed, Currency::USD)
            .expect("valid sensitivity fixture");

        assert_eq!(baseline, with_zero, "zero vega must not move the margin");
        assert!(!breakdown.contains_key("Credit_Qualifying_Vega"));
        assert!(!breakdown.contains_key("Credit_NonQualifying_Vega"));
        assert!(!breakdown.contains_key("Commodity_Vega"));

        assert_eq!(
            calc.calculate_credit_qualifying_vega(&HashMap::default()),
            0.0
        );
        assert_eq!(calc.calculate_credit_non_qualifying_vega(0.0), 0.0);
        assert_eq!(calc.calculate_commodity_vega(&HashMap::default()), 0.0);
    }

    /// A single name / single bucket reduces to `|vega * VRW|` for each of the
    /// three risk classes, mirroring the equivalent single-factor delta cases.
    #[test]
    fn single_factor_credit_and_commodity_vega_match_closed_form() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut cq: HashMap<(SimmCreditSector, String, String), f64> = HashMap::default();
        cq.insert(
            (
                SimmCreditSector::Financial,
                "BANK_A".to_string(),
                "5Y".to_string(),
            ),
            25_000.0,
        );
        let expected_cq = 25_000.0 * calc.params.cq_vega_weight;
        assert!((calc.calculate_credit_qualifying_vega(&cq) - expected_cq).abs() < 1e-6);

        let expected_cnq = 15_000.0 * calc.params.cnq_vega_weight;
        assert!((calc.calculate_credit_non_qualifying_vega(15_000.0) - expected_cnq).abs() < 1e-6);
        assert!(
            (calc.calculate_credit_non_qualifying_vega(-15_000.0) - expected_cnq).abs() < 1e-6,
            "vega margin is sign-insensitive, matching the delta path"
        );

        let mut commodity: HashMap<String, f64> = HashMap::default();
        commodity.insert("Crude".to_string(), 20_000.0);
        let expected_commodity = 20_000.0 * calc.params.commodity_vega_weight;
        assert!((calc.calculate_commodity_vega(&commodity) - expected_commodity).abs() < 1e-6);
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
        let mut published: Vec<&str> = crate::SIMM_TENORS.to_vec();
        published.sort_unstable();
        assert_eq!(registry, published, "registry tenors drifted");
    }

    #[test]
    fn typed_calculation_rejects_unknown_tenor_instead_of_zero_margin() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("date");
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "7Y", 50_000.0);
        let error = calc
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect_err("parts must reject unknown tenors too");
        assert!(error.to_string().contains("7Y"));
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
        let (amount, breakdown) = calc
            .calculate_from_sensitivities_parts(&good, Currency::USD)
            .expect("valid parts");
        assert_eq!(amount, result.amount.amount());
        assert_eq!(
            breakdown
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            result.breakdown
        );
    }
}
