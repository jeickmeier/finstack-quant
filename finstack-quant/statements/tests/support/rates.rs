use finstack_quant_core::{
    dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor},
    money::Money,
    types::{CurveId, InstrumentId},
};
use finstack_quant_valuations::instruments::rates::irs::{
    FloatingLegCompounding, InterestRateSwap,
};
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec, PayReceive};
use rust_decimal::Decimal;

pub fn usd_irs_swap(
    id: InstrumentId,
    notional: Money,
    fixed_rate: f64,
    start: Date,
    end: Date,
    side: PayReceive,
) -> finstack_quant_core::Result<InterestRateSwap> {
    let rate_decimal = Decimal::try_from(fixed_rate).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "Invalid fixed rate: {} cannot be converted to Decimal. \
             Check for NaN, infinity, or values exceeding Decimal range.",
            fixed_rate
        ))
    })?;

    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: rate_decimal,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::None,
        start,
        end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    };

    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start,
        end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };

    let swap = InterestRateSwap::builder()
        .id(id)
        .notional(notional)
        .side(side)
        .fixed(fixed)
        .float(float)
        .build()?;

    swap.validate()?;
    Ok(swap)
}
