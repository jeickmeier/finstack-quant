//! Core data types for liquidity risk modeling.
//!
//! This module defines the per-instrument liquidity profile, liquidity tier
//! classification, and configuration parameters used throughout the liquidity
//! submodule.

use finstack_quant_core::Result;

use super::invalid_input;
use serde::{Deserialize, Serialize};

/// How [`LiquidityProfile::spread_volatility`] should be interpreted.
///
/// Bangia et al. (1999) phrase the LVaR add-on in terms of the volatility of the
/// **relative** (proportional) spread. Some data providers quote spread
/// volatility in absolute price units instead. This enum selects the convention;
/// the LVaR calculator normalizes absolute spread volatilities to relative
/// before combining with `z_alpha` and position value.
///
/// The default is [`SpreadVolatilityKind::Relative`] to match the original
/// Bangia convention.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpreadVolatilityKind {
    /// Standard deviation of the relative (fractional) bid-ask spread
    /// (`spread / mid`). This matches Bangia et al. (1999).
    #[default]
    Relative,
    /// Standard deviation of the absolute bid-ask spread in price units
    /// (`ask - bid`). The LVaR calculator divides by `mid` before use.
    Absolute,
}

/// Market microstructure data for a single instrument.
///
/// Users supply this data from their market data systems; the module does not
/// fetch it.
///
/// # Units
///
/// - Prices (`mid`, `bid`, `ask`) are in the instrument's native currency.
/// - `avg_daily_volume` is in shares/contracts per day.
/// - `avg_trade_size` is in shares/contracts per trade.
/// - `spread_volatility` is the standard deviation of the relative spread
///   (spread / mid) over the observation window.
///
/// # References
///
/// - Bid-ask spread conventions: `docs/REFERENCES.md#hasbrouck-2007`
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LiquidityProfile {
    /// Instrument identifier (must match `Position::instrument_id`).
    pub instrument_id: String,

    /// Mid-price (average of bid and ask).
    pub mid: f64,

    /// Best bid price.
    pub bid: f64,

    /// Best ask price.
    pub ask: f64,

    /// Average daily trading volume in shares/contracts.
    pub avg_daily_volume: f64,

    /// Average trade size in shares/contracts.
    pub avg_trade_size: f64,

    /// Standard deviation of the bid-ask spread.
    ///
    /// Used in the Bangia et al. (1999) LVaR formula. Set to 0.0 if
    /// spread volatility data is unavailable (degrades to exogenous LVaR).
    ///
    /// The interpretation (relative vs. absolute) is controlled by
    /// [`Self::spread_volatility_kind`], which defaults to
    /// [`SpreadVolatilityKind::Relative`] (the original Bangia convention).
    pub spread_volatility: f64,

    /// Interpretation of [`Self::spread_volatility`]: relative or absolute.
    pub spread_volatility_kind: SpreadVolatilityKind,

    /// Observation window in trading days for volume/spread statistics.
    ///
    /// Defaults to 20 (one calendar month). Used to qualify the
    /// statistical reliability of ADV and spread estimates.
    pub observation_days: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiquidityProfileWire {
    instrument_id: String,
    mid: f64,
    bid: f64,
    ask: f64,
    avg_daily_volume: f64,
    avg_trade_size: f64,
    spread_volatility: f64,
    spread_volatility_kind: SpreadVolatilityKind,
    observation_days: u32,
}

impl<'de> Deserialize<'de> for LiquidityProfile {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = LiquidityProfileWire::deserialize(deserializer)?;
        if wire.observation_days == 0 {
            return Err(serde::de::Error::custom(
                "minor 12: observation_days must be positive",
            ));
        }
        let mut profile = Self::new(
            wire.instrument_id,
            wire.mid,
            wire.bid,
            wire.ask,
            wire.avg_daily_volume,
            wire.avg_trade_size,
            wire.spread_volatility,
        )
        .map_err(serde::de::Error::custom)?;
        profile.spread_volatility_kind = wire.spread_volatility_kind;
        profile.observation_days = wire.observation_days;
        Ok(profile)
    }
}

