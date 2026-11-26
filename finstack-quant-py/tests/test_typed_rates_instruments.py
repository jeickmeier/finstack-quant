"""Typed InterestRateSwap / FixedLegSpec / FloatLegSpec bindings."""

from __future__ import annotations

import datetime
import json
import math

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import (
    FixedLegSpec,
    FloatLegSpec,
    InterestRateSwap,
    TermLoan,
    price_instrument,
)
from tests.tests_typed_helpers import build_irs as _payer_swap

# Every serde wire value of the Rust `BusinessDayConvention` enum
# (`#[serde(rename_all = "snake_case")]` over Unadjusted/Following/
# ModifiedFollowing/Preceding/ModifiedPreceding). Kept in sync manually with
# `finstack-quant/core/src/dates/calendar/business_days.rs`; a stub `Literal`
# that drifts from this set is exactly the bug this test guards against.
_VALID_BDC_VALUES = (
    "unadjusted",
    "following",
    "modified_following",
    "preceding",
    "modified_preceding",
)

# Every canonical snake_case serde wire value of the Rust `StubKind` enum.
# Kept in sync manually with
# `finstack-quant/core/src/dates/schedule_iter.rs`; a stub `Literal` that
# drifts from this set is exactly the bug this test guards against.
_VALID_STUB_VALUES = (
    "none",
    "short_front",
    "short_back",
    "long_front",
    "long_back",
)


def _market_json() -> str:
    return json.dumps({
        "schema_version": 1,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "act_360",
                "knot_points": [[0.0, 1.0], [5.0, 0.90], [10.0, 0.80]],
                "interp_style": "monotone_convex",
                "extrapolation": "extend",
            },
            {
                "type": "forward",
                "id": "USD-SOFR-3M",
                "base": "2024-01-01",
                "day_count": "act_360",
                "index_tenor": {"count": 3, "unit": "months"},
                # Forward rate rising linearly from 4% to 4.5% over 10y.
                "knot_points": [[float(t), math.exp(-(0.04 * t + 0.00025 * t * t))] for t in range(11)],
                "interp_style": "log_linear",
                "extrapolation": "extend",
            },
        ],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "hierarchy": None,
    })


def _payer_swap_hand_written_json() -> str:
    """Hand-authored instrument envelope for the same economic swap as `_payer_swap`.

    Deliberately NOT derived from `InterestRateSwap.to_json()` — this is the
    independent side of the typed-vs-JSON golden. If Rust's envelope
    serializer ever mis-maps a field, this literal (built directly from the
    canonical `FixedLegSpec`/`FloatLegSpec`/`InterestRateSwap` struct
    definitions and their serde wire names) will diverge from the typed path
    instead of silently matching it.
    """
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "interest_rate_swap",
            "spec": {
                "id": "IRS-1",
                "notional": {"amount": "10000000", "currency": "USD"},
                "side": "pay",
                "fixed_leg": {
                    "discount_curve_id": "USD-OIS",
                    "rate": "0.04",
                    "frequency": {"count": 6, "unit": "months"},
                    "day_count": "30_360",
                    "business_day_convention": "modified_following",
                    "calendar_id": None,
                    "stub": "short_front",
                    "start": "2024-01-15",
                    "end": "2029-01-15",
                    "payment_lag_days": 0,
                    "end_of_month": False,
                },
                "float_leg": {
                    "discount_curve_id": "USD-OIS",
                    "index_id": "USD-SOFR-3M",
                    "forward_curve_id": "USD-SOFR-3M",
                    "spread_bp": "0",
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_360",
                    "business_day_convention": "modified_following",
                    "calendar_id": None,
                    "stub": "short_front",
                    "reset_lag_days": 0,
                    "fixing_calendar_id": None,
                    "start": "2024-01-15",
                    "end": "2029-01-15",
                    "compounding": "simple",
                    "payment_lag_days": 0,
                    "end_of_month": False,
                },
                "attributes": {},
            },
        },
    })


def _without_timestamp(result: ValuationResult) -> dict[str, object]:
    parsed = json.loads(result.to_json())
    parsed["meta"].pop("timestamp", None)
    return parsed


