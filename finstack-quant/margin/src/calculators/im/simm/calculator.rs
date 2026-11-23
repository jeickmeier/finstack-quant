//! ISDA SIMM initial-margin calculator implementation.

use super::params::{
    BucketSink, DetailCollector, IrTenorCorrelationMatrix, SimmVersion, concentration, fx_category,
    fx_high_volatility, ir_currency_group, resolve_simm_params, sector_label, single_bucket,
};
use crate::Marginable;
use crate::calculators::traits::{
    ImCalculator, ImResult, SimmBucketDetail, SimmWeightedSensitivity,
};
use crate::registry::{MarginRegistry, SimmParams, embedded_registry, margin_registry_from_config};
use crate::regulatory::frtb::aggregation::{correlated_norm, inter_bucket_pairwise};
use crate::types::ImMethodology;
use crate::types::{SimmCreditSector, SimmRiskClass, SimmSensitivities};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use tracing::debug;

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
    pub(super) params: SimmParams,
    pub(super) ir_corr_matrix: IrTenorCorrelationMatrix,
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
    pub(super) fn build_from_registry(
        version: SimmVersion,
        registry: &MarginRegistry,
    ) -> Result<Self> {
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
    pub(super) fn aggregate_ir(
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
    pub(super) fn ir_tenor_norm(&self, indexed: &[(usize, f64)]) -> f64 {
        let ws: Vec<f64> = indexed.iter().map(|(_, ws)| *ws).collect();
        correlated_norm(&ws, |i, j| {
            self.ir_corr_matrix.correlation(indexed[i].0, indexed[j].0)
        })
    }

    /// Combine per-currency IR margins with the SIMM inter-currency
    /// correlation `γ` (uniform off-diagonal).
    pub(super) fn aggregate_ir_currencies(&self, buckets: &[(f64, f64, f64)]) -> f64 {
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
    pub(super) fn aggregate_credit_qualifying(
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
    pub(super) fn calculate_equity_delta(&self, equity_delta: f64) -> (f64, f64, f64) {
        let weight = self.params.equity_delta_weight;
        let cf = self.concentration_factor(SimmRiskClass::Equity, equity_delta);
        (weight, cf, equity_delta * weight * cf)
    }

    /// Equity delta or vega margin across names (zero inter-name correlation).
    ///
    /// Each name's margin is the absolute value of its weighted sensitivity;
    /// the margins are summed in ascending order so the result does not
    /// depend on map iteration order.
    pub(super) fn equity_margin(
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
    pub(super) fn calculate_fx_delta_bucketed(
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
    pub(super) fn aggregate_commodity(
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
    pub(super) fn calculate_equity_vega(&self, total_vega: f64) -> (f64, f64, f64) {
        let raw = total_vega * self.params.equity_historical_volatility_ratio;
        let weight = self.params.equity_vega_weight;
        let cf = concentration(
            raw,
            self.params.vega_concentration_thresholds[&SimmRiskClass::Equity],
        );
        (weight, cf, raw * weight * cf)
    }

    pub(super) fn fx_vega_bucketed(
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

    pub(super) fn non_qualifying_margin(
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
    pub(super) fn concentration_factor(
        &self,
        risk_class: SimmRiskClass,
        net_sensitivity: f64,
    ) -> f64 {
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
    pub(super) fn aggregate_risk_classes(
        &self,
        risk_class_margins: &FxHashMap<SimmRiskClass, f64>,
    ) -> f64 {
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
