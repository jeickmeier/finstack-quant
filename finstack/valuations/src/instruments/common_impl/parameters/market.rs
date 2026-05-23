//! Market parameter types for instrument pricing.

use finstack_core::dates::{Date, DayCount};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, Percentage, Rate};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

use serde::{Deserialize, Serialize};

/// Option type for pricing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
pub enum OptionType {
    /// Call option
    Call,
    /// Put option
    Put,
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionType::Call => write!(f, "call"),
            OptionType::Put => write!(f, "put"),
        }
    }
}

impl std::str::FromStr for OptionType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "call" | "buy" | "buy_protection" => Ok(OptionType::Call),
            "put" | "sell" | "sell_protection" => Ok(OptionType::Put),
            other => Err(format!("Unknown option type: {}", other)),
        }
    }
}

/// Exercise style for options
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStyle {
    /// European exercise (only at expiry)
    #[default]
    European,
    /// American exercise (any time before/at expiry)
    American,
    /// Bermudan exercise (specific dates before expiry)
    Bermudan,
}

impl std::fmt::Display for ExerciseStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExerciseStyle::European => write!(f, "european"),
            ExerciseStyle::American => write!(f, "american"),
            ExerciseStyle::Bermudan => write!(f, "bermudan"),
        }
    }
}

impl std::str::FromStr for ExerciseStyle {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "european" => Ok(ExerciseStyle::European),
            "american" => Ok(ExerciseStyle::American),
            "bermudan" => Ok(ExerciseStyle::Bermudan),
            other => Err(format!("Unknown exercise style: {}", other)),
        }
    }
}

/// Settlement type for options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettlementType {
    /// Physical delivery
    Physical,
    /// Cash settlement
    Cash,
}

/// Position direction for futures and forwards.
///
/// Indicates whether the holder is long (buyer) or short (seller) of the contract.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// Long position (buyer of futures/forward contract).
    ///
    /// Profits when the underlying price increases.
    #[default]
    Long,
    /// Short position (seller of futures/forward contract).
    ///
    /// Profits when the underlying price decreases.
    Short,
}

impl Position {
    /// Returns the sign multiplier for this position (+1.0 for Long, -1.0 for Short).
    #[inline]
    pub fn sign(&self) -> f64 {
        match self {
            Position::Long => 1.0,
            Position::Short => -1.0,
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Long => write!(f, "long"),
            Position::Short => write!(f, "short"),
        }
    }
}

impl std::str::FromStr for Position {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "long" | "buy" | "buyer" => Ok(Position::Long),
            "short" | "sell" | "seller" => Ok(Position::Short),
            other => Err(format!("Unknown position: {}", other)),
        }
    }
}

impl std::fmt::Display for SettlementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementType::Physical => write!(f, "physical"),
            SettlementType::Cash => write!(f, "cash"),
        }
    }
}

impl std::str::FromStr for SettlementType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "physical" => Ok(SettlementType::Physical),
            "cash" => Ok(SettlementType::Cash),
            other => Err(format!("Unknown settlement type: {}", other)),
        }
    }
}

/// Market parameters for equity options
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EquityOptionParams {
    /// Option strike price
    pub strike: f64,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Option type (Call/Put)
    pub option_type: OptionType,
    /// Exercise style (European/American/Bermudan)
    pub exercise_style: ExerciseStyle,
    /// Settlement type (Cash/Physical)
    pub settlement: SettlementType,
    /// Contract notional
    pub notional: Money,
}

impl EquityOptionParams {
    /// Create new equity option parameters.
    ///
    /// Validation is provided separately by [`EquityOptionParams::validate`] so
    /// this constructor's signature stays infallible; callers that need the
    /// invariants enforced should call `validate` (instrument constructors do
    /// so before pricing).
    pub fn new(strike: f64, expiry: Date, option_type: OptionType, notional: Money) -> Self {
        Self {
            strike,
            expiry,
            option_type,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Physical,
            notional,
        }
    }

