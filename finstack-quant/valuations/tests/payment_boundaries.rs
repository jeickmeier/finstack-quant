use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::{deposit::Deposit, repo::Repo};
use finstack_quant_valuations::metrics::collect_period_cash;

const CASH_TOLERANCE: f64 = 1e-6;

fn flat_discount(id: &str, as_of: Date) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots([(0.0, 1.0), (30.0, 1.0)])
        .build()
        .expect("zero curve")
}

fn assert_cash_conservation(
    instrument: &dyn Instrument,
    market: &MarketContext,
    start: Date,
    end: Date,
) {
    assert_cash_conservation_on(instrument, market, market, start, end);
}

fn assert_cash_conservation_on(
    instrument: &dyn Instrument,
    opening_market: &MarketContext,
    closing_market: &MarketContext,
    start: Date,
    end: Date,
) {
    let opening = instrument
        .value(opening_market, start)
        .expect("opening value");
    let closing = instrument
        .value(closing_market, end)
        .expect("closing value");
    let cash = collect_period_cash(instrument, opening_market, start, end, opening.currency())
        .expect("period cash");
    let residual = opening.amount() - closing.amount() - cash.total.amount();
    assert!(
        residual.abs() < CASH_TOLERANCE,
        "{} from {start} to {end} has cash conservation residual {residual}",
        instrument.key()
    );
}

fn assert_payment_boundary(instrument: &dyn Instrument, market: &MarketContext, payment: Date) {
    let previous = payment.previous_day().expect("day before payment");
    let next = payment.next_day().expect("day after payment");
    let (start, end) = if instrument.includes_valuation_date_cashflows() {
        (payment, next)
    } else {
        (previous, payment)
    };
    let currency = instrument
        .value(market, start)
        .expect("opening value")
        .currency();
    let period_cash =
        collect_period_cash(instrument, market, start, end, currency).expect("payment-date cash");
    assert_ne!(
        period_cash.total.amount(),
        0.0,
        "{} has no payment",
        instrument.key()
    );
    assert_cash_conservation(instrument, market, previous, payment);
    assert_cash_conservation(instrument, market, payment, next);
}

#[test]
fn holder_value_and_period_cash_use_one_payment_boundary() {
    let deposit = Deposit::example().expect("deposit");
    let repo = Repo::example().expect("repo");
    let cases: [(&dyn Instrument, &str); 2] = [
        (&deposit, deposit.discount_curve_id.as_str()),
        (&repo, repo.discount_curve_id.as_str()),
    ];
    for (instrument, curve) in cases {
        let maturity = instrument.expiry().expect("maturity");
        let previous = maturity.previous_day().expect("previous day");
        let next = maturity.next_day().expect("next day");
        let market = MarketContext::new()
            .try_insert(flat_discount(curve, previous))
            .expect("market");
        for (start, end) in [(previous, maturity), (maturity, next)] {
            assert_cash_conservation(instrument, &market, start, end);
        }
    }
}

