"""Behavioral tests for the typed finstack_quant.cashflows bindings."""

from __future__ import annotations

import datetime as dt
from typing import TYPE_CHECKING

import pytest

from finstack_quant.core.money import Money

if TYPE_CHECKING:
    from finstack_quant.cashflows.builder import CashFlowSchedule


class TestPrimitives:
    def test_cfkind_classattrs_parse_and_display(self) -> None:
        from finstack_quant.cashflows.primitives import CFKind

        assert CFKind.parse("fixed") == CFKind.FIXED
        assert CFKind.FLOAT_RESET.name == "float_reset"
        assert str(CFKind.AMORTIZATION) == "amortization"
        assert CFKind.PIK != CFKind.FIXED
        assert CFKind.FIXED.is_interest_like()
        assert not CFKind.FEE.is_interest_like()
        assert CFKind.NOTIONAL.is_principal_like()
        assert not CFKind.FIXED.is_principal_like()
        with pytest.raises(ValueError, match="unknown cashflow kind"):
            CFKind.parse("amort")
        with pytest.raises(ValueError, match="unknown cashflow kind"):
            CFKind.parse("not_a_kind")

    def test_cashflow_construction_getters_and_validate(self) -> None:
        from finstack_quant.cashflows.primitives import CashFlow, CFKind

        cf = CashFlow(
            date=dt.date(2025, 6, 15),
            amount=Money(50_000.0, "USD"),
            kind=CFKind.FIXED,
            accrual_factor=0.5,
            rate=0.05,
        )
        assert cf.date == dt.date(2025, 6, 15)
        assert cf.reset_date is None
        assert cf.amount.amount == pytest.approx(50_000.0)
        assert cf.kind == CFKind.FIXED
        assert cf.accrual_factor == pytest.approx(0.5)
        assert cf.rate == pytest.approx(0.05)
        cf.validate()  # must not raise

    def test_cashflow_validate_rejects_negative_accrual_factor(self) -> None:
        from finstack_quant.cashflows.primitives import CashFlow, CFKind

        cf = CashFlow(
            date=dt.date(2025, 6, 15),
            amount=Money(1.0, "USD"),
            kind=CFKind.FIXED,
            accrual_factor=-0.1,
        )
        with pytest.raises(ValueError, match="accrual_factor"):
            cf.validate()

    def test_is_cash_settlement_kind(self) -> None:
        from finstack_quant.cashflows.primitives import CFKind, is_cash_settlement_kind

        assert is_cash_settlement_kind(CFKind.FIXED)
        assert is_cash_settlement_kind(CFKind.AMORTIZATION)
        assert not is_cash_settlement_kind(CFKind.PIK)
        assert not is_cash_settlement_kind(CFKind.DEFAULTED_NOTIONAL)
        # str form is accepted too
        assert not is_cash_settlement_kind("pik")


