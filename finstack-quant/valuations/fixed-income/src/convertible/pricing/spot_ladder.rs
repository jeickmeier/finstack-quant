//! Price, delta, parity, premium and regime of a convertible at a ladder of share prices.
//!
//! Each row reprices the bond on the market with only the share price moved, so
//! volatility (read at the contractual conversion strike), rates, dividends and
//! credit stay as supplied, and the issuer's credit does not respond to the
//! share price. Price, delta, parity, conversion value, premium and floor are
//! the same definitions as the metrics of those names.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::{ConversionPolicy, GreekBumps, Instrument};
use finstack_quant_valuations_core::metrics::sensitivities::config as sensitivities;
use serde::{Deserialize, Serialize};

use super::engine::{
    compute_conversion_value, prepare_for_pricing, price_bond_floor, price_convertible_bond,
    spot_bump_prices,
};
use crate::convertible::ConvertibleBond;

/// Multiples of the market share price on the default ladder. The market share
/// price itself is one of them.
const DEFAULT_SPOT_MULTIPLES: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];

/// Hedge ratio below which a convertible is [`ConvertibleRegime::Busted`].
const BUSTED_BELOW: f64 = 0.40;

/// Hedge ratio above which a convertible is [`ConvertibleRegime::EquitySensitive`].
const EQUITY_SENSITIVE_ABOVE: f64 = 0.80;

/// Which of bond or stock drives a convertible's value at a share price.
///
/// Classified by [`ConvertibleSpotRow::hedge_ratio`] on the market bands of 40 and
/// 80 delta. Managers do not agree on the cutoffs: these are the bands Putnam and
/// Lord Abbett publish, and MSCI uses thirds. See
/// `docs/REFERENCES.md#convertible-delta-bands`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConvertibleRegime {
    /// Hedge ratio below 0.40: value follows the bond floor and the issuer's credit.
    Busted,
    /// Hedge ratio from 0.40 to 0.80 inclusive: value responds to both bond and stock.
    Balanced,
    /// Hedge ratio above 0.80: value follows the shares.
    EquitySensitive,
}

impl ConvertibleRegime {
    /// Wire name of the regime (`busted`, `balanced` or `equity_sensitive`), the serde spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Busted => "busted",
            Self::Balanced => "balanced",
            Self::EquitySensitive => "equity_sensitive",
        }
    }

    fn from_hedge_ratio(hedge_ratio: f64) -> Self {
        if hedge_ratio < BUSTED_BELOW {
            Self::Busted
        } else if hedge_ratio > EQUITY_SENSITIVE_ABOVE {
            Self::EquitySensitive
        } else {
            Self::Balanced
        }
    }
}

/// A convertible at one share price: a row of a [`ConvertibleSpotLadder`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConvertibleSpotRow {
    /// Share price of this row, in the bond's currency.
    pub spot: f64,
    /// Whether [`spot`](Self::spot) is the share price the market holds.
    pub is_market_spot: bool,
    /// Tree value of the whole position in [`currency`](ConvertibleSpotLadder::currency),
    /// the PV of the bond priced on this share price.
    pub price: f64,
    /// Change in [`price`](Self::price) per unit of share price, by central difference
    /// with the bond's spot bump, the `delta` metric's definition. `null` if not finite.
    pub delta: Option<f64>,
    /// [`delta`](Self::delta) as a share of the effective conversion ratio, the decimal
    /// the regime bands read. `null` on a mandatory-variable bond, whose share count
    /// moves with the stock, if the ratio is not positive, or if the delta is `null`.
    pub hedge_ratio: Option<f64>,
    /// Regime of this row; `null` where [`hedge_ratio`](Self::hedge_ratio) is.
    pub regime: Option<ConvertibleRegime>,
    /// Immediate conversion value of the position in currency, the `conversion_value` metric.
    pub conversion_value: f64,
    /// Conversion value over notional, the `parity` metric (`1.0` is par, not one dollar).
    pub parity: f64,
    /// [`price`](Self::price) over conversion value, less one, as a decimal; the
    /// `conversion_premium` metric. `null` when conversion value is not positive or the
    /// ratio is not finite.
    pub conversion_premium: Option<f64>,
    /// Value of the coupons and principal with no conversion or exercise rights, the
    /// `bond_floor` metric in currency. It does not depend on the share price, so every
    /// row holds the same number.
    pub bond_floor: f64,
}