#[test]
fn fra_drops_settled_cash_and_fx_trades_keep_cash_due_today() {
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_valuations::instruments::{
        fx::{fx_forward::FxForward, fx_spot::FxSpot, fx_swap::FxSwap, ndf::Ndf},
        rates::fra::ForwardRateAgreement,
    };
    use std::sync::Arc;

    let mut fra = ForwardRateAgreement::example().expect("FRA");
    fra.observed_fixing = Some(0.0);
    let previous = fra.start_date.previous_day().expect("day before FRA cash");
    let next = fra.start_date.next_day().expect("day after FRA cash");
    let fra_market = MarketContext::new()
        .try_insert(flat_discount(fra.discount_curve_id.as_str(), previous))
        .expect("FRA market");
    for (start, end) in [(previous, fra.start_date), (fra.start_date, next)] {
        assert_cash_conservation(&fra, &fra_market, start, end);
    }

    let forward = FxForward::example().expect("FX forward");
    let swap = FxSwap::example().expect("FX swap");
    let mut spot = FxSpot::example().expect("FX spot");
    let mut ndf = Ndf::example().expect("NDF");
    spot.settlement_date = Some(swap.near_date);
    ndf.observed_fixing = Some(7.40);
    let base = swap.near_date.previous_day().expect("base date");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(forward.base_currency, forward.quote_currency, 1.20)])
        .expect("constant FX");
    let fx_market = MarketContext::new()
        .try_insert(flat_discount(
            forward.domestic_discount_curve_id.as_str(),
            base,
        ))
        .expect("domestic discount")
        .try_insert(flat_discount(
            forward.foreign_discount_curve_id.as_str(),
            base,
        ))
        .expect("foreign discount")
        .insert_fx(FxMatrix::new(provider, None));
    let cases: [(&dyn Instrument, Date); 5] = [
        (&spot, swap.near_date),
        (&swap, swap.near_date),
        (&swap, swap.far_date),
        (&forward, forward.maturity),
        (&ndf, ndf.maturity),
    ];
    for (instrument, payment) in cases {
        let previous = payment.previous_day().expect("day before cash");
        let next = payment.next_day().expect("day after cash");
        for (start, end) in [(previous, payment), (payment, next)] {
            assert_cash_conservation(instrument, &fx_market, start, end);
        }
    }
}

#[test]
fn cds_and_index_use_the_same_election_for_coupon_and_upfront() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::credit_derivatives::{
        cds::CreditDefaultSwap, cds_index::CdsIndex,
    };
    use finstack_quant_valuations::market::conventions::CdsValuationConvention;
    use time::macros::date;

    let payment = date!(2029 - 03 - 20);
    let before = payment.previous_day().unwrap();
    let after = payment.next_day().unwrap();
    let mut cds = CreditDefaultSwap::example().expect("cds");
    cds.valuation_convention = CdsValuationConvention::IsdaDirty;
    let mut index = CdsIndex::example().expect("index");
    index.premium_leg.end = cds.premium_leg.end;
    let upfront = Money::new(125.0, cds.notional.currency()).expect("upfront");
    cds.upfront = Some((payment, upfront));
    index.upfront = Some((payment, upfront));
    let mut market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", before))
        .expect("market");
    for protection in [&cds.protection_leg, &index.protection_leg] {
        let hazard = HazardCurve::builder(protection.credit_curve_id.clone())
            .base_date(before)
            .day_count(DayCount::Act365F)
            .recovery_rate(protection.recovery_rate)
            .knots([(5.0, 0.0)])
            .build()
            .expect("zero hazard");
        market.try_insert_mut(hazard).expect("hazard insert");
    }
    for include_today in [false, true] {
        cds.include_valuation_date_cashflows = include_today;
        index.include_valuation_date_cashflows = include_today;
        for instrument in [&cds as &dyn Instrument, &index] {
            let value = instrument.value(&market, payment).expect("payment-date PV");
            if include_today {
                assert!(
                    value.amount() < -upfront.amount(),
                    "coupon and upfront are due"
                );
            } else {
                assert_eq!(value.amount(), 0.0, "both payments are settled");
            }
            assert_eq!(
                instrument.includes_valuation_date_cashflows(),
                include_today
            );
            for (start, end) in [(before, payment), (payment, after)] {
                assert_cash_conservation(instrument, &market, start, end);
            }
        }
    }
}

