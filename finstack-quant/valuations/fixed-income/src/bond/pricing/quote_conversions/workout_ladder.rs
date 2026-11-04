//! Yield and I-spread to every admissible exit of a bond at one market price.
//!
//! [`bond_workout_ladder`] walks the same exits and solves the same yields as
//! the `ytw` metric, through
//! [`solve_workout_exits`](super::yield_price::solve_workout_exits), and keeps
//! each exit instead of discarding all but the worst, so the two cannot
//! disagree.

use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
use finstack_quant_valuations_core::instruments::Instrument;
use serde::{Deserialize, Serialize};

use super::yield_price::solve_workout_exits;
use super::{curve_par_rate, i_spread_horizon};
use crate::bond::Bond;
use crate::bond::pricing::settlement::{QuoteDateContext, model_dirty_at_quote_date};

/// How the holder leaves the bond on a [`BondWorkoutRow`]'s date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkoutExitKind {
    /// The issuer calls the bond; a lowered return floor also appears as a call.
    Call,
    /// The holder puts the bond.
    Put,
    /// The bond runs to its final contractual payment.
    Maturity,
}

impl WorkoutExitKind {
    /// Wire name of the exit kind (`call`, `put` or `maturity`), the serde spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Put => "put",
            Self::Maturity => "maturity",
        }
    }
}

/// One admissible exit of a bond: a row of a [`BondWorkoutLadder`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondWorkoutRow {
    /// Exit date. A maturity row carries the business-day-adjusted final payment
    /// date when that is later than the contractual maturity.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Whether the exit is a call, a put or maturity.
    pub kind: WorkoutExitKind,
    /// Clean redemption as percent of the principal outstanding at the exit
    /// (`103.0` = 103%). A make-whole call shows its make-whole price, which can
    /// exceed the contractual strike.
    pub redemption_price_pct: f64,
    /// Clean cash the exit redeems, in the bond's currency; accrued interest is
    /// paid on top.
    pub redemption_amount: Money,
    /// Whether the redemption is a make-whole price off a reference curve.
    pub make_whole: bool,
    /// Street yield to this exit as a decimal, compounded at the coupon
    /// frequency, timed from the quote date. `None` when the path has no finite
    /// yield: an exit on the quote date, or same-day cash at or above the price. A
    /// worst row can be `None`, where the `ytw` metric is negative infinity.
    pub yield_rate: Option<f64>,
    /// `yield_rate` less the swap par rate from the quote date, as a decimal, the
    /// convention of the `i_spread` metric. The par rate runs to `date` when the
    /// bond has an exit other than maturity, and to contractual maturity when it
    /// has none, which differs from `date` if the final payment rolls. `None` when
    /// `yield_rate` is `None`.
    pub i_spread: Option<f64>,
    /// Whether this is the worst exit, the one the `ytw` metric selects. At most
    /// one row is marked; none is when no exit has a yield and each pays at least
    /// the price at once.
    pub is_worst: bool,
}

/// Yield and spread to every exit of a bond at one market price; the return
/// value of [`bond_workout_ladder`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondWorkoutLadder {
    /// Identifier of the bond.
    pub instrument_id: String,
    /// Valuation date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Settlement date of the quote; every yield is timed from it.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub quote_date: Date,
    /// Clean price every row is solved at, as percent of the original notional:
    /// the quoted clean price, or for a bond with no exercise rights and no quote
    /// the model price at the quote date.
    pub clean_price_pct: f64,
    /// `clean_price_pct` plus accrued interest at the quote date, in the bond's
    /// currency. Exits are alternatives for one price, so the rows share it.
    pub dirty_price: Money,
    /// One row per candidate exit, in date order. Adjacent call windows that share
    /// a boundary date each keep a row; a put and a call on one date each keep
    /// theirs. A bond with no exercise rights has one maturity row.
    pub rows: Vec<BondWorkoutRow>,
}