/// A convertible at a ladder of share prices. Returned by [`convertible_spot_ladder`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConvertibleSpotLadder {
    /// Identifier of the bond.
    pub instrument_id: String,
    /// Valuation date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Currency of the bond, which every money field is in.
    pub currency: Currency,
    /// Share price the market holds.
    pub market_spot: f64,
    /// One row per share price, in ascending order.
    pub rows: Vec<ConvertibleSpotRow>,
}

impl ConvertibleSpotLadder {
    /// Export one row per share price, in ladder order, as a [`TableEnvelope`].
    ///
    /// Columns: `spot` (float), `is_market_spot` (bool), `price` (float),
    /// `delta` and `hedge_ratio` (nullable float), `regime` (nullable string:
    /// `busted`, `balanced` or `equity_sensitive`), `conversion_value` and
    /// `parity` (float), `conversion_premium` (nullable float) and
    /// `bond_floor` (float); money columns are in [`currency`](Self::currency).
    /// The columns remain when there are no rows.
    #[must_use]
    pub fn to_table(&self) -> TableEnvelope {
        let rows = &self.rows;
        let float = |f: fn(&ConvertibleSpotRow) -> f64| {
            TableColumnData::Float64(rows.iter().map(f).collect())
        };
        let nullable = |f: fn(&ConvertibleSpotRow) -> Option<f64>| {
            TableColumnData::NullableFloat64(rows.iter().map(f).collect())
        };
        TableEnvelope {
            row_count: rows.len(),
            columns: vec![
                TableColumn::new("spot", float(|r| r.spot)),
                TableColumn::new(
                    "is_market_spot",
                    TableColumnData::Boolean(rows.iter().map(|r| r.is_market_spot).collect()),
                ),
                TableColumn::new("price", float(|r| r.price)),
                TableColumn::new("delta", nullable(|r| r.delta)),
                TableColumn::new("hedge_ratio", nullable(|r| r.hedge_ratio)),
                TableColumn::new(
                    "regime",
                    TableColumnData::NullableString(
                        rows.iter()
                            .map(|r| r.regime.map(|regime| regime.as_str().to_string()))
                            .collect(),
                    ),
                ),
                TableColumn::new("conversion_value", float(|r| r.conversion_value)),
                TableColumn::new("parity", float(|r| r.parity)),
                TableColumn::new("conversion_premium", nullable(|r| r.conversion_premium)),
                TableColumn::new("bond_floor", float(|r| r.bond_floor)),
            ],
            metadata: Default::default(),
        }
    }
}

