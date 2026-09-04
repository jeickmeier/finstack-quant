"""Typed rates / fixed-income builder ergonomics.

Covers the B1a remediation slice: ``InterestRateSwap.from_conventions``,
``FloatLegSpec.reset_lag_days`` defaulting to ``0`` (spot fixing) so a swap
starting on the valuation date prices off forwards without fixings, the
``compounding`` keyword, ``Bond.builder`` with a credit curve priced under
``hazard_rate`` / ``rates_credit``, typed getters, leg-spec pickling, ``float | Rate`` and
ISO-string date acceptance, and builder ``__repr__``.
"""

from __future__ import annotations

import datetime
import json
import pickle

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    HazardCurve,
    MarketContext,
)
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import (
    Bond,
    BondBuilder,
    CapFloor,
    CapFloorBuilder,
    FixedLegSpec,
    FloatLegSpec,
    InterestRateSwap,
    InterestRateSwapBuilder,
    PremiumLegSpec,
    ProtectionLegSpec,
    Swaption,
    SwaptionBuilder,
    TermLoan,
    TermLoanBuilder,
    list_models_grouped,
    price_instrument,
)

AS_OF = datetime.date(2025, 1, 15)
END = datetime.date(2030, 1, 15)


def _market() -> MarketContext:
    knots = [(0.0, 0.04), (10.0, 0.045)]
    mc = MarketContext()
    mc.insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.04))
    mc.insert(ForwardCurve("USD-SOFR-3M", 0.25, AS_OF, knots))
    mc.insert(ForwardCurve("USD-SOFR", 1.0 / 360.0, AS_OF, knots))
    mc.insert(HazardCurve("ACME-HZD", AS_OF, [(0.0, 0.02), (5.0, 0.025)], recovery_rate=0.4))
    return mc


def _fixed_leg(**overrides: object) -> FixedLegSpec:
    kwargs: dict[str, object] = {
        "discount_curve_id": "USD-OIS",
        "rate": 0.04,
        "frequency": Tenor.semi_annual(),
        "day_count": DayCount.THIRTY_360,
        "start": AS_OF,
        "end": END,
        "compounding_simple": True,
    }
    kwargs.update(overrides)
    return FixedLegSpec(**kwargs)  # type: ignore[arg-type]


def _float_leg(**overrides: object) -> FloatLegSpec:
    kwargs: dict[str, object] = {
        "discount_curve_id": "USD-OIS",
        "forward_curve_id": "USD-SOFR-3M",
        "spread_bp": 0.0,
        "frequency": Tenor.quarterly(),
        "day_count": DayCount.ACT_360,
        "start": AS_OF,
        "end": END,
    }
    kwargs.update(overrides)
    return FloatLegSpec(**kwargs)  # type: ignore[arg-type]


# --------------------------------------------------------------------------- swaps


def test_swap_from_legs_prices_on_start_date_without_fixings() -> None:
    swap = (
        InterestRateSwap
        .builder()
        .id("IRS-SPOT")
        .notional(10_000_000.0, currency="USD")
        .side("pay")
        .fixed(_fixed_leg())
        .float(_float_leg())
        .build()
    )
    assert swap.float.reset_lag_days == 0
    result = swap.price(_market(), AS_OF, metrics=["dv01"])
    assert isinstance(result, ValuationResult)
    assert result.currency == "USD"
    assert result.get_metric("dv01") is not None
    # Same answer through the free function and through ``metric``.
    via_fn = price_instrument(swap, _market(), AS_OF)
    assert via_fn.price == pytest.approx(result.price)
    assert swap.metric(_market(), AS_OF, "dv01") == pytest.approx(result.get_metric("dv01"))


def test_swap_from_conventions_prices_on_start_date_without_fixings() -> None:
    swap = InterestRateSwap.from_conventions(
        "IRS-5Y-CONV",
        10_000_000.0,
        "pay",
        0.035,
        AS_OF,
        END,
        "USD-SOFR",
        "USD-OIS",
        "USD-SOFR",
        currency="USD",
    )
    assert swap.side == "pay"
    assert swap.float.reset_lag_days == 0
    assert swap.float.compounding != "simple"
    assert swap.fixed.rate == pytest.approx(0.035)
    result = swap.price(_market(), AS_OF)
    assert result.currency == "USD"


def test_reset_lag_days_defaults_to_zero_and_round_trips() -> None:
    leg = _float_leg()
    assert leg.reset_lag_days == 0
    assert FloatLegSpec.from_json(leg.to_json()).reset_lag_days == 0
    assert "reset_lag_days" in leg.to_dict()
    lagged = _float_leg(reset_lag_days=2)
    assert lagged.reset_lag_days == 2


