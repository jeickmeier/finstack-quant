//! SA-CCR types and data structures.
//!
//! Defines the asset class taxonomy, trade representation, netting set
//! configuration, and EAD result per BCBS 279.

use crate::types::NettingSetId;
use finstack_quant_core::dates::Date;
use finstack_quant_core::HashMap;

/// SA-CCR asset class for add-on computation.
///
/// Each derivative trade is assigned to exactly one asset class.
/// The add-on formula and supervisory parameters differ by class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SaCcrAssetClass {
    /// Interest rate derivatives.
    InterestRate,
    /// Foreign exchange derivatives.
    ForeignExchange,
    /// Credit derivatives.
    Credit,
    /// Equity derivatives.
    Equity,
    /// Commodity derivatives.
    Commodity,
}

impl SaCcrAssetClass {
    /// All asset classes in canonical order.
    pub const ALL: &'static [SaCcrAssetClass] = &[
        SaCcrAssetClass::InterestRate,
        SaCcrAssetClass::ForeignExchange,
        SaCcrAssetClass::Credit,
        SaCcrAssetClass::Equity,
        SaCcrAssetClass::Commodity,
    ];
}

impl std::fmt::Display for SaCcrAssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InterestRate => write!(f, "Interest Rate"),
            Self::ForeignExchange => write!(f, "Foreign Exchange"),
            Self::Credit => write!(f, "Credit"),
            Self::Equity => write!(f, "Equity"),
            Self::Commodity => write!(f, "Commodity"),
        }
    }
}

/// SA-CCR option type for delta computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SaCcrOptionType {
    /// Long call option.
    CallLong,
    /// Short call option.
    CallShort,
    /// Long put option.
    PutLong,
    /// Short put option.
    PutShort,
}

/// A single derivative trade for SA-CCR EAD computation.
///
/// Captures the trade-level attributes required by the SA-CCR formula:
/// notional, maturity dates, direction, underlier, and option characteristics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaCcrTrade {
    /// Unique trade identifier.
    pub trade_id: String,
    /// Asset class assignment.
    pub asset_class: SaCcrAssetClass,
    /// Adjusted notional in reporting currency.
    pub notional: f64,
    /// Trade start date (for maturity factor computation).
    pub start_date: Date,
    /// Trade end date / maturity.
    pub end_date: Date,
    /// Underlier reference (e.g., currency pair, issuer, equity name, commodity).
    pub underlier: String,
    /// Hedging set identifier within the asset class.
    /// Trades with the same hedging set can partially offset.
    pub hedging_set: String,
    /// Long (+1.0) or short (-1.0) direction.
    pub direction: f64,
    /// Supervisory delta adjustment.
    /// For linear trades: +1 (long) or -1 (short).
    /// For options: delta from Black-Scholes or equivalent.
    pub supervisory_delta: f64,
    /// Current mark-to-market value.
    pub mtm: f64,
    /// Whether this trade is an option.
    pub is_option: bool,
    /// Option exercise type if applicable.
    pub option_type: Option<SaCcrOptionType>,
}

/// Netting set configuration for SA-CCR.
///
/// Captures the collateral terms that determine whether the margined
/// or unmargined RC/PFE formulas apply.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaCcrNettingSetConfig {
    /// Valuation date used for forward-start and remaining-maturity calculations.
    pub as_of: Date,
    /// Netting set identifier.
    pub netting_set_id: NettingSetId,
    /// Whether the netting set is subject to a margin agreement.
    pub is_margined: bool,
    /// Net current collateral held (positive = bank holds collateral).
    pub collateral: f64,
    /// Threshold amount (TH) under the margin agreement.
    pub threshold: f64,
    /// Minimum transfer amount (MTA).
    pub mta: f64,
    /// Net independent collateral amount (NICA).
    pub nica: f64,
    /// Margin period of risk in business days (default: 10 for bilateral).
    pub mpor_days: u32,
}