class TestInterestRateSwapTyped:
    def test_builder_produces_swap_with_id(self) -> None:
        swap = _payer_swap()
        assert swap.id == "IRS-1"
        assert "IRS-1" in repr(swap)

    def test_to_json_is_tagged(self) -> None:
        payload = json.loads(_payer_swap().to_json())
        assert payload["instrument"]["type"] == "interest_rate_swap"
        assert payload["instrument"]["spec"]["id"] == "IRS-1"

    def test_from_json_round_trip(self) -> None:
        original = _payer_swap().to_json()
        assert json.loads(InterestRateSwap.from_json(original).to_json()) == json.loads(original)

    def test_from_json_rejects_wrong_type(self) -> None:
        with pytest.raises(ValueError, match="interest_rate_swap"):
            InterestRateSwap.from_json(TermLoan.example().to_json())

    def test_builder_missing_required_field_raises(self) -> None:
        with pytest.raises(ValueError, match="missing required field"):
            InterestRateSwap.builder().id("IRS-BAD").build()

    def test_invalid_side_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid side"):
            InterestRateSwap.builder().side("sideways")

    def test_golden_typed_pv_equals_json_pv(self) -> None:
        """Payer IRS: typed path and an independently hand-written JSON payload produce identical ValuationResult.

        The JSON side is NOT derived from ``swap.to_json()`` (that would make
        both sides call the same Rust serializer, so a bug inside
        the envelope serializer's own field mapping would be identically wrong on
        both sides and pass). See `_payer_swap_hand_written_json`.
        """
        swap = _payer_swap()
        typed = price_instrument(swap, _market_json(), "2024-01-01", "discounting")
        via_json = price_instrument(_payer_swap_hand_written_json(), _market_json(), "2024-01-01", "discounting")
        assert _without_timestamp(typed) == _without_timestamp(via_json)

    def test_builder_setters_accept_keyword_value(self) -> None:
        """Every builder setter's `value` parameter name must match its text_signature."""
        start = datetime.date(2024, 1, 15)
        end = datetime.date(2029, 1, 15)
        fixed = FixedLegSpec(
            discount_curve_id="USD-OIS",
            rate="0.04",
            frequency=Tenor.semi_annual(),
            day_count=DayCount.THIRTY_360,
            start=start,
            end=end,
        )
        float_leg = FloatLegSpec(
            discount_curve_id="USD-OIS",
            index_id="USD-SOFR-3M",
            forward_curve_id="USD-SOFR-3M",
            spread_bp="0",
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            start=start,
            end=end,
        )
        swap = (
            InterestRateSwap
            .builder()
            .id(value="IRS-KW")
            .notional(value=Money(10_000_000.0, Currency("USD")))
            .side(value="pay")
            .fixed_leg(value=fixed)
            .float_leg(value=float_leg)
            .build()
        )
        assert swap.id == "IRS-KW"


class TestFixedLegSpecTyped:
    def test_keyword_arguments(self) -> None:
        """Every keyword-only parameter name must match its text_signature."""
        leg = FixedLegSpec(
            discount_curve_id="USD-OIS",
            rate="0.04",
            frequency=Tenor.semi_annual(),
            day_count=DayCount.THIRTY_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            business_day_convention="modified_following",
            calendar_id=None,
            stub="short_front",
            payment_lag_days=0,
            end_of_month=False,
        )
        assert "0.04" in repr(leg)

    @pytest.mark.parametrize("business_day_convention", _VALID_BDC_VALUES)
    def test_every_business_day_convention_literal_value_accepted(self, business_day_convention: str) -> None:
        """Every value in the `business_day_convention` stub Literal must be a real accepted wire value."""
        leg = FixedLegSpec(
            discount_curve_id="USD-OIS",
            rate="0.04",
            frequency=Tenor.semi_annual(),
            day_count=DayCount.THIRTY_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            business_day_convention=business_day_convention,
        )
        assert "0.04" in repr(leg)

    @pytest.mark.parametrize("stub", _VALID_STUB_VALUES)
    def test_every_stub_literal_value_accepted(self, stub: str) -> None:
        """Every value in the `stub` Literal (including `"none"`) must be a real accepted wire value."""
        leg = FixedLegSpec(
            discount_curve_id="USD-OIS",
            rate="0.04",
            frequency=Tenor.semi_annual(),
            day_count=DayCount.THIRTY_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            stub=stub,
        )
        assert "0.04" in repr(leg)


class TestFloatLegSpecTyped:
    @pytest.mark.parametrize("business_day_convention", _VALID_BDC_VALUES)
    def test_every_business_day_convention_literal_value_accepted(self, business_day_convention: str) -> None:
        """Every value in the `business_day_convention` stub Literal must be a real accepted wire value."""
        leg = FloatLegSpec(
            discount_curve_id="USD-OIS",
            index_id="USD-SOFR-3M",
            forward_curve_id="USD-SOFR-3M",
            spread_bp="0",
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            business_day_convention=business_day_convention,
        )
        assert "spread_bp=0" in repr(leg)

    @pytest.mark.parametrize("stub", _VALID_STUB_VALUES)
    def test_every_stub_literal_value_accepted(self, stub: str) -> None:
        """Every value in the `stub` Literal (including `"none"`) must be a real accepted wire value."""
        leg = FloatLegSpec(
            discount_curve_id="USD-OIS",
            index_id="USD-SOFR-3M",
            forward_curve_id="USD-SOFR-3M",
            spread_bp="0",
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            stub=stub,
        )
        assert "spread_bp=0" in repr(leg)

    def test_keyword_arguments(self) -> None:
        """Every keyword-only parameter name must match its text_signature."""
        leg = FloatLegSpec(
            discount_curve_id="USD-OIS",
            index_id="USD-SOFR-3M",
            forward_curve_id="USD-SOFR-3M",
            spread_bp="0",
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            start=datetime.date(2024, 1, 15),
            end=datetime.date(2029, 1, 15),
            business_day_convention="modified_following",
            calendar_id=None,
            stub="short_front",
            reset_lag_days=2,
            fixing_calendar_id=None,
            payment_lag_days=0,
            end_of_month=False,
        )
        assert "spread_bp=0" in repr(leg)