#[test]
fn bonds_loans_swaps_and_funds_drop_cash_on_the_payment_date() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::{
        MarketScalar, ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_valuations::instruments::{
        equity::{equity_trs::EquityTotalReturnSwap, pe_fund::PrivateMarketsFund},
        fixed_income::{
            bond::Bond,
            fi_trs::FiIndexTotalReturnSwap,
            revolving_credit::{DrawRepaySpec, RevolvingCredit},
            term_loan::TermLoan,
        },
        rates::{basis_swap::BasisSwap, irs::InterestRateSwap, xccy_swap::XccySwap},
    };
    use std::sync::Arc;
    use time::{Duration, macros::date};

    let base = date!(2024 - 01 - 01);
    let quotes = Arc::new(SimpleFxProvider::new());
    quotes
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("constant FX");
    let mut market = MarketContext::new()
        .insert_fx(FxMatrix::new(quotes, None))
        .try_insert(flat_discount("USD-TREASURY", base))
        .expect("Treasury curve")
        .try_insert(flat_discount("USD-OIS", base))
        .expect("USD discount curve")
        .try_insert(flat_discount("EUR-OIS", base))
        .expect("EUR discount curve");
    for index in ["USD-SOFR-1M", "USD-SOFR-3M", "EUR-EURIBOR-3M"] {
        let forward = ForwardCurve::flat(index, base, 0.0, DayCount::Act360).expect("zero forward");
        market.try_insert_mut(forward).expect("forward insert");
        let observations = (-7..=200)
            .map(|offset| (base + Duration::days(offset), 0.0))
            .collect();
        let series = ScalarTimeSeries::new(
            index,
            observations,
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("fixings");
        market
            .insert_fixing(index, series)
            .expect("registered fixings");
    }

    let bond = Bond::example().expect("bond");
    let basis = BasisSwap::example().expect("basis swap");
    let irs = InterestRateSwap::example().expect("IRS");
    let cross_currency = XccySwap::example().expect("cross-currency swap");
    let loan = TermLoan::example().expect("term loan");
    let mut revolver = RevolvingCredit::example().expect("revolving facility");
    revolver.draw_repay_spec = DrawRepaySpec::Deterministic(Vec::new());
    let fund = PrivateMarketsFund::example().expect("private markets fund");
    let payment = date!(2024 - 04 - 01);
    let mut equity_trs = EquityTotalReturnSwap::example().expect("equity TRS");
    equity_trs.underlying.div_yield_id = None;
    equity_trs.initial_level = Some(100.0);
    equity_trs.past_fixings = vec![(payment, 100.0)];
    let mut fi_trs = FiIndexTotalReturnSwap::example().expect("fixed-income TRS");
    fi_trs.fixing_series_id = Some("INDEX-LEVEL".into());
    let index_levels = ScalarTimeSeries::new(
        "INDEX-LEVEL",
        vec![(base, 100.0), (payment, 100.0)],
        None,
        SeriesInterpolation::Step,
        SeriesExtrapolation::Error,
    )
    .expect("fixed-income index history");
    let market = market
        .insert_price(
            "SPX-SPOT",
            MarketScalar::unitless(100.0).expect("equity spot"),
        )
        .insert_price(
            "US-CORP-INDEX",
            MarketScalar::unitless(100.0).expect("index level"),
        )
        .insert_price(
            "US-CORP-YIELD",
            MarketScalar::unitless(0.0).expect("zero index yield"),
        )
        .insert_series(index_levels);
    let cases: [(&dyn Instrument, Date); 9] = [
        (&bond, date!(2024 - 07 - 15)),
        (&basis, date!(2024 - 04 - 03)),
        (&irs, date!(2024 - 07 - 02)),
        (&cross_currency, date!(2024 - 04 - 03)),
        (&loan, date!(2024 - 04 - 01)),
        (&revolver, date!(2024 - 04 - 01)),
        (&fund, date!(2024 - 06 - 15)),
        (&equity_trs, payment),
        (&fi_trs, payment),
    ];
    for (instrument, payment) in cases {
        assert_payment_boundary(instrument, &market, payment);
    }
}

#[test]
fn inflation_instruments_drop_cash_on_the_payment_date() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DateExt, DayCount};
    use finstack_quant_core::market_data::scalars::{InflationIndex, InflationObservation};
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_valuations::instruments::{
        fixed_income::inflation_linked_bond::InflationLinkedBond,
        rates::inflation_swap::{InflationSwap, YoYInflationSwap},
    };
    use time::macros::date;

    let base = date!(2023 - 01 - 01);
    let observations = (0..=135)
        .map(|offset| {
            let reference_month = base.add_months(offset).expect("CPI reference month");
            InflationObservation {
                reference_month,
                available_on: reference_month.add_months(1).expect("publication month"),
                value: 100.0,
            }
        })
        .collect();
    let history = InflationIndex::new("US-CPI", observations, Currency::USD, date!(2034 - 05 - 01))
        .expect("flat CPI observations");
    let projection = InflationCurve::builder("US-CPI")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (30.0, 100.0)])
        .build()
        .expect("flat CPI curve");
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", base))
        .expect("discount curve")
        .try_insert(projection)
        .expect("CPI curve")
        .insert_inflation_index(history);

    let bond = InflationLinkedBond::example().expect("inflation-linked bond");
    let swap = InflationSwap::example().expect("zero-coupon inflation swap");
    let yoy = YoYInflationSwap::example().expect("year-on-year inflation swap");
    let cases: [(&dyn Instrument, Date); 3] = [
        (&bond, date!(2024 - 07 - 15)),
        (&swap, swap.maturity),
        (&yoy, yoy.maturity),
    ];
    for (instrument, payment) in cases {
        assert_payment_boundary(instrument, &market, payment);
    }
}

