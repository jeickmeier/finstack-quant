//! ISDA Standard Initial Margin Model (SIMM) calculator.
//!
//! Implements the ISDA SIMM methodology for calculating initial margin
//! on non-centrally cleared OTC derivatives.
//!
//! # ISDA SIMM Methodology
//!
//! SIMM calculates IM based on sensitivities across risk classes:
//! - Interest Rate (IR): DV01-style currency sensitivities by tenor bucket
//! - Credit Qualifying (CQ): CS01-style currency sensitivities for investment-grade credit
//! - Credit Non-Qualifying (CNQ): CS01-style currency sensitivities for high-yield credit
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
//! > **Implementation note:** `calculate_from_sensitivities` applies intra-bucket
//! > tenor correlations for IR delta, vega margin (IR, equity, FX), curvature
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

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::registry::{embedded_registry, margin_registry_from_config, MarginRegistry, SimmParams};
use crate::traits::Marginable;
use crate::types::ImMethodology;
use crate::types::{
    ordered_credit_sector_pair, ordered_risk_class_pair, ordered_tenor_pair, SimmCreditSector,
    SimmRiskClass, SimmSensitivities,
};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_core::Result;
use tracing::debug;

/// SIMM version identifier.
///
/// Version choice controls the registry-backed risk weights, correlations, and
/// concentration thresholds used by the calculator.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum SimmVersion {
    /// SIMM v2.5 (2022)
    V2_5,
    /// SIMM v2.6 (2023)
    #[default]
    V2_6,
}

