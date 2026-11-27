"""Regression tests for Layer 4c Python bindings (rates, fixed income, and shared builders)."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations.instruments import (
    AgencyCmo,
    AgencyMbsPassthrough,
    Bond,
    BondBreakevenLadder,
    BondBreakevenRow,
    BondFuture,
    CapFloor,
    ConvertibleBond,
    InterestRateFuture,
    InterestRateSwap,
    RevolvingCredit,
    Swaption,
    TermLoan,
    bond_breakeven_ladder,
)


def test_h26_agency_cmo_and_mbs_plain_string_ids() -> None:
    cmo = AgencyCmo.example()
    assert isinstance(cmo.deal_name, str)
    edited_cmo = cmo.to_builder().deal_name("FNMA-TEST").build()
    assert edited_cmo.deal_name == "FNMA-TEST"

    mbs = AgencyMbsPassthrough.example()
    assert isinstance(mbs.pool_id, str)
    edited_mbs = mbs.to_builder().pool_id("POOL-TEST").build()
    assert edited_mbs.pool_id == "POOL-TEST"


def test_h28_bond_breakeven_ladder_wire_and_config() -> None:
    bond = Bond.example()
    market = MarketContext().insert(
        DiscountCurve(
            bond.discount_curve_id,
            bond.issue_date,
            [(0.0, 1.0), (10.0, 0.70)],
            interp="log_linear",
            extrapolation="extend",
            day_count="act_365f",
        )
    )
    cfg = {"target": "z_spread", "mode": "linear"}
    ladder_dict = bond_breakeven_ladder(bond, market, bond.issue_date, ["3M", "1Y"], cfg)
    ladder_json = bond_breakeven_ladder(bond, market, bond.issue_date, ["3M", "1Y"], json.loads(json.dumps(cfg)))
    with pytest.raises(ValueError, match="invalid breakeven config: invalid type: string"):
        bond_breakeven_ladder(bond, market, bond.issue_date, ["3M", "1Y"], json.dumps(cfg))

    assert ladder_dict.config == cfg
    assert ladder_json.config == cfg
    assert ladder_dict.to_json() == ladder_json.to_json()
    assert BondBreakevenLadder.from_json(ladder_dict.to_json()).to_dict() == ladder_dict.to_dict()
    assert "BondBreakevenLadder(" in repr(ladder_dict)

    reducer, args = ladder_dict.__reduce__()
    assert reducer(*args).to_dict() == ladder_dict.to_dict()

    row = ladder_dict.rows[0]
    row_reducer, row_args = row.__reduce__()
    assert row_reducer(*row_args).to_dict() == row.to_dict()
    assert BondBreakevenRow.from_json(row.to_json()).to_dict() == row.to_dict()
    assert "BondBreakevenRow(" in repr(row)


def test_h29_term_loan_builder_currency_accepts_currency_and_str() -> None:
    loan_from_ccy = TermLoan.example().to_builder().currency(Currency("USD")).build()
    loan_from_str = TermLoan.example().to_builder().currency("USD").build()
    assert loan_from_ccy.currency == "USD"
    assert loan_from_str.currency == "USD"


def test_h30_futures_expose_full_contract_getters() -> None:
    bf = BondFuture.example()
    assert isinstance(bf.delivery_start, datetime.date)
    assert isinstance(bf.contract_specs, dict)
    assert len(bf.deliverable_basket) > 0
    assert bf.ctd_bond_id is None or isinstance(bf.ctd_bond_id, str)
    assert bf.ctd_bond is None or isinstance(bf.ctd_bond, Bond)
    assert isinstance(bf.discount_curve_id, str)
    assert bf.repo_curve_id is None or isinstance(bf.repo_curve_id, str)

    irf = InterestRateFuture.example()
    assert irf.fixing_date is None or isinstance(irf.fixing_date, datetime.date)
    assert irf.period_start is None or isinstance(irf.period_start, datetime.date)
    assert irf.period_end is None or isinstance(irf.period_end, datetime.date)
    assert irf.day_count is not None
    assert isinstance(irf.contract_specs, dict)
    assert isinstance(irf.discount_curve_id, str)
    assert isinstance(irf.forward_curve_id, str)
    assert irf.compounding is not None
    assert irf.index_id is None or isinstance(irf.index_id, str)
    assert irf.fixing_calendar_id is None or isinstance(irf.fixing_calendar_id, str)
    assert irf.vol_surface_id is None or isinstance(irf.vol_surface_id, str)


def test_m1_core_instruments_to_builder_round_trip() -> None:
    for cls, new_id in [
        (Bond, "BOND-EDIT"),
        (TermLoan, "TL-EDIT"),
        (RevolvingCredit, "RC-EDIT"),
        (ConvertibleBond, "CB-EDIT"),
        (InterestRateSwap, "IRS-EDIT"),
        (Swaption, "SW-EDIT"),
        (CapFloor, "CF-EDIT"),
    ]:
        rebuilt = cls.example().to_builder().id(new_id).build()
        assert rebuilt.id == new_id