impl LiquidityProfile {
    /// Create a new liquidity profile with validation.
    ///
    /// # Errors
    ///
    /// Returns `finstack_quant_core::Error::Validation` if:
    /// - `mid`, `bid`, `ask` are not finite or non-positive
    /// - `bid > ask` (crossed market)
    /// - `avg_daily_volume` is not finite or negative
    /// - `avg_trade_size` is not finite or negative
    /// - `spread_volatility` is not finite or negative
    pub fn new(
        instrument_id: impl Into<String>,
        mid: f64,
        bid: f64,
        ask: f64,
        avg_daily_volume: f64,
        avg_trade_size: f64,
        spread_volatility: f64,
    ) -> Result<Self> {
        if !mid.is_finite() || mid <= 0.0 {
            return Err(invalid_input("mid must be finite and positive"));
        }
        if !bid.is_finite() || bid <= 0.0 {
            return Err(invalid_input("bid must be finite and positive"));
        }
        if !ask.is_finite() || ask <= 0.0 {
            return Err(invalid_input("ask must be finite and positive"));
        }
        if bid > ask {
            return Err(invalid_input(format!(
                "crossed market: bid ({bid}) > ask ({ask})"
            )));
        }
        if !avg_daily_volume.is_finite() || avg_daily_volume < 0.0 {
            return Err(invalid_input(
                "avg_daily_volume must be finite and non-negative",
            ));
        }
        if !avg_trade_size.is_finite() || avg_trade_size < 0.0 {
            return Err(invalid_input(
                "avg_trade_size must be finite and non-negative",
            ));
        }
        if !spread_volatility.is_finite() || spread_volatility < 0.0 {
            return Err(invalid_input(
                "spread_volatility must be finite and non-negative",
            ));
        }

        Ok(Self {
            instrument_id: instrument_id.into(),
            mid,
            bid,
            ask,
            avg_daily_volume,
            avg_trade_size,
            spread_volatility,
            spread_volatility_kind: SpreadVolatilityKind::default(),
            observation_days: 20,
        })
    }

    /// Override the [`SpreadVolatilityKind`] for this profile.
    ///
    /// Defaults to [`SpreadVolatilityKind::Relative`] (Bangia convention); use
    /// [`SpreadVolatilityKind::Absolute`] when the stored `spread_volatility`
    /// is in absolute price units (e.g., the standard deviation of `ask - bid`).
    #[must_use]
    pub fn with_spread_volatility_kind(mut self, kind: SpreadVolatilityKind) -> Self {
        self.spread_volatility_kind = kind;
        self
    }

    /// Spread volatility expressed as a fraction of mid-price.
    ///
    /// For [`SpreadVolatilityKind::Relative`] this is the stored value; for
    /// [`SpreadVolatilityKind::Absolute`] the stored value is divided by `mid`.
    /// Returns 0.0 when `mid` is non-positive.
    #[inline]
    pub fn relative_spread_volatility(&self) -> f64 {
        match self.spread_volatility_kind {
            SpreadVolatilityKind::Relative => self.spread_volatility,
            SpreadVolatilityKind::Absolute => {
                if self.mid > 0.0 {
                    self.spread_volatility / self.mid
                } else {
                    0.0
                }
            }
        }
    }

    /// Absolute bid-ask spread.
    #[inline]
    pub fn spread(&self) -> f64 {
        self.ask - self.bid
    }

    /// Relative spread as a fraction of mid-price.
    #[inline]
    pub fn relative_spread(&self) -> f64 {
        self.spread() / self.mid
    }

    /// Half-spread (one-way transaction cost at mid).
    #[inline]
    pub fn half_spread(&self) -> f64 {
        0.5 * self.spread()
    }
}