/// Value a convertible at a ladder of share prices.
///
/// Answers how price, delta, parity, conversion premium and regime move with the
/// stock, with the bond floor to read them against. Prices on the convertible tree,
/// the bond's only model, at `model_config.tree_steps` steps. Each row runs the tree
/// three times, so the cost is linear in the rows.
///
/// # Arguments
///
/// * `bond` - Convertible to value. Its `metric_pricing_overrides.bump_config`
///   sets the spot bump behind delta (default 1% of each row's share price).
/// * `market` - Holds the bond's discount curve, share price, volatility and any
///   dividend yield, hazard curve and floating index. Only the share price moves
///   between rows; volatility stays at its conversion-strike value and the hazard
///   curve is unchanged.
/// * `as_of` - Valuation date and origin of the tree's ACT/365F clock.
/// * `spots` - Share prices in the bond's currency, each finite and positive. They
///   are sorted ascending and duplicates dropped. `None` uses
///   0.25, 0.5, 0.75, 1, 1.25, 1.5, 2 and 3 times the market's share price.
///
/// # Returns
///
/// A row per share price and the market's share price. Fields that can be undefined
/// are `None`, as each documents, and no field is ever infinite or NaN.
///
/// # Errors
///
/// Returns a validation error when `spots` is empty or holds a value that is not
/// finite and positive, when the market's share price is not positive and `spots`
/// is `None`, or when the bond fails validation. Market data the bond needs and the
/// market lacks is a not-found error, a share price in another currency a currency
/// mismatch, and a share price or value too large for `Money` a conversion error.
///
/// # References
///
/// - Delta bands: `docs/REFERENCES.md#convertible-delta-bands`
/// - Tree: `docs/REFERENCES.md#tsiveriotis-fernandes-1998`
pub fn convertible_spot_ladder(
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
    spots: Option<&[f64]>,
) -> Result<ConvertibleSpotLadder> {
    bond.validate_for_pricing()?;
    let market_spot = bond.spot_value(market.get_price(&bond.spot_id)?)?;
    let spots = ladder_spots(market_spot, spots)?;
    let bumps = GreekBumps::from(&sensitivities::from_context_or_default(
        &FinstackConfig::default(),
        Some(&bond.metric_pricing_overrides),
    )?);
    let inputs = prepare_for_pricing(bond, market, as_of)?;
    let bond_floor = price_bond_floor(bond, market, as_of)?;
    // A mandatory-variable bond delivers a share count that moves with the stock, so
    // no one ratio is the share exposure the delta bands read.
    let fixed_ratio = match bond.conversion.policy {
        ConversionPolicy::MandatoryVariable { .. } => None,
        _ => bond
            .effective_conversion_ratio()
            .filter(|ratio| *ratio > 0.0),
    };

    let rows = spots
        .into_iter()
        .map(|spot| {
            let moved = inputs.market_at_spot(market, spot)?;
            let price = price_convertible_bond(bond, &moved, as_of)?.amount();
            let delta =
                spot_bump_prices(bond, &moved, &inputs, spot, bumps.spot_bump_decimal, as_of)?
                    .map(|bump| bump.delta())
                    .filter(|delta| delta.is_finite());
            let hedge_ratio = delta
                .zip(fixed_ratio)
                .map(|(delta, ratio)| delta / ratio)
                .filter(|hedge_ratio| hedge_ratio.is_finite());
            let conversion_value = compute_conversion_value(bond, spot)?;
            let conversion_premium = if conversion_value > 0.0 {
                Some(bond.conversion_premium(&moved, price)?).filter(|value| value.is_finite())
            } else {
                None
            };
            Ok(ConvertibleSpotRow {
                spot,
                is_market_spot: spot.to_bits() == market_spot.to_bits(),
                price,
                delta,
                hedge_ratio,
                regime: hedge_ratio.map(ConvertibleRegime::from_hedge_ratio),
                conversion_value,
                parity: bond.parity(&moved)?,
                conversion_premium,
                bond_floor,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ConvertibleSpotLadder {
        instrument_id: bond.id.to_string(),
        as_of,
        currency: bond.notional.currency(),
        market_spot,
        rows,
    })
}

/// The share prices of a ladder: `spots`, or the default multiples of `market_spot`,
/// validated, ascending and with no duplicates.
fn ladder_spots(market_spot: f64, spots: Option<&[f64]>) -> Result<Vec<f64>> {
    let mut spots = match spots {
        Some(spots) => spots.to_vec(),
        None => DEFAULT_SPOT_MULTIPLES
            .iter()
            .map(|multiple| multiple * market_spot)
            .collect(),
    };
    if spots.is_empty() {
        return Err(Error::Validation(
            "convertible spot ladder needs at least one share price".into(),
        ));
    }
    if let Some(bad) = spots.iter().find(|spot| !spot.is_finite() || **spot <= 0.0) {
        return Err(Error::Validation(format!(
            "convertible spot ladder share prices must be finite and positive, got {bad}"
        )));
    }
    spots.sort_by(f64::total_cmp);
    spots.dedup();
    Ok(spots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bond::CashflowSpec;
    use finstack_quant_cashflows::builder::{
        CouponType, FixedCouponSpec, RollRule, ScheduleParams,
    };
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };
    use time::macros::date;

    const AS_OF: Date = date!(2025 - 01 - 01);
    const RATE: f64 = 0.04;
    const VOL: f64 = 0.30;
    const NOTIONAL: f64 = 1_000_000.0;
    const RATIO: f64 = 10_000.0;

    /// Zero-coupon convertible of 1,825 days (5.0 years on ACT/365F), strike 100, no
    /// dividends and no credit: never
    /// converted early, so its value is `notional * DF + ratio * BlackScholesCall`.
    fn bond() -> ConvertibleBond {
        ConvertibleBond::builder()
            .id("LADDER".into())
            .notional(Money::new(NOTIONAL, Currency::USD).expect("notional"))
            .issue_date(AS_OF)
            .maturity(date!(2029 - 12 - 31))
            .discount_curve_id("USD-OIS".into())
            .conversion(ConversionSpec {
                ratio: Some(RATIO),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            })
            .spot_id("STOCK".into())
            .vol_surface_id("STOCK-VOL".into())
            .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: rust_decimal::Decimal::ZERO,
                schedule: ScheduleParams {
                    frequency: Tenor::semi_annual(),
                    day_count: DayCount::Act365F,
                    business_day_convention: BusinessDayConvention::Following,
                    calendar_id: "weekends_only".into(),
                    stub: StubKind::ShortFront,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: false,
                    roll_rule: RollRule::None,
                },
            }))
            .attributes(Default::default())
            .build()
            .expect("bond")
    }

    fn market() -> MarketContext {
        let knots: Vec<(f64, f64)> = [0.0_f64, 1.0, 5.0, 10.0]
            .into_iter()
            .map(|t| (t, (-RATE * t).exp()))
            .collect();
        MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(AS_OF)
                    .knots(knots)
                    .build()
                    .expect("curve"),
            )
            .insert_price(
                "STOCK",
                MarketScalar::Price(Money::new(100.0, Currency::USD).expect("spot")),
            )
            .insert_price("STOCK-VOL", MarketScalar::Unitless(VOL))
    }

    /// Hand-checked by the closed form above at r 4%, vol 30%, T 5 and K 100:
    /// the floor is 1,000,000 * exp(-0.2) = 818,730.75, and `hedge_ratio` is N(d1).
    ///
    /// | spot | N(d1)  | price        | parity | premium | regime           |
    /// |   25 | 0.0759 |   823,223.86 |   0.25 |  2.2929 | busted           |
    /// |   50 | 0.3447 |   874,657.70 |   0.50 |  0.7493 | busted           |
    /// |  100 | 0.7368 | 1,158,348.78 |   1.00 |  0.1583 | balanced         |
    /// |  150 | 0.8921 | 1,571,793.63 |   1.50 |  0.0479 | equity_sensitive |
    /// |  300 | 0.9884 | 3,010,129.09 |   3.00 |  0.0034 | equity_sensitive |
    #[test]
    fn rows_match_the_closed_form_in_every_regime() {
        use ConvertibleRegime::{Balanced, Busted, EquitySensitive};
        let cases = [
            (25.0, 0.0759, 823_223.86, 2.2929, Busted),
            (50.0, 0.3447, 874_657.70, 0.7493, Busted),
            (100.0, 0.7368, 1_158_348.78, 0.1583, Balanced),
            (150.0, 0.8921, 1_571_793.63, 0.0479, EquitySensitive),
            (300.0, 0.9884, 3_010_129.09, 0.0034, EquitySensitive),
        ];
        let spots: Vec<f64> = cases.iter().map(|case| case.0).collect();
        let ladder =
            convertible_spot_ladder(&bond(), &market(), AS_OF, Some(&spots)).expect("ladder");
        assert_eq!(ladder.currency, Currency::USD);
        assert_eq!(ladder.market_spot, 100.0);
        for (row, (spot, n_d1, price, premium, regime)) in ladder.rows.iter().zip(cases) {
            assert_eq!(row.spot, spot);
            assert_eq!(row.is_market_spot, spot == 100.0);
            assert_eq!(row.conversion_value, RATIO * spot);
            assert_eq!(row.parity, RATIO * spot / NOTIONAL);
            // The lattice is within 0.1% of the closed form at 200 steps.
            assert!((row.price - price).abs() < 1e-3 * price, "{row:?}");
            let hedge_ratio = row.hedge_ratio.expect("hedge ratio");
            // A 1% bump spans few lattice nodes: delta is 0.3347 against N(d1) 0.3447
            // at spot 50, where the price itself is within 2 of 874,657.70.
            assert!((hedge_ratio - n_d1).abs() < 0.015, "{row:?}");
            assert_eq!(hedge_ratio, row.delta.expect("delta") / RATIO);
            let row_premium = row.conversion_premium.expect("premium");
            assert!((row_premium - premium).abs() < 2e-3, "{row:?}");
            assert_eq!(row_premium, row.price / row.conversion_value - 1.0);
            assert_eq!(row.regime, Some(regime));
            assert!(row.price >= row.bond_floor);
            assert!((row.bond_floor - 818_730.75).abs() < 0.01, "{row:?}");
        }
    }

    #[test]
    fn market_row_is_what_the_greeks_and_terms_report() {
        let (bond, market) = (bond(), market());
        let ladder = convertible_spot_ladder(&bond, &market, AS_OF, None).expect("ladder");
        let row = ladder
            .rows
            .iter()
            .find(|row| row.is_market_spot)
            .expect("the default ladder holds the market spot");
        let greeks = bond.greeks(&market, AS_OF).expect("greeks");
        assert_eq!(
            row.price,
            price_convertible_bond(&bond, &market, AS_OF)
                .expect("pv")
                .amount()
        );
        assert_eq!(row.delta, Some(greeks.delta));
        assert_eq!(row.parity, bond.parity(&market).expect("parity"));
        assert_eq!(
            row.conversion_premium,
            Some(
                bond.conversion_premium(&market, row.price)
                    .expect("premium")
            )
        );
        assert_eq!(
            row.bond_floor,
            price_bond_floor(&bond, &market, AS_OF).expect("floor")
        );
    }

    /// The floor every row carries is the floor of the bond at that row's share price.
    #[test]
    fn floor_does_not_move_with_the_share_price() {
        let (bond, market) = (bond(), market());
        let moved = market.clone().insert_price(
            "STOCK",
            MarketScalar::Price(Money::new(300.0, Currency::USD).expect("spot")),
        );
        assert_eq!(
            price_bond_floor(&bond, &market, AS_OF).expect("floor"),
            price_bond_floor(&bond, &moved, AS_OF).expect("floor")
        );
    }

    #[test]
    fn default_ladder_runs_from_a_quarter_to_three_times_the_market() {
        let ladder = convertible_spot_ladder(&bond(), &market(), AS_OF, None).expect("ladder");
        let spots: Vec<f64> = ladder.rows.iter().map(|row| row.spot).collect();
        assert_eq!(spots, [25.0, 50.0, 75.0, 100.0, 125.0, 150.0, 200.0, 300.0]);
        assert_eq!(
            ladder.rows.iter().filter(|row| row.is_market_spot).count(),
            1
        );
    }

    #[test]
    fn supplied_spots_sort_and_drop_duplicates() {
        let ladder =
            convertible_spot_ladder(&bond(), &market(), AS_OF, Some(&[150.0, 50.0, 150.0]))
                .expect("ladder");
        let spots: Vec<f64> = ladder.rows.iter().map(|row| row.spot).collect();
        assert_eq!(spots, [50.0, 150.0]);
        assert!(ladder.rows.iter().all(|row| !row.is_market_spot));
    }

    #[test]
    fn rejects_spots_the_tree_cannot_value() {
        for spots in [&[][..], &[0.0], &[-1.0], &[f64::NAN], &[f64::INFINITY]] {
            let error = convertible_spot_ladder(&bond(), &market(), AS_OF, Some(spots))
                .expect_err("rejected");
            assert!(matches!(error, Error::Validation(_)), "{spots:?}: {error}");
        }
    }

    /// A price too large for `Money` errors, so no row carries a non-finite number.
    #[test]
    fn share_price_with_no_representable_value_is_an_error() {
        assert!(convertible_spot_ladder(&bond(), &market(), AS_OF, Some(&[1e300])).is_err());
    }

    #[test]
    fn regime_bands_include_their_balanced_edges() {
        use ConvertibleRegime::{Balanced, Busted, EquitySensitive};
        assert_eq!(ConvertibleRegime::from_hedge_ratio(0.399), Busted);
        assert_eq!(ConvertibleRegime::from_hedge_ratio(0.40), Balanced);
        assert_eq!(ConvertibleRegime::from_hedge_ratio(0.80), Balanced);
        assert_eq!(ConvertibleRegime::from_hedge_ratio(0.801), EquitySensitive);
    }

    #[test]
    fn mandatory_variable_terms_give_no_regime() {
        let mut bond = bond();
        bond.conversion.policy = ConversionPolicy::MandatoryVariable {
            conversion_date: bond.maturity,
            lower_conversion_price: 80.0,
            upper_conversion_price: 120.0,
        };
        let ladder =
            convertible_spot_ladder(&bond, &market(), AS_OF, Some(&[100.0])).expect("ladder");
        let row = &ladder.rows[0];
        assert_eq!(row.hedge_ratio, None);
        assert_eq!(row.regime, None);
        assert_eq!(row.conversion_value, NOTIONAL);
        assert_eq!(row.parity, 1.0);
        assert!(row.delta.is_some());
    }

    #[test]
    fn ladder_round_trips_and_refuses_unknown_fields() {
        let ladder =
            convertible_spot_ladder(&bond(), &market(), AS_OF, Some(&[100.0])).expect("ladder");
        let json = serde_json::to_string(&ladder).expect("json");
        assert_eq!(
            serde_json::from_str::<ConvertibleSpotLadder>(&json).expect("parse"),
            ladder
        );
        let extra = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<ConvertibleSpotLadder>(&extra).is_err());
    }
}