class TestBuilderSpecs:
    def test_schedule_params_presets(self) -> None:
        from finstack_quant.cashflows.builder import ScheduleParams

        p = ScheduleParams.usd_sofr_swap()
        assert p.calendar_id == "usny"
        assert p.payment_lag_days == 2
        assert p.adjust_accrual_dates is True
        q = ScheduleParams.quarterly_act360()
        assert q.calendar_id == "weekends_only"
        assert q.payment_lag_days == 0
        assert q.adjust_accrual_dates is False
        # all 10 presets exist and return ScheduleParams
        for name in (
            "quarterly_act360",
            "semiannual_30360",
            "annual_actact",
            "usd_sofr_swap",
            "usd_corporate_bond",
            "usd_treasury",
            "eur_estr_swap",
            "eur_gov_bond",
            "gbp_sonia_swap",
            "jpy_tona_swap",
        ):
            assert isinstance(getattr(ScheduleParams, name)(), ScheduleParams)

    def test_schedule_params_constructor(self) -> None:
        from finstack_quant.cashflows.builder import ScheduleParams
        from finstack_quant.core.dates import DayCount, Tenor

        p = ScheduleParams(
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            calendar_id="weekends_only",
        )
        assert p.calendar_id == "weekends_only"
        assert p.end_of_month is False

    def test_fixed_coupon_spec_decimal_rate(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams

        spec = FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360())
        assert spec.rate == Decimal("0.05")

    def test_coupon_type_and_roll_rule(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import CouponType, PrincipalExchange, RollRule

        assert CouponType.CASH is not None
        assert CouponType.PIK is not None
        assert CouponType.split(Decimal("0.5"), Decimal("0.5")) is not None
        assert RollRule.NONE != RollRule.CDS_IMM
        assert PrincipalExchange.NONE != PrincipalExchange.INITIAL_AND_FINAL

    def test_amortization_and_notional(self) -> None:
        from finstack_quant.cashflows.builder import AmortizationSpec, Notional

        # Fieldless variant: class-attribute singleton (consistent with
        # RollRule.NONE / CouponType.CASH), not a snake_case staticmethod.
        assert AmortizationSpec.NONE is not None
        n = Notional.par(1_000_000.0, "USD")
        assert n.initial.amount == pytest.approx(1_000_000.0)
        assert n.currency().code == "USD"
        n.validate()
        spec = AmortizationSpec.linear_to(Money(0.0, "USD"))
        n2 = Notional(Money(1_000_000.0, "USD"), amort=spec)
        n2.validate()
        with pytest.raises(ValueError, match="cannot exceed"):
            # LinearTo target above initial notional is invalid
            Notional(
                Money(100.0, "USD"),
                amort=AmortizationSpec.linear_to(Money(200.0, "USD")),
            ).validate()

    def test_amortization_spec_hash_matches_equality(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import AmortizationSpec

        pos_zero = AmortizationSpec.percent_of_original_per_period(0.0)
        neg_zero = AmortizationSpec.percent_of_original_per_period(-0.0)
        assert pos_zero == neg_zero
        assert hash(pos_zero) == hash(neg_zero)

        scale_a = AmortizationSpec.linear_to(Money(Decimal("1.0"), "USD"))
        scale_b = AmortizationSpec.linear_to(Money(Decimal("1.00"), "USD"))
        assert scale_a == scale_b
        assert hash(scale_a) == hash(scale_b)

    def test_fee_spec_factories(self) -> None:
        from finstack_quant.cashflows.builder import FeeAccrualBasis, FeeBase, FeeSpec
        from finstack_quant.core.dates import DayCount, Tenor

        # Fieldless variant: class-attribute singleton (consistent with
        # RollRule.NONE / CouponType.CASH), not a snake_case staticmethod.
        assert FeeBase.DRAWN is not None
        fixed = FeeSpec.fixed(dt.date(2025, 1, 15), Money(-5_000.0, "USD"))
        assert fixed is not None
        periodic = FeeSpec.periodic_bp(
            base=FeeBase.undrawn(facility_limit=Money(10_000_000.0, "USD")),
            bp=50,
            frequency=Tenor.quarterly(),
            day_count=DayCount.ACT_360,
            business_day_convention="modified_following",
            calendar_id="weekends_only",
            accrual_basis=FeeAccrualBasis.TIME_WEIGHTED_AVERAGE,
        )
        assert periodic is not None

    def test_floating_rate_spec_validate(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import FloatingRateSpec

        spec = FloatingRateSpec(
            index_id="USD-SOFR-3M",
            spread_bp=Decimal("200"),
            reset_frequency="3M",
            index_floor_bp=Decimal("0"),
        )
        spec.validate()
        bad = FloatingRateSpec(
            index_id="USD-SOFR-3M",
            spread_bp=Decimal("200"),
            reset_frequency="3M",
            index_floor_bp=Decimal("100"),
            index_cap_bp=Decimal("50"),
        )
        with pytest.raises(ValueError, match="index_floor_bp"):
            bad.validate()

    def test_schedule_params_and_fixed_coupon_defaults_match_json(self) -> None:
        """Typed constructors must not hand-re-encode private Rust serde defaults.

        `ScheduleParams.new` and `FixedCouponSpec.new` re-derive several
        defaults (business_day_convention, stub, roll_rule, coupon_type) that live in Rust's
        private `serde_defaults.rs` / `Default` impls rather than a shared
        public constant. Build the same schedule through the typed path
        (all optional fields omitted) and through the JSON path (same
        fields omitted from the wire payload) and assert the results are
        byte-for-byte identical, so a future change to a Rust default either
        flows through automatically or fails here.

        The 13-month horizon (2025-01-15 -> 2026-02-15) is deliberately not
        an exact multiple of the 3M frequency, so the default `stub`
        (ShortFront) actually produces a short front period instead of an
        exact grid -- a 12-month horizon would make the stub default inert.
        The quarterly anchors this produces (2025-02-15, 2025-11-15,
        2026-02-15) land on a Saturday/Saturday/Sunday, so the default
        `business_day_convention` (ModifiedFollowing) actually rolls those payment dates
        forward -- an all-weekday horizon would make `business_day_convention` (and, since
        `adjust_accrual_dates` defaults to `false`, the fact that accrual
        boundaries stay unadjusted while payment dates roll) inert too.
        """
        from decimal import Decimal
        import json

        from finstack_quant.cashflows import build_cashflow_schedule_json
        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FixedCouponSpec,
            ScheduleParams,
        )
        from finstack_quant.core.dates import DayCount

        issue = dt.date(2025, 1, 15)
        maturity = dt.date(2026, 2, 15)

        typed_schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), issue, maturity)
            .fixed_cf(
                FixedCouponSpec(
                    rate=Decimal("0.05"),
                    schedule=ScheduleParams(frequency="3M", day_count=DayCount.ACT_360, calendar_id="weekends_only"),
                )
            )
            .build(None)
        )

        json_spec = json.dumps({
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue": "2025-01-15",
            "maturity": "2026-02-15",
            "coupon_program": [
                {
                    "kind": "fixed",
                    "spec": {
                        "rate": "0.05",
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "act_360",
                        "calendar_id": "weekends_only",
                    },
                }
            ],
        })
        json_schedule = build_cashflow_schedule_json(json_spec)

        # Sanity check that this fixture genuinely exercises the defaults it
        # claims to: a short front stub (5 coupons, not the 4 an exact
        # 12-month/3M grid would produce) and at least one payment date
        # rolled off a weekend by the default ModifiedFollowing business_day_convention.
        typed_coupons = typed_schedule.coupons()
        assert len(typed_coupons) == 5  # stub + 4 regular quarters
        coupon_dates = [cf.date for cf in typed_coupons]
        assert dt.date(2025, 2, 17) in coupon_dates  # rolled off Sat 2025-02-15
        assert dt.date(2025, 2, 15) not in coupon_dates

        assert json.loads(typed_schedule.to_json()) == json.loads(json_schedule)

    def test_floating_rate_spec_defaults_match_json(self) -> None:
        """Pin `FloatingRateSpec.new` defaults against the JSON path.

        It re-encodes private Rust defaults (gearing, reset_lag_days,
        gearing_includes_spread, fallback); build the same spec through both
        paths with the same optional fields omitted and assert byte-for-byte
        equality.

        This is a term-rate leg (no `overnight_compounding`), so the
        `overnight_index_constraints` default is inert here -- it only
        affects overnight-compounded legs. See
        `test_overnight_index_constraints_default_matches_json` below, which
        pins that default on a leg where it changes the result. Likewise,
        `fallback` never triggers here because `market` supplies the curve;
        see `test_floating_rate_fallback_default_matches_json_on_missing_curve`
        for a pinning of `fallback = Error` via a shared failure mode.
        """
        from decimal import Decimal
        import json

        from finstack_quant.cashflows import build_cashflow_schedule_json
        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateSpec,
            ScheduleParams,
        )
        from finstack_quant.core.dates import DayCount
        from finstack_quant.core.market_data import ForwardCurve, MarketContext

        # base_date must be on/before every reset date; the first reset
        # trails the issue date by the default 2-business-day reset lag.
        curve = ForwardCurve(
            id="USD-SOFR-3M",
            tenor=0.25,
            knots=[(0.0, 0.04), (2.0, 0.04)],
            base_date=dt.date(2025, 1, 1),
        )
        market = MarketContext().insert(curve)

        typed_schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .floating_cf(
                FloatingCouponSpec(
                    rate_spec=FloatingRateSpec(
                        index_id="USD-SOFR-3M",
                        spread_bp=Decimal("150"),
                        reset_frequency="3M",
                    ),
                    schedule=ScheduleParams(frequency="3M", day_count=DayCount.ACT_360, calendar_id="weekends_only"),
                )
            )
            .build(market)
        )

        json_spec = json.dumps({
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue": "2025-01-15",
            "maturity": "2026-01-15",
            "coupon_program": [
                {
                    "kind": "floating",
                    "spec": {
                        "rate_spec": {
                            "index_id": "USD-SOFR-3M",
                            "spread_bp": "150",
                            "reset_frequency": {"count": 3, "unit": "months"},
                        },
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "act_360",
                        "calendar_id": "weekends_only",
                    },
                }
            ],
        })
        json_schedule = build_cashflow_schedule_json(json_spec, market.to_json())

        assert json.loads(typed_schedule.to_json()) == json.loads(json_schedule)

    def test_overnight_index_constraints_default_matches_json(self) -> None:
        """Pin `FloatingRateSpec.overnight_index_constraints` (default: Daily).

        `overnight_index_constraints` only affects the result on an
        overnight-*compounded* leg (`overnight_compounding` set) with an
        `index_floor_bp`/`index_cap_bp`: `Daily` clamps each sampled daily
        fixing before compounding, `Period` compounds the raw daily fixings
        first and clamps once at the end. The two only diverge when the
        curve varies enough within a period that some days breach the floor
        while the period-average does not, so the forward curve here is a
        near step function (near-zero for the first ~18 days of the first
        accrual period, then a high flat rate) that crosses a 3% floor
        partway through -- a flat or slowly-varying curve would make this
        default inert, as it is in `test_floating_rate_spec_defaults_match_json`.
        """
        from decimal import Decimal
        import json

        from finstack_quant.cashflows import build_cashflow_schedule_json
        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateSpec,
            OvernightCompoundingMethod,
            OvernightIndexConstraintApplication,
            ScheduleParams,
        )
        from finstack_quant.core.dates import DayCount
        from finstack_quant.core.market_data import ForwardCurve, MarketContext

        curve = ForwardCurve(
            id="USD-SOFR-3M",
            tenor=0.25,
            knots=[(0.0, 0.001), (0.05, 0.001), (0.06, 0.30), (1.0, 0.30)],
            base_date=dt.date(2025, 1, 1),
        )
        market = MarketContext().insert(curve)

        def build_schedule(overnight_index_constraints: object | None) -> CashFlowSchedule:
            kwargs = {}
            if overnight_index_constraints is not None:
                kwargs["overnight_index_constraints"] = overnight_index_constraints
            rate_spec = FloatingRateSpec(
                index_id="USD-SOFR-3M",
                spread_bp=Decimal("0"),
                reset_frequency="3M",
                index_floor_bp=Decimal("300"),
                overnight_compounding=OvernightCompoundingMethod.COMPOUNDED_IN_ARREARS,
                **kwargs,
            )
            return (
                CashFlowSchedule
                .builder()
                .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
                .floating_cf(
                    FloatingCouponSpec(
                        rate_spec=rate_spec,
                        schedule=ScheduleParams(
                            frequency="3M", day_count=DayCount.ACT_360, calendar_id="weekends_only"
                        ),
                    )
                )
                .build(market)
            )

        typed_schedule = build_schedule(None)

        json_spec = json.dumps({
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue": "2025-01-15",
            "maturity": "2026-01-15",
            "coupon_program": [
                {
                    "kind": "floating",
                    "spec": {
                        "rate_spec": {
                            "index_id": "USD-SOFR-3M",
                            "spread_bp": "0",
                            "reset_frequency": {"count": 3, "unit": "months"},
                            "index_floor_bp": "300",
                            "overnight_compounding": "compounded_in_arrears",
                        },
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "act_360",
                        "calendar_id": "weekends_only",
                    },
                }
            ],
        })
        json_schedule = build_cashflow_schedule_json(json_spec, market.to_json())

        # Sanity check that this fixture genuinely exercises the default:
        # explicitly requesting the non-default `Period` application must
        # change the first coupon's rate relative to the omitted-field
        # (Daily) build, or this fixture would not be able to catch a
        # `FloatingRateSpec.new` binding that hardcoded the wrong default.
        period_schedule = build_schedule(OvernightIndexConstraintApplication.PERIOD)
        daily_schedule = build_schedule(OvernightIndexConstraintApplication.DAILY)
        assert json.loads(typed_schedule.to_json()) == json.loads(daily_schedule.to_json())
        assert json.loads(typed_schedule.to_json()) != json.loads(period_schedule.to_json())

        assert json.loads(typed_schedule.to_json()) == json.loads(json_schedule)

    def test_floating_rate_fallback_default_matches_json_on_missing_curve(self) -> None:
        """Pin `FloatingRateSpec.fallback` (default: Error) via a shared failure.

        `test_floating_rate_spec_defaults_match_json` always supplies a
        market with the requested curve, so the `fallback` default never
        triggers there. Build the same floating spec through the typed and
        JSON paths with no market context at all and assert both fail with
        the same exception type and message, pinning `fallback = Error`
        (rather than e.g. `SpreadOnly`, which would build successfully
        instead of raising).
        """
        from decimal import Decimal
        import json

        from finstack_quant.cashflows import build_cashflow_schedule_json
        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateSpec,
            ScheduleParams,
        )
        from finstack_quant.core.dates import DayCount

        typed_builder = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .floating_cf(
                FloatingCouponSpec(
                    rate_spec=FloatingRateSpec(
                        index_id="USD-SOFR-3M",
                        spread_bp=Decimal("150"),
                        reset_frequency="3M",
                    ),
                    schedule=ScheduleParams(frequency="3M", day_count=DayCount.ACT_360, calendar_id="weekends_only"),
                )
            )
        )
        with pytest.raises(KeyError) as typed_exc:
            typed_builder.build(None)

        json_spec = json.dumps({
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue": "2025-01-15",
            "maturity": "2026-01-15",
            "coupon_program": [
                {
                    "kind": "floating",
                    "spec": {
                        "rate_spec": {
                            "index_id": "USD-SOFR-3M",
                            "spread_bp": "150",
                            "reset_frequency": {"count": 3, "unit": "months"},
                        },
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "act_360",
                        "calendar_id": "weekends_only",
                    },
                }
            ],
        })
        with pytest.raises(KeyError) as json_exc:
            build_cashflow_schedule_json(json_spec)

        assert type(typed_exc.value) is type(json_exc.value)
        assert str(typed_exc.value) == str(json_exc.value)

    def test_credit_model_specs(self) -> None:
        from finstack_quant.cashflows.builder import (
            DefaultModelSpec,
            PrepaymentModelSpec,
            RecoveryModelSpec,
        )

        psa = PrepaymentModelSpec.psa(1.0)
        assert psa.smm(30) > psa.smm(1)
        assert PrepaymentModelSpec.constant_cpr(0.06).cpr == pytest.approx(0.06)
        sda = DefaultModelSpec.sda(1.0)
        assert sda.mdr(30) > sda.mdr(1)
        assert DefaultModelSpec.cdr_2pct().cdr == pytest.approx(0.02)
        rec = RecoveryModelSpec(rate=0.40, recovery_lag=12)
        rec.validate()
        assert rec.recovery_lag == 12
        with pytest.raises(ValueError, match="must be in"):
            RecoveryModelSpec(rate=1.5, recovery_lag=0).validate()