    /// Create European call parameters
    pub fn european_call(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Call, notional)
    }

    /// Create European put parameters
    pub fn european_put(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Put, notional)
    }

    /// Set exercise style
    #[must_use]
    pub fn with_exercise_style(mut self, style: ExerciseStyle) -> Self {
        self.exercise_style = style;
        self
    }

    /// Set settlement type
    #[must_use]
    pub fn with_settlement(mut self, settlement: SettlementType) -> Self {
        self.settlement = settlement;
        self
    }

    /// Validate the structural invariants of these option parameters.
    ///
    /// Enforces `strike > 0`: a non-positive strike makes the lognormal option
    /// payoff ill-defined.
    ///
    /// The `notional` is a [`Money`], whose constructor already rejects
    /// non-finite amounts, so no separate notional-finiteness check is needed
    /// here — the type guarantees it.
    ///
    /// The constructors ([`EquityOptionParams::new`] and friends) do not call
    /// this — the struct also has public fields and is built by serde — so
    /// call `validate` after construction to enforce the invariant.
    ///
    /// # Errors
    /// Returns an error stating the attempted value when the strike is not
    /// strictly positive.
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.strike,
            "EquityOptionParams.strike",
        )
    }
}

/// Market parameters for FX options
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FxOptionParams {
    /// Strike rate (FX rate)
    pub strike: f64,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Option type (Call/Put)
    pub option_type: OptionType,
    /// Exercise style (European/American/Bermudan)
    pub exercise_style: ExerciseStyle,
    /// Settlement type (Cash/Physical)
    pub settlement: SettlementType,
    /// Notional amount
    pub notional: Money,
}

impl FxOptionParams {
    /// Create new FX option parameters.
    ///
    /// Validation is provided separately by [`FxOptionParams::validate`] so
    /// this constructor's signature stays infallible; callers that need the
    /// invariants enforced should call `validate`.
    pub fn new(strike: f64, expiry: Date, option_type: OptionType, notional: Money) -> Self {
        Self {
            strike,
            expiry,
            option_type,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Physical,
            notional,
        }
    }

    /// Create European call option parameters
    pub fn european_call(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Call, notional)
    }

    /// Create European put option parameters
    pub fn european_put(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Put, notional)
    }

    /// Set exercise style
    #[must_use]
    pub fn with_exercise_style(mut self, style: ExerciseStyle) -> Self {
        self.exercise_style = style;
        self
    }

    /// Set settlement type
    #[must_use]
    pub fn with_settlement(mut self, settlement: SettlementType) -> Self {
        self.settlement = settlement;
        self
    }

    /// Validate the structural invariants of these FX option parameters.
    ///
    /// Enforces `strike > 0`: the strike is an FX rate, which is strictly
    /// positive.
    ///
    /// The `notional` is a [`Money`], whose constructor already rejects
    /// non-finite amounts, so no separate notional-finiteness check is needed
    /// here — the type guarantees it.
    ///
    /// The constructors ([`FxOptionParams::new`] and friends) do not call this
    /// — the struct also has public fields and is built by serde — so call
    /// `validate` after construction to enforce the invariant.
    ///
    /// # Errors
    /// Returns an error stating the attempted value when the strike is not
    /// strictly positive.
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.strike,
            "FxOptionParams.strike",
        )
    }
}

/// Credit parameters for CDS instruments
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditParams {
    /// Reference entity (issuer being protected)
    pub reference_entity: String,
    /// Recovery rate (0.0 to 1.0)
    pub recovery_rate: f64,
    /// Credit curve identifier
    pub credit_curve_id: CurveId,
}

