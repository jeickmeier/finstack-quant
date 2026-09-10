//! Expiry-resolved volatility risk used by historical SIMM curvature.

use super::{commodity_bucket_id, SimmCreditSector, SimmRiskClass, SIMM_TENORS};
use finstack_quant_core::{currency::Currency, Error, Result};

/// One volatility-weighted vega input before SIMM curvature scaling.
///
/// The amount is `sigma * dPV/dsigma` in the sensitivity container's base
/// currency, before HVR, vega risk weights or concentration. For equity, FX
/// and commodity, `sigma` is the paragraph 10(b) prescribed volatility proxy;
/// for rates and credit it is the matching quoted ATM volatility. Preserve
/// separate expiries until `SF(t) = 0.5 * min(1, 14/t_days)` has been applied.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmCurvatureSensitivity {
    /// SIMM risk class selecting the prescribed bucket and correlation rules.
    pub risk_class: SimmRiskClass,
    /// Currency code for IR, credit sector for qualifying credit, numeric
    /// commodity bucket, `fx` for FX, or `residual` for equity/non-qualifying credit.
    pub bucket: String,
    /// IR subcurve name, credit issuer, equity underlier, commodity name,
    /// or an FX currency pair such as `EUR/USD`; must be nonempty.
    pub factor: String,
    /// IR or credit risk-factor tenor. Required for IR and credit; absent for
    /// equity, commodity and FX. This is distinct from the option expiry.
    pub risk_tenor: Option<String>,
    /// Option-expiry tenor from [`SIMM_TENORS`]. Two weeks means 14 calendar
    /// days; months use 365/12 days and years use 365 days.
    pub expiry_tenor: String,
    /// Signed base-currency `sigma * dPV/dsigma` before SF, HVR, VRW or concentration.
    pub volatility_weighted_vega: f64,
}

impl SimmCurvatureSensitivity {
    /// Check bucket, factor, tenor and amount invariants before aggregation.
    ///
    /// # Errors
    ///
    /// Returns a validation error for unsupported buckets, missing or invalid
    /// risk tenors, invalid FX pairs, empty factors or non-finite amounts.
    pub fn validate(&self) -> Result<()> {
        let invalid = |reason: &str| Error::Validation(format!("SIMM curvature: {reason}"));
        if self.factor.trim().is_empty() || !self.volatility_weighted_vega.is_finite() {
            return Err(invalid(
                "factor must be nonempty and volatility_weighted_vega finite",
            ));
        }
        if tenor_days(&self.expiry_tenor).is_none() {
            return Err(invalid("expiry_tenor must be a SIMM tenor"));
        }
        match self.risk_class {
            SimmRiskClass::InterestRate => {
                self.bucket
                    .parse::<Currency>()
                    .map_err(|_| invalid("IR bucket must be a currency code"))?;
                if !self
                    .risk_tenor
                    .as_deref()
                    .is_some_and(|t| SIMM_TENORS.contains(&t))
                {
                    return Err(invalid("IR risk_tenor must be a SIMM tenor"));
                }
            }
            SimmRiskClass::CreditQualifying | SimmRiskClass::CreditNonQualifying => {
                if self.risk_class == SimmRiskClass::CreditQualifying {
                    self.bucket
                        .parse::<SimmCreditSector>()
                        .map_err(|_| invalid("unknown credit sector bucket"))?;
                } else if self.bucket != "residual" {
                    return Err(invalid(
                        "non-qualifying credit requires the residual bucket",
                    ));
                }
                if !self
                    .risk_tenor
                    .as_deref()
                    .is_some_and(|t| ["1Y", "2Y", "3Y", "5Y", "10Y"].contains(&t))
                {
                    return Err(invalid("credit risk_tenor must be 1Y, 2Y, 3Y, 5Y or 10Y"));
                }
            }
            SimmRiskClass::Equity | SimmRiskClass::Commodity | SimmRiskClass::Fx => {
                if self.risk_tenor.is_some() {
                    return Err(invalid("non-rate/non-credit risk_tenor must be absent"));
                }
                match self.risk_class {
                    SimmRiskClass::Equity if self.bucket != "residual" => {
                        return Err(invalid("unclassified equity requires the residual bucket"))
                    }
                    SimmRiskClass::Commodity if commodity_bucket_id(&self.bucket).is_none() => {
                        return Err(invalid("unknown commodity bucket"))
                    }
                    SimmRiskClass::Fx => {
                        if self.bucket != "fx" {
                            return Err(invalid("FX bucket must be fx"));
                        }
                        fx_pair(&self.factor)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn fx_pair(value: &str) -> Result<(Currency, Currency)> {
    let invalid = || {
        Error::Validation(
            "SIMM curvature FX factor must be two distinct currency codes separated by /"
                .to_owned(),
        )
    };
    let (a, b) = value.split_once('/').ok_or_else(invalid)?;
    let a = a.parse::<Currency>().map_err(|_| invalid())?;
    let b = b.parse::<Currency>().map_err(|_| invalid())?;
    if a == b {
        return Err(invalid());
    }
    Ok((a.min(b), a.max(b)))
}

pub(crate) fn tenor_days(tenor: &str) -> Option<f64> {
    Some(match tenor {
        "2W" => 14.0,
        "1M" => 365.0 / 12.0,
        "3M" => 365.0 / 4.0,
        "6M" => 365.0 / 2.0,
        "1Y" => 365.0,
        "2Y" => 730.0,
        "3Y" => 1095.0,
        "5Y" => 1825.0,
        "10Y" => 3650.0,
        "15Y" => 5475.0,
        "20Y" => 7300.0,
        "30Y" => 10950.0,
        _ => return None,
    })
}