def test_compounding_keyword_accepts_string_and_dict() -> None:
    simple = _float_leg(compounding="simple")
    assert simple.compounding == "simple"
    ois = _float_leg(
        forward_curve_id="USD-SOFR",
        compounding={"compounded_in_arrears": {"lookback_days": 0}},
    )
    assert ois.compounding == {"compounded_in_arrears": {"lookback_days": 0}}
    assert ois.to_dict()["compounding"] == {"compounded_in_arrears": {"lookback_days": 0}}
    with pytest.raises(ValueError, match=r"unknown variant `not_a_variant`"):
        _float_leg(compounding="not_a_variant")


def test_par_method_keyword() -> None:
    leg = _fixed_leg(par_method="discount_ratio")
    assert leg.par_method == "discount_ratio"
    assert _fixed_leg().par_method is None


def test_swap_getters_return_typed_values() -> None:
    swap = InterestRateSwap.example_standard()
    assert isinstance(swap.notional, Money)
    assert swap.notional.amount == pytest.approx(10_000_000.0)
    assert swap.side == "pay"
    assert isinstance(swap.fixed, FixedLegSpec)
    assert isinstance(swap.float, FloatLegSpec)
    assert isinstance(swap.fixed.start, datetime.date)
    assert isinstance(swap.fixed.frequency, Tenor)
    assert isinstance(swap.fixed.day_count, DayCount)
    assert isinstance(swap.fixed.stub, StubKind)
    assert swap.float.reset_lag_days == 2
    assert swap.margin_spec is None
    assert isinstance(swap.attributes, Attributes)
    assert swap.default_model == "discounting"
    deps = swap.market_dependencies()
    assert "curves" in deps
    spec = swap.to_dict()
    assert spec["id"] == swap.id
    assert "IRS-5Y-USD-STD" in repr(swap)


def test_swap_builder_margin_spec_and_attributes() -> None:
    swap = (
        InterestRateSwap
        .builder()
        .id("IRS-ATTR")
        .notional(Money(1_000_000.0, Currency("USD")))
        .side("receive")
        .fixed(_fixed_leg())
        .float(_float_leg())
        .attributes({"desk": "rates", "tags": ["hedge"]})
        .build()
    )
    assert swap.attributes.get_meta("desk") == "rates"
    assert swap.attributes.has_tag("hedge")


def test_builder_build_does_not_run_pricing_validation() -> None:
    """``build()`` mirrors Rust: structural invariants only."""
    swap = (
        InterestRateSwap
        .builder()
        .id("IRS-NO-PRICING-CHECK")
        .notional(1_000_000.0, currency="USD")
        .side("pay")
        .fixed(_fixed_leg())
        .float(_float_leg())
        .build()
    )
    assert swap.id == "IRS-NO-PRICING-CHECK"


def test_builder_names_missing_field() -> None:
    with pytest.raises(ValueError, match=r"InterestRateSwapBuilder.*'float'"):
        (
            InterestRateSwap
            .builder()
            .id("IRS-MISSING")
            .notional(1_000_000.0, currency="USD")
            .side("pay")
            .fixed(_fixed_leg())
            .build()
        )
    with pytest.raises(ValueError, match="missing required field 'id'"):
        InterestRateSwap.builder().notional(1_000_000.0, currency="USD").build()


def test_builder_repr_renders_fields_set_so_far() -> None:
    builder = InterestRateSwap.builder().id("IRS-REPR").side("pay")
    assert isinstance(builder, InterestRateSwapBuilder)
    text = repr(builder)
    assert text.startswith("InterestRateSwapBuilder(")
    assert 'id="IRS-REPR"' in text
    assert 'side="pay"' in text
    assert repr(Swaption.builder().id("S")).startswith('SwaptionBuilder(id="S"')
    assert repr(CapFloor.builder().id("C")).startswith('CapFloorBuilder(id="C"')
    assert repr(Bond.builder().id("B")).startswith('BondBuilder(id="B"')
    assert repr(TermLoan.builder().id("T")).startswith('TermLoanBuilder(id="T"')
    assert isinstance(Swaption.builder(), SwaptionBuilder)
    assert isinstance(CapFloor.builder(), CapFloorBuilder)
    assert isinstance(Bond.builder(), BondBuilder)
    assert isinstance(TermLoan.builder(), TermLoanBuilder)