#[test]
fn cms_swap_drops_a_fixed_coupon_on_its_payment_date() {
    use finstack_quant_core::market_data::scalars::{
        ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_valuations::instruments::rates::cms_swap::CmsSwap;
    use time::macros::date;

    let cms = CmsSwap::example().expect("CMS");
    let payment = date!(2026 - 03 - 20);
    let prior_fixing = date!(2025 - 12 - 22);
    let surface = VolSurface::builder(cms.vol_surface_id.as_str())
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[1.0, 2.0])
        .secondary_values(&[0.01, 0.05])
        .row(&[0.25, 0.25])
        .row(&[0.25, 0.25])
        .build()
        .expect("CMS volatility");
    let mut market = MarketContext::new()
        .try_insert(flat_discount(cms.discount_curve_id.as_str(), prior_fixing))
        .expect("discount curve")
        .insert_surface(surface);
    let fixings = ScalarTimeSeries::new(
        "USD-CMS-10Y",
        vec![(prior_fixing, 0.02)],
        None,
        SeriesInterpolation::Step,
        SeriesExtrapolation::Error,
    )
    .expect("CMS fixings");
    market
        .insert_fixing("USD-CMS-10Y", fixings)
        .expect("CMS fixing registration");
    assert_payment_boundary(&cms, &market, payment);
}

#[test]
fn cds_tranche_keeps_earned_coupon_cash_due_today() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{
        BaseCorrelationCurve, CreditIndexData, HazardCurve,
    };
    use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche;
    use std::sync::Arc;
    use time::macros::date;

    let mut tranche = CdsTranche::example().expect("CDS tranche");
    tranche.start_date = Some(date!(2029 - 03 - 20));
    let payment = date!(2029 - 06 - 20);
    let previous = payment.previous_day().expect("day before coupon");
    let next = payment.next_day().expect("day after coupon");
    let hazard = HazardCurve::builder("INDEX-HAZARD")
        .base_date(previous)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.40)
        .knots([(5.0, 0.0)])
        .build()
        .expect("zero hazard");
    let correlation = BaseCorrelationCurve::builder("INDEX-CORRELATION")
        .knots([(3.0, 0.25), (7.0, 0.25)])
        .build()
        .expect("constant base correlation");
    let index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(hazard.clone()))
        .base_correlation_curve(Arc::new(correlation.clone()))
        .build()
        .expect("credit index");
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", previous))
        .expect("discount curve")
        .try_insert(hazard)
        .expect("hazard curve")
        .try_insert(correlation)
        .expect("correlation curve")
        .insert_credit_index(tranche.credit_index_id.as_str(), index)
        .expect("credit index registration");
    let same_day = market.condition(payment).expect("condition on coupon date");
    let after = market.condition(next).expect("condition on following day");
    let cash = collect_period_cash(
        &tranche,
        &same_day,
        payment,
        next,
        tranche.notional.currency(),
    )
    .expect("coupon cash");
    assert_ne!(cash.total.amount(), 0.0);
    assert_cash_conservation_on(&tranche, &market, &same_day, previous, payment);
    assert_cash_conservation_on(&tranche, &same_day, &after, payment, next);
}