/// Liquidity tier classification based on days-to-liquidate.
///
/// Tiers follow a common industry convention where Tier 1 is the most
/// liquid (intraday liquidation) and Tier 5 is the least liquid
/// (months to unwind). The thresholds are configurable via
/// [`LiquidityConfig`].
///
/// # References
///
/// - AIFMD liquidity bucketing: `docs/REFERENCES.md#aifmd-liquidity-management`
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidityTier {
    /// Tier 1: < 1 day to liquidate (highly liquid).
    Tier1,
    /// Tier 2: 1-5 days to liquidate.
    Tier2,
    /// Tier 3: 5-20 days to liquidate.
    Tier3,
    /// Tier 4: 20-60 days to liquidate.
    Tier4,
    /// Tier 5: > 60 days to liquidate (illiquid).
    Tier5,
}

impl std::fmt::Display for LiquidityTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tier1 => write!(f, "Tier1"),
            Self::Tier2 => write!(f, "Tier2"),
            Self::Tier3 => write!(f, "Tier3"),
            Self::Tier4 => write!(f, "Tier4"),
            Self::Tier5 => write!(f, "Tier5"),
        }
    }
}

impl LiquidityTier {
    /// Lowercase label used by the Python and WASM convenience bindings.
    #[must_use]
    pub const fn as_binding_str(self) -> &'static str {
        match self {
            Self::Tier1 => "tier1",
            Self::Tier2 => "tier2",
            Self::Tier3 => "tier3",
            Self::Tier4 => "tier4",
            Self::Tier5 => "tier5",
        }
    }
}

/// Configuration for liquidity calculations.
///
/// Holds the tier-classification thresholds consumed by [`classify_tier`].
/// All thresholds use trading days (not calendar days).
///
/// Historical note (breaking wire change, 2026-08): the former
/// `participation_rate`, `risk_aversion`, `holding_period`,
/// `confidence_level` and `endogenous_spread_coef` fields were validated and
/// serde-required but consumed by nothing — their docs referenced a
/// `lvar_bangia(profile, config, ...)` entry point that does not exist — so
/// they were removed. Inbound payloads still carrying those keys now fail
/// closed under `deny_unknown_fields`. Per-call knobs live where they are
/// used instead: participation is an explicit argument to
/// [`days_to_liquidate`], risk aversion rides on
/// [`crate::liquidity::TradeParams::risk_aversion`], and the VaR confidence
/// is an explicit argument to [`crate::liquidity::lvar_bangia_scalar`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiquidityConfig {
    /// Days-to-liquidate thresholds for tier boundaries.
    ///
    /// Array of 4 thresholds: \[tier1_max, tier2_max, tier3_max, tier4_max\].
    /// Default: \[1.0, 5.0, 20.0, 60.0\].
    pub tier_thresholds: [f64; 4],
}

impl Default for LiquidityConfig {
    #[allow(clippy::expect_used)] // Embedded liquidity defaults are a compile-time asset.
    fn default() -> Self {
        super::registry::embedded_liquidity_defaults()
            .expect("embedded models liquidity defaults are compile-time assets")
            .default_config
            .clone()
    }
}

/// Classify a position into a liquidity tier based on days-to-liquidate.
///
/// Pure function using the configured thresholds.
///
/// # Arguments
///
/// * `days_to_liquidate` - Estimated trading days needed to liquidate the
///   position under the selected participation assumption.
/// * `thresholds` - Four ascending day-count boundaries separating Tiers 1–5.
pub fn classify_tier(days_to_liquidate: f64, thresholds: &[f64; 4]) -> LiquidityTier {
    if days_to_liquidate < thresholds[0] {
        LiquidityTier::Tier1
    } else if days_to_liquidate < thresholds[1] {
        LiquidityTier::Tier2
    } else if days_to_liquidate < thresholds[2] {
        LiquidityTier::Tier3
    } else if days_to_liquidate < thresholds[3] {
        LiquidityTier::Tier4
    } else {
        LiquidityTier::Tier5
    }
}

