//! Historical SIMM v2.6 paragraph 11: scale expiries before factor netting.

use super::{correlated_norm, SimmCalculator, SimmCreditSector, SimmRiskClass, SIMM_CURVATURE_Z};
use crate::types::simm_curvature::{fx_pair, tenor_days};
use crate::types::{commodity_bucket_id, ordered_tenor_pair, SimmCurvatureSensitivity};
use finstack_quant_core::{currency::Currency, Error, HashMap, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FactorKey {
    class: SimmRiskClass,
    bucket: String,
    factor: String,
    tenor: Option<String>,
}

struct BucketMargin {
    bucket: String,
    k: f64,
    signed: f64,
    sum: f64,
    absolute_sum: f64,
}

fn curvature_charge(sum: f64, absolute_sum: f64, norm: f64) -> f64 {
    if absolute_sum == 0.0 {
        return 0.0;
    }
    let theta = (sum / absolute_sum).min(0.0);
    let lambda = (SIMM_CURVATURE_Z * SIMM_CURVATURE_Z - 1.0) * (1.0 + theta) - theta;
    (sum + lambda * norm).max(0.0)
}

impl SimmCalculator {
    /// Curvature is part of each risk-class margin before risk-class correlation.
    pub(super) fn curvature_by_risk_class(
        &self,
        inputs: &[SimmCurvatureSensitivity],
    ) -> Result<HashMap<SimmRiskClass, f64>> {
        let mut by_factor: HashMap<FactorKey, Vec<f64>> = HashMap::default();
        for input in inputs {
            input.validate()?;
            let mut factor = input.factor.clone();
            let bucket = match input.risk_class {
                SimmRiskClass::InterestRate => input
                    .bucket
                    .parse::<Currency>()
                    .map_err(|e| Error::Validation(e.to_string()))?
                    .to_string(),
                SimmRiskClass::CreditQualifying => finstack_quant_core::wire::serde_label(
                    &input
                        .bucket
                        .parse::<SimmCreditSector>()
                        .map_err(Error::Validation)?,
                )?,
                SimmRiskClass::Commodity => commodity_bucket_id(&input.bucket)
                    .ok_or_else(|| Error::Validation("invalid curvature commodity bucket".into()))?
                    .to_string(),
                SimmRiskClass::Fx => {
                    let (a, b) = fx_pair(&factor)?;
                    factor = format!("{a}/{b}");
                    "fx".into()
                }
                _ => input.bucket.clone(),
            };
            let days = tenor_days(&input.expiry_tenor)
                .ok_or_else(|| Error::Validation("invalid curvature expiry tenor".into()))?;
            let cvr = input.volatility_weighted_vega * 0.5 * (14.0 / days).min(1.0);
            let key = FactorKey {
                class: input.risk_class,
                bucket,
                factor,
                tenor: input.risk_tenor.clone(),
            };
            by_factor.entry(key).or_default().push(cvr);
        }

        // Sum in canonical order after SF; raw opposite vegas never net early.
        let mut factors: Vec<_> = by_factor
            .into_iter()
            .map(|(key, mut values)| {
                values.sort_by(f64::total_cmp);
                (key, values.iter().sum::<f64>())
            })
            .collect();
        factors.sort_by(|(a, _), (b, _)| {
            (a.class as u8, &a.bucket, &a.factor, &a.tenor).cmp(&(
                b.class as u8,
                &b.bucket,
                &b.factor,
                &b.tenor,
            ))
        });
        if factors.iter().any(|(_, amount)| !amount.is_finite()) {
            return Err(Error::Validation(
                "SIMM curvature factor sum overflowed".into(),
            ));
        }
        let mut by_class: HashMap<SimmRiskClass, Vec<BucketMargin>> = HashMap::default();
        let mut start = 0;
        while start < factors.len() {
            let first = &factors[start].0;
            let end = factors[start..]
                .iter()
                .position(|(key, _)| key.class != first.class || key.bucket != first.bucket)
                .map_or(factors.len(), |offset| start + offset);
            let entries = &factors[start..end];
            let amounts: Vec<_> = entries.iter().map(|(_, value)| *value).collect();
            let k = correlated_norm(&amounts, |i, j| {
                let rho = self.curvature_factor_correlation(&entries[i].0, &entries[j].0);
                rho * rho
            });
            let sum = amounts.iter().sum::<f64>();
            by_class.entry(first.class).or_default().push(BucketMargin {
                bucket: first.bucket.clone(),
                k,
                signed: sum.clamp(-k, k),
                sum,
                absolute_sum: amounts.iter().map(|v| v.abs()).sum(),
            });
            start = end;
        }
        let mut result = HashMap::default();
        for (class, buckets) in by_class {
            let mut non_residual = Vec::new();
            let mut residual = 0.0;
            for bucket in &buckets {
                if bucket.bucket == "residual" {
                    residual += curvature_charge(bucket.sum, bucket.absolute_sum, bucket.k);
                } else {
                    non_residual.push(bucket);
                }
            }
            let mut variance: f64 = non_residual.iter().map(|b| b.k * b.k).sum();
            for (i, a) in non_residual.iter().enumerate() {
                for b in &non_residual[..i] {
                    let rho = self.curvature_bucket_correlation(class, &a.bucket, &b.bucket)?;
                    variance += 2.0 * rho * rho * a.signed * b.signed;
                }
            }
            let sum = non_residual.iter().map(|b| b.sum).sum();
            let absolute_sum = non_residual.iter().map(|b| b.absolute_sum).sum();
            let mut margin =
                residual + curvature_charge(sum, absolute_sum, variance.max(0.0).sqrt());
            if class == SimmRiskClass::InterestRate {
                margin /= self.params.ir_historical_volatility_ratio.powi(2);
            }
            if !margin.is_finite() {
                return Err(Error::Validation("SIMM curvature margin overflowed".into()));
            }
            result.insert(class, margin);
        }
        Ok(result)
    }

    fn curvature_factor_correlation(&self, a: &FactorKey, b: &FactorKey) -> f64 {
        match a.class {
            SimmRiskClass::InterestRate => {
                let tenor_rho = if a.tenor == b.tenor {
                    1.0
                } else {
                    self.params.ir_tenor_correlations[&ordered_tenor_pair(
                        a.tenor.as_deref().unwrap_or_default(),
                        b.tenor.as_deref().unwrap_or_default(),
                    )]
                };
                tenor_rho
                    * if a.factor == b.factor {
                        1.0
                    } else {
                        self.params.ir_subcurve_correlation
                    }
            }
            SimmRiskClass::CreditQualifying => {
                if a.bucket == "residual" {
                    self.params.credit_residual_correlation
                } else if a.factor == b.factor {
                    self.params.cq_same_issuer_correlation
                } else {
                    self.params.cq_intra_bucket_correlation
                }
            }
            SimmRiskClass::CreditNonQualifying => self.params.credit_residual_correlation,
            SimmRiskClass::Equity => 0.0,
            SimmRiskClass::Commodity => self.params.commodity_intra_bucket_correlations[&a.bucket],
            SimmRiskClass::Fx => 0.5,
        }
    }

    fn curvature_bucket_correlation(&self, class: SimmRiskClass, a: &str, b: &str) -> Result<f64> {
        Ok(match class {
            SimmRiskClass::InterestRate => self.params.ir_inter_currency_correlation,
            SimmRiskClass::CreditQualifying => self.params.cq_inter_bucket_correlation(
                a.parse::<SimmCreditSector>().map_err(Error::Validation)?,
                b.parse::<SimmCreditSector>().map_err(Error::Validation)?,
            ),
            SimmRiskClass::Commodity => self.params.commodity_inter_bucket_correlation(
                a.parse::<u8>()
                    .map_err(|e| Error::Validation(e.to_string()))?,
                b.parse::<u8>()
                    .map_err(|e| Error::Validation(e.to_string()))?,
            ),
            _ => 0.0,
        })
    }
}