class TestCashFlowBuilder:
    @staticmethod
    def _quarterly_bond() -> CashFlowSchedule:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FixedCouponSpec,
            ScheduleParams,
        )

        return (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360()))
            .build(None)
        )

    def test_fixed_quarterly_bond_flows(self) -> None:
        from finstack_quant.cashflows.primitives import CFKind

        schedule = self._quarterly_bond()
        flows = schedule.get_flows()
        assert len(flows) == 6

        # Initial funding: -1MM notional at issue.
        assert flows[0].date == dt.date(2025, 1, 15)
        assert flows[0].kind == CFKind.NOTIONAL
        assert flows[0].amount.amount == pytest.approx(-1_000_000.0)

        # Four Act/360 coupons: 90/91/92/92 day accrual periods.
        expected_coupons = [
            (dt.date(2025, 4, 15), 1_000_000.0 * 0.05 * 90 / 360),
            (dt.date(2025, 7, 15), 1_000_000.0 * 0.05 * 91 / 360),
            (dt.date(2025, 10, 15), 1_000_000.0 * 0.05 * 92 / 360),
            (dt.date(2026, 1, 15), 1_000_000.0 * 0.05 * 92 / 360),
        ]
        coupon_flows = [f for f in flows if f.kind == CFKind.FIXED]
        assert len(coupon_flows) == 4
        for flow, (date, amount) in zip(coupon_flows, expected_coupons, strict=True):
            assert flow.date == date
            assert flow.amount.amount == pytest.approx(amount, abs=1e-6)
            assert flow.amount.currency.code == "USD"
            assert flow.rate == pytest.approx(0.05)

        # Redemption at maturity, after the same-date final coupon.
        assert flows[5].date == dt.date(2026, 1, 15)
        assert flows[5].kind == CFKind.NOTIONAL
        assert flows[5].amount.amount == pytest.approx(1_000_000.0)

    def test_build_requires_principal(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FixedCouponSpec,
            ScheduleParams,
        )

        builder = CashFlowSchedule.builder().fixed_cf(
            FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360())
        )
        with pytest.raises(KeyError):
            # Missing principal maps to InputError::NotFound -> KeyError.
            builder.build(None)

    def test_floating_without_curve_raises(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateSpec,
            ScheduleParams,
        )

        spec = FloatingCouponSpec(
            rate_spec=FloatingRateSpec(index_id="USD-SOFR-3M", spread_bp=Decimal("200"), reset_frequency="3M"),
            schedule=ScheduleParams.quarterly_act360(),
        )
        builder = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .floating_cf(spec)
        )
        # Default FloatingRateFallback.ERROR: missing curve must fail the build.
        with pytest.raises((KeyError, ValueError)):
            builder.build(None)

    def test_floating_with_market_context_builds(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateSpec,
            ScheduleParams,
        )
        from finstack_quant.cashflows.primitives import CFKind
        from finstack_quant.core.market_data import ForwardCurve, MarketContext

        # base_date must be on/before every reset date; the first reset
        # trails the issue date by the default 2-business-day reset lag.
        curve = ForwardCurve(
            id="USD-SOFR-3M",
            tenor=0.25,
            knots=[(0.0, 0.04), (2.0, 0.04)],
            base_date=dt.date(2025, 1, 1),
        )
        market = MarketContext().insert(curve)
        spec = FloatingCouponSpec(
            rate_spec=FloatingRateSpec(index_id="USD-SOFR-3M", spread_bp=Decimal("0"), reset_frequency="3M"),
            schedule=ScheduleParams.quarterly_act360(),
        )
        schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .floating_cf(spec)
            .build(market)
        )
        floats = [f for f in schedule.get_flows() if f.kind == CFKind.FLOAT_RESET]
        assert len(floats) == 4
        assert all(f.amount.amount > 0.0 for f in floats)

    def test_fixed_to_float_builds(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            CouponType,
            FixedWindow,
            FloatingCouponSpec,
            FloatingRateFallback,
            FloatingRateSpec,
            ScheduleParams,
        )
        from finstack_quant.cashflows.primitives import CFKind

        floating = FloatingCouponSpec(
            rate_spec=FloatingRateSpec(
                index_id="USD-SOFR-3M",
                spread_bp=Decimal("200"),
                reset_frequency="3M",
                fallback=FloatingRateFallback.SPREAD_ONLY,
            ),
            schedule=ScheduleParams.quarterly_act360(),
        )
        schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2027, 1, 15))
            .fixed_to_float(
                dt.date(2026, 1, 15),
                FixedWindow(Decimal("0.04"), ScheduleParams.semiannual_30360()),
                floating,
                CouponType.CASH,
            )
            .build(None)
        )
        kinds = {f.kind for f in schedule.get_flows()}
        assert CFKind.FIXED in kinds
        assert CFKind.FLOAT_RESET in kinds

    def test_float_margin_stepup_builds(self) -> None:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FloatingCouponSpec,
            FloatingRateFallback,
            FloatingRateSpec,
            ScheduleParams,
        )
        from finstack_quant.cashflows.primitives import CFKind

        spec = FloatingCouponSpec(
            rate_spec=FloatingRateSpec(
                index_id="USD-SOFR-3M",
                spread_bp=Decimal("200"),
                reset_frequency="3M",
                fallback=FloatingRateFallback.SPREAD_ONLY,
            ),
            schedule=ScheduleParams.quarterly_act360(),
        )
        schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .float_margin_stepup([(dt.date(2025, 7, 15), Decimal("250"))], spec)
            .build(None)
        )
        floats = [f for f in schedule.get_flows() if f.kind == CFKind.FLOAT_RESET]
        assert floats
        assert all(f.amount.amount > 0.0 for f in floats)

    def test_add_principal_event_requires_kind(self) -> None:
        from finstack_quant.cashflows.builder import CashFlowSchedule

        builder = CashFlowSchedule.builder().principal(
            Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15)
        )
        # `kind` has no default in Rust (builder/principal.rs) or in the JSON
        # spec (PrincipalEventSpec in json.rs); the binding must not invent one.
        with pytest.raises(TypeError):
            builder.add_principal_event(dt.date(2025, 6, 15), Money(-100_000.0, "USD"))

    def test_add_principal_event_repayment_with_explicit_kind(self) -> None:
        from finstack_quant.cashflows.builder import CashFlowSchedule
        from finstack_quant.cashflows.primitives import CFKind

        schedule = (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .add_principal_event(dt.date(2025, 6, 15), Money(-100_000.0, "USD"), CFKind.AMORTIZATION)
            .build(None)
        )
        flows = schedule.get_flows()
        repayment = next(f for f in flows if f.date == dt.date(2025, 6, 15))
        assert repayment.kind == CFKind.AMORTIZATION
        assert repayment.amount.amount == pytest.approx(100_000.0)