impl BondWorkoutLadder {
    /// Export one row per exit, in ladder order, as a [`TableEnvelope`].
    ///
    /// Columns: `date` (ISO string), `kind` (`call`, `put` or `maturity`),
    /// `redemption_price_pct` (float), `redemption_amount` (float, in
    /// `currency`), `currency` (ISO code), `make_whole` (bool), `yield_rate`
    /// and `i_spread` (nullable float, decimal) and `is_worst` (bool). The
    /// columns remain when there are no rows.
    #[must_use]
    pub fn to_table(&self) -> TableEnvelope {
        let rows = &self.rows;
        TableEnvelope {
            row_count: rows.len(),
            columns: vec![
                TableColumn::new(
                    "date",
                    TableColumnData::String(rows.iter().map(|r| r.date.to_string()).collect()),
                ),
                TableColumn::new(
                    "kind",
                    TableColumnData::String(
                        rows.iter().map(|r| r.kind.as_str().to_string()).collect(),
                    ),
                ),
                TableColumn::new(
                    "redemption_price_pct",
                    TableColumnData::Float64(rows.iter().map(|r| r.redemption_price_pct).collect()),
                ),
                TableColumn::new(
                    "redemption_amount",
                    TableColumnData::Float64(
                        rows.iter().map(|r| r.redemption_amount.amount()).collect(),
                    ),
                ),
                TableColumn::new(
                    "currency",
                    TableColumnData::String(
                        rows.iter()
                            .map(|r| r.redemption_amount.currency().to_string())
                            .collect(),
                    ),
                ),
                TableColumn::new(
                    "make_whole",
                    TableColumnData::Boolean(rows.iter().map(|r| r.make_whole).collect()),
                ),
                TableColumn::new(
                    "yield_rate",
                    TableColumnData::NullableFloat64(rows.iter().map(|r| r.yield_rate).collect()),
                ),
                TableColumn::new(
                    "i_spread",
                    TableColumnData::NullableFloat64(rows.iter().map(|r| r.i_spread).collect()),
                ),
                TableColumn::new(
                    "is_worst",
                    TableColumnData::Boolean(rows.iter().map(|r| r.is_worst).collect()),
                ),
            ],
            metadata: Default::default(),
        }
    }
}

/// Dirty price yield-to-worst is solved to, in the bond's currency.
///
/// The quoted clean price plus accrued at the quote date. A bond with exercise
/// rights needs that quote, because model value already embeds a view on
/// exercise; one with no rights falls back to its model price at the quote date.
///
/// # Arguments
///
/// * `bond` - Bond whose quote, notional and exercise rights decide the target.
/// * `curves` - Market context used to carry a model value to the quote date.
/// * `as_of` - Valuation date the model value is anchored at.
/// * `quote_ctx` - Quote date and the accrued interest there.
/// * `model_pv` - Model present value at `as_of` in currency, evaluated only
///   for a bond with no quote and no exercise rights.
pub(crate) fn workout_dirty_target(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_ctx: &QuoteDateContext,
    model_pv: impl FnOnce() -> Result<f64>,
) -> Result<f64> {
    match bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct
    {
        Some(clean_px) => Ok(quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount())),
        None if !bond.has_exercise_rights() => {
            model_dirty_at_quote_date(bond, curves, as_of, quote_ctx.quote_date, model_pv()?)
        }
        None => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: "bond.instrument_pricing_overrides.market_quotes.quoted_clean_price_pct"
                    .to_string(),
            },
        )),
    }
}