# --------------------------------------------------------------------------- leg specs


def test_leg_specs_pickle_round_trip() -> None:
    fixed = _fixed_leg(calendar_id="usny", stub="none")
    floating = _float_leg(reset_lag_days=2, fixing_calendar_id="usny")
    premium = PremiumLegSpec("2024-03-20", "2029-06-20", Tenor.quarterly(), DayCount.ACT_360, 100.0, "USD-OIS")
    protection = ProtectionLegSpec("ACME-HZD", 0.4, 3)
    for leg in (fixed, floating, premium, protection):
        clone = pickle.loads(pickle.dumps(leg))  # noqa: S301
        assert clone.to_json() == leg.to_json()
        assert repr(clone) == repr(leg)
    assert pickle.loads(pickle.dumps(fixed)).calendar_id == "usny"  # noqa: S301
    assert pickle.loads(pickle.dumps(floating)).reset_lag_days == 2  # noqa: S301
    assert pickle.loads(pickle.dumps(premium)).spread_bp == pytest.approx(100.0)  # noqa: S301
    assert pickle.loads(pickle.dumps(protection)).settlement_delay == 3  # noqa: S301


def test_leg_specs_accept_rate_and_bps_objects() -> None:
    fixed = _fixed_leg(rate=Rate(0.04))
    assert fixed.rate == pytest.approx(0.04)
    floating = _float_leg(spread_bp=Bps(25))
    assert floating.spread_bp == pytest.approx(25.0)
    premium = PremiumLegSpec(AS_OF, END, Tenor.quarterly(), DayCount.ACT_360, Bps(100), "USD-OIS")
    assert premium.spread_bp == pytest.approx(100.0)
    with pytest.raises(TypeError):
        _fixed_leg(rate="4%")


def test_leg_specs_accept_iso_string_dates_and_stub_names() -> None:
    fixed = _fixed_leg(start="2025-01-15", end="2030-01-15", stub="none")
    assert fixed.start == AS_OF
    assert fixed.end == END
    assert fixed.stub == StubKind.NONE
    floating = _float_leg(start="2025-01-15", end="2030-01-15", stub=StubKind.SHORT_FRONT)
    assert floating.start == AS_OF
    assert floating.stub == StubKind.SHORT_FRONT


def test_leg_spec_repr_is_python_style() -> None:
    text = repr(_fixed_leg())
    assert text.startswith("FixedLegSpec(")
    assert "compounding_simple=True" in text
    assert "calendar_id=None" in text
    assert 'discount_curve_id="USD-OIS"' in text
    assert "0.04" in text
    assert "spread_bp=0" in repr(_float_leg())


# --------------------------------------------------------------------------- bonds


def test_bond_fixed_accepts_floats_strings_and_convention() -> None:
    bond = Bond.fixed("BOND-STR", 1_000_000.0, 0.05, "2024-01-15", "2034-01-15", "none", "USD-OIS", currency="USD")
    assert bond.issue_date == datetime.date(2024, 1, 15)
    assert bond.maturity == datetime.date(2034, 1, 15)
    assert bond.notional.amount == pytest.approx(1_000_000.0)
    assert bond.settlement_days == 1
    typed = Bond.fixed(
        "BOND-TYPED",
        Money(1_000_000.0, Currency("USD")),
        Rate(0.05),
        datetime.date(2024, 1, 15),
        datetime.date(2034, 1, 15),
        StubKind.NONE,
        "USD-OIS",
    )
    assert typed.to_dict()["cashflow_spec"] == bond.to_dict()["cashflow_spec"]
    bund = Bond.fixed(
        "BUND",
        1_000_000.0,
        0.025,
        "2024-01-15",
        "2034-01-15",
        "none",
        "EUR-OIS",
        convention="german_bund",
        currency="EUR",
    )
    assert bund.settlement_days == 2
    with pytest.raises(ValueError, match=r"needs a currency"):
        Bond.fixed("X", 1.0, 0.05, "2024-01-15", "2034-01-15", "none", "USD-OIS")
    with pytest.raises(ValueError, match=r"unknown variant `martian`"):
        Bond.fixed("X", 1.0, 0.05, "2024-01-15", "2034-01-15", "none", "USD-OIS", currency="USD", convention="martian")