/// Compute the number of trading days required to liquidate a position
/// at the given participation rate.
///
/// # Arguments
///
/// * `position_quantity` - Absolute number of shares/contracts to liquidate.
/// * `adv` - Average daily volume.
/// * `participation_rate` - Fraction of ADV to trade per day.
///
/// # Returns
///
/// Days to fully liquidate. Returns `f64::INFINITY` if ADV or participation
/// rate is zero.
pub fn days_to_liquidate(position_quantity: f64, adv: f64, participation_rate: f64) -> f64 {
    let daily_capacity = participation_rate * adv;
    if daily_capacity <= 0.0 {
        return f64::INFINITY;
    }
    position_quantity.abs() / daily_capacity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_profile_construction() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let p = LiquidityProfile::new("AAPL", 150.0, 149.90, 150.10, 50_000_000.0, 200.0, 0.001)?;
        assert_eq!(p.instrument_id, "AAPL");
        assert_eq!(p.observation_days, 20);
        Ok(())
    }

    #[test]
    fn profile_rejects_negative_mid() {
        let result = LiquidityProfile::new("X", -1.0, 1.0, 2.0, 100.0, 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn profile_rejects_crossed_market() {
        let result = LiquidityProfile::new("X", 100.0, 101.0, 99.0, 100.0, 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn profile_rejects_negative_volume() {
        let result = LiquidityProfile::new("X", 100.0, 99.0, 101.0, -1.0, 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn profile_rejects_nan_spread_vol() {
        let result = LiquidityProfile::new("X", 100.0, 99.0, 101.0, 100.0, 10.0, f64::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn spread_calculations() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let p = LiquidityProfile::new("TEST", 100.0, 99.0, 101.0, 1000.0, 50.0, 0.01)?;
        assert!((p.spread() - 2.0).abs() < 1e-10);
        assert!((p.relative_spread() - 0.02).abs() < 1e-10);
        assert!((p.half_spread() - 1.0).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn tier_classification() {
        let thresholds = [1.0, 5.0, 20.0, 60.0];
        assert_eq!(classify_tier(0.5, &thresholds), LiquidityTier::Tier1);
        assert_eq!(classify_tier(1.0, &thresholds), LiquidityTier::Tier2);
        assert_eq!(classify_tier(3.0, &thresholds), LiquidityTier::Tier2);
        assert_eq!(classify_tier(5.0, &thresholds), LiquidityTier::Tier3);
        assert_eq!(classify_tier(19.0, &thresholds), LiquidityTier::Tier3);
        assert_eq!(classify_tier(20.0, &thresholds), LiquidityTier::Tier4);
        assert_eq!(classify_tier(59.0, &thresholds), LiquidityTier::Tier4);
        assert_eq!(classify_tier(60.0, &thresholds), LiquidityTier::Tier5);
        assert_eq!(classify_tier(100.0, &thresholds), LiquidityTier::Tier5);
    }

    #[test]
    fn tier_binding_labels_are_lowercase() {
        assert_eq!(LiquidityTier::Tier1.as_binding_str(), "tier1");
        assert_eq!(LiquidityTier::Tier2.as_binding_str(), "tier2");
        assert_eq!(LiquidityTier::Tier3.as_binding_str(), "tier3");
        assert_eq!(LiquidityTier::Tier4.as_binding_str(), "tier4");
        assert_eq!(LiquidityTier::Tier5.as_binding_str(), "tier5");
    }

    #[test]
    fn days_to_liquidate_basic() {
        // 100k shares, ADV=1M, 10% participation = 100k/100k = 1 day
        assert!((super::days_to_liquidate(100_000.0, 1_000_000.0, 0.10) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn days_to_liquidate_zero_adv() {
        assert!(super::days_to_liquidate(100.0, 0.0, 0.10).is_infinite());
    }

    #[test]
    fn days_to_liquidate_zero_participation() {
        assert!(super::days_to_liquidate(100.0, 1000.0, 0.0).is_infinite());
    }

    #[test]
    fn default_config() {
        let c = LiquidityConfig::default();
        assert_eq!(c.tier_thresholds, [1.0, 5.0, 20.0, 60.0]);
    }

    #[test]
    fn serde_round_trip_profile() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let p = LiquidityProfile::new("AAPL", 150.0, 149.90, 150.10, 50_000_000.0, 200.0, 0.001)?;
        let json = serde_json::to_string(&p)?;
        let p2: LiquidityProfile = serde_json::from_str(&json)?;
        assert_eq!(p, p2);
        Ok(())
    }

    #[test]
    fn minor12_profile_serde_rejects_invalid_mid_and_unknown_fields() {
        let invalid_mid = r#"{
            "instrument_id": "AAPL",
            "mid": 0.0,
            "bid": 149.90,
            "ask": 150.10,
            "avg_daily_volume": 50000000.0,
            "avg_trade_size": 200.0,
            "spread_volatility": 0.001,
            "spread_volatility_kind": "relative",
            "observation_days": 20
        }"#;
        let err = serde_json::from_str::<LiquidityProfile>(invalid_mid)
            .expect_err("minor 12: zero mid must fail serde validation");
        assert!(err.to_string().contains("mid"), "unexpected error: {err}");

        let unknown_field = r#"{
            "instrument_id": "AAPL",
            "mid": 150.0,
            "bid": 149.90,
            "ask": 150.10,
            "avg_daily_volume": 50000000.0,
            "avg_trade_size": 200.0,
            "spread_volatility": 0.001,
            "spread_volatility_kind": "relative",
            "observation_days": 20,
            "extra": true
        }"#;
        let err = serde_json::from_str::<LiquidityProfile>(unknown_field)
            .expect_err("minor 12: unknown liquidity profile fields must fail");
        assert!(
            err.to_string().contains("unknown field"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn serde_round_trip_tier() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let tier = LiquidityTier::Tier3;
        let json = serde_json::to_string(&tier)?;
        let t2: LiquidityTier = serde_json::from_str(&json)?;
        assert_eq!(tier, t2);
        Ok(())
    }

    /// BREAKING wire change (audit fix 8): `participation_rate`,
    /// `risk_aversion`, `holding_period`, `confidence_level` and
    /// `endogenous_spread_coef` were validated, documented and
    /// serde-required but consumed by nothing in the workspace (the docs
    /// referenced a nonexistent `lvar_bangia(profile, config, ...)`), so
    /// they were deleted. `tier_thresholds` — the only consumed field —
    /// remains, and any legacy payload still carrying a removed key now
    /// fails closed under `deny_unknown_fields`.
    #[test]
    fn liquidity_config_is_tier_thresholds_only() {
        let json = r#"{"tier_thresholds": [1.0, 5.0, 20.0, 60.0]}"#;
        let config: LiquidityConfig =
            serde_json::from_str(json).expect("tier-thresholds-only payload must parse");
        assert_eq!(config.tier_thresholds, [1.0, 5.0, 20.0, 60.0]);

        for legacy in [
            "participation_rate",
            "risk_aversion",
            "holding_period",
            "confidence_level",
            "endogenous_spread_coef",
        ] {
            let payload =
                format!(r#"{{"tier_thresholds": [1.0, 5.0, 20.0, 60.0], "{legacy}": 0.5}}"#);
            let err = serde_json::from_str::<LiquidityConfig>(&payload)
                .expect_err("removed legacy fields must fail closed");
            assert!(
                err.to_string().contains("unknown field"),
                "{legacy}: unexpected error: {err}"
            );
        }
    }

    #[test]
    fn serde_round_trip_config() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let c = LiquidityConfig::default();
        let json = serde_json::to_string(&c)?;
        let c2: LiquidityConfig = serde_json::from_str(&json)?;
        assert_eq!(c, c2);
        Ok(())
    }
}