impl CreditParams {
    /// Create new credit parameters.
    ///
    /// Recovery-rate validation is provided separately by
    /// [`CreditParams::validate`] so this constructor's signature stays
    /// infallible. Callers that need the `recovery_rate ∈ [0.0, 1.0)`
    /// invariant enforced — consistently with `ProtectionLegSpec` — should
    /// call `validate`.
    pub fn new(
        reference_entity: impl Into<String>,
        recovery_rate: f64,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            reference_entity: reference_entity.into(),
            recovery_rate,
            credit_curve_id: credit_curve_id.into(),
        }
    }

    /// Create new credit parameters using typed percentage recovery.
    pub fn new_pct(
        reference_entity: impl Into<String>,
        recovery_rate: Percentage,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            reference_entity: reference_entity.into(),
            recovery_rate: recovery_rate.as_decimal(),
            credit_curve_id: credit_curve_id.into(),
        }
    }

    /// Standard corporate credit with 40% recovery
    pub fn corporate_standard(
        reference_entity: impl Into<String>,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self::new(reference_entity, 0.40, credit_curve_id)
    }

    /// Sovereign credit with 30% recovery
    pub fn sovereign_standard(
        reference_entity: impl Into<String>,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self::new(reference_entity, 0.30, credit_curve_id)
    }

    /// Validate that the recovery rate is within valid bounds `[0.0, 1.0)`.
    ///
    /// Delegates to the shared internal recovery-rate validator — the same one
    /// used by `ProtectionLegSpec::new` — so credit instruments enforce a
    /// single, consistent recovery-rate invariant.
    ///
    /// `CreditParams::new` does not call this (the struct also has public
    /// fields and is built by serde), which is why the audit flagged
    /// `CreditParams` as inconsistent with `ProtectionLegSpec::new`. Call
    /// `validate` after construction to close that gap.
    ///
    /// # Errors
    /// Returns an error stating the attempted value and the required range when
    /// `recovery_rate` is not finite or lies outside `[0.0, 1.0)`.
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_recovery_rate(self.recovery_rate)
    }
}

/// Interest rate option parameters (caps/floors)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InterestRateOptionParams {
    /// Strike rate for the option
    pub strike: f64,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Option type (Cap/Floor)
    pub option_type: OptionType,
    /// Underlying rate tenor
    pub tenor: String,
    /// Day count convention
    pub day_count: DayCount,
    /// Notional amount
    pub notional: Money,
}

impl InterestRateOptionParams {
    /// Create new IR option parameters.
    ///
    /// Validation is provided separately by
    /// [`InterestRateOptionParams::validate`] so this constructor's signature
    /// stays infallible.
    pub fn new(
        strike: f64,
        expiry: Date,
        option_type: OptionType,
        tenor: impl Into<String>,
        notional: Money,
    ) -> Self {
        Self {
            strike,
            expiry,
            option_type,
            tenor: tenor.into(),
            day_count: DayCount::Act360,
            notional,
        }
    }

    /// Create new IR option parameters using a typed strike rate.
    pub fn new_rate(
        strike: Rate,
        expiry: Date,
        option_type: OptionType,
        tenor: impl Into<String>,
        notional: Money,
    ) -> Self {
        Self {
            strike: strike.as_decimal(),
            expiry,
            option_type,
            tenor: tenor.into(),
            day_count: DayCount::Act360,
            notional,
        }
    }