impl std::fmt::Display for SimmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimmVersion::V2_5 => write!(f, "SIMM v2.5"),
            SimmVersion::V2_6 => write!(f, "SIMM v2.6"),
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
        let key = bucket_id_from_label(bucket)
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
    fn canonical_cq_sector(sector: SimmCreditSector) -> SimmCreditSector {
        match sector {
            SimmCreditSector::Index | SimmCreditSector::Securitized => SimmCreditSector::Residual,
            other => other,
        }
    }

    fn cq_bucket_weight(&self, sector: SimmCreditSector) -> f64 {
        let sector = Self::canonical_cq_sector(sector);
        self.cq_bucket_weights.get(&sector).copied().unwrap_or(0.0)
    }

    fn cq_inter_bucket_correlation(&self, a: SimmCreditSector, b: SimmCreditSector) -> f64 {
        let a = Self::canonical_cq_sector(a);
        let b = Self::canonical_cq_sector(b);
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
        let sector = Self::canonical_cq_sector(sector);
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

    /// Commodity inter-bucket correlation lookup.
    ///
    /// Looks up `(a, b)` in the row-major 17×17 matrix stored on
    /// [`SimmParams::commodity_inter_bucket_correlations`]. The calling code
    /// passes 1-based bucket ids (1..=17); this method translates to the
    /// 0-based flat index. Returns `0.0` for out-of-range buckets so bad
    /// bucket labels degrade to a zero-correlation contribution rather than
    /// a panic.
    ///
    /// Routing through `SimmParams` (rather than a free function over a
    /// hard-coded `const`) keeps this matrix under the same registry-
    /// load PSD validation as the other SIMM correlation matrices.
    fn commodity_inter_bucket_correlation(&self, a: u8, b: u8) -> f64 {
        let n = COMMODITY_BUCKET_COUNT;
        if !(1..=u8::try_from(n).unwrap_or(u8::MAX)).contains(&a)
            || !(1..=u8::try_from(n).unwrap_or(u8::MAX)).contains(&b)
        {
            return 0.0;
        }
        // After validate_simm_correlations_psd, the field has exactly n*n entries.
        // Defensive guard: if somehow the invariant is violated, fall back to 0.
        let i = (a - 1) as usize;
        let j = (b - 1) as usize;
        let idx = i * n + j;
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
) -> finstack_core::Result<&SimmParams> {
    if let Some(found) = registry.simm.values().find(|p| p.version == version) {
        return Ok(found);
    }
    let available: Vec<String> = registry
        .simm
        .values()
        .map(|p| format!("{:?}", p.version))
        .collect();
    Err(finstack_core::Error::Validation(format!(
        "SIMM registry does not contain version {:?}. Available versions: [{}]. \
         Update the registry overlay or pass a supported SimmVersion.",
        version,
        available.join(", ")
    )))
}

/// Validate SIMM parameter completeness before constructing a calculator.
///
/// ISDA SIMM specifies risk weights, correlations, and concentration thresholds
/// exhaustively for each version. A missing key indicates incomplete registry
/// data (bad config overlay, truncated JSON, corrupted embed). Catching it at
/// construction time prevents silent regulatory miscalculation in hot paths.
///
/// Checked invariants:
///
/// * Every `(tenor_i, tenor_j)` pair from `ir_delta_weights` must have a
///   corresponding entry in `ir_tenor_correlations` (ordered pair form).
/// * `cq_delta_weights` must contain the `"corporates"` key used by the legacy
///   scalar qualifying-credit path.
/// * SIMM v2.6 must include explicit CQ bucket weights, inter-bucket
///   correlations, and per-bucket concentration thresholds.
fn validate_simm_params(params: &SimmParams) -> finstack_core::Result<()> {
    if !params.cq_delta_weights.contains_key("corporates") {
        return Err(finstack_core::Error::Validation(format!(
            "SIMM registry {:?}: cq_delta_weights missing required 'corporates' key",
            params.version
        )));
    }

    let tenors: Vec<&String> = params.ir_delta_weights.keys().collect();
    let mut missing_pairs: Vec<(String, String)> = Vec::new();
    for (i, tenor_i) in tenors.iter().enumerate() {
        for tenor_j in tenors.iter().skip(i + 1) {
            let key = ordered_tenor_pair(tenor_i, tenor_j);
            if !params.ir_tenor_correlations.contains_key(&key) {
                missing_pairs.push(key);
            }
        }
    }
    if !missing_pairs.is_empty() {
        // Keep the error bounded — show up to 5 missing pairs.
        let sample = missing_pairs
            .iter()
            .take(5)
            .map(|(a, b)| format!("({a},{b})"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(finstack_core::Error::Validation(format!(
            "SIMM registry {:?}: ir_tenor_correlations missing {} tenor pair(s) (showing up to 5: {})",
            params.version,
            missing_pairs.len(),
            sample
        )));
    }

    if params.version == SimmVersion::V2_6 {
        use SimmCreditSector::*;
        let sectors = [
            Sovereign,
            Financial,
            BasicMaterials,
            ConsumerGoods,
            TechnologyMedia,
            HealthCare,
            HighYieldSovereign,
            HighYieldFinancial,
            HighYieldBasicMaterials,
            HighYieldConsumerGoods,
            HighYieldTechnologyMedia,
            HighYieldHealthCare,
            Residual,
        ];
        for sector in sectors {
            if !params.cq_bucket_weights.contains_key(&sector) {
                return Err(finstack_core::Error::Validation(format!(
                    "SIMM registry {:?}: cq_bucket_weights missing {sector:?}",
                    params.version
                )));
            }
            if !params.cq_concentration_thresholds.contains_key(&sector) {
                return Err(finstack_core::Error::Validation(format!(
                    "SIMM registry {:?}: cq_concentration_thresholds missing {sector:?}",
                    params.version
                )));
            }
        }
        for (i, &a) in sectors.iter().enumerate() {
            for &b in sectors.iter().skip(i + 1) {
                let key = ordered_credit_sector_pair(a, b);
                if !params.cq_inter_bucket_correlations.contains_key(&key) {
                    return Err(finstack_core::Error::Validation(format!(
                        "SIMM registry {:?}: cq_inter_bucket_correlations missing ({a:?},{b:?})",
                        params.version
                    )));
                }
            }
        }
    }

    Ok(())
}

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
                // Post-`validate_simm_params`: every tenor pair is
                // guaranteed present in `ir_tenor_correlations`. The 0.5
                // fallback is a defensive safety net that should be dead
                // code after successful validation; hitting it indicates
                // a registry bug bypassing the constructor's validation.
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
        cfg: &finstack_core::config::FinstackConfig,
    ) -> finstack_core::Result<Self> {
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

    /// Margin period of risk (days).
    #[must_use]
    pub fn mpor_days(&self) -> u32 {
        self.params.mpor_days
    }

    /// Set margin period of risk.
    ///
    /// # Arguments
    ///
    /// * `days` - Margin period of risk in calendar days
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
        // Group sensitivities by currency.
        let mut by_currency: HashMap<Currency, HashMap<String, f64>> = HashMap::default();
        for ((ccy, tenor), delta) in ir_delta {
            *by_currency
                .entry(*ccy)
                .or_default()
                .entry(tenor.clone())
                .or_insert(0.0) += delta;
        }

        // For each currency: weight the sensitivities, derive the per-
        // currency concentration factor from the net weighted amount,
        // then compute K_c using the (scaled) weighted sensitivities.
        let k_values: Vec<f64> = by_currency
            .values()
            .map(|tenor_map| {
                // Compute WS per tenor, then net_ws, then CR, then K.
                let weighted: Vec<(usize, f64)> = tenor_map
                    .iter()
                    .filter_map(|(tenor, dv01)| {
                        let w = self.params.ir_delta_weights.get(tenor)?;
                        let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor)?;
                        Some((*idx, dv01 * w))
                    })
                    .collect();
                let net_ws: f64 = weighted.iter().map(|(_, ws)| *ws).sum();
                let cf = self.concentration_factor(SimmRiskClass::InterestRate, net_ws);
                let mut sum = 0.0;
                for &(idx_i, ws_i) in &weighted {
                    for &(idx_j, ws_j) in &weighted {
                        let rho = self.ir_corr_matrix.correlation(idx_i, idx_j);
                        sum += rho * (ws_i * cf) * (ws_j * cf);
                    }
                }
                sum.max(0.0).sqrt()
            })
            .collect();

        if k_values.len() <= 1 {
            return k_values.first().copied().unwrap_or(0.0);
        }

        let gamma = self.params.ir_inter_currency_correlation;
        let mut total = 0.0;
        for (i, k_i) in k_values.iter().enumerate() {
            for (j, k_j) in k_values.iter().enumerate() {
                let corr = if i == j { 1.0 } else { gamma };
                total += corr * k_i * k_j;
            }
        }
        total.max(0.0).sqrt()
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
        let weighted: Vec<(usize, f64)> = dv01_by_tenor
            .iter()
            .filter_map(|(tenor, dv01)| {
                let weight = self.params.ir_delta_weights.get(tenor)?;
                let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor)?;
                Some((*idx, dv01 * weight))
            })
            .collect();

        let mut sum = 0.0;
        for &(idx_i, ws_i) in &weighted {
            for &(idx_j, ws_j) in &weighted {
                let rho = self.ir_corr_matrix.correlation(idx_i, idx_j);
                sum += rho * ws_i * ws_j;
            }
        }
        sum.max(0.0).sqrt()
    }

    /// Calculate credit delta margin from CS01-style sensitivities.
    ///
    /// # Arguments
    ///
    /// * `cs01` - Signed currency CS01 per 1bp par-spread move
    /// * `qualifying` - Whether the credit is investment grade (qualifying)
    ///
    /// # Returns
    ///
    /// The credit delta margin contribution after the applicable SIMM risk weight.
    ///
    /// # Invariant
    ///
    /// For `qualifying = true`, the `"corporates"` key must be present in
    /// `params.cq_delta_weights`. `validate_simm_params` enforces this at
    /// construction time, so the fallback below should be dead code; it is
    /// retained as a defensive safety net that logs loudly if ever hit.
    pub fn calculate_credit_delta(&self, cs01: f64, qualifying: bool) -> f64 {
        let weight = if qualifying {
            match self.params.cq_delta_weights.get("corporates").copied() {
                Some(w) => w,
                None => {
                    // Post-validation, this branch is unreachable. If it fires,
                    // registry mutation bypassed the constructor's validation.
                    // Fall back to the non-qualifying weight (still a
                    // registry-sourced value) rather than a magic literal that
                    // could hide a schedule drift.
                    tracing::error!(
                        "SIMM: cq_delta_weights['corporates'] missing post-validation; \
                         falling back to cnq_delta_weight"
                    );
                    self.params.cnq_delta_weight
                }
            }
        } else {
            self.params.cnq_delta_weight
        };

        (cs01 * weight).abs()
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
    pub fn calculate_credit_delta_bucketed(
        &self,
        bucketed_delta: &HashMap<(SimmCreditSector, String, String), f64>,
    ) -> f64 {
        // Group sensitivities by sector bucket.
        let mut by_sector: HashMap<SimmCreditSector, Vec<f64>> = HashMap::default();
        for ((sector, _issuer, _tenor), delta) in bucketed_delta {
            let weight = self.params.cq_bucket_weight(*sector);
            let ws = *delta * weight;
            by_sector.entry(*sector).or_default().push(ws);
        }

        let rho = self.params.cq_intra_bucket_correlation;

        // Compute K_b and S_b (capped) for each bucket.
        let mut bucket_results: Vec<(SimmCreditSector, f64, f64)> = Vec::new();
        for (sector, weighted_sensitivities) in &by_sector {
            // Per-bucket concentration factor on the raw net weighted sum.
            let raw_net: f64 = weighted_sensitivities.iter().sum();
            let cf = self.params.cq_concentration_factor(*sector, raw_net);

            // K_b = sqrt(sum_i sum_j rho_ij * (CR*WS_i) * (CR*WS_j))
            //     = |CR| * sqrt(sum_i sum_j rho_ij * WS_i * WS_j)
            // Build it from the scaled WS directly for clarity.
            let scaled: Vec<f64> = weighted_sensitivities.iter().map(|ws| ws * cf).collect();
            let mut k_squared = 0.0;
            for (i, ws_i) in scaled.iter().enumerate() {
                for (j, ws_j) in scaled.iter().enumerate() {
                    let corr = if i == j { 1.0 } else { rho };
                    k_squared += corr * ws_i * ws_j;
                }
            }
            let k_b = k_squared.max(0.0).sqrt();

            // S_b = max(-K_b, min(K_b, sum CR*WS))
            let net_scaled: f64 = scaled.iter().sum();
            let s_b = net_scaled.clamp(-k_b, k_b);

            bucket_results.push((*sector, k_b, s_b));
        }

        // Inter-bucket aggregation:
        //   K = sqrt(sum_b K_b^2 + sum_{b != c} gamma_bc * S_b * S_c)
        let mut total = 0.0;
        for (i, &(sector_i, k_i, s_i)) in bucket_results.iter().enumerate() {
            total += k_i * k_i;
            for (j, &(sector_j, _k_j, s_j)) in bucket_results.iter().enumerate() {
                if i != j {
                    let gamma = self.params.cq_inter_bucket_correlation(sector_i, sector_j);
                    total += gamma * s_i * s_j;
                }
            }
        }
        total.max(0.0).sqrt()
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

    /// Calculate FX delta margin.
    ///
    /// # Arguments
    ///
    /// * `fx_delta` - Signed currency FX delta sensitivity
    ///
    /// # Returns
    ///
    /// The weighted FX delta margin contribution.
    pub fn calculate_fx_delta(&self, fx_delta: f64) -> f64 {
        (fx_delta * self.params.fx_delta_weight).abs()
    }

    /// Calculate FX delta margin across currency risk factors.
    ///
    /// Each currency sensitivity is weighted and concentration-scaled
    /// independently, then aggregated with the SIMM FX intra-bucket correlation
    /// between distinct currency risk factors. This prevents opposite-signed
    /// currency deltas from receiving full rho=1 offset.
    pub fn calculate_fx_delta_bucketed(&self, fx_delta: &HashMap<Currency, f64>) -> f64 {
        let weighted: Vec<f64> = fx_delta
            .values()
            .map(|delta| {
                let ws = delta * self.params.fx_delta_weight;
                let cf = self.concentration_factor(SimmRiskClass::Fx, ws);
                ws * cf
            })
            .collect();

        let mut total = 0.0;
        for (i, ws_i) in weighted.iter().enumerate() {
            for (j, ws_j) in weighted.iter().enumerate() {
                let corr = if i == j {
                    1.0
                } else {
                    self.params.fx_intra_bucket_correlation
                };
                total += corr * ws_i * ws_j;
            }
        }

        total.max(0.0).sqrt()
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
        let weighted_buckets: Vec<(u8, f64)> = delta_by_bucket
            .iter()
            .filter_map(|(bucket, delta)| {
                let bucket_id = bucket_id_from_label(bucket)?;
                let weight = self.params.commodity_bucket_weight(bucket);
                Some((bucket_id, delta * weight))
            })
            .collect();

        let mut sum = 0.0;
        for &(bucket_i, weighted_i) in &weighted_buckets {
            for &(bucket_j, weighted_j) in &weighted_buckets {
                let rho = if bucket_i == bucket_j {
                    1.0
                } else {
                    self.params
                        .commodity_inter_bucket_correlation(bucket_i, bucket_j)
                };
                sum += rho * weighted_i * weighted_j;
            }
        }
        sum.max(0.0).sqrt()
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
        let indexed: Vec<(usize, f64)> = vega_by_tenor
            .iter()
            .filter_map(|(tenor, vega)| {
                let idx = self.ir_corr_matrix.tenor_to_idx.get(tenor.as_str())?;
                Some((*idx, *vega * weight))
            })
            .collect();

        let mut sum = 0.0;
        for &(idx_i, wv_i) in &indexed {
            for &(idx_j, wv_j) in &indexed {
                let rho = self.ir_corr_matrix.correlation(idx_i, idx_j);
                sum += rho * wv_i * wv_j;
            }
        }
        sum.max(0.0).sqrt()
    }

    /// Calculate credit vega margin.
    ///
    /// `total_vega` is expected to be a signed currency vega amount already
    /// aggregated across the caller's chosen credit buckets.
    pub fn calculate_credit_vega(&self, total_vega: f64, qualifying: bool) -> f64 {
        let weight = if qualifying {
            self.params.cq_vega_weight
        } else {
            self.params.cnq_vega_weight
        };
        (total_vega * weight).abs()
    }

    /// Calculate equity vega margin from a signed currency vega amount.
    pub fn calculate_equity_vega(&self, total_vega: f64) -> f64 {
        (total_vega * self.params.equity_vega_weight).abs()
    }

    /// Calculate FX vega margin from a signed currency vega amount.
    pub fn calculate_fx_vega(&self, total_vega: f64) -> f64 {
        (total_vega * self.params.fx_vega_weight).abs()
    }

    /// Calculate commodity vega margin from a signed currency vega amount.
    pub fn calculate_commodity_vega(&self, total_vega: f64) -> f64 {
        (total_vega * self.params.commodity_vega_weight).abs()
    }

    /// Calculate curvature margin across risk classes.
    ///
    /// SIMM curvature risk = scale_factor x max(0, sum of curvature CVR)
    ///
    /// `curvature_by_risk_class` should contain signed currency curvature
    /// contributions before the SIMM scale factor is applied.
    pub fn calculate_curvature(
        &self,
        curvature_by_risk_class: &HashMap<SimmRiskClass, f64>,
    ) -> f64 {
        let scale = self.params.curvature_scale_factor;
        let mut sum = 0.0;
        for (risk_i, cvr_i) in curvature_by_risk_class {
            let weighted_i = cvr_i * scale;
            for (risk_j, cvr_j) in curvature_by_risk_class {
                let weighted_j = cvr_j * scale;
                let rho = self.params.correlation(*risk_i, *risk_j);
                sum += rho * weighted_i * weighted_j;
            }
        }
        sum.max(0.0).sqrt()
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

    /// Calculate SIMM margin from pre-computed sensitivities.
    ///
    /// This is the primary entry point for SIMM calculation when you have
    /// `SimmSensitivities` from a `Marginable` instrument.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - SIMM sensitivities by risk class using the units documented on [`SimmSensitivities`]
    /// * `currency` - Currency in which returned [`Money`] amounts will be labeled
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
    /// use finstack_core::currency::Currency;
    /// use finstack_margin::{SimmCalculator, SimmSensitivities, SimmVersion};
    ///
    /// # fn main() -> finstack_core::Result<()> {
    /// let calc = SimmCalculator::new(SimmVersion::V2_6)?;
    /// let mut sensitivities = SimmSensitivities::new(Currency::USD);
    /// sensitivities.add_ir_delta(Currency::USD, "5y", 50_000.0);
    ///
    /// let (total, breakdown) =
    ///     calc.calculate_from_sensitivities(&sensitivities, Currency::USD);
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
    pub fn calculate_from_sensitivities(
        &self,
        sensitivities: &SimmSensitivities,
        currency: Currency,
    ) -> (f64, HashMap<String, Money>) {
        let mut breakdown = HashMap::default();
        let mut risk_class_margins = HashMap::default();

        // IR Delta — per-currency calculation with inter-currency aggregation
        if !sensitivities.ir_delta.is_empty() {
            let ir_margin = self.calculate_ir_delta_multi_currency(&sensitivities.ir_delta);
            if ir_margin > 0.0 {
                breakdown.insert("IR_Delta".to_string(), Money::new(ir_margin, currency));
                risk_class_margins.insert(SimmRiskClass::InterestRate, ir_margin);
            }
        }

        // IR Vega
        if !sensitivities.ir_vega.is_empty() {
            let ir_vega_map: HashMap<String, f64> = sensitivities
                .ir_vega
                .iter()
                .map(|((_, tenor), vega)| (tenor.clone(), *vega))
                .collect();
            let ir_vega_margin = self.calculate_ir_vega(&ir_vega_map);
            if ir_vega_margin > 0.0 {
                breakdown.insert("IR_Vega".to_string(), Money::new(ir_vega_margin, currency));
                *risk_class_margins
                    .entry(SimmRiskClass::InterestRate)
                    .or_insert(0.0) += ir_vega_margin;
            }
        }

        // Credit Delta (Qualifying) -- use bucketed path when available
        if !sensitivities.credit_qualifying_delta_bucketed.is_empty() {
            let credit_margin = self
                .calculate_credit_delta_bucketed(&sensitivities.credit_qualifying_delta_bucketed);
            if credit_margin > 0.0 {
                breakdown.insert(
                    "Credit_Qualifying_Delta".to_string(),
                    Money::new(credit_margin, currency),
                );
                risk_class_margins.insert(SimmRiskClass::CreditQualifying, credit_margin);
            }
        } else {
            let qualifying_total = sensitivities.credit_qualifying_delta.values().sum::<f64>();
            if qualifying_total.abs() > 0.0 {
                let credit_margin = self.calculate_credit_delta(qualifying_total, true);
                if credit_margin > 0.0 {
                    breakdown.insert(
                        "Credit_Qualifying_Delta".to_string(),
                        Money::new(credit_margin, currency),
                    );
                    risk_class_margins.insert(SimmRiskClass::CreditQualifying, credit_margin);
                }
            }
        }

        // Credit Delta (Non-Qualifying)
        let non_qual_total = sensitivities
            .credit_non_qualifying_delta
            .values()
            .sum::<f64>();
        if non_qual_total.abs() > 0.0 {
            let credit_margin = self.calculate_credit_delta(non_qual_total, false);
            if credit_margin > 0.0 {
                breakdown.insert(
                    "Credit_NonQualifying_Delta".to_string(),
                    Money::new(credit_margin, currency),
                );
                risk_class_margins.insert(SimmRiskClass::CreditNonQualifying, credit_margin);
            }
        }

        // Equity Delta
        let total_equity = sensitivities.total_equity_delta();
        if total_equity.abs() > 0.0 {
            let equity_margin = self.calculate_equity_delta(total_equity);
            if equity_margin > 0.0 {
                breakdown.insert(
                    "Equity_Delta".to_string(),
                    Money::new(equity_margin, currency),
                );
                risk_class_margins.insert(SimmRiskClass::Equity, equity_margin);
            }
        }

        // Equity Vega
        let total_equity_vega: f64 = sensitivities.equity_vega.values().sum();
        if total_equity_vega.abs() > 0.0 {
            let equity_vega_margin = self.calculate_equity_vega(total_equity_vega);
            if equity_vega_margin > 0.0 {
                breakdown.insert(
                    "Equity_Vega".to_string(),
                    Money::new(equity_vega_margin, currency),
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
                breakdown.insert("FX_Delta".to_string(), Money::new(fx_margin, currency));
                risk_class_margins.insert(SimmRiskClass::Fx, fx_margin);
            }
        }

        // FX Vega
        let total_fx_vega: f64 = sensitivities.fx_vega.values().sum();
        if total_fx_vega.abs() > 0.0 {
            let fx_vega_margin = self.calculate_fx_vega(total_fx_vega);
            if fx_vega_margin > 0.0 {
                breakdown.insert("FX_Vega".to_string(), Money::new(fx_vega_margin, currency));
                *risk_class_margins.entry(SimmRiskClass::Fx).or_insert(0.0) += fx_vega_margin;
            }
        }

        // Commodity Delta
        if !sensitivities.commodity_delta.is_empty() {
            let commodity_margin = self.calculate_commodity_delta(&sensitivities.commodity_delta);
            if commodity_margin > 0.0 {
                breakdown.insert(
                    "Commodity_Delta".to_string(),
                    Money::new(commodity_margin, currency),
                );
                risk_class_margins.insert(SimmRiskClass::Commodity, commodity_margin);
            }
        }

        // Apply concentration factors for the remaining risk classes.
        //
        // - InterestRate: per-currency CF already applied inside
        //   `calculate_ir_delta_multi_currency`.
        // - Fx: per-currency CF already applied in the FX block above.
        // - CreditQualifying (bucketed path): per-bucket CF already
        //   applied inside `calculate_credit_delta_bucketed`.
        //
        // For the legacy-scalar credit paths and for Equity / Commodity
        // (where the inputs are pooled by construction), the pool-level
        // CF is the best available approximation.
        let uses_bucketed_cq = !sensitivities.credit_qualifying_delta_bucketed.is_empty();
        let net_sensitivities: HashMap<SimmRiskClass, f64> = [
            (
                SimmRiskClass::CreditQualifying,
                sensitivities.credit_qualifying_delta.values().sum::<f64>(),
            ),
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
                SimmRiskClass::InterestRate | SimmRiskClass::Fx => continue,
                SimmRiskClass::CreditQualifying if uses_bucketed_cq => continue,
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
                breakdown.insert("Curvature".to_string(), Money::new(cm, currency));
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

        (total_im, breakdown)
    }

    /// Aggregate risk class margins with the SIMM inter-risk-class correlation matrix.
    ///
    /// `Total = sqrt(sum_i sum_j rho(i,j) * K_i * K_j)`
    pub fn aggregate_risk_classes(&self, risk_class_margins: &HashMap<SimmRiskClass, f64>) -> f64 {
        let mut sum = 0.0;
        for (risk_i, margin_i) in risk_class_margins {
            for (risk_j, margin_j) in risk_class_margins {
                let rho = self.params.correlation(*risk_i, *risk_j);
                sum += rho * margin_i * margin_j;
            }
        }
        sum.max(0.0).sqrt()
    }
}

fn bucket_id_from_label(bucket: &str) -> Option<u8> {
    let trimmed = bucket.trim();
    if let Ok(value) = trimmed.parse::<u8>() {
        return Some(value);
    }
    let normalized: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "coal" => Some(1),
        "crude" => Some(2),
        "lightends" => Some(3),
        "middledistillates" => Some(4),
        "heavydistillates" => Some(5),
        "northamericannaturalgas" => Some(6),
        "europeannaturalgas" => Some(7),
        "northamericanpowerandcarbon" => Some(8),
        "europeanpowerandcarbon" => Some(9),
        "freight" => Some(10),
        "basemetals" => Some(11),
        "preciousmetals" => Some(12),
        "grainsandoilseed" => Some(13),
        "softsandotheragriculturals" => Some(14),
        "livestockanddairy" => Some(15),
        "other" => Some(16),
        "indexes" | "indices" => Some(17),
        _ => None,
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
        let (total_im, breakdown) = self.calculate_from_sensitivities(&sensitivities, currency);

        debug!(
            instrument = instrument.id(),
            total_im,
            risk_classes = breakdown.len(),
            "SIMM IM calculated"
        );

        Ok(ImResult::with_breakdown(
            Money::new(total_im, currency),
            ImMethodology::Simm,
            as_of,
            self.mpor_days(),
            breakdown,
        ))
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
    }

    #[test]
    fn embedded_simm_registries_pass_validation() {
        for version in [SimmVersion::V2_5, SimmVersion::V2_6] {
            SimmCalculator::new(version)
                .unwrap_or_else(|e| panic!("SIMM {version:?} should validate: {e}"));
        }
    }

    #[test]
    fn validate_rejects_missing_corporates_weight() {
        let mut params = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .params
            .clone();
        params.cq_delta_weights.remove("corporates");
        let err = validate_simm_params(&params).expect_err("should reject missing corporates");
        let msg = err.to_string();
        assert!(
            msg.contains("corporates"),
            "error should name the key: {msg}"
        );
    }

    #[test]
    fn validate_rejects_missing_ir_tenor_pair() {
        let mut params = SimmCalculator::new(SimmVersion::V2_6)
            .expect("registry should load")
            .params
            .clone();
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
            ("5y".to_string(), 100_000.0), // $100K DV01 at 5y
        ]
        .into_iter()
        .collect();

        let ir_margin = calc.calculate_ir_delta(&dv01_by_tenor);

        // Risk weight for 5y is 51, so margin = 100K * 51 = 5.1M
        assert!((ir_margin - 5_100_000.0).abs() < 1.0);
    }

    #[test]
    fn credit_delta_calculation() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let cs01 = 50_000.0; // $50K CS01

        let cq_margin = calc.calculate_credit_delta(cs01, true);
        let cnq_margin = calc.calculate_credit_delta(cs01, false);

        // Qualifying uses lower weight (~73), non-qualifying uses 500
        assert!(cq_margin < cnq_margin);
        assert!((cq_margin - 3_650_000.0).abs() < 1.0); // 50K * 73
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
        assert!(calc.params.ir_delta_weights.contains_key("5y"));
        assert!(calc.params.cq_delta_weights.contains_key("corporates"));
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
        sens.add_ir_delta(Currency::USD, "5y", 100_000.0);
        sens.add_equity_delta("AAPL", 100_000.0);

        let (total_im, breakdown) = calc.calculate_from_sensitivities(&sens, Currency::USD);

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
            ("5y".to_string(), 100_000.0),
            ("10y".to_string(), -80_000.0), // Partially hedged
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
            [("5y".to_string(), 500_000.0)].into_iter().collect();

        let ir_vega_margin = calc.calculate_ir_vega(&vega_by_tenor);
        // Single tenor: sqrt((500K * 0.21)^2) = 500K * 0.21 = 105K
        assert!((ir_vega_margin - 105_000.0).abs() < 1.0);
    }

    #[test]
    fn curvature_uses_correlated_aggregation_across_risk_classes() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
        let curvature_by_risk_class: HashMap<SimmRiskClass, f64> = [
            (SimmRiskClass::InterestRate, 1_000_000.0),
            (SimmRiskClass::Equity, -600_000.0),
        ]
        .into_iter()
        .collect();

        let actual = calc.calculate_curvature(&curvature_by_risk_class);
        let scale = calc.params.curvature_scale_factor;
        let rho = calc
            .params
            .correlation(SimmRiskClass::InterestRate, SimmRiskClass::Equity);
        let ir = 1_000_000.0 * scale;
        let eq = -600_000.0 * scale;
        let expected = (ir * ir + eq * eq + 2.0 * rho * ir * eq).sqrt();

        assert!(
            (actual - expected).abs() < 1.0,
            "expected correlated curvature {}, got {}",
            expected,
            actual
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
        sensitivities.add_ir_delta(Currency::USD, "5y", 50_000.0);
        sensitivities.add_equity_delta("AAPL", 100_000.0);
        sensitivities.add_fx_delta(Currency::EUR, 80_000.0);

        let instrument = MarginableTestInstrument::new(
            Money::new(1_000_000.0, Currency::USD),
            sensitivities.clone(),
        );
        let market = MarketContext::new();

        let expected = calc.calculate_from_sensitivities(&sensitivities, Currency::USD);
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

    // -------------------------------------------------------------------------
    // Bucketed credit qualifying delta tests
    // -------------------------------------------------------------------------

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
        let bucketed_margin = calc.calculate_credit_delta_bucketed(&bucketed);

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
        let bucketed_margin = calc.calculate_credit_delta_bucketed(&bucketed);
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
        let actual = calc.calculate_credit_delta_bucketed(&bucketed);

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
        let actual = calc.calculate_credit_delta_bucketed(&bucketed);

        assert!(
            (actual - expected).abs() < 1.0,
            "intra-bucket two names: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn calculate_from_sensitivities_uses_bucketed_when_available() {
        // Verify that calculate_from_sensitivities dispatches to the bucketed
        // path when credit_qualifying_delta_bucketed is populated.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_delta_bucketed(SimmCreditSector::Sovereign, "GOVT_A", "5Y", 50_000.0);
        sens.add_credit_delta_bucketed(SimmCreditSector::Financial, "BANK_A", "5Y", 50_000.0);

        let (total_im, breakdown) = calc.calculate_from_sensitivities(&sens, Currency::USD);
        assert!(total_im > 0.0, "total IM should be positive");
        assert!(
            breakdown.contains_key("Credit_Qualifying_Delta"),
            "breakdown should contain Credit_Qualifying_Delta"
        );

        // The bucketed margin should match the direct bucketed calculation.
        let expected = calc.calculate_credit_delta_bucketed(&sens.credit_qualifying_delta_bucketed);
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

    #[test]
    fn legacy_scalar_path_still_works() {
        // Verify backward compatibility: when only the old
        // credit_qualifying_delta map is populated, the scalar path is used.
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");

        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_credit_delta("CDX.NA.IG", true, "5Y", 100_000.0);

        let (total_im, breakdown) = calc.calculate_from_sensitivities(&sens, Currency::USD);
        assert!(total_im > 0.0, "total IM should be positive");
        assert!(
            breakdown.contains_key("Credit_Qualifying_Delta"),
            "breakdown should contain Credit_Qualifying_Delta"
        );

        // Should match scalar calculation.
        let expected = calc.calculate_credit_delta(100_000.0, true);
        let actual = breakdown
            .get("Credit_Qualifying_Delta")
            .expect("CQ delta breakdown entry")
            .amount();
        assert!(
            (actual - expected).abs() < 1.0,
            "legacy scalar path: expected {expected}, got {actual}"
        );
    }
}