impl SaCcrNettingSetConfig {
    /// Validate collateral and margin-agreement inputs.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        for (name, value) in [
            ("collateral", self.collateral),
            ("threshold", self.threshold),
            ("mta", self.mta),
            ("nica", self.nica),
        ] {
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SA-CCR netting set '{}': {name} must be finite, got {value}",
                    self.netting_set_id
                )));
            }
        }
        if self.threshold < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SA-CCR netting set '{}': threshold must be non-negative",
                self.netting_set_id
            )));
        }
        if self.mta < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SA-CCR netting set '{}': MTA must be non-negative",
                self.netting_set_id
            )));
        }
        if self.is_margined && self.mpor_days == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SA-CCR netting set '{}': margined MPOR must be positive",
                self.netting_set_id
            )));
        }
        Ok(())
    }

    /// Create an unmargined netting set configuration.
    #[must_use]
    pub fn unmargined(netting_set_id: NettingSetId, collateral: f64, as_of: Date) -> Self {
        Self {
            as_of,
            netting_set_id,
            is_margined: false,
            collateral,
            threshold: 0.0,
            mta: 0.0,
            nica: 0.0,
            mpor_days: 10,
        }
    }

    /// Create a margined netting set configuration.
    #[must_use]
    pub fn margined(
        netting_set_id: NettingSetId,
        collateral: f64,
        threshold: f64,
        mta: f64,
        nica: f64,
        mpor_days: u32,
        as_of: Date,
    ) -> Self {
        Self {
            as_of,
            netting_set_id,
            is_margined: true,
            collateral,
            threshold,
            mta,
            nica,
            mpor_days,
        }
    }
}

/// SA-CCR Exposure at Default result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EadResult {
    /// Exposure at Default: `alpha * (RC + PFE)`.
    pub ead: f64,
    /// Replacement cost component.
    pub rc: f64,
    /// Potential future exposure component.
    pub pfe: f64,
    /// PFE multiplier (accounts for over-collateralization).
    pub multiplier: f64,
    /// Aggregate add-on before multiplier.
    pub add_on_aggregate: f64,
    /// Add-on by asset class.
    pub add_on_by_asset_class: HashMap<SaCcrAssetClass, f64>,
    /// Alpha multiplier (1.4 per regulation).
    pub alpha: f64,
    /// Maturity factor applied.
    pub maturity_factor: f64,
}

fn validate_finite_trade_value(
    id: &str,
    field: &str,
    value: f64,
) -> finstack_quant_core::Result<()> {
    if value.is_finite() {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "SA-CCR trade {id}: {field} must be finite (got {value})"
    )))
}