def test_irs_accrual_flags_round_trip_and_builder() -> None:
    from tests.tests_typed_helpers import irs_legs

    original = _payer_swap()
    assert original.adjust_fixed_accrual_dates is False
    assert original.adjust_float_accrual_dates is False
    fixed, floating = irs_legs()
    adjusted = (
        InterestRateSwap
        .builder()
        .id("IRS-ACCRUAL-FLAGS")
        .notional(original.notional)
        .side(original.side)
        .fixed_leg(fixed)
        .float_leg(floating)
        .adjust_fixed_accrual_dates(True)
        .adjust_float_accrual_dates(False)
        .build()
    )
    restored = InterestRateSwap.from_json(adjusted.to_json())
    assert restored.adjust_fixed_accrual_dates is True
    assert restored.adjust_float_accrual_dates is False


def test_inflation_linked_bond_preserves_exact_contract_during_edits() -> None:
    from decimal import Decimal

    from finstack_quant.valuations.instruments import InflationLinkedBond

    source = InflationLinkedBond.example()
    coupon = Decimal("0.025000000000000000000000001")
    changed = source.to_builder().id("TIPS-EDITED").real_coupon(coupon).lag("90D").build()
    assert changed.id == "TIPS-EDITED"
    assert changed.real_coupon == coupon
    assert changed.lag == "90D"
    assert source.lag == "3M"
    restored = InflationLinkedBond.from_json(changed.to_json())
    assert restored.real_coupon == coupon
    assert restored.to_json() == changed.to_json()


def test_bond_future_uses_typed_terms_and_live_and_final_marks() -> None:
    import json
    import pickle

    from finstack_quant.core.market_data import MarketContext
    from finstack_quant.valuations.instruments import BondFuture, ListedFutureTerms

    initial = BondFuture.example()
    terms = ListedFutureTerms(2, 1000, "USD", 112.25, "2025-09-19", "2025-09-30", "long")
    edited = initial.to_builder().terms(terms.with_marks(112.75, 112.50)).build()
    assert price_instrument(edited, MarketContext(), "2025-09-01").value.amount == 1000.0
    assert price_instrument(edited, MarketContext(), "2025-09-23").value.amount == 500.0
    assert json.loads(initial.terms.to_json())["contracts"] == 10.0
    assert BondFuture.from_json(edited.to_json()).to_json() == edited.to_json()
    assert pickle.loads(pickle.dumps(edited.terms)).to_json() == edited.terms.to_json()  # noqa: S301
    with pytest.raises(ValueError, match="must be finite"):
        terms.with_marks(float("nan"), None)


def test_listed_options_use_native_typed_terms_and_preserve_price() -> None:
    from finstack_quant.core.market_data import DiscountCurve, MarketContext
    from finstack_quant.valuations.instruments import (
        EquityFutureOption,
        FutureOptionTerms,
        FxFutureOption,
        InterestRateFutureOption,
        VolatilityIndexFutureOption,
    )

    as_of = datetime.date(2026, 10, 1)
    market = MarketContext()
    for curve_id in ("USD-OIS", "EUR-OIS"):
        market.insert(
            DiscountCurve(
                curve_id,
                as_of,
                [(0.0, 1.0), (1.0, 0.96), (2.0, 0.92)],
                interp="log_linear",
                extrapolation="extend",
                day_count=DayCount.ACT_365F,
            )
        )

    for instrument in (EquityFutureOption, FxFutureOption, InterestRateFutureOption, VolatilityIndexFutureOption):
        option = instrument.example()
        terms = FutureOptionTerms(**option.terms.to_dict())
        edited = option.to_builder().id("EDITED").terms(terms).build()
        assert option.id != edited.id
        assert option.terms.to_dict() == edited.terms.to_dict()
        price = price_instrument(edited, market, as_of).value
        wire_price = price_instrument(edited.to_json(), market, as_of).value
        assert price == wire_price == price_instrument(option, market, as_of).value


def test_other_listed_futures_roundtrip_typed_terms_at_exchange_marks() -> None:
    from finstack_quant.core.market_data import MarketContext
    from finstack_quant.valuations.instruments import (
        EquityFuture,
        FxFuture,
        InterestRateFuture,
        VolatilityIndexFuture,
    )

    for cls in (EquityFuture, FxFuture, InterestRateFuture, VolatilityIndexFuture):
        original = cls.example()
        terms = original.terms.with_marks(99.0, None)
        edited = original.to_builder().id("EDITED").terms(terms).build()
        assert original.id != edited.id
        assert edited.terms.to_json() == terms.to_json()
        as_of = datetime.date(2025, 1, 2)
        market = MarketContext()
        assert edited.price(market, as_of).value == price_instrument(edited.to_json(), market, as_of).value