def test_bond_constructors_and_examples() -> None:
    assert (
        Bond.zero_coupon("ZC", 1_000_000.0, "2024-01-01", "2029-01-01", "USD-OIS", currency="USD").has_floating_coupons
        is False
    )
    gilt = Bond.with_convention(
        "GILT", 1_000_000.0, 0.04, "2024-01-01", "2034-01-01", "uk_gilt", "GBP-OIS", currency="GBP"
    )
    assert gilt.settlement_days == 1
    frn = Bond.floating(
        "FRN",
        1000.0,
        "USD-SOFR-3M",
        125.0,
        "2024-01-01",
        "2029-01-01",
        Tenor.quarterly(),
        DayCount.ACT_360,
        "USD-OIS",
        currency="USD",
    )
    assert frn.has_floating_coupons
    # The index lives in the floating cashflow spec; ``forward_curve_id`` is an
    # explicit instrument-level override that these constructors leave unset.
    assert frn.forward_curve_id is None
    assert frn.cashflow_spec["floating"]["rate_spec"]["index_id"] == "USD-SOFR-3M"
    frn_eur = Bond.floating_with_convention(
        "FRN-EUR",
        1000.0,
        "EUR-EURIBOR-3M",
        Bps(80),
        "2024-01-01",
        "2029-01-01",
        Tenor.quarterly(),
        DayCount.ACT_360,
        "eur_corporate",
        "EUR-OIS",
        currency="EUR",
    )
    assert frn_eur.settlement_days == 2
    assert Bond.example().discount_curve_id == "USD-TREASURY"
    assert Bond.example_floating().has_floating_coupons
    assert Bond.example_callable().call_put is not None
    assert "amortizing" in Bond.example_amortizing().cashflow_spec
    assert Bond.example().min_moic(1.25).return_floor is not None
    assert Bond.example().min_xirr(0.12).return_floor is not None


def test_bond_builder_credit_models_preserve_explicit_model_contract() -> None:
    base = Bond.example().to_dict()

    def build_credit_bond(bond_id: str, call_put: dict[str, object] | None = None) -> Bond:
        builder = (
            Bond
            .builder()
            .id(bond_id)
            .notional(1_000_000.0, currency="USD")
            .issue_date("2024-01-15")
            .maturity("2034-01-15")
            .cashflow_spec(base["cashflow_spec"])
            .discount_curve_id("USD-OIS")
            .credit_curve_id("ACME-HZD")
            .attributes({"issuer": "ACME"})
        )
        if call_put is not None:
            builder = builder.call_put(call_put)
        return builder.build()

    bullet = build_credit_bond("BULLET-CREDIT")
    callable_bond = build_credit_bond(
        "CALLABLE-CREDIT",
        {
            "calls": [
                {
                    "start_date": AS_OF.isoformat(),
                    "end_date": AS_OF.isoformat(),
                    "price_pct_of_par": 80.0,
                }
            ],
            "puts": [],
        },
    )
    puttable_bond = build_credit_bond(
        "PUTTABLE-CREDIT",
        {
            "calls": [],
            "puts": [
                {
                    "start_date": AS_OF.isoformat(),
                    "end_date": AS_OF.isoformat(),
                    "price_pct_of_par": 120.0,
                }
            ],
        },
    )
    assert callable_bond.credit_curve_id == "ACME-HZD"
    assert callable_bond.call_put is not None
    assert callable_bond.call_put["calls"][0]["price_pct_of_par"] == pytest.approx(80.0)
    assert callable_bond.attributes.get_meta("issuer") == "ACME"

    market = _market()
    bond_models = set(list_models_grouped()["bond"])
    assert {"discounting", "hazard_rate", "tree", "rates_credit"} <= bond_models

    bullet_result = bullet.price(market, AS_OF, model="rates_credit")
    callable_result = callable_bond.price(market, AS_OF, model="rates_credit")
    puttable_result = puttable_bond.price(market, AS_OF, model="rates_credit")
    hazard_result = bullet.price(market, AS_OF, model="hazard_rate")
    tree_result = callable_bond.price(market, AS_OF, model="tree")
    assert bullet_result.currency == "USD"
    assert 0.0 < callable_result.price < bullet_result.price < puttable_result.price < 1_500_000.0
    assert 0.0 < hazard_result.price < 1_500_000.0
    assert 0.0 < tree_result.price < 1_500_000.0
    with pytest.raises(ValueError, match="non-callable"):
        callable_bond.price(market, AS_OF, model="hazard_rate")

    # Explicit discounting is rates-only even when the bond carries a credit
    # curve identifier; it must not silently switch to the hazard kernel.
    risk_free = bullet.price(market, AS_OF, model="discounting")
    assert hazard_result.price < risk_free.price
    assert bullet_result.price < risk_free.price

    stochastic_document = json.loads(bullet.to_json())
    stochastic_spec = stochastic_document["instrument"]["spec"]
    stochastic_spec["maturity"] = "2026-01-15"
    stochastic_spec["instrument_pricing_overrides"] = {
        "model_config": {"hazard_volatility": 0.01, "mc_paths": 2, "tree_steps": 4}
    }
    stochastic_result = price_instrument(json.dumps(stochastic_document), market, AS_OF, model="rates_credit")
    assert stochastic_result.details is not None
    assert stochastic_result.details["type"] == "monte_carlo"
    diagnostics = stochastic_result.details["data"]
    assert diagnostics["model_key"] == "rates_credit"
    assert diagnostics["training_paths"] == 0
    assert diagnostics["training_simulated_paths"] == 0
    assert diagnostics["make_whole_training_paths"] == 0
    assert diagnostics["make_whole_training_simulated_paths"] == 0
    assert diagnostics["estimator_paths"] == 2
    assert diagnostics["simulated_paths"] == 4
    assert diagnostics["seed"] > 2**53 - 1


