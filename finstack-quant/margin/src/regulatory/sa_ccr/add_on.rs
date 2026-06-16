//! Per-asset-class add-on computation for SA-CCR.
//!
//! For a linear trade:
//!   `d_i = supervisory_delta * adjusted_notional * maturity_factor * SF_class`
//!
//! For an interest-rate trade the adjusted notional picks up an extra
//! supervisory-duration factor (see [`supervisory_duration`]). Trades
//! within the same hedging set offset per the asset-class rule; hedging
//! sets are then aggregated per the asset-class cross-HS rule.
//!
//! # Known scope limitations
//!
//! The add-on aggregation across hedging sets for non-IR asset classes
//! uses the classic `sqrt((sum rho*d_HS)^2 + sum (1 - rho^2) * d_HS^2)`
//! decomposition applied at the hedging-set level, which is the correct
//! formulation when each hedging set corresponds to a single reference
//! entity / commodity (Credit / Equity / Commodity under MAR52 / CRE52).
//! Deployments that put multiple entities in the same hedging-set
//! bucket should perform the systematic/idiosyncratic split at the
//! entity level upstream and pass one hedging-set-per-entity here.

use super::maturity_factor::{maturity_factor_margined, maturity_factor_unmargined};
use super::params::{supervisory_correlation, supervisory_factor};
use super::types::{SaCcrAssetClass, SaCcrNettingSetConfig, SaCcrTrade};
use finstack_quant_core::HashMap;

/// IR supervisory correlation continuous-compounding rate per CRE52.54.
const IR_SUPERVISORY_DISCOUNT_RATE: f64 = 0.05;

/// Minimum supervisory duration in years (10 business days).
///
/// CRE52.48 imposes a 10-business-day floor on remaining maturity for the
/// unmargined maturity factor; we apply the same floor to supervisory
/// duration so in-flight IR trades with nearly-zero remaining tenor do
/// not collapse to zero effective notional. 10 business days ≈ 10/250
/// years.
const MIN_SUPERVISORY_DURATION_YEARS: f64 = 10.0 / 250.0;

/// IR maturity-bucket correlation matrix off-diagonal entries per
/// CRE52.54 (1.4 for adjacent buckets, 0.6 for non-adjacent).
const IR_ADJACENT_BUCKET_CORR: f64 = 1.4;
const IR_NONADJACENT_BUCKET_CORR: f64 = 0.6;

/// Per-trade maturity factor.
///
/// * Margined netting sets: all trades share `MF = 1.5 * sqrt(MPOR/250)`
///   per CRE52.52.
/// * Unmargined: each trade has its own `MF_i = sqrt(min(M_i, 1y)/1y)`
///   per CRE52.48, where `M_i` is the remaining maturity of trade i
///   (with a 10-business-day floor).
///
/// Aggregating via a single netting-set-level average MF (the previous
/// behavior) is not SA-CCR-compliant for mixed-tenor unmargined books.
fn trade_maturity_factor(config: &SaCcrNettingSetConfig, trade: &SaCcrTrade) -> f64 {
    if config.is_margined {
        maturity_factor_margined(config.mpor_days)
    } else {
        let days = (trade.end_date - config.as_of).whole_days().max(0) as f64;
        let m_years = days / 365.0;
        maturity_factor_unmargined(m_years)
    }
}

/// Compute the add-on for a single asset class.
///
/// Per-trade maturity factors are derived from `config` so unmargined
/// netting sets with mixed maturities are handled correctly.
///
/// # Arguments
///
/// * `asset_class` - SA-CCR asset class whose trades should be included.
/// * `trades` - Netting-set trades; trades from other asset classes are ignored.
/// * `config` - Netting-set collateral and margin-agreement terms used for
///   maturity-factor selection.
///
/// # Returns
///
/// The non-negative add-on for `asset_class`.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
pub fn asset_class_add_on(
    asset_class: SaCcrAssetClass,
    trades: &[SaCcrTrade],
    config: &SaCcrNettingSetConfig,
) -> f64 {
    match asset_class {
        SaCcrAssetClass::InterestRate => ir_add_on(trades, config),
        other => non_ir_add_on(other, trades, config),
    }
}