#[test]
fn mbs_pool_last_payment_is_settled_in_the_public_schedule() {
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
    use time::macros::date;

    let mut pool = AgencyMbsPassthrough::example().expect("MBS");
    pool.wam_months = 1;
    let payment = pool
        .payment_date_for_accrual_period(date!(2024 - 01 - 01))
        .expect("agency remittance date");
    let previous = payment.previous_day().expect("day before remittance");
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", previous))
        .expect("discount curve");
    assert_cash_conservation(&pool, &market, previous, payment);
    assert!(
        pool.cashflow_schedule(&market, payment)
            .expect("public schedule")
            .get_flows()
            .is_empty()
    );
}

#[test]
fn agency_cmo_rolls_the_collateral_and_tranche_balances_after_payment() {
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::{
        cmo::{AgencyCmo, CmoTranche, CmoWaterfall},
        mbs_passthrough::{AgencyMbsPassthrough, pricer::generate_cashflows},
    };
    use time::macros::date;

    let mut pool = AgencyMbsPassthrough::example().expect("collateral pool");
    pool.wam_months = 12;
    let payment = pool
        .payment_date_for_accrual_period(date!(2024 - 01 - 01))
        .expect("remittance date");
    let previous = payment.previous_day().expect("day before remittance");
    let next = payment.next_day().expect("day after remittance");
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", previous))
        .expect("discount curve");

    let mut cmo = AgencyCmo::example().expect("agency CMO");
    let mut tranche = CmoTranche::sequential("A", pool.original_face, pool.coupon, 1);
    tranche.current_face = pool.current_face;
    cmo.waterfall = CmoWaterfall::new(vec![tranche]);
    cmo.collateral = Some(Box::new(pool.clone()));
    assert_cash_conservation(&cmo, &market, previous, payment);

    let opening = cmo.value(&market, payment).expect("PV before remittance");
    let cash = collect_period_cash(&cmo, &market, payment, next, opening.currency())
        .expect("remittance cash");
    assert!(cash.total.amount() > 0.0);
    let projection = generate_cashflows(&pool, payment, Some(1)).expect("remittance projection");
    let closing_face = Money::new(projection[0].ending_balance, opening.currency())
        .expect("balance after payment");
    pool.current_face = closing_face;
    pool.wam_months -= 1;
    pool.last_paid_accrual_end = Some(date!(2024 - 01 - 31));
    cmo.waterfall.tranches[0].current_face = closing_face;
    cmo.collateral = Some(Box::new(pool));
    let closing = cmo.value(&market, next).expect("PV after remittance");
    assert!((opening.amount() - closing.amount() - cash.total.amount()).abs() < CASH_TOLERANCE);
}

#[test]
fn deterministic_structured_credit_and_facility_drop_settled_distributions() {
    use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations::instruments::fixed_income::{
        asset_backed_facility::AssetBackedFacility,
        structured_credit::{StructuredCredit, StructuredCreditTranche},
    };

    let mut deal = StructuredCredit::example().expect("CLO");
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    let tranche_id = deal.tranches.tranches[0].id.to_string();
    let tranche =
        StructuredCreditTranche::new(InstrumentId::new("CLO-NOTE"), deal.clone(), tranche_id)
            .expect("held note");
    let mut facility = AssetBackedFacility::example().expect("warehouse facility");
    facility.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    facility.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", deal.closing_date))
        .expect("discount curve");

    let cases: [(&dyn Instrument, Date); 3] = [
        (&deal, deal.closing_date),
        (&tranche, deal.closing_date),
        (&facility, facility.closing_date),
    ];
    for (instrument, closing) in cases {
        let schedule = instrument
            .cashflow_schedule(&market, closing)
            .expect("projected payments");
        let payment = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.get_date() > closing && flow.get_amount().amount() > 0.0)
            .expect("first distribution")
            .get_date();
        assert_payment_boundary(instrument, &market, payment);
    }
}