impl SaCcrTrade {
    /// Validate supervisory-delta / direction / option-type coherence.
    ///
    /// The add-on path [`super::add_on::asset_class_add_on`] uses
    /// `supervisory_delta * |notional|` as the adjusted notional — the
    /// sign of the entire contribution comes from `supervisory_delta`.
    /// If a caller misconfigures `supervisory_delta` (e.g. passes +1
    /// for a short linear trade, or the wrong-signed Black-Scholes
    /// delta for a put) the add-on would silently compute a reversed-
    /// direction contribution, producing a 10–20% IM miss on options
    /// without an obvious failure signal; this validator catches that
    /// class of caller bug up front.
    ///
    /// Checks performed per BCBS 279 ¶104–112:
    ///
    /// 1. All numeric fields (`notional`, `direction`, `supervisory_delta`,
    ///    `mtm`) must be finite.
    /// 2. `direction` must be strictly nonzero (treated as long > 0 / short < 0).
    /// 3. `supervisory_delta` ∈ [-1, +1].
    /// 4. **Linear trades** (`is_option = false`): `supervisory_delta` must be
    ///    ±1 (within 1e-6 tolerance) AND `sign(supervisory_delta) ==
    ///    sign(direction)` per BCBS 279 ¶111.
    /// 5. **Options**: `supervisory_delta` must be nonzero and sign-consistent
    ///    with `option_type` per BCBS 279 ¶112:
    ///       * `CallLong` / `PutShort` → positive
    ///       * `CallShort` / `PutLong` → negative
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] with a message naming the
    /// trade id and the specific invariant that failed.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let id = self.trade_id.as_str();

        validate_finite_trade_value(id, "notional", self.notional)?;
        validate_finite_trade_value(id, "direction", self.direction)?;
        validate_finite_trade_value(id, "supervisory_delta", self.supervisory_delta)?;
        validate_finite_trade_value(id, "mtm", self.mtm)?;

        if self.direction == 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SA-CCR trade {id}: direction must be nonzero (long > 0, short < 0) per BCBS 279 ¶111"
            )));
        }

        if !(-1.0..=1.0).contains(&self.supervisory_delta) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SA-CCR trade {id}: supervisory_delta {d} outside [-1, +1] (BCBS 279 ¶111–112)",
                d = self.supervisory_delta
            )));
        }

        if self.is_option {
            let opt = self.option_type.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "SA-CCR trade {id}: is_option = true requires option_type"
                ))
            })?;
            if self.supervisory_delta == 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SA-CCR trade {id}: option supervisory_delta must be nonzero"
                )));
            }
            // Sign convention per BCBS 279 ¶112:
            //   CallLong = +N(d1)   PutLong  = −N(−d1)
            //   CallShort = −N(d1)  PutShort = +N(−d1)
            let expected_positive =
                matches!(opt, SaCcrOptionType::CallLong | SaCcrOptionType::PutShort);
            let is_positive = self.supervisory_delta > 0.0;
            if expected_positive != is_positive {
                let opt_name = match opt {
                    SaCcrOptionType::CallLong => "CallLong (δ > 0)",
                    SaCcrOptionType::CallShort => "CallShort (δ < 0)",
                    SaCcrOptionType::PutLong => "PutLong (δ < 0)",
                    SaCcrOptionType::PutShort => "PutShort (δ > 0)",
                };
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SA-CCR trade {id}: supervisory_delta {d} sign mismatches option_type {opt_name} per BCBS 279 ¶112",
                    d = self.supervisory_delta
                )));
            }
        } else {
            // Linear: δ must be ±1 (exact per BCBS 279 ¶111) and same sign as direction.
            if (self.supervisory_delta.abs() - 1.0).abs() > 1e-6 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SA-CCR trade {id}: linear trade supervisory_delta must be ±1 per BCBS 279 ¶111 (got {d})",
                    d = self.supervisory_delta
                )));
            }
            if self.supervisory_delta * self.direction <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SA-CCR trade {id}: supervisory_delta ({d}) and direction ({dir}) must agree in sign per BCBS 279 ¶111",
                    d = self.supervisory_delta,
                    dir = self.direction
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;
    use finstack_quant_core::dates::Date;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).expect("valid"), day).expect("valid")
    }

    /// Base linear IR trade: long 100M 5Y, direction and supervisory_delta both +1.
    fn linear_long() -> SaCcrTrade {
        SaCcrTrade {
            trade_id: "LIN-LONG".into(),
            asset_class: SaCcrAssetClass::InterestRate,
            notional: 100_000_000.0,
            start_date: d(2025, 1, 15),
            end_date: d(2030, 1, 15),
            underlier: "USD".into(),
            hedging_set: "USD-IR".into(),
            direction: 1.0,
            supervisory_delta: 1.0,
            mtm: 0.0,
            is_option: false,
            option_type: None,
        }
    }

    fn option(opt: SaCcrOptionType, direction: f64, delta: f64) -> SaCcrTrade {
        SaCcrTrade {
            trade_id: "OPT".into(),
            asset_class: SaCcrAssetClass::Equity,
            notional: 10_000_000.0,
            start_date: d(2025, 1, 15),
            end_date: d(2026, 1, 15),
            underlier: "SPX".into(),
            hedging_set: "SPX".into(),
            direction,
            supervisory_delta: delta,
            mtm: 0.0,
            is_option: true,
            option_type: Some(opt),
        }
    }

    // ---- Positive cases ----

    #[test]
    fn accepts_linear_long() {
        linear_long().validate().expect("linear long valid");
    }

    #[test]
    fn accepts_linear_short() {
        let mut t = linear_long();
        t.direction = -1.0;
        t.supervisory_delta = -1.0;
        t.validate().expect("linear short valid");
    }

    #[test]
    fn accepts_call_long_positive_delta() {
        option(SaCcrOptionType::CallLong, 1.0, 0.6)
            .validate()
            .expect("call long δ > 0 valid");
    }

    #[test]
    fn accepts_call_short_negative_delta() {
        option(SaCcrOptionType::CallShort, -1.0, -0.6)
            .validate()
            .expect("call short δ < 0 valid");
    }

    #[test]
    fn accepts_put_long_negative_delta() {
        option(SaCcrOptionType::PutLong, 1.0, -0.4)
            .validate()
            .expect("put long δ < 0 valid per BCBS 279 ¶112");
    }

    #[test]
    fn accepts_put_short_positive_delta() {
        option(SaCcrOptionType::PutShort, -1.0, 0.4)
            .validate()
            .expect("put short δ > 0 valid per BCBS 279 ¶112");
    }

    // ---- Negative cases ----

    #[test]
    fn rejects_linear_sign_mismatch() {
        // Linear long direction with short supervisory_delta. Silent 100% sign
        // inversion of the add-on contribution without this check.
        let mut t = linear_long();
        t.supervisory_delta = -1.0;
        let err = t.validate().expect_err("sign mismatch must be rejected");
        assert!(
            err.to_string().contains("agree in sign"),
            "expected sign-mismatch message: {err}"
        );
    }

    #[test]
    fn rejects_linear_non_unit_delta() {
        // Linear trade with |δ| ≠ 1 — the ±1 rule is exact per BCBS 279 ¶111.
        let mut t = linear_long();
        t.supervisory_delta = 0.5;
        let err = t
            .validate()
            .expect_err("non-unit linear δ must be rejected");
        assert!(err.to_string().contains("±1"), "expected ±1 message: {err}");
    }

    #[test]
    fn rejects_delta_greater_than_one() {
        let mut t = linear_long();
        t.supervisory_delta = 1.5;
        let err = t.validate().expect_err("|δ| > 1 must be rejected");
        assert!(
            err.to_string().contains("[-1, +1]"),
            "expected range message: {err}"
        );
    }

    #[test]
    fn rejects_zero_direction() {
        let mut t = linear_long();
        t.direction = 0.0;
        let err = t.validate().expect_err("zero direction must be rejected");
        assert!(
            err.to_string().contains("nonzero"),
            "expected nonzero-direction message: {err}"
        );
    }

    #[test]
    fn rejects_non_finite_notional() {
        let mut t = linear_long();
        t.notional = f64::NAN;
        let err = t.validate().expect_err("NaN notional must be rejected");
        assert!(
            err.to_string().contains("notional"),
            "expected notional message: {err}"
        );
    }

    #[test]
    fn rejects_call_long_negative_delta() {
        // BCBS 279 ¶112: CallLong δ must be +N(d1) (positive).
        let t = option(SaCcrOptionType::CallLong, 1.0, -0.5);
        let err = t
            .validate()
            .expect_err("CallLong with δ<0 must be rejected");
        assert!(
            err.to_string().contains("CallLong"),
            "expected option-type message: {err}"
        );
    }

    #[test]
    fn rejects_put_long_positive_delta() {
        // BCBS 279 ¶112: PutLong δ must be -N(-d1) (negative).
        let t = option(SaCcrOptionType::PutLong, 1.0, 0.3);
        let err = t.validate().expect_err("PutLong with δ>0 must be rejected");
        assert!(
            err.to_string().contains("PutLong"),
            "expected option-type message: {err}"
        );
    }

    #[test]
    fn rejects_option_without_option_type() {
        let mut t = linear_long();
        t.is_option = true;
        t.option_type = None;
        let err = t
            .validate()
            .expect_err("is_option without option_type must be rejected");
        assert!(
            err.to_string().contains("option_type"),
            "expected option_type message: {err}"
        );
    }

    #[test]
    fn rejects_option_with_zero_delta() {
        let t = option(SaCcrOptionType::CallLong, 1.0, 0.0);
        let err = t.validate().expect_err("option with δ=0 must be rejected");
        assert!(
            err.to_string().contains("nonzero"),
            "expected nonzero message: {err}"
        );
    }

    #[test]
    fn strict_serde_rejects_unknown_trade_fields() {
        let mut value = serde_json::to_value(linear_long()).expect("serialize trade");
        value
            .as_object_mut()
            .expect("trade should serialize as object")
            .insert("supervisory_factor".to_string(), serde_json::json!(0.05));

        let err = serde_json::from_value::<SaCcrTrade>(value)
            .expect_err("unknown SA-CCR trade field must be rejected");
        assert!(
            err.to_string().contains("unknown field"),
            "expected unknown-field serde error: {err}"
        );
    }

    #[test]
    fn strict_serde_rejects_unknown_config_fields() {
        let config = SaCcrNettingSetConfig::unmargined(
            NettingSetId::bilateral("BANK_A", "CSA-001"),
            0.0,
            d(2025, 1, 15),
        );
        let mut value = serde_json::to_value(config).expect("serialize config");
        value
            .as_object_mut()
            .expect("config should serialize as object")
            .insert("collateral_typo".to_string(), serde_json::json!(1.0));

        let err = serde_json::from_value::<SaCcrNettingSetConfig>(value)
            .expect_err("unknown SA-CCR config field must be rejected");
        assert!(
            err.to_string().contains("unknown field"),
            "expected unknown-field serde error: {err}"
        );
    }
}