/// Yield and I-spread to every call, put and maturity exit of a bond at one price.
///
/// Answers which call date is the worst workout and what the yield to each is.
/// Exits are the ones the `ytw` metric walks, a return floor lowered into calls
/// first, and the row marked worst holds that metric's yield and the `i_spread`
/// metric's spread. Z-spread is not reported per exit: the workout path that
/// metric discounts is the worst one only, and no per-exit convention exists.
///
/// # Arguments
///
/// * `bond` - Bond to walk. A bond with exercise rights must carry
///   `quoted_clean_price_pct`; one with none falls back to its model price.
/// * `market` - Market context holding the bond's discount curve (the source of
///   the swap par rates), any floating index and make-whole reference curves.
/// * `as_of` - Valuation date; the quote date and coupon entitlement follow the
///   bond's settlement convention.
///
/// # Returns
///
/// A [`BondWorkoutLadder`] with one row per exit in date order.
///
/// # Errors
///
/// Returns `InputError::NotFound` when a bond with exercise rights has no quoted
/// clean price, as the `ytw` metric does. Also fails when a curve the bond needs
/// is missing, or a yield cannot be solved.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
/// use finstack_quant_valuations_fixed_income::bond::pricing::quote_conversions::bond_workout_ladder;
/// use finstack_quant_valuations_fixed_income::bond::Bond;
/// use time::macros::date;
///
/// let as_of = date!(2025 - 01 - 15);
/// let mut bond = Bond::example_callable()?;
/// bond.instrument_pricing_overrides =
///     InstrumentPricingOverrides::default().with_quoted_clean_price_pct(108.0);
/// let curve = DiscountCurve::builder(bond.discount_curve_id.as_str())
///     .base_date(as_of)
///     .knots([(0.0, 1.0), (10.0, 0.70)])
///     .build()?;
/// let market = MarketContext::new().insert(curve);
///
/// let ladder = bond_workout_ladder(&bond, &market, as_of)?;
/// assert_eq!(ladder.rows.iter().filter(|row| row.is_worst).count(), 1);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn bond_workout_ladder(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
) -> Result<BondWorkoutLadder> {
    let quote_ctx = QuoteDateContext::new(bond, market, as_of)?;
    let quote_date = quote_ctx.quote_date;
    let flows = quote_ctx.entitled_flows(bond, market, as_of)?;
    let notional = bond.notional.amount();
    let currency = bond.notional.currency();
    let quoted_clean = bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct;
    let dirty = workout_dirty_target(bond, market, as_of, &quote_ctx, || {
        Ok(Instrument::value(bond, market, as_of)?.amount())
    })?;

    let effective_bond = bond.effective_for_pricing(market, as_of)?;
    let schedule = effective_bond.full_cashflow_schedule(market)?;
    let dirty_price = Money::new(dirty, currency)?;
    let (exits, worst) = solve_workout_exits(
        &effective_bond,
        market,
        &flows,
        quote_date,
        dirty_price,
        &schedule,
    )?;

    let disc = market.get_discount(&bond.discount_curve_id)?;
    let has_workout_path = effective_bond
        .call_put
        .as_ref()
        .is_some_and(|schedule| schedule.has_options());
    let mut rows = Vec::with_capacity(exits.len());
    for (index, exit) in exits.iter().enumerate() {
        let date = exit.path.exit.date;
        let yield_rate = exit.yield_rate.is_finite().then_some(exit.yield_rate);
        let i_spread = match yield_rate {
            Some(rate) if date > quote_date => {
                let horizon = i_spread_horizon(bond, has_workout_path.then_some(date));
                Some(rate - curve_par_rate(bond, disc.as_ref(), quote_date, horizon)?)
            }
            _ => None,
        };
        rows.push(BondWorkoutRow {
            date,
            kind: exit.path.exit.kind,
            redemption_price_pct: exit.path.redemption.price_pct,
            redemption_amount: Money::new(exit.path.redemption.clean_amount, currency)?,
            make_whole: exit.path.exit.make_whole.is_some(),
            yield_rate,
            i_spread,
            is_worst: worst == Some(index),
        });
    }
    rows.sort_by_key(|row| row.date);

    Ok(BondWorkoutLadder {
        instrument_id: bond.id.as_str().to_string(),
        as_of,
        quote_date,
        clean_price_pct: quoted_clean
            .unwrap_or((dirty - quote_ctx.accrued_at_quote_date) / notional * 100.0),
        dirty_price,
        rows,
    })
}
