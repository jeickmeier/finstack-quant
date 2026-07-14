//! Serializable rate-curve calibration replay conventions.

use crate::currency::Currency;
use crate::dates::{Date, DayCount, Tenor};
use crate::types::{CurveId, IndexId};

/// Serialized identifier for an interest-rate futures convention.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct RateCalibrationFutureContractId(String);

impl RateCalibrationFutureContractId {
    /// Create an identifier from its canonical convention-registry key.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the canonical convention-registry key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Numerical method used to calibrate a rate curve.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateCalibrationMethod {
    /// Sequential bootstrap.
    Bootstrap,
    /// Simultaneous solve of all curve parameters.
    GlobalSolve {
        /// Whether the original solve requested its specialized Jacobian.
        #[serde(default)]
        use_analytical_jacobian: bool,
    },
}

/// OIS floating-leg compounding convention used during calibration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateCalibrationOisCompounding {
    /// Simple term-rate accrual.
    Simple,
    /// Daily compounding in arrears, optionally with a lookback or observation shift.
    CompoundedInArrears {
        /// Business-day lookback applied to rate observations.
        lookback_days: i32,
        /// Optional business-day observation shift.
        observation_shift: Option<i32>,
    },
    /// Daily compounding with an ISDA observation shift.
    CompoundedWithObservationShift {
        /// Business days by which observations and accrual weights are shifted.
        shift_days: i32,
    },
    /// Daily compounding with the final observed rate held through a cutoff window.
    CompoundedWithRateCutoff {
        /// Number of business days in the rate-cutoff window.
        cutoff_days: i32,
    },
}

/// Role of a curve and its linked rate-curve identifier during calibration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateCalibrationCurveRole {
    /// Discount curve, linked to the projection curve used to price its instruments.
    Discount {
        /// Projection curve identifier used by the calibration instruments.
        projection_curve_id: CurveId,
    },
    /// Projection curve, linked to the discount curve used to price its instruments.
    Projection {
        /// Discount curve identifier used by the calibration instruments.
        discount_curve_id: CurveId,
    },
}

/// Typed maturity specification retained from an original market quote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateCalibrationPillar {
    /// Relative tenor resolved from the calibration base date.
    Tenor(Tenor),
    /// Absolute calendar date.
    Date(Date),
}

/// Lossless rate quote representation used by calibration replay.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateCalibrationQuote {
    /// Money-market deposit quote.
    Deposit {
        /// Referenced rate index.
        index_id: IndexId,
        /// Relative-tenor or absolute-date pillar.
        pillar: RateCalibrationPillar,
        /// Quoted deposit rate.
        rate: f64,
    },
    /// Forward-rate agreement quote.
    Fra {
        /// Referenced rate index.
        index_id: IndexId,
        /// FRA start pillar.
        start: RateCalibrationPillar,
        /// FRA end pillar.
        end: RateCalibrationPillar,
        /// Quoted FRA rate.
        rate: f64,
    },
    /// Interest-rate futures quote.
    Futures {
        /// Convention-registry identifier for the futures contract.
        contract: RateCalibrationFutureContractId,
        /// Futures expiry date.
        expiry: Date,
        /// Quoted futures price.
        price: f64,
        /// Optional pre-computed convexity adjustment.
        convexity_adjustment: Option<f64>,
        /// Optional volatility surface used for the adjustment.
        vol_surface_id: Option<CurveId>,
    },
    /// Interest-rate swap quote.
    Swap {
        /// Referenced floating-rate index.
        index_id: IndexId,
        /// Relative-tenor or absolute-date maturity pillar.
        pillar: RateCalibrationPillar,
        /// Quoted fixed rate.
        rate: f64,
        /// Optional floating-leg spread.
        spread_decimal: Option<f64>,
    },
}

/// Typed conventions required to replay a rate-curve calibration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateCalibrationRecipe {
    /// Currency of the calibrated curve.
    ///
    /// Optional only for backward compatibility with early serialized recipes.
    #[serde(default)]
    pub currency: Option<Currency>,
    /// Numerical calibration method.
    pub method: RateCalibrationMethod,
    /// Day count used for the curve's time axis.
    pub curve_day_count: DayCount,
    /// Optional OIS floating-leg compounding override.
    pub ois_compounding: Option<RateCalibrationOisCompounding>,
    /// Discount/projection role and linked curve identifier.
    pub role: RateCalibrationCurveRole,
    /// Complete typed quote set required for exact replay.
    ///
    /// Defaults empty for recipes serialized before quote replay became lossless.
    #[serde(default)]
    pub quotes: Vec<RateCalibrationQuote>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_rate_quotes_round_trip_pillars_and_swap_spread() {
        let json = serde_json::json!({
            "method": "bootstrap",
            "curve_day_count": "Act365F",
            "ois_compounding": null,
            "role": {
                "discount": {
                    "projection_curve_id": "USD-OIS"
                }
            },
            "quotes": [
                {
                    "deposit": {
                        "index_id": "USD-SOFR-OIS",
                        "pillar": { "date": "2025-02-03" },
                        "rate": 0.043
                    }
                },
                {
                    "fra": {
                        "index_id": "USD-SOFR-3M",
                        "start": {
                            "tenor": {
                                "count": 3,
                                "unit": "months"
                            }
                        },
                        "end": { "date": "2025-07-02" },
                        "rate": 0.041
                    }
                },
                {
                    "futures": {
                        "contract": "CME:SR3",
                        "expiry": "2025-09-17",
                        "price": 95.75,
                        "convexity_adjustment": 0.0001,
                        "vol_surface_id": "USD-SR3-VOL"
                    }
                },
                {
                    "swap": {
                        "index_id": "USD-SOFR-OIS",
                        "pillar": {
                            "tenor": {
                                "count": 5,
                                "unit": "years"
                            }
                        },
                        "rate": 0.039,
                        "spread_decimal": 0.00025
                    }
                }
            ]
        });

        let recipe: RateCalibrationRecipe =
            serde_json::from_value(json).expect("mixed typed rate recipe");
        let serialized = serde_json::to_value(recipe).expect("serialize mixed typed rate recipe");
        let _restored: RateCalibrationRecipe =
            serde_json::from_value(serialized.clone()).expect("round-trip mixed typed rate recipe");

        assert_eq!(
            serialized["quotes"][0]["deposit"]["pillar"]["date"],
            "2025-02-03"
        );
        assert_eq!(serialized["quotes"][1]["fra"]["start"]["tenor"]["count"], 3);
        assert_eq!(serialized["quotes"][3]["swap"]["spread_decimal"], 0.00025);
    }
}
