//! Loader for rate index conventions embedded in JSON registries.

use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::market::conventions::defs::{RateIndexConventions, RateIndexKind};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::types::IndexId;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RateIndexConventionsRecord {
    currency: Currency,
    kind: RateIndexKind,
    #[serde(default)]
    tenor: Option<String>,
    day_count: DayCount,
    default_payment_frequency: String,
    default_payment_lag_days: i32,
    default_reset_lag_days: i32,
    #[serde(default)]
    ois_compounding: Option<OisCompoundingSpec>,
    market_calendar_id: String,
    market_settlement_days: i32,
    market_business_day_convention: BusinessDayConvention,
    default_fixed_leg_day_count: DayCount,
    default_fixed_leg_frequency: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum OisCompoundingSpec {
    Sofr,
    Sonia,
    Estr,
    Tona,
    Fedfunds,
    Saron,
    CompoundedInArrears {
        lookback_days: i32,
        observation_shift: Option<i32>,
    },
}

impl OisCompoundingSpec {
    fn to_compounding(&self) -> FloatingLegCompounding {
        match self {
            Self::Sofr => FloatingLegCompounding::sofr(),
            Self::Sonia => FloatingLegCompounding::sonia(),
            Self::Estr => FloatingLegCompounding::estr(),
            Self::Tona => FloatingLegCompounding::tona(),
            Self::Fedfunds => FloatingLegCompounding::fedfunds(),
            Self::Saron => FloatingLegCompounding::saron(),
            Self::CompoundedInArrears {
                lookback_days,
                observation_shift,
            } => FloatingLegCompounding::CompoundedInArrears {
                lookback_days: *lookback_days,
                observation_shift: *observation_shift,
            },
        }
    }
}

impl RateIndexConventionsRecord {
    fn into_conventions(self) -> Result<RateIndexConventions, Error> {
        let tenor = match self.tenor {
            Some(s) => Some(Tenor::parse(&s).map_err(|e| {
                Error::Validation(format!(
                    "Invalid `tenor` in rate index conventions registry: '{}': {}",
                    s, e
                ))
            })?),
            None => None,
        };

        let default_payment_frequency = Tenor::parse(&self.default_payment_frequency).map_err(
            |e| {
                Error::Validation(format!(
                    "Invalid `default_payment_frequency` in rate index conventions registry: '{}': {}",
                    self.default_payment_frequency, e
                ))
            },
        )?;

        let default_fixed_leg_frequency =
            Tenor::parse(&self.default_fixed_leg_frequency).map_err(|e| {
                Error::Validation(format!(
                    "Invalid `default_fixed_leg_frequency` in rate index conventions registry: '{}': {}",
                    self.default_fixed_leg_frequency, e
                ))
            })?;

        let ois_compounding = self.ois_compounding.as_ref().map(|s| s.to_compounding());

        // Basic invariants
        match self.kind {
            RateIndexKind::OvernightRfr => {
                if tenor.is_some() {
                    return Err(Error::Validation(
                        "Overnight RFR index conventions must not specify a tenor".to_string(),
                    ));
                }
                if ois_compounding.is_none() {
                    return Err(Error::Validation(
                        "Overnight RFR index conventions must specify `ois_compounding`"
                            .to_string(),
                    ));
                }
            }
            RateIndexKind::Term => {
                if ois_compounding.is_some() {
                    return Err(Error::Validation(
                        "Term index conventions must not specify `ois_compounding`".to_string(),
                    ));
                }
            }
        }

        // Reset lag validation: must be non-negative (T-minus semantics: positive value means
        // fixing date = accrual_start - reset_lag_days business days).
        if self.default_reset_lag_days < 0 {
            return Err(Error::Validation(format!(
                "default_reset_lag_days must be non-negative (got {}); use positive T-minus semantics",
                self.default_reset_lag_days
            )));
        }
        if self.default_reset_lag_days > 31 {
            return Err(Error::Validation(format!(
                "default_reset_lag_days exceeds reasonable limit of 31 (got {})",
                self.default_reset_lag_days
            )));
        }

        Ok(RateIndexConventions {
            currency: self.currency,
            kind: self.kind,
            tenor,
            day_count: self.day_count,
            default_payment_frequency,
            default_payment_lag_days: self.default_payment_lag_days,
            default_reset_lag_days: self.default_reset_lag_days,
            ois_compounding,
            market_calendar_id: self.market_calendar_id,
            market_settlement_days: self.market_settlement_days,
            market_business_day_convention: self.market_business_day_convention,
            default_fixed_leg_day_count: self.default_fixed_leg_day_count,
            default_fixed_leg_frequency,
        })
    }
}

/// Load the rate index conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<HashMap<IndexId, RateIndexConventions>, Error> {
    let json = include_str!("../../../../data/conventions/rate_index_conventions.json");
    super::json::parse_and_rekey(
        json,
        "rate index",
        IndexId::new,
        |rec: &RateIndexConventionsRecord| rec.clone().into_conventions(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chf_saron_uses_true_observation_shift() {
        let registry = load_registry().expect("rate index registry");
        let chf_saron = registry
            .get(&IndexId::new("CHF-SARON-OIS"))
            .expect("CHF-SARON-OIS conventions");

        assert_eq!(
            chf_saron.ois_compounding,
            Some(FloatingLegCompounding::saron())
        );
    }

    #[test]
    fn usd_sofr_alias_resolves_to_sofr_conventions() {
        let registry = load_registry().expect("rate index registry");
        let sofr = registry
            .get(&IndexId::new("USD-SOFR"))
            .expect("USD-SOFR conventions");

        assert_eq!(sofr.ois_compounding, Some(FloatingLegCompounding::sofr()));
    }

    #[test]
    fn eur_euribor_six_month_index_is_available_for_swaption_conventions() {
        let registry = load_registry().expect("rate index registry");
        let euribor = registry
            .get(&IndexId::new("EUR-EURIBOR-6M"))
            .expect("EUR-EURIBOR-6M conventions");

        assert_eq!(euribor.currency, Currency::EUR);
        assert_eq!(euribor.kind, RateIndexKind::Term);
        assert_eq!(
            euribor.tenor,
            Some(Tenor::parse("6M").expect("valid tenor"))
        );
    }
}