def test_bond_getters_and_pricing_helpers() -> None:
    bond = Bond.fixed("BOND-GETTERS", 1_000_000.0, 0.05, "2024-01-15", "2034-01-15", "none", "USD-OIS", currency="USD")
    assert isinstance(bond.notional, Money)
    assert isinstance(bond.issue_date, datetime.date)
    assert bond.credit_curve_id is None
    assert bond.accrual_method == "linear"
    assert isinstance(bond.attributes, Attributes)
    assert bond.default_model == "discounting"
    assert "curves" in bond.market_dependencies()
    result = bond.price(_market(), AS_OF, metrics=["ytm", "dv01"], pricing_options={"theta_period": "1D"})
    assert result.get_metric("ytm") is not None
    assert bond.metric(_market(), AS_OF, "ytm") == pytest.approx(result.get_metric("ytm"))
    assert bond.id in repr(bond)
    assert "Money(" in repr(bond)


def test_bond_builder_missing_field_is_named() -> None:
    with pytest.raises(ValueError, match=r"BondBuilder.*missing required field"):
        Bond.builder().id("B").build()


# --------------------------------------------------------------------------- term loans


def test_term_loan_builder_and_getters() -> None:
    loan = (
        TermLoan
        .builder()
        .id("TL-1")
        .currency("USD")
        .notional_limit(10_000_000.0, currency="USD")
        .issue_date("2024-01-01")
        .maturity("2029-01-01")
        .rate(0.06)
        .frequency(Tenor.quarterly())
        .day_count(DayCount.ACT_360)
        .stub("none")
        .discount_curve_id("USD-OIS")
        .amortization({"percent_per_period": {"bp": 250}})
        .build()
    )
    assert loan.rate == {"fixed": {"rate_bp": 600}}
    assert loan.currency == "USD"
    assert loan.notional_limit.amount == pytest.approx(10_000_000.0)
    assert loan.stub == StubKind.NONE
    assert loan.settlement_days == 2
    assert loan.amortization == {"percent_per_period": {"bp": 250}}
    example = TermLoan.example()
    assert loan.to_dict()["rate"] == example.to_dict()["rate"]
    assert TermLoan.example_floating_with_ddtl().ddtl is not None
    assert TermLoan.example_callable().call_schedule is not None
    result = example.price(_market(), AS_OF)
    assert result.currency == "USD"
    floating = (
        TermLoan
        .builder()
        .id("TL-FLT")
        .currency("USD")
        .notional_limit(Money(5_000_000.0, Currency("USD")))
        .issue_date("2024-01-01")
        .maturity("2029-01-01")
        .rate(TermLoan.example_floating_with_ddtl().rate)
        .frequency(Tenor.quarterly())
        .day_count(DayCount.ACT_360)
        .discount_curve_id("USD-OIS")
        .amortization("none")
        .build()
    )
    assert "floating" in floating.rate


# --------------------------------------------------------------------------- swaptions / caps