class TestCashFlowSchedule:
    @staticmethod
    def _bond() -> CashFlowSchedule:
        return TestCashFlowBuilder._quarterly_bond()

    def test_accessors(self) -> None:
        from finstack_quant.core.dates import DayCount

        schedule = self._bond()
        assert len(schedule.coupons()) == 4
        assert schedule.dates()[0] == dt.date(2025, 1, 15)
        assert schedule.get_notional().initial.amount == pytest.approx(1_000_000.0)
        assert schedule.get_day_count() == DayCount.ACT_360
        meta = schedule.get_meta()
        assert meta.issue_date == dt.date(2025, 1, 15)
        assert meta.maturity_date == dt.date(2026, 1, 15)
        assert "weekends_only" in meta.calendar_ids
        schedule.validate()

    def test_scale_amounts_flips_direction(self) -> None:
        schedule = self._bond()
        flipped = schedule.scale_amounts(-1.0)
        # Original untouched; new schedule has negated amounts.
        assert schedule.get_flows()[0].amount.amount == pytest.approx(-1_000_000.0)
        total_orig = sum(f.amount.amount for f in schedule.get_flows())
        total_flip = sum(f.amount.amount for f in flipped.get_flows())
        assert total_flip == pytest.approx(-total_orig, abs=1e-9)
        with pytest.raises(ValueError, match="finite"):
            schedule.scale_amounts(float("nan"))

    def test_weighted_average_life_bullet(self) -> None:
        schedule = self._bond()
        # Single 1MM principal repayment exactly one non-leap year out: Act/365F WAL = 1.0.
        wal = schedule.weighted_average_life(dt.date(2025, 1, 15))
        assert wal == pytest.approx(1.0, abs=1e-12)

    def test_outstanding_by_date(self) -> None:
        schedule = self._bond()
        path = schedule.outstanding_by_date()
        first_date, first_balance = path[0]
        last_date, last_balance = path[-1]
        assert first_date == dt.date(2025, 1, 15)
        assert first_balance.amount == pytest.approx(1_000_000.0)
        assert last_date == dt.date(2026, 1, 15)
        assert last_balance.amount == pytest.approx(0.0, abs=1e-9)

    def test_pv_by_period_zero_rate_equals_nominal(self) -> None:
        from finstack_quant.core.dates import build_periods
        from finstack_quant.core.market_data import DiscountCurve, MarketContext

        schedule = self._bond()
        periods = build_periods("2025Q1..2026Q1").periods
        market = MarketContext().insert(DiscountCurve.flat("USD-OIS", dt.date(2025, 1, 15), 0.0))
        pv = schedule.pv_by_period(
            periods=periods,
            market=market,
            disc_curve_id="USD-OIS",
            base=dt.date(2025, 1, 15),
        )
        # Zero rate: PV == nominal sum per period. 2025Q2 holds the 90-day coupon.
        assert pv["2025Q2"]["USD"].amount == pytest.approx(12_500.0, abs=1e-6)
        assert list(pv.to_dataframe().columns) == ["period", "currency", "amount"]

    def test_to_dataframe(self) -> None:
        pd = pytest.importorskip("pandas")

        schedule = self._bond()
        df = schedule.to_dataframe()
        assert list(df.columns) == [
            "date",
            "reset_date",
            "kind",
            "amount",
            "currency",
            "accrual_factor",
            "rate",
            "accrual",
            "principal_delta",
        ]
        assert str(df["date"].dtype).startswith("datetime64")
        assert "outstanding" in schedule.to_dataframe(outstanding=True).columns
        assert len(df) == 6
        assert set(df["kind"]) == {"fixed", "notional"}
        assert set(df["currency"]) == {"USD"}
        assert df["amount"].sum() == pytest.approx(sum(f.amount.amount for f in schedule.get_flows()), abs=1e-9)
        assert isinstance(df, pd.DataFrame)

    def test_json_round_trip_matches_json_bridge(self) -> None:
        from finstack_quant.cashflows import validate_cashflow_schedule_json
        from finstack_quant.cashflows.builder import CashFlowSchedule

        schedule = self._bond()
        payload = schedule.to_json()
        # The typed schedule serializes to the same canonical JSON the bridge accepts.
        canonical = validate_cashflow_schedule_json(payload)
        round_tripped = CashFlowSchedule.from_json(canonical)
        assert len(round_tripped.get_flows()) == len(schedule.get_flows())

    def test_merge_cashflow_schedules(self) -> None:
        from finstack_quant.cashflows.builder import Notional, merge_cashflow_schedules
        from finstack_quant.core.dates import DayCount

        a = self._bond()
        b = self._bond().scale_amounts(-1.0)
        merged = merge_cashflow_schedules([a, b], Notional.par(1_000_000.0, "USD"), DayCount.ACT_360)
        assert len(merged.get_flows()) == 12
        total = sum(f.amount.amount for f in merged.get_flows())
        assert total == pytest.approx(0.0, abs=1e-9)