/// Interest-rate add-on with supervisory duration and 3x3 maturity buckets.
///
/// For each IR trade:
/// * Compute supervisory duration
///   `SD = (exp(-0.05 * S) - exp(-0.05 * E)) / 0.05`
///   where `S` and `E` are the start and end offsets in years. Trades
///   already in flight (`start_date <= as_of`) have `S = 0`.
/// * Adjusted notional `d_i = supervisory_delta * |notional| * SD * MF`.
/// * Bucket by end-maturity: `< 1y` -> D1, `1-5y` -> D2, `> 5y` -> D3.
/// * Per-hedging-set effective notional via CRE52.54:
///   `EN_HS = sqrt(D1^2 + D2^2 + D3^2 + 1.4*D1*D2 + 1.4*D2*D3 + 0.6*D1*D3)`
/// * Across hedging sets (currencies): simple absolute sum.
fn ir_add_on(trades: &[SaCcrTrade], config: &SaCcrNettingSetConfig) -> f64 {
    let sf = supervisory_factor(SaCcrAssetClass::InterestRate);

    // (D1, D2, D3) per hedging set.
    let mut by_hs: HashMap<String, [f64; 3]> = HashMap::default();
    for trade in trades
        .iter()
        .filter(|t| t.asset_class == SaCcrAssetClass::InterestRate)
    {
        let start_years = ((trade.start_date - config.as_of).whole_days().max(0) as f64) / 365.0;
        let end_years = ((trade.end_date - config.as_of).whole_days().max(0) as f64) / 365.0;
        let sd = supervisory_duration(start_years, end_years);
        let mf = trade_maturity_factor(config, trade);
        let d_i = trade.supervisory_delta * trade.notional.abs() * sd * mf;

        let bucket = if end_years < 1.0 {
            0
        } else if end_years <= 5.0 {
            1
        } else {
            2
        };

        let entry = by_hs.entry(trade.hedging_set.clone()).or_insert([0.0; 3]);
        entry[bucket] += d_i;
    }

    let mut add_on = 0.0;
    for d in by_hs.values() {
        let d1 = d[0];
        let d2 = d[1];
        let d3 = d[2];
        let en2 = d1 * d1
            + d2 * d2
            + d3 * d3
            + IR_ADJACENT_BUCKET_CORR * d1 * d2
            + IR_ADJACENT_BUCKET_CORR * d2 * d3
            + IR_NONADJACENT_BUCKET_CORR * d1 * d3;
        let en_hs = en2.max(0.0).sqrt();
        add_on += sf * en_hs;
    }
    add_on
}

/// Supervisory duration per CRE52.54.
///
/// `SD = (exp(-r*S) - exp(-r*E)) / r` with `r = 0.05`, where `S` is the
/// start-offset (years from as-of to trade start) and `E` is the
/// end-offset (years from as-of to trade end). Floored at a small
/// positive value so in-flight trades (`S = 0`) with nearly-matured tails
/// don't collapse to zero effective notional.
///
/// # Arguments
///
/// * `start_years` - Forward-start offset in years, floored at zero.
/// * `end_years` - Maturity offset in years, floored at `start_years`.
///
/// # Returns
///
/// Supervisory duration in years.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[must_use]
pub fn supervisory_duration(start_years: f64, end_years: f64) -> f64 {
    let r = IR_SUPERVISORY_DISCOUNT_RATE;
    let s = start_years.max(0.0);
    let e = end_years.max(s);
    let sd = ((-r * s).exp() - (-r * e).exp()) / r;
    sd.max(MIN_SUPERVISORY_DURATION_YEARS)
}