#[test]
fn commodity_forward_and_swap_keep_cash_due_today() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};
    use finstack_quant_valuations::instruments::commodity::{
        commodity_forward::CommodityForward, commodity_swap::CommoditySwap,
    };
    use time::{Duration, macros::date};

    let base = date!(2025 - 01 - 01);
    let mut forward = CommodityForward::example().expect("commodity forward");
    forward.quoted_forward = Some(75.0);
    forward.contract_price = Some(72.0);
    let mut swap = CommoditySwap::example().expect("commodity swap");
    swap.past_fixings = (0..365)
        .map(|offset| (base + Duration::days(offset), 4.0))
        .collect();
    let price = PriceCurve::builder(swap.forward_curve_id.as_str())
        .kind(PriceCurveKind::Price)
        .day_count(DayCount::Act365F)
        .base_date(base)
        .knots([(0.0, 4.0), (1.0, 4.0)])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .build()
        .expect("flat commodity price curve");
    let market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", base))
        .expect("discount curve")
        .try_insert(price)
        .expect("commodity curve");
    let cases: [(&dyn Instrument, Date); 2] = [
        (&forward, forward.payment_date().expect("forward payment")),
        (&swap, swap.payment_schedule().expect("swap schedule")[0]),
    ];
    for (instrument, payment) in cases {
        assert_payment_boundary(instrument, &market, payment);
    }
}

#[test]
fn held_real_estate_keeps_net_operating_income_due_today() {
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::equity::real_estate::{
        LeveredRealEstateEquity, RealEstateAsset,
    };
    use time::macros::date;

    let mut property = RealEstateAsset::example().expect("commercial property");
    property.discount_rate = Some(0.0);
    let mut equity = LeveredRealEstateEquity::example().expect("real estate equity");
    equity.asset = property.clone();
    equity.asset.purchase_price = Some(Money::zero(equity.currency));
    for instrument in [&property as &dyn Instrument, &equity] {
        assert_payment_boundary(instrument, &MarketContext::new(), date!(2026 - 01 - 01));
    }
}

#[test]
fn dated_dcf_reconciles_a_paid_receipt_when_its_forecast_rolls() {
    use finstack_quant_valuations::instruments::equity::dcf_equity::{
        DiscountedCashFlow, ForecastTiming, TerminalValueSpec,
    };

    let mut opening = DiscountedCashFlow::example().expect("DCF");
    let payment = opening.flows[0].0;
    let previous = payment.previous_day().expect("day before receipt");
    let next = payment.next_day().expect("day after receipt");
    opening.valuation_date = previous;
    opening.wacc = 0.0;
    opening.forecast_timing = ForecastTiming::DatedReceipts;
    opening.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(8_000_000.0),
        multiple: 10.0,
    };
    opening.normalized_annual_fcff = None;
    let mut closing = opening.clone();
    closing.valuation_date = next;
    closing.flows.remove(0);
    let market = MarketContext::new();
    let cash = collect_period_cash(&opening, &market, previous, next, opening.currency)
        .expect("settled receipt");
    assert_eq!(cash.total, opening.flows[0].1);
    let start_value = opening
        .value(&market, previous)
        .expect("opening forecast PV");
    let end_value = closing.value(&market, next).expect("rolled forecast PV");
    assert!(
        (start_value.amount() - end_value.amount() - cash.total.amount()).abs() < CASH_TOLERANCE
    );
}

#[test]
fn live_convertible_drops_the_scheduled_coupon_on_payment_date() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
    use time::macros::date;

    let mut bond = ConvertibleBond::example().expect("convertible");
    bond.credit_curve_id = None;
    bond.recovery_rate = None;
    let surface = VolSurface::builder(bond.vol_surface_id.as_str())
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[1.0, 10.0])
        .secondary_values(&[50.0, 150.0])
        .row(&[0.01, 0.01])
        .row(&[0.01, 0.01])
        .build()
        .expect("out-of-the-money conversion volatility");
    let market = MarketContext::new()
        .try_insert(flat_discount(
            bond.discount_curve_id.as_str(),
            bond.issue_date,
        ))
        .expect("discount curve")
        .insert_surface(surface)
        .insert_price(
            bond.spot_id.as_str(),
            MarketScalar::unitless(100.0).expect("equity spot"),
        );
    assert_payment_boundary(&bond, &market, date!(2024 - 07 - 15));
}
