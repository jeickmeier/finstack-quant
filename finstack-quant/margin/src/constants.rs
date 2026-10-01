//! Constants for margin calculations.
//!
//! Financial constants used across margin computation modules,
//! following ISDA SIMM and industry standard conventions.

/// Standard days per year for year fraction calculations (ACT/365 Fixed).
///
/// Re-exported from `finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR`.
pub use finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR;

/// Duration approximation factor.
///
/// Approximates modified duration as `years_to_maturity * DURATION_FACTOR`
/// assuming reasonable yield levels (2-5% range).
pub const DURATION_APPROXIMATION_FACTOR: f64 = 0.9;

/// One basis point (0.01%).
///
/// Used for DV01 and CS01 calculations.
pub const ONE_BP: f64 = 0.0001;

/// Standard CDS maturity for SIMM bucketing.
///
/// 5Y is the most liquid CDS tenor and standard for SIMM sensitivity assignment.
pub const STANDARD_CDS_MATURITY_YEARS: f64 = 5.0;

/// SIMM tenor bucket boundaries in years.
pub mod tenor_buckets {
    /// 3 month bucket threshold (short-dated).
    pub const BUCKET_3M: f64 = 0.25;
    /// 6 month bucket threshold.
    pub const BUCKET_6M: f64 = 0.5;
    /// 1 year bucket threshold.
    pub const BUCKET_1Y: f64 = 1.0;
    /// 2 year bucket threshold.
    pub const BUCKET_2Y: f64 = 2.0;
    /// 3 year bucket threshold.
    pub const BUCKET_3Y: f64 = 3.0;
    /// 5 year bucket threshold.
    pub const BUCKET_5Y: f64 = 5.0;
    /// 10 year bucket threshold.
    pub const BUCKET_10Y: f64 = 10.0;
    /// 15 year bucket threshold.
    pub const BUCKET_15Y: f64 = 15.0;
    /// 20 year bucket threshold.
    pub const BUCKET_20Y: f64 = 20.0;
}

/// The margin constants a host needs to interpret inputs and results, as one
/// serializable value.
///
/// Field names serialize in the spelling of the Rust constants they mirror.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarginConstants {
    /// [`CALENDAR_DAYS_PER_YEAR`]: days per year for ACT/365 Fixed year fractions.
    #[serde(rename = "CALENDAR_DAYS_PER_YEAR")]
    pub calendar_days_per_year: f64,
    /// [`DURATION_APPROXIMATION_FACTOR`]: modified duration per year to maturity.
    #[serde(rename = "DURATION_APPROXIMATION_FACTOR")]
    pub duration_approximation_factor: f64,
    /// [`ONE_BP`]: one basis point as a decimal.
    #[serde(rename = "ONE_BP")]
    pub one_bp: f64,
    /// [`STANDARD_CDS_MATURITY_YEARS`]: CDS tenor used for SIMM bucketing, in years.
    #[serde(rename = "STANDARD_CDS_MATURITY_YEARS")]
    pub standard_cds_maturity_years: f64,
    /// SIMM tenor bucket boundaries in years, keyed by the [`tenor_buckets`] constant names.
    pub tenor_buckets: TenorBucketYears,
    /// Registry id of the BCBS-IOSCO regulatory IM schedule.
    #[serde(rename = "BCBS_IOSCO_SCHEDULE_ID")]
    pub bcbs_iosco_schedule_id: String,
    /// Margin period of risk, in business days, stamped on haircut-based IM results.
    #[serde(rename = "HAIRCUT_MPOR_DAYS")]
    pub haircut_mpor_days: u32,
    /// SIMM tenor bucket labels accepted by the sensitivity containers, shortest first.
    #[serde(rename = "SIMM_TENORS")]
    pub simm_tenors: Vec<String>,
    /// Number of SIMM commodity buckets.
    #[serde(rename = "SIMM_COMMODITY_BUCKET_COUNT")]
    pub simm_commodity_bucket_count: u8,
}

/// SIMM tenor bucket boundaries in years (the [`tenor_buckets`] constants).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, rename_all = "SCREAMING_SNAKE_CASE")]
pub struct TenorBucketYears {
    /// 3 month bucket threshold.
    pub bucket_3m: f64,
    /// 6 month bucket threshold.
    pub bucket_6m: f64,
    /// 1 year bucket threshold.
    pub bucket_1y: f64,
    /// 2 year bucket threshold.
    pub bucket_2y: f64,
    /// 3 year bucket threshold.
    pub bucket_3y: f64,
    /// 5 year bucket threshold.
    pub bucket_5y: f64,
    /// 10 year bucket threshold.
    pub bucket_10y: f64,
    /// 15 year bucket threshold.
    pub bucket_15y: f64,
    /// 20 year bucket threshold.
    pub bucket_20y: f64,
}

impl MarginConstants {
    /// The constants compiled into this build.
    #[must_use]
    pub fn current() -> Self {
        Self {
            calendar_days_per_year: CALENDAR_DAYS_PER_YEAR,
            duration_approximation_factor: DURATION_APPROXIMATION_FACTOR,
            one_bp: ONE_BP,
            standard_cds_maturity_years: STANDARD_CDS_MATURITY_YEARS,
            tenor_buckets: TenorBucketYears {
                bucket_3m: tenor_buckets::BUCKET_3M,
                bucket_6m: tenor_buckets::BUCKET_6M,
                bucket_1y: tenor_buckets::BUCKET_1Y,
                bucket_2y: tenor_buckets::BUCKET_2Y,
                bucket_3y: tenor_buckets::BUCKET_3Y,
                bucket_5y: tenor_buckets::BUCKET_5Y,
                bucket_10y: tenor_buckets::BUCKET_10Y,
                bucket_15y: tenor_buckets::BUCKET_15Y,
                bucket_20y: tenor_buckets::BUCKET_20Y,
            },
            bcbs_iosco_schedule_id: crate::BCBS_IOSCO_SCHEDULE_ID.to_string(),
            haircut_mpor_days: crate::calculators::im::HAIRCUT_MPOR_DAYS,
            simm_tenors: crate::SIMM_TENORS.iter().map(ToString::to_string).collect(),
            simm_commodity_bucket_count: crate::types::SIMM_COMMODITY_BUCKET_COUNT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_serializes_under_the_rust_constant_names() {
        let value = serde_json::to_value(MarginConstants::current()).expect("serialize");
        assert_eq!(value["CALENDAR_DAYS_PER_YEAR"], 365.0);
        assert_eq!(value["ONE_BP"], 0.0001);
        assert_eq!(value["tenor_buckets"]["BUCKET_3M"], 0.25);
        assert_eq!(value["tenor_buckets"]["BUCKET_20Y"], 20.0);
        assert_eq!(value["HAIRCUT_MPOR_DAYS"], 2);
        assert_eq!(value["SIMM_TENORS"][0], "2W");
        assert_eq!(value["SIMM_COMMODITY_BUCKET_COUNT"], 17);
        assert_eq!(
            serde_json::from_value::<MarginConstants>(value).expect("round trip"),
            MarginConstants::current()
        );
    }
}