class TestAccrual:
    @staticmethod
    def _semiannual_bond() -> CashFlowSchedule:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import (
            CashFlowSchedule,
            FixedCouponSpec,
            ScheduleParams,
        )

        return (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360()))
            .build(None)
        )

    def test_accrued_interest_golden(self) -> None:
        from finstack_quant.cashflows.accrual import accrued_interest_amount

        schedule = self._semiannual_bond()
        # Hand-computed: coupon 25,000 per period; 30/360 elapsed 90/360 = 0.25 of a
        # 0.5-year period -> 25,000 * 0.5 = 12,500.00 exactly (see plan Task 5).
        accrued = accrued_interest_amount(schedule, dt.date(2025, 4, 15))
        assert accrued.currency.code == "USD"
        assert accrued.amount == pytest.approx(12_500.0, abs=1e-6)

    def test_accrued_zero_outside_periods(self) -> None:
        from finstack_quant.cashflows.accrual import accrued_interest_amount

        schedule = self._semiannual_bond()
        assert accrued_interest_amount(schedule, dt.date(2024, 12, 31)).amount == pytest.approx(0.0)

    def test_accrual_method_is_hashable(self) -> None:
        from finstack_quant.cashflows.accrual import AccrualMethod

        assert hash(AccrualMethod.LINEAR) == hash(AccrualMethod.LINEAR)
        assert AccrualMethod.LINEAR in {AccrualMethod.LINEAR}
        assert AccrualMethod.LINEAR != AccrualMethod.COMPOUNDED

    def test_accrual_index_matches_one_shot(self) -> None:
        from finstack_quant.cashflows.accrual import (
            AccrualConfig,
            AccrualIndex,
            AccrualMethod,
            accrued_interest_amount,
        )

        schedule = self._semiannual_bond()
        cfg = AccrualConfig(method=AccrualMethod.LINEAR)
        index = AccrualIndex.build(schedule, cfg)
        for as_of in (dt.date(2025, 2, 1), dt.date(2025, 4, 15), dt.date(2025, 9, 1)):
            assert index.accrued_at(as_of).amount == pytest.approx(
                accrued_interest_amount(schedule, as_of, cfg).amount, abs=1e-12
            )

    def test_accrual_index_build_keyword_args(self) -> None:
        """Regression test: AccrualIndex.build accepts 'schedule=' keyword argument.

        This ensures the parameter name matches the advertised text_signature and .pyi.
        """
        from finstack_quant.cashflows.accrual import AccrualConfig, AccrualIndex, AccrualMethod

        schedule = self._semiannual_bond()
        cfg = AccrualConfig(method=AccrualMethod.LINEAR)

        # Test keyword form (regression test for signature/text_signature mismatch)
        index_kw = AccrualIndex.build(schedule=schedule, config=cfg)

        # Test positional form still works
        index_pos = AccrualIndex.build(schedule, cfg)

        # Both forms produce equivalent results
        as_of = dt.date(2025, 4, 15)
        assert index_kw.accrued_at(as_of).amount == pytest.approx(index_pos.accrued_at(as_of).amount)

    def test_ex_coupon_rule(self) -> None:
        from finstack_quant.cashflows.accrual import ExCouponRule

        rule = ExCouponRule(days_before_coupon=7)
        assert rule.ex_date(dt.date(2025, 7, 15)) == dt.date(2025, 7, 8)
        assert rule.days_before_coupon == 7
        assert rule.calendar_id is None
        with pytest.raises(ValueError, match="exceeds the maximum"):
            ExCouponRule(days_before_coupon=400).ex_date(dt.date(2025, 7, 15))