def test_swaption_examples_getters_and_accessors() -> None:
    swpn = Swaption.example()
    assert swpn.option_type == "call"
    assert swpn.get_strike() == pytest.approx(0.03)
    assert swpn.get_swap_start() == datetime.date(2027, 1, 17)
    assert swpn.get_swap_end() == datetime.date(2032, 1, 17)
    assert isinstance(swpn.underlying_fixed_leg, FixedLegSpec)
    assert swpn.sabr_params is None
    assert swpn.exercise_style == "european"
    assert Swaption.example_bermudan().exercise_style == "bermudan"
    assert isinstance(swpn.expiry, datetime.date)
    rate = swpn.forward_swap_rate(_market(), AS_OF)
    assert 0.0 < rate < 0.2
    built = (
        Swaption
        .builder()
        .id("SWPT-ATTR")
        .option_type("put")
        .notional(1_000_000.0, currency="USD")
        .expiry("2025-01-13")
        .settlement("physical")
        .cash_settlement_method("par_yield")
        .vol_model("normal")
        .vol_surface_id("USD-SWPT-VOL")
        .underlying_fixed_leg(_fixed_leg())
        .underlying_float_leg(_float_leg())
        .sabr_params({"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": None})
        .attributes({"book": "vol"})
        .build()
    )
    assert built.sabr_params is not None
    assert built.attributes.get_meta("book") == "vol"


def test_cap_floor_example_getters_and_new_setters() -> None:
    cap = CapFloor.example()
    assert cap.rate_option_type == "cap"
    assert cap.strike == pytest.approx(0.03)
    assert cap.vol_type == "auto"
    assert cap.premium is None
    assert cap.stub == StubKind.SHORT_FRONT
    assert isinstance(cap.start_date, datetime.date)
    built = (
        CapFloor
        .builder()
        .id("CAP-KW")
        .rate_option_type("floor")
        .notional(5_000_000.0, currency="USD")
        .strike(Rate(0.02))
        .spread(0.0)
        .premium("2025-01-20", 10_000.0, currency="USD")
        .start_date("2025-01-15")
        .maturity("2028-01-15")
        .frequency(Tenor.quarterly())
        .day_count(DayCount.ACT_360)
        .stub("none")
        .business_day_convention("following")
        .exercise_style("european")
        .settlement("cash")
        .calendar_id("usny")
        .discount_curve_id("USD-OIS")
        .forward_curve_id("USD-SOFR-3M")
        .vol_surface_id("USD-CAP-VOL")
        .attributes({"desk": "options"})
        .build()
    )
    assert built.strike == pytest.approx(0.02)
    assert built.stub == StubKind.NONE
    assert built.business_day_convention == "following"
    premium = built.premium
    assert premium is not None
    assert premium[0] == datetime.date(2025, 1, 20)
    assert premium[1].amount == pytest.approx(10_000.0)
    assert built.vol_type == "auto"
    assert built.attributes.get_meta("desk") == "options"


def test_metric_keys_are_human_readable_not_escaped() -> None:
    """Composite metric keys reach Python decoded: no ``_x2d`` / ``_x5f`` escapes.

    Curve ids such as ``USD-OIS`` used to surface as ``pv01::USD_x2dOIS`` on
    every Python-facing metric surface. The canonical Rust codec now writes
    ordinary identifiers literally, so ``metric_keys()``, the ``metrics``
    mapping, ``metric_series`` / ``to_long_dataframe`` components and the wide
    ``to_dataframe()`` columns must all be free of escape markers, and
    ``get_metric`` / ``__getitem__`` must accept the decoded spelling.
    """
    swap = (
        InterestRateSwap
        .builder()
        .id("IRS-KEYS")
        .notional(10_000_000.0, currency="USD")
        .side("pay")
        .fixed(_fixed_leg())
        .float(_float_leg())
        .build()
    )
    result = swap.price(_market(), AS_OF, metrics=["pv01", "bucketed_dv01"])

    keys = result.metric_keys()
    assert keys, "expected pv01 / bucketed_dv01 measures"
    composite = [k for k in keys if "::" in k]
    assert composite, f"expected composite metric keys, got {keys}"
    for key in keys:
        assert "_x2d" not in key, key
        assert "_x5f" not in key, key
    assert set(result.metrics) == set(keys)

    for key in composite:
        assert result.get_metric(key) is not None
        assert result[key] == result.get_metric(key)
        assert key in result

    # A curve id with a hyphen round-trips literally through the key.
    assert any("USD-OIS" in k for k in composite), composite

    for components, _value in result.metric_series("bucketed_dv01"):
        for component in components:
            assert "_x2d" not in component, component
            assert "_x5f" not in component, component

    frame = result.to_dataframe()
    for column in frame.columns:
        assert "_x2d" not in column, column
        assert "_x5f" not in column, column

    long_frame = result.to_long_dataframe()
    for value in long_frame["curve"].dropna().tolist():
        assert "_x2d" not in value, value
        assert "_x5f" not in value, value