/// Non-IR add-on with the simplified hedging-set-level
/// systematic/idiosyncratic decomposition. See module docs for the
/// single-entity-per-HS assumption.
fn non_ir_add_on(
    asset_class: SaCcrAssetClass,
    trades: &[SaCcrTrade],
    config: &SaCcrNettingSetConfig,
) -> f64 {
    let sf = supervisory_factor(asset_class);
    let rho = supervisory_correlation(asset_class);

    let mut by_hedging_set: HashMap<String, f64> = HashMap::default();
    for trade in trades.iter().filter(|t| t.asset_class == asset_class) {
        let mf = trade_maturity_factor(config, trade);
        let d_i = trade.supervisory_delta * trade.notional.abs() * mf;
        *by_hedging_set
            .entry(trade.hedging_set.clone())
            .or_insert(0.0) += d_i;
    }

    if by_hedging_set.is_empty() {
        return 0.0;
    }

    let hedging_set_values: Vec<f64> = by_hedging_set.values().copied().collect();

    // FX hedging sets are currency pairs. Offset within a pair, but do not
    // let opposite deltas in different pairs collapse the total add-on.
    if asset_class == SaCcrAssetClass::ForeignExchange {
        let add_on_raw: f64 = hedging_set_values.iter().map(|hs| hs.abs()).sum();
        return add_on_raw * sf;
    }

    // For Credit / Equity / Commodity (rho < 1) each hedging set is the
    // systematic/idiosyncratic unit per the single-entity-per-HS caveat
    // in the module docs.
    let systematic: f64 = hedging_set_values.iter().sum::<f64>() * rho;
    let idiosyncratic: f64 = hedging_set_values
        .iter()
        .map(|hs| (1.0 - rho * rho) * hs * hs)
        .sum::<f64>();

    let add_on_raw = (systematic * systematic + idiosyncratic).sqrt();
    add_on_raw * sf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisory_duration_matches_analytic_formula() {
        // SD for E=5y, S=0 = (1 - e^{-0.25}) / 0.05
        let sd = supervisory_duration(0.0, 5.0);
        let expected = (1.0 - (-0.25f64).exp()) / 0.05;
        assert!(
            (sd - expected).abs() < 1e-12,
            "SD(0, 5) = {sd}, expected {expected}"
        );
    }

    #[test]
    fn supervisory_duration_handles_forward_starting() {
        // Forward-starting 1Yx4Y: S=1, E=5. SD = (e^{-0.05} - e^{-0.25})/0.05
        let sd = supervisory_duration(1.0, 5.0);
        let expected = ((-0.05f64).exp() - (-0.25f64).exp()) / 0.05;
        assert!(
            (sd - expected).abs() < 1e-12,
            "SD(1, 5) = {sd}, expected {expected}"
        );
    }

    #[test]
    fn supervisory_duration_non_negative_for_swapped_args() {
        // Defensive: if caller supplies end < start, clamp to 0.
        let sd = supervisory_duration(5.0, 1.0);
        assert!(sd >= 0.0);
    }

    mod per_trade_mf {
        use super::*;
        use crate::regulatory::sa_ccr::types::{SaCcrNettingSetConfig, SaCcrTrade};
        use crate::types::NettingSetId;
        use finstack_quant_core::dates::Date;

        fn d(y: i32, m: u8, day: u8) -> Date {
            Date::from_calendar_date(y, time::Month::try_from(m).expect("valid"), day)
                .expect("valid")
        }

        fn fx_trade(end: Date, notional: f64) -> SaCcrTrade {
            SaCcrTrade {
                trade_id: "T".into(),
                asset_class: SaCcrAssetClass::ForeignExchange,
                notional,
                start_date: d(2025, 1, 15),
                end_date: end,
                underlier: "EURUSD".into(),
                hedging_set: "EURUSD".into(),
                direction: 1.0,
                supervisory_delta: 1.0,
                mtm: 0.0,
                is_option: false,
                option_type: None,
            }
        }

        fn ir_trade(start: Date, end: Date, notional: f64) -> SaCcrTrade {
            SaCcrTrade {
                trade_id: "IR".into(),
                asset_class: SaCcrAssetClass::InterestRate,
                notional,
                start_date: start,
                end_date: end,
                underlier: "USD-SOFR".into(),
                hedging_set: "USD".into(),
                direction: 1.0,
                supervisory_delta: 1.0,
                mtm: 0.0,
                is_option: false,
                option_type: None,
            }
        }

        /// Unmargined SA-CCR uses per-trade MF. A 6M trade must get a
        /// smaller MF than a 1Y trade, so adding a 6M trade alongside
        /// a 1Y trade should grow the add-on by strictly less than
        /// adding a second 1Y trade.
        #[test]
        fn unmargined_per_trade_mf_shrinks_short_tenor_contribution() {
            let cfg = SaCcrNettingSetConfig::unmargined(
                NettingSetId::bilateral("BANK_A", "CSA"),
                0.0,
                d(2025, 1, 15),
            );

            let one_year = d(2026, 1, 15);
            let six_months = d(2025, 7, 15);

            let trade_1y_a = fx_trade(one_year, 100_000_000.0);
            let trade_1y_b = fx_trade(one_year, 100_000_000.0);
            let trade_6m = fx_trade(six_months, 100_000_000.0);

            let ao_two_one_year = asset_class_add_on(
                SaCcrAssetClass::ForeignExchange,
                &[trade_1y_a.clone(), trade_1y_b],
                &cfg,
            );
            let ao_one_year_plus_six_month = asset_class_add_on(
                SaCcrAssetClass::ForeignExchange,
                &[trade_1y_a, trade_6m],
                &cfg,
            );

            assert!(
                ao_one_year_plus_six_month < ao_two_one_year,
                "mixed-tenor add-on must reflect per-trade MF: \
                 1y+6m={ao_one_year_plus_six_month} vs 1y+1y={ao_two_one_year}"
            );
        }

        /// Margined netting sets must apply a single MPOR-based MF to
        /// every trade regardless of remaining maturity, so the
        /// per-trade MF must collapse to the shared MPOR formula.
        #[test]
        fn margined_uses_shared_mpor_mf() {
            let cfg = SaCcrNettingSetConfig::margined(
                NettingSetId::bilateral("BANK_A", "CSA"),
                0.0,
                0.0,
                0.0,
                0.0,
                10,
                d(2025, 1, 15),
            );

            let t_short = fx_trade(d(2025, 3, 15), 50_000_000.0);
            let t_long = fx_trade(d(2030, 1, 15), 50_000_000.0);

            let mf_short = super::trade_maturity_factor(&cfg, &t_short);
            let mf_long = super::trade_maturity_factor(&cfg, &t_long);

            let expected = 1.5 * (10.0f64 / 250.0).sqrt();
            assert!(
                (mf_short - expected).abs() < 1e-12 && (mf_long - expected).abs() < 1e-12,
                "margined MF must be constant = {expected}, got short={mf_short} long={mf_long}"
            );
        }

        #[test]
        fn unmargined_uses_remaining_maturity_from_as_of() {
            let as_of = d(2026, 7, 15);
            let cfg = SaCcrNettingSetConfig::unmargined(
                NettingSetId::bilateral("BANK_A", "CSA"),
                0.0,
                as_of,
            );

            let aged_trade = fx_trade(d(2027, 1, 15), 100_000_000.0);
            let add_on = asset_class_add_on(SaCcrAssetClass::ForeignExchange, &[aged_trade], &cfg);

            let remaining_years = (d(2027, 1, 15) - as_of).whole_days() as f64 / 365.0;
            let expected = 100_000_000.0 * maturity_factor_unmargined(remaining_years) * 0.04;
            assert!(
                (add_on - expected).abs() < 1.0,
                "aged FX trade must use remaining maturity from as_of: expected {expected}, got {add_on}"
            );
        }

        #[test]
        fn ir_supervisory_duration_uses_forward_start_from_as_of() {
            let as_of = d(2025, 1, 15);
            let cfg = SaCcrNettingSetConfig::unmargined(
                NettingSetId::bilateral("BANK_A", "CSA"),
                0.0,
                as_of,
            );
            let start = d(2026, 1, 15);
            let end = d(2030, 1, 15);
            let trade = ir_trade(start, end, 100_000_000.0);

            let add_on = asset_class_add_on(SaCcrAssetClass::InterestRate, &[trade], &cfg);
            let start_years = (start - as_of).whole_days() as f64 / 365.0;
            let end_years = (end - as_of).whole_days() as f64 / 365.0;
            let expected = 100_000_000.0
                * supervisory_duration(start_years, end_years)
                * maturity_factor_unmargined(end_years)
                * 0.005;

            assert!(
                (add_on - expected).abs() < 1.0,
                "forward-starting IR trade must use S/E offsets from as_of: expected {expected}, got {add_on}"
            );
        }

        #[test]
        fn fx_aggregation_does_not_offset_different_currency_pairs() {
            let cfg = SaCcrNettingSetConfig::unmargined(
                NettingSetId::bilateral("BANK_A", "CSA"),
                0.0,
                d(2025, 1, 15),
            );
            let mut eur_usd = fx_trade(d(2026, 1, 15), 100_000_000.0);
            eur_usd.hedging_set = "EURUSD".into();
            eur_usd.underlier = "EURUSD".into();

            let mut usd_jpy = fx_trade(d(2026, 1, 15), 100_000_000.0);
            usd_jpy.hedging_set = "USDJPY".into();
            usd_jpy.underlier = "USDJPY".into();
            usd_jpy.direction = -1.0;
            usd_jpy.supervisory_delta = -1.0;

            let add_on =
                asset_class_add_on(SaCcrAssetClass::ForeignExchange, &[eur_usd, usd_jpy], &cfg);
            let expected = 2.0 * 100_000_000.0 * 0.04;
            assert!(
                (add_on - expected).abs() < 1.0,
                "different FX pairs should not offset: expected {expected}, got {add_on}"
            );
        }
    }
}