class TestAggregation:
    def test_aggregate_cashflows_checked(self) -> None:
        from finstack_quant.cashflows.aggregation import aggregate_cashflows_checked

        flows = [
            (dt.date(2025, 1, 15), Money(50_000.0, "USD")),
            (dt.date(2025, 7, 15), Money(50_000.0, "USD")),
        ]
        total = aggregate_cashflows_checked(flows, "USD")
        assert total.amount == pytest.approx(100_000.0)
        assert total.currency.code == "USD"
        # Currency mismatch is rejected, never silently converted.
        with pytest.raises(ValueError, match="Currency mismatch"):
            aggregate_cashflows_checked(flows, "EUR")

    def test_aggregate_cashflows_checked_empty(self) -> None:
        from finstack_quant.cashflows.aggregation import aggregate_cashflows_checked

        total = aggregate_cashflows_checked([], "USD")
        assert total.amount == pytest.approx(0.0)

    def test_aggregate_by_period(self) -> None:
        from finstack_quant.cashflows.aggregation import aggregate_by_period
        from finstack_quant.core.dates import build_periods

        flows = [
            (dt.date(2025, 3, 15), Money(100.0, "USD")),
            (dt.date(2025, 8, 1), Money(25.0, "USD")),
            (dt.date(2025, 8, 2), Money(25.0, "EUR")),
        ]
        periods = build_periods("2025Q1..Q4").periods
        out = aggregate_by_period(flows, periods)
        assert out["2025Q1"]["USD"].amount == pytest.approx(100.0)
        assert out["2025Q3"]["USD"].amount == pytest.approx(25.0)
        assert out["2025Q3"]["EUR"].amount == pytest.approx(25.0)
        # Periods with no flows are omitted.
        assert "2025Q2" not in out
        assert out.periods == ["2025Q1", "2025Q3"]
        frame = out.to_dataframe()
        assert list(frame.columns) == ["period", "currency", "amount"]
        assert len(frame) == 3

    def test_calendar_year_ladder(self) -> None:
        from finstack_quant.cashflows.aggregation import calendar_year_ladder

        rows = calendar_year_ladder(
            [dt.date(2027, 3, 15), dt.date(2027, 9, 15), dt.date(2034, 3, 15)],
            ["coupon", "fixed", "Notional"],
            [100.0, 50.0, 1000.0],
            [90.0, 40.0, 700.0],
        )
        assert rows[0] == (2027, 150.0, 0.0, 130.0)
        assert rows[1] == (2034, 0.0, 1000.0, 700.0)

    def test_calendar_year_ladder_rejects_unknown_kind(self) -> None:
        from finstack_quant.cashflows.aggregation import calendar_year_ladder

        with pytest.raises(ValueError, match="unknown cashflow kind"):
            calendar_year_ladder(
                [dt.date(2027, 3, 15)],
                ["mystery"],
                [100.0],
                [90.0],
            )

    def test_calendar_year_ladder_rejects_non_finite_amount_and_pv(self) -> None:
        from finstack_quant.cashflows.aggregation import calendar_year_ladder

        with pytest.raises(ValueError, match="amount must be finite"):
            calendar_year_ladder(
                [dt.date(2027, 3, 15)],
                ["coupon"],
                [float("nan")],
                [90.0],
            )
        with pytest.raises(ValueError, match="pv must be finite"):
            calendar_year_ladder(
                [dt.date(2027, 3, 15)],
                ["coupon"],
                [100.0],
                [float("inf")],
            )

    def test_aggregate_by_period_keyword_args(self) -> None:
        """Regression test: aggregate_by_period accepts keyword arguments.

        This ensures the parameter names match the advertised text_signature
        and .pyi (see the AccrualIndex.build lesson from task 5).
        """
        from finstack_quant.cashflows.aggregation import aggregate_by_period
        from finstack_quant.core.dates import build_periods

        flows = [(dt.date(2025, 3, 15), Money(100.0, "USD"))]
        periods = build_periods("2025Q1..Q4").periods
        out = aggregate_by_period(flows=flows, periods=periods)
        assert out["2025Q1"]["USD"].amount == pytest.approx(100.0)

    def test_aggregate_cashflows_checked_keyword_args(self) -> None:
        """Regression test: aggregate_cashflows_checked accepts keyword arguments."""
        from finstack_quant.cashflows.aggregation import aggregate_cashflows_checked

        flows = [(dt.date(2025, 1, 15), Money(50_000.0, "USD"))]
        total = aggregate_cashflows_checked(flows=flows, target="USD")
        assert total.amount == pytest.approx(50_000.0)