    /// Validate the structural invariants of these IR option parameters.
    ///
    /// Enforces a finite `strike`. The strike is a rate (cap/floor strike) and
    /// may legitimately be zero or negative in a negative-rate regime, so only
    /// finiteness is required — not positivity. A deserialized `f64` strike
    /// can be `NaN`/`inf`, so the check is meaningful.
    ///
    /// The `notional` is a [`Money`], whose constructor already rejects
    /// non-finite amounts, so no separate notional-finiteness check is needed
    /// here — the type guarantees it.
    ///
    /// The constructors do not call this — the struct also has public fields
    /// and is built by serde — so call `validate` after construction to
    /// enforce the invariant.
    ///
    /// # Errors
    /// Returns an error stating the attempted value when the strike is not
    /// finite.
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_f64_finite(
            self.strike,
            "InterestRateOptionParams.strike",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn enum_parsing_display_and_position_sign_cover_aliases() {
        assert_eq!(OptionType::Call.to_string(), "call");
        assert_eq!("buy".parse::<OptionType>(), Ok(OptionType::Call));
        assert_eq!("sell_protection".parse::<OptionType>(), Ok(OptionType::Put));
        assert!("weird".parse::<OptionType>().is_err());

        assert_eq!(ExerciseStyle::default(), ExerciseStyle::European);
        assert_eq!(
            "american".parse::<ExerciseStyle>(),
            Ok(ExerciseStyle::American)
        );
        assert_eq!(ExerciseStyle::Bermudan.to_string(), "bermudan");
        assert!("odd".parse::<ExerciseStyle>().is_err());

        assert_eq!(SettlementType::Cash.to_string(), "cash");
        assert_eq!(
            "physical".parse::<SettlementType>(),
            Ok(SettlementType::Physical)
        );
        assert!("gross".parse::<SettlementType>().is_err());

        assert_eq!(Position::default(), Position::Long);
        assert_eq!(Position::Long.sign(), 1.0);
        assert_eq!(Position::Short.sign(), -1.0);
        assert_eq!("buyer".parse::<Position>(), Ok(Position::Long));
        assert_eq!("sell".parse::<Position>(), Ok(Position::Short));
        assert!("flat".parse::<Position>().is_err());
    }

    #[test]
    fn equity_and_fx_option_builders_apply_defaults_and_overrides() {
        let expiry = date!(2026 - 06 - 15);
        let notional = Money::new(1_000_000.0, Currency::USD);

        let equity = EquityOptionParams::european_call(100.0, expiry, notional)
            .with_exercise_style(ExerciseStyle::American)
            .with_settlement(SettlementType::Cash);
        assert_eq!(equity.option_type, OptionType::Call);
        assert_eq!(equity.exercise_style, ExerciseStyle::American);
        assert_eq!(equity.settlement, SettlementType::Cash);
        assert!(equity.validate().is_ok());

        let fx = FxOptionParams::european_put(1.12, expiry, notional)
            .with_exercise_style(ExerciseStyle::Bermudan)
            .with_settlement(SettlementType::Physical);
        assert_eq!(fx.option_type, OptionType::Put);
        assert_eq!(fx.exercise_style, ExerciseStyle::Bermudan);
        assert_eq!(fx.settlement, SettlementType::Physical);
        assert!(fx.validate().is_ok());
    }

    #[test]
    fn credit_and_ir_option_typed_constructors_preserve_typed_inputs() {
        let credit = CreditParams::new_pct("ACME", Percentage::new(35.0), "ACME-CDS");
        assert_eq!(credit.reference_entity, "ACME");
        assert!((credit.recovery_rate - 0.35).abs() < 1e-12);
        assert_eq!(credit.credit_curve_id.as_str(), "ACME-CDS");

        let corp = CreditParams::corporate_standard("CORP", "CORP-CDS");
        let sov = CreditParams::sovereign_standard("UST", "UST-CDS");
        assert!((corp.recovery_rate - 0.40).abs() < 1e-12);
        assert!((sov.recovery_rate - 0.30).abs() < 1e-12);

        let ir = InterestRateOptionParams::new_rate(
            Rate::from_bps(325),
            date!(2027 - 01 - 01),
            OptionType::Put,
            "6M",
            Money::new(5_000_000.0, Currency::USD),
        );
        assert!((ir.strike - 0.0325).abs() < 1e-12);
        assert_eq!(ir.option_type, OptionType::Put);
        assert_eq!(ir.tenor, "6M");
        assert_eq!(ir.day_count, DayCount::Act360);
    }

    #[test]
    fn equity_option_validate_rejects_non_positive_strike() {
        // Failure mode: a non-positive strike makes the lognormal option payoff
        // ill-defined; it was previously unvalidated.
        let expiry = date!(2026 - 06 - 15);
        let notional = Money::new(1_000_000.0, Currency::USD);

        let zero = EquityOptionParams::new(0.0, expiry, OptionType::Call, notional);
        let err = zero
            .validate()
            .expect_err("strike 0.0 must be rejected by validate");
        assert!(
            err.to_string().contains("strike"),
            "error should name the strike: {err}"
        );
        assert!(
            EquityOptionParams::new(-10.0, expiry, OptionType::Call, notional)
                .validate()
                .is_err()
        );
        assert!(EquityOptionParams::european_put(-1.0, expiry, notional)
            .validate()
            .is_err());
        // A well-formed equity option passes validation.
        assert!(
            EquityOptionParams::new(100.0, expiry, OptionType::Call, notional)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn option_notional_finiteness_is_guaranteed_by_money_type() {
        // The `notional` field is a `Money`; `Money::new` itself rejects
        // non-finite amounts, so an `EquityOptionParams`/`FxOptionParams`
        // notional cannot be NaN/inf and needs no separate check in
        // `validate`. A finite (incl. zero/negative) notional is accepted.
        let expiry = date!(2026 - 06 - 15);
        let zero_notional = Money::new(0.0, Currency::USD);
        assert!(
            EquityOptionParams::new(100.0, expiry, OptionType::Call, zero_notional)
                .validate()
                .is_ok()
        );
        assert!(
            FxOptionParams::new(1.10, expiry, OptionType::Put, zero_notional)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn fx_option_validate_rejects_non_positive_strike() {
        // An FX option strike is an FX rate and must be strictly positive.
        let expiry = date!(2026 - 06 - 15);
        let notional = Money::new(1_000_000.0, Currency::USD);
        assert!(FxOptionParams::new(0.0, expiry, OptionType::Call, notional)
            .validate()
            .is_err());
        assert!(FxOptionParams::european_call(-1.20, expiry, notional)
            .validate()
            .is_err());
        assert!(
            FxOptionParams::new(1.10, expiry, OptionType::Call, notional)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn credit_params_validate_enforces_recovery_rate_bounds() {
        // Failure mode: `CreditParams::new` skips `validate_recovery_rate`,
        // unlike `ProtectionLegSpec::new`. `CreditParams::validate` closes the
        // gap with the same shared validator.
        let above = CreditParams::new("ACME", 1.5, "ACME-CDS");
        let err = above
            .validate()
            .expect_err("recovery rate 1.5 must be rejected by validate");
        assert!(
            err.to_string().to_lowercase().contains("recovery rate"),
            "error should name the recovery rate: {err}"
        );
        // R = 1.0 is rejected (zero LGD degenerates protection legs), matching
        // the shared validator used by ProtectionLegSpec.
        assert!(CreditParams::new("ACME", 1.0, "ACME-CDS")
            .validate()
            .is_err());
        assert!(CreditParams::new("ACME", -0.1, "ACME-CDS")
            .validate()
            .is_err());
        assert!(CreditParams::new("ACME", f64::NAN, "ACME-CDS")
            .validate()
            .is_err());
        // Valid mid-range recovery is accepted.
        assert!(CreditParams::new("ACME", 0.4, "ACME-CDS")
            .validate()
            .is_ok());
        // The shared corporate/sovereign presets are within bounds.
        assert!(CreditParams::corporate_standard("CORP", "CORP-CDS")
            .validate()
            .is_ok());
        assert!(CreditParams::sovereign_standard("UST", "UST-CDS")
            .validate()
            .is_ok());
        // A struct-literal spec that bypassed `new` is still checkable.
        let bad = CreditParams {
            reference_entity: "X".to_string(),
            recovery_rate: 1.2,
            credit_curve_id: CurveId::new("X-CDS"),
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn ir_option_validate_accepts_negative_strike_but_rejects_non_finite() {
        // An interest-rate option strike is a rate; negative strikes are valid
        // in negative-rate regimes and must NOT be rejected.
        let expiry = date!(2026 - 06 - 15);
        let notional = Money::new(1_000_000.0, Currency::USD);
        assert!(
            InterestRateOptionParams::new(-0.005, expiry, OptionType::Put, "3M", notional)
                .validate()
                .is_ok(),
            "negative cap/floor strike must be accepted"
        );
        // A non-finite f64 strike (possible via deserialization) is rejected.
        assert!(
            InterestRateOptionParams::new(f64::NAN, expiry, OptionType::Put, "3M", notional)
                .validate()
                .is_err()
        );
        assert!(InterestRateOptionParams::new(
            f64::INFINITY,
            expiry,
            OptionType::Put,
            "3M",
            notional
        )
        .validate()
        .is_err());
        // A struct-literal spec that bypassed `new` is still checkable.
        let bad = InterestRateOptionParams {
            strike: f64::NAN,
            expiry,
            option_type: OptionType::Put,
            tenor: "3M".to_string(),
            day_count: DayCount::Act360,
            notional,
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn base_constructors_and_serde_roundtrip_preserve_defaults() {
        let expiry = date!(2026 - 06 - 15);
        let notional = Money::new(2_000_000.0, Currency::USD);

        let equity = EquityOptionParams::new(95.0, expiry, OptionType::Put, notional);
        let fx = FxOptionParams::new(1.05, expiry, OptionType::Call, notional);
        let credit = CreditParams::new("Issuer", 0.4, "ISSUER-CDS");
        let ir = InterestRateOptionParams::new(0.03, expiry, OptionType::Call, "3M", notional);

        assert_eq!(equity.exercise_style, ExerciseStyle::European);
        assert_eq!(equity.settlement, SettlementType::Physical);
        assert_eq!(fx.exercise_style, ExerciseStyle::European);
        assert_eq!(fx.settlement, SettlementType::Physical);
        assert_eq!(credit.recovery_rate, 0.4);
        assert_eq!(ir.day_count, DayCount::Act360);

        let equity_json = serde_json::to_string(&equity);
        let fx_json = serde_json::to_string(&fx);
        let credit_json = serde_json::to_string(&credit);
        let ir_json = serde_json::to_string(&ir);
        assert!(equity_json.is_ok());
        assert!(fx_json.is_ok());
        assert!(credit_json.is_ok());
        assert!(ir_json.is_ok());

        if let Ok(json) = equity_json {
            let roundtrip = serde_json::from_str::<EquityOptionParams>(&json);
            assert!(roundtrip.is_ok());
            if let Ok(back) = roundtrip {
                assert_eq!(back.option_type, OptionType::Put);
                assert_eq!(back.exercise_style, ExerciseStyle::European);
            }
        }
        if let Ok(json) = fx_json {
            let roundtrip = serde_json::from_str::<FxOptionParams>(&json);
            assert!(roundtrip.is_ok());
            if let Ok(back) = roundtrip {
                assert_eq!(back.option_type, OptionType::Call);
                assert_eq!(back.settlement, SettlementType::Physical);
            }
        }
        if let Ok(json) = credit_json {
            let roundtrip = serde_json::from_str::<CreditParams>(&json);
            assert!(roundtrip.is_ok());
            if let Ok(back) = roundtrip {
                assert_eq!(back.reference_entity, "Issuer");
                assert_eq!(back.credit_curve_id.as_str(), "ISSUER-CDS");
            }
        }
        if let Ok(json) = ir_json {
            let roundtrip = serde_json::from_str::<InterestRateOptionParams>(&json);
            assert!(roundtrip.is_ok());
            if let Ok(back) = roundtrip {
                assert_eq!(back.tenor, "3M");
                assert_eq!(back.option_type, OptionType::Call);
            }
        }
    }
}