class TestTypedJsonEquivalence:
    def test_typed_accrual_matches_json_bridge(self) -> None:
        from finstack_quant.cashflows import accrued_interest
        from finstack_quant.cashflows.accrual import accrued_interest_amount

        schedule = TestAccrual._semiannual_bond()
        typed = accrued_interest_amount(schedule, dt.date(2025, 4, 15))
        via_json = accrued_interest(schedule.to_json(), "2025-04-15")
        assert typed.amount == pytest.approx(via_json, abs=1e-9)
        assert typed.currency.code == "USD"
        assert typed.amount == pytest.approx(12_500.0, abs=1e-6)

    def test_typed_flows_match_dated_flows_json(self) -> None:
        import json

        from finstack_quant.cashflows import dated_flows_json
        from finstack_quant.cashflows.primitives import is_cash_settlement_kind

        schedule = TestCashFlowBuilder._quarterly_bond()
        bridge_rows = json.loads(dated_flows_json(schedule.to_json()))
        typed_cash = [f for f in schedule.get_flows() if is_cash_settlement_kind(f.kind)]
        assert len(bridge_rows) == len(typed_cash)


class TestTypedTwinsAndWire:
    """Typed twins, spec wire round-trips, presets and DataFrame exits added by the parity remediation."""

    def _schedule(self) -> object:
        from decimal import Decimal

        from finstack_quant.cashflows.builder import CashFlowSchedule, FixedCouponSpec, ScheduleParams

        return (
            CashFlowSchedule
            .builder()
            .principal(Money(1_000_000.0, "USD"), dt.date(2025, 1, 15), dt.date(2026, 1, 15))
            .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360()))
            .build(None)
        )

    def test_build_cashflow_schedule_typed_matches_json(self) -> None:
        import json

        from finstack_quant.cashflows import build_cashflow_schedule, build_cashflow_schedule_json, dated_flows

        spec = {
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue": "2025-01-15",
            "maturity": "2026-01-15",
            "coupon_program": [
                {
                    "kind": "fixed",
                    "spec": {
                        "rate": "0.05",
                        "frequency": {"count": 6, "unit": "months"},
                        "day_count": "30_360",
                        "calendar_id": "weekends_only",
                    },
                }
            ],
        }
        typed = build_cashflow_schedule(spec)
        via_json = json.loads(build_cashflow_schedule_json(json.dumps(spec)))
        assert len(typed.get_flows()) == len(via_json["flows"])
        flows = dated_flows(typed)
        assert flows
        assert isinstance(flows[0][1], Money)
        assert dated_flows(typed.to_json()) == flows

    def test_schedule_from_flows_and_meta(self) -> None:
        import pickle

        from finstack_quant.cashflows import (
            ScheduleBuildOpts,
            schedule_from_classified_flows,
            schedule_from_dated_flows,
        )
        from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule, Notional
        from finstack_quant.cashflows.primitives import CashFlow, CFKind
        from finstack_quant.core.dates import DayCount

        dated = [(dt.date(2025, 6, 15), Money(100.0, "USD"))]
        s1 = schedule_from_dated_flows(dated, "fixed", DayCount.ACT_360)
        assert s1.get_flows()[0].kind == CFKind.FIXED
        flow = CashFlow(dt.date(2025, 6, 15), Money(100.0, "USD"), CFKind.PIK)
        opts = ScheduleBuildOpts(notional_hint=Money(500.0, "USD"), meta=CashFlowMeta("projected"))
        s2 = schedule_from_classified_flows([flow], DayCount.ACT_360, opts)
        assert s2.get_notional().initial.amount == pytest.approx(500.0)
        assert s2.get_meta().representation == "projected"
        assert s2.with_representation("placeholder").get_meta().representation == "placeholder"
        s3 = CashFlowSchedule.from_flows([flow], Notional.par(1.0, "USD"), DayCount.ACT_360)
        assert pickle.loads(pickle.dumps(s3.get_meta())).representation == "contractual"  # noqa: S301 - trusted in-process round trip
        s4 = CashFlowSchedule.from_parts([flow], Notional.par(1.0, "USD"), DayCount.ACT_360, CashFlowMeta())
        assert s4.get_flows()[0].amount.amount == 100.0

    def test_from_flows_accepts_dataframe(self) -> None:
        pd = pytest.importorskip("pandas")
        from finstack_quant.cashflows.builder import CashFlowSchedule, Notional
        from finstack_quant.core.dates import DayCount

        original = self._schedule()
        frame = original.to_dataframe()
        rebuilt = CashFlowSchedule.from_flows(frame, original.get_notional(), original.get_day_count())
        assert [f.amount.amount for f in rebuilt.get_flows()] == pytest.approx([
            f.amount.amount for f in original.get_flows()
        ])
        assert isinstance(frame, pd.DataFrame)
        with pytest.raises(ValueError, match="missing required column"):
            CashFlowSchedule.from_flows(frame.drop(columns=["kind"]), Notional.par(1.0, "USD"), DayCount.ACT_360)

    def test_calendar_year_ladder_dataframe(self) -> None:
        pytest.importorskip("pandas")
        schedule = self._schedule()
        pvs = [f.amount.amount for f in schedule.get_flows()]
        ladder = schedule.calendar_year_ladder(pvs)
        assert list(ladder.columns) == ["year", "non_principal", "principal", "pv"]
        assert ladder["pv"].sum() == pytest.approx(sum(pvs))
        with pytest.raises(ValueError, match="equal lengths"):
            schedule.calendar_year_ladder(pvs[:-1])

    def test_spec_wire_round_trips_and_pickle(self) -> None:
        from decimal import Decimal
        import pickle

        from finstack_quant.cashflows.builder import (
            AmortizationSpec,
            FeeBase,
            FeeSpec,
            FixedCouponSpec,
            FloatingCouponSpec,
            FloatingRateSpec,
            Notional,
            PrepaymentModelSpec,
            RecoveryModelSpec,
            ScheduleParams,
            StepUpCouponSpec,
        )
        from finstack_quant.core.dates import DayCount

        params = ScheduleParams("3M", DayCount.ACT_360, "weekends_only")
        assert params.business_day_convention is not None
        assert params.stub is not None
        assert params.roll_rule is not None
        specs = [
            params,
            FixedCouponSpec(Decimal("0.05"), params),
            FloatingRateSpec.sofr(50),
            FloatingCouponSpec(FloatingRateSpec.euribor_3m("100"), params),
            StepUpCouponSpec(0.05, [(dt.date(2026, 1, 15), 0.06)], params),
            AmortizationSpec.linear_to(Money(0.0, "USD")),
            Notional.par(1.0, "USD"),
            FeeBase.undrawn(Money(10.0, "USD")),
            FeeSpec.periodic_bp(FeeBase.DRAWN, 25, "3M", DayCount.ACT_360, "weekends_only"),
            PrepaymentModelSpec.psa_100(),
            RecoveryModelSpec(0.4, 12),
        ]
        for spec in specs:
            restored = type(spec).from_json(spec.to_json())
            assert restored.to_json() == spec.to_json()
            assert pickle.loads(pickle.dumps(spec)).to_json() == spec.to_json()  # noqa: S301 - trusted in-process round trip
            assert "Debug" not in repr(spec)

    def test_presets_and_getters(self) -> None:
        from finstack_quant.cashflows.builder import (
            FeeBase,
            FeeSpec,
            FloatingRateSpec,
            Notional,
            OvernightCompoundingMethod,
        )
        from finstack_quant.core.dates import DayCount

        sofr = FloatingRateSpec.sofr(50)
        assert sofr.index_id == "USD-SOFR"
        assert sofr.overnight_compounding == OvernightCompoundingMethod.COMPOUNDED_IN_ARREARS
        assert sofr.overnight_basis == DayCount.ACT_360
        assert sofr.reset_lag_days == 0
        assert FloatingRateSpec.sonia(10).index_id == "GBP-SONIA"
        assert FloatingRateSpec.euribor_3m(10).index_tenor is not None
        assert sofr.fallback is not None
        assert sofr.gearing_includes_spread is True
        assert Notional.par(5.0, "USD").amort.kind == "none"
        fee = FeeSpec.periodic_bp(FeeBase.DRAWN, 25, "3M", DayCount.ACT_360, "weekends_only")
        assert fee.kind == "periodic_bp"
        assert fee.business_day_convention is not None
        assert fee.stub is not None
        assert fee.date is None
        assert FeeBase.DRAWN.kind == "drawn"

    def test_schedule_params_validation_and_str_decimals(self) -> None:
        from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams
        from finstack_quant.core.dates import DayCount

        with pytest.raises(ValueError, match="unknown calendar_id"):
            ScheduleParams("3M", DayCount.ACT_360, "not-a-calendar")
        with pytest.raises(ValueError, match="payment_lag_days"):
            ScheduleParams("3M", DayCount.ACT_360, "weekends_only", payment_lag_days=-1)
        params = ScheduleParams("3M", DayCount.ACT_360, "weekends_only")
        assert str(FixedCouponSpec("0.05", params).rate) == "0.05"

    def test_credit_rate_error_text(self) -> None:
        from finstack_quant.cashflows import cpr_to_smm

        with pytest.raises(ValueError, match=r"must be a decimal in \[0,1\]"):
            cpr_to_smm(-0.05)

    def test_period_aggregation_wire(self) -> None:
        import pickle

        from finstack_quant.cashflows.aggregation import PeriodAggregation, aggregate_by_period
        from finstack_quant.core.dates import build_periods

        agg = aggregate_by_period([(dt.date(2025, 3, 15), Money(100.0, "USD"))], build_periods("2025Q1..Q4").periods)
        assert isinstance(agg, PeriodAggregation)
        assert agg.get("2025Q1", "USD").amount == pytest.approx(100.0)
        assert agg.get("2025Q1", "EUR") is None
        assert len(agg) == 1
        restored = pickle.loads(pickle.dumps(agg))  # noqa: S301 - trusted in-process round trip
        assert restored.to_json() == agg.to_json()
        assert agg.to_dict()["2025Q1"]["USD"].amount == pytest.approx(100.0)


def test_cashflow_metadata_dataframe_roundtrip() -> None:
    import json

    from finstack_quant.cashflows import build_cashflow_schedule
    from finstack_quant.cashflows.accrual import accrued_interest_amount
    from finstack_quant.cashflows.builder import CashFlowSchedule
    from finstack_quant.cashflows.primitives import CashFlow, CashFlowAccrual, CFKind
    from finstack_quant.core.dates import DayCount

    spec = {
        "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
        "issue": "2025-01-01",
        "maturity": "2025-07-01",
        "coupon_program": [
            {
                "kind": "fixed",
                "spec": {
                    "rate": "0.1",
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_360",
                    "business_day_convention": "unadjusted",
                    "calendar_id": "weekends_only",
                    "stub": "short_back",
                    "payment_lag_days": 2,
                },
            }
        ],
        "principal_events": [
            {
                "date": "2025-02-01",
                "kind": "notional",
                "delta": {"amount": "100", "currency": "USD"},
                "cash": {"amount": "98", "currency": "USD"},
            }
        ],
    }
    original = build_cashflow_schedule(spec)
    rebuilt = CashFlowSchedule.from_flows(
        original.to_dataframe(), original.get_notional(), original.get_day_count(), original.get_meta()
    )
    assert json.loads(rebuilt.to_json()) == json.loads(original.to_json())
    assert accrued_interest_amount(rebuilt, "2025-05-01").amount == pytest.approx(
        accrued_interest_amount(original, "2025-05-01").amount
    )
    coupon = original.coupons()[0]
    assert coupon.accrual.start == dt.date(2025, 1, 1)
    assert coupon.accrual.calendar_id == "weekends_only"
    metadata = CashFlowAccrual(
        "2025-01-01", "2025-04-01", DayCount.ACT_360, projected_index_rate=0.03, calendar_id="weekends_only"
    )
    row = CashFlow("2025-04-03", Money(25, "USD"), CFKind.FIXED).with_accrual(metadata)
    assert row.accrual.projected_index_rate == 0.03
    draw = CashFlow("2025-02-01", Money(-98, "USD"), CFKind.NOTIONAL).with_principal_delta(Money(100, "USD"))
    draw.validate()
    assert CashFlow.from_json(draw.to_json()).principal_delta.amount == 100
