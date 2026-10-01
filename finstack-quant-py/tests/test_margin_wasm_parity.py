"""Cross-host goldens for the margin surface bound in WASM.

``tests/data/margin_wasm_parity.json`` holds the Python outputs for a fixed set
of inputs. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/margin_parity.test.mjs`` pins the WASM
``margin`` namespace (classes and free-function twins) to the same file, so
both hosts are held to one set of numbers and wire shapes.

Regenerate the file after an intentional change with
``uv run --no-sync python finstack-quant-py/tests/test_margin_wasm_parity.py``.
"""

from __future__ import annotations

from datetime import date
import json
import math
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.core.market_data import DiscountCurve, HazardCurve
from finstack_quant.margin import (
    CONSTANTS,
    ClearingStatus,
    CollateralAssetClass,
    CsaSpec,
    EligibleCollateralSchedule,
    ExcessCollateral,
    ExposureProfile,
    FrtbSbaEngine,
    FrtbSensitivities,
    FundingConfig,
    Haircut01,
    HaircutImCalculator,
    ImDecayProfile,
    ImMethodology,
    ImProfile,
    MarginCallType,
    MarginFundingCost,
    MarginTenor,
    MarginUtilization,
    NettingSetId,
    SaCcrEngine,
    SaCcrNettingSetConfig,
    SaCcrTrade,
    ScheduleImCalculator,
    SimmCalculator,
    SimmCurvatureSensitivity,
    SimmSensitivities,
    VmCalculator,
    compute_mva,
    frtb_sba_charge,
    im_profile_from_simm,
    saccr_ead,
)

GOLDEN = Path(__file__).parent / "data" / "margin_wasm_parity.json"

ASSET_CLASSES = [
    "cash",
    "government_bonds",
    "agency_bonds",
    "covered_bonds",
    "corporate_bonds",
    "equity",
    "gold",
    "mutual_funds",
]


LIMITED_SCHEDULE = {
    "eligible": [
        {"asset_class": "cash", "haircut": 0.0, "fx_haircut_addon": 0.08},
        {"asset_class": "equity", "haircut": 0.15, "fx_haircut_addon": 0.08, "concentration_limit": 0.5},
    ],
    "rehypothecation_allowed": False,
}


def _wire(obj: Any) -> Any:
    """Canonical JSON of a typed value, without the run-specific ``meta`` stamp."""
    value = json.loads(obj.to_json())
    if isinstance(value, dict):
        value.pop("meta", None)
    return value


def _im_collateral(result: Any) -> dict[str, Any]:
    return {
        "gross_initial_margin": result.gross_initial_margin,
        "required_collateral": result.required_collateral,
        "current_collateral": result.current_collateral,
        "transfer": result.transfer,
        "currency": result.currency,
        "segregated": result.segregated,
    }


def simm_sensitivities() -> SimmSensitivities:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 50_000.0)
    sens.add_ir_delta("USD", "10Y", -20_000.0)
    sens.add_ir_vega("USD", "5Y", 4_000.0)
    sens.add_credit_qualifying_delta("financial", "ACME", "5Y", 3_000.0)
    sens.add_credit_qualifying_vega("financial", "ACME", "5Y", 500.0)
    sens.add_credit_non_qualifying_delta("RMBS_A", "5Y", 1_500.0)
    sens.add_credit_non_qualifying_vega("RMBS_A", "5Y", 200.0)
    sens.add_equity_delta("AAPL", 10_000.0)
    sens.add_equity_vega("AAPL", 800.0)
    sens.add_fx_delta("EUR", 5_000.0)
    sens.add_fx_vega("EUR", "USD", 300.0)
    sens.add_commodity_delta("Crude", 2_000.0)
    sens.add_commodity_vega("Crude", 150.0)
    sens.add_curvature(SimmCurvatureSensitivity("equity", "residual", "AAPL", "1Y", 1_000.0))
    return sens


def frtb_sensitivities() -> FrtbSensitivities:
    sens = FrtbSensitivities("USD")
    sens.add_girr_delta("5Y", 100_000.0)
    sens.add_girr_delta("10Y", -40_000.0, "EUR")
    sens.add_girr_inflation_delta(5_000.0)
    sens.add_girr_xccy_basis_delta(2_000.0, "EUR")
    sens.add_girr_vega("1Y", "5Y", 3_000.0)
    sens.add_girr_curvature(-1_500.0, -2_500.0)
    sens.add_csr_nonsec_delta("ACME", 3, "5Y", "bond", 5_000.0)
    sens.add_csr_nonsec_vega("ACME", 3, "1Y", 700.0)
    sens.add_csr_nonsec_curvature("ACME", 3, -300.0, -500.0)
    sens.add_csr_sec_ctp_delta("TRANCHE_A", 2, "5Y", "cds", 1_200.0)
    sens.add_csr_sec_ctp_vega("TRANCHE_A", 2, "1Y", 150.0)
    sens.add_csr_sec_ctp_curvature("TRANCHE_A", 2, -80.0, -120.0)
    sens.add_csr_sec_nonctp_delta("RMBS_A", 1, "5Y", "bond", 900.0)
    sens.add_csr_sec_nonctp_vega("RMBS_A", 1, "1Y", 110.0)
    sens.add_csr_sec_nonctp_curvature("RMBS_A", 1, -60.0, -90.0)
    sens.add_equity_delta("AAPL", 1, 25_000.0)
    sens.add_equity_repo_delta("AAPL", 1, 1_000.0)
    sens.add_equity_vega("AAPL", 1, "1Y", 2_000.0)
    sens.add_equity_curvature("AAPL", 1, -1_000.0, -2_000.0)
    sens.add_fx_delta("EUR", "USD", 10_000.0)
    sens.add_fx_vega("EUR", "USD", "1Y", 400.0)
    sens.add_fx_curvature("EUR", "USD", -200.0, -350.0)
    sens.add_commodity_delta("WTI", 2, "1Y", "cushing", 2_000.0)
    sens.add_commodity_vega("WTI", 2, "1Y", 250.0)
    sens.add_commodity_curvature("WTI", 2, -100.0, -180.0)
    sens.add_drc_position("ACME", 1_000_000.0, 4, "corporate", "senior_unsecured", "corporate", 2.0)
    sens.add_drc_position("ACME", -250_000.0, 4, "corporate", "subordinated", "corporate", 0.5, -1_000.0)
    sens.add_rrao_position("EXOTIC_1", 1_000_000.0, True)
    sens.add_rrao_position("GAP_1", 2_000_000.0)
    return sens


def saccr_trades() -> list[SaCcrTrade]:
    return [
        SaCcrTrade("t1", "interest_rate", 1_000_000.0, "2025-01-15", "2030-01-15", "USD", "USD", 1.0, 1.0, 12_000.0),
        SaCcrTrade("t2", "interest_rate", 500_000.0, "2025-01-15", "2027-01-15", "USD", "USD", -1.0, -1.0, -3_000.0),
    ]


def compute() -> dict[str, Any]:
    """Every golden value, computed through the Python bindings."""
    out: dict[str, Any] = {"constants": CONSTANTS}

    out["labels"] = {
        "im_methodology": [
            str(ImMethodology.haircut()),
            str(ImMethodology.simm()),
            str(ImMethodology.schedule()),
            str(ImMethodology.internal_model()),
            str(ImMethodology.clearing_house()),
            str(ImMethodology.from_str("simm")),
        ],
        "margin_tenor": [
            str(MarginTenor.daily()),
            str(MarginTenor.weekly()),
            str(MarginTenor.monthly()),
            str(MarginTenor.on_demand()),
            str(MarginTenor.from_str("weekly")),
        ],
        "margin_call_type": [
            str(MarginCallType.initial_margin()),
            str(MarginCallType.variation_margin_post()),
            str(MarginCallType.variation_margin_collect()),
            str(MarginCallType.top_up()),
            str(MarginCallType.substitution()),
            str(MarginCallType.from_str("top_up")),
        ],
        "collateral_asset_class": [
            str(CollateralAssetClass.cash()),
            str(CollateralAssetClass.government_bonds()),
            str(CollateralAssetClass.agency_bonds()),
            str(CollateralAssetClass.covered_bonds()),
            str(CollateralAssetClass.corporate_bonds()),
            str(CollateralAssetClass.equity()),
            str(CollateralAssetClass.gold()),
            str(CollateralAssetClass.mutual_funds()),
            str(CollateralAssetClass.from_str("gold")),
        ],
        "clearing_status": [
            ClearingStatus.bilateral().is_bilateral,
            ClearingStatus.cleared("LCH").is_cleared,
        ],
    }
    out["collateral_asset_class"] = {
        "standard_haircut": {label: CollateralAssetClass.from_str(label).standard_haircut() for label in ASSET_CLASSES},
        "fx_addon": {label: CollateralAssetClass.from_str(label).fx_addon() for label in ASSET_CLASSES},
    }
    out["netting_set_id"] = {
        "bilateral": _wire(NettingSetId.bilateral("CPTY", "CSA")),
        "cleared": _wire(NettingSetId.cleared("LCH")),
    }

    usd = CsaSpec.usd_regulatory()
    out["csa"] = {
        "usd": _wire(usd),
        "eur": _wire(CsaSpec.eur_regulatory()),
        "regulatory_gbp": _wire(CsaSpec.regulatory("GBP", "GBP-CSA", "GBP-SONIA")),
        "with_vm_threshold": _wire(usd.with_vm_threshold(1_000_000.0, 250_000.0, 10_000.0, 500_000.0)),
        "with_vm_threshold_defaults": _wire(usd.with_vm_threshold(1_000_000.0, 250_000.0)),
        "with_im": _wire(usd.with_im("schedule", 10, 50_000_000.0, 500_000.0, False)),
        "with_im_default_segregated": _wire(usd.with_im("simm", 10, 0.0, 0.0)),
        "apply_im_terms": _im_collateral(usd.apply_im_terms(60_000_000.0, 5_000_000.0)),
    }

    bcbs = EligibleCollateralSchedule.bcbs_standard()
    cash_only = EligibleCollateralSchedule.cash_only()
    treasuries = EligibleCollateralSchedule.us_treasuries()
    out["schedule"] = {
        "bcbs": _wire(bcbs),
        "cash_only": _wire(cash_only),
        "us_treasuries": _wire(treasuries),
        "is_eligible": [bcbs.is_eligible("equity"), cash_only.is_eligible("equity")],
        "haircut_for": [bcbs.haircut_for("government_bonds"), cash_only.haircut_for("equity")],
        "haircut_for_maturity": [
            bcbs.haircut_for_maturity("government_bonds", 0.5),
            bcbs.haircut_for_maturity("government_bonds", 7.0),
        ],
        "limited": LIMITED_SCHEDULE,
        "breaches": EligibleCollateralSchedule
        .from_json(json.dumps(LIMITED_SCHEDULE))
        .check_concentration_limits([("equity", 80.0), ("cash", 20.0)])
        .to_dict("records"),
        "no_breaches": bcbs.check_concentration_limits([("government_bonds", 80.0), ("cash", 20.0)]).to_dict("records"),
    }

    utilization = MarginUtilization(8_000_000.0, 10_000_000.0, "USD")
    excess = ExcessCollateral(12_000_000.0, 10_000_000.0, "USD")
    funding_cost = MarginFundingCost(10_000_000.0, 0.05, 0.03, "USD")
    haircut01 = Haircut01(10_000_000.0, 0.02, "USD")
    out["metrics"] = {
        "utilization": {
            "wire": _wire(utilization),
            "ratio": utilization.ratio,
            "is_adequate": utilization.is_adequate(),
            "shortfall": utilization.shortfall(),
        },
        "excess": {
            "wire": _wire(excess),
            "has_excess": excess.has_excess(),
            "has_shortfall": excess.has_shortfall(),
            "excess_percentage": excess.excess_percentage(),
        },
        "funding_cost": {
            "wire": _wire(funding_cost),
            "spread": funding_cost.spread(),
            "cost_for_period": funding_cost.cost_for_period(0.25),
        },
        "haircut01": {"wire": _wire(haircut01), "haircut_bp": haircut01.haircut_bp()},
    }

    funding = FundingConfig(50.0, 20.0, None, 35.0)
    symmetric = FundingConfig(50.0)
    linear = ImDecayProfile.linear_to_maturity(5.0)
    sqrt_time = ImDecayProfile.sqrt_time(4.0)
    out["xva"] = {
        "funding": {
            "wire": _wire(funding),
            "effective_benefit_bp": funding.effective_benefit_bp(),
            "effective_margin_spread_bp": funding.effective_margin_spread_bp(),
            "symmetric_wire": _wire(symmetric),
            "symmetric_benefit_bp": symmetric.effective_benefit_bp(),
            "symmetric_margin_spread_bp": symmetric.effective_margin_spread_bp(),
        },
        "decay": {
            "constant": _wire(ImDecayProfile.constant()),
            "linear": _wire(linear),
            "sqrt_time": _wire(sqrt_time),
            "factors": [ImDecayProfile.constant().factor(3.0), linear.factor(3.0), sqrt_time.factor(3.0)],
        },
    }

    sens = simm_sensitivities()
    calculator = SimmCalculator()
    im = calculator.calculate_from_sensitivities(sens, "USD", "2025-01-15")
    doubled = sens.scaled(2.0)
    in_eur = sens.scaled_to_currency("EUR", 0.9)
    merged = simm_sensitivities()
    merged.merge(sens)
    out["simm"] = {
        "wire": _wire(sens),
        "base_currency": sens.base_currency,
        "is_empty": [SimmSensitivities("USD").is_empty(), sens.is_empty()],
        "total_ir_delta": sens.total_ir_delta(),
        "total_equity_delta": sens.total_equity_delta(),
        "scaled_total_ir_delta": doubled.total_ir_delta(),
        "eur": {"base_currency": in_eur.base_currency, "total_ir_delta": in_eur.total_ir_delta()},
        "merged_total_ir_delta": merged.total_ir_delta(),
        "calculator": {"version": calculator.version, "mpor_days": calculator.mpor_days},
        "calculator_mpor_override": SimmCalculator("v2_6", 5).mpor_days,
        "im": _wire(im),
        "breakdown_keys": im.breakdown_keys(),
        "breakdown_ir_delta": im.breakdown_amount("IR_Delta"),
        "breakdown_missing": im.breakdown_amount("nope"),
    }

    profile = im_profile_from_simm(calculator, sens, "USD", linear, [1.0, 2.0, 4.0])
    discount = DiscountCurve.flat("USD-OIS", date(2025, 1, 1), 0.03)
    hazard = HazardCurve.flat("BANK", date(2025, 1, 1), 0.02, 0.4)
    spreads = [(0.0, 50.0), (5.0, 80.0)]
    out["mva"] = {
        "profile": _wire(profile),
        "without_survival": _wire(compute_mva(profile, spreads, discount)),
        "with_survival": _wire(compute_mva(profile, spreads, discount, hazard)),
    }

    schedule = ScheduleImCalculator.bcbs_standard()
    out["schedule_im"] = {
        "rate": schedule.rate("interest_rate", 5.0),
        "defaults": {
            "asset_class": schedule.default_asset_class,
            "maturity_years": schedule.default_maturity_years,
            "mpor_days": schedule.mpor_days,
        },
        "with_asset_class": schedule.with_asset_class("credit").default_asset_class,
        "with_maturity": schedule.with_maturity(2.0).default_maturity_years,
        "from_registry_rate": ScheduleImCalculator.from_registry_id(CONSTANTS["BCBS_IOSCO_SCHEDULE_ID"]).rate(
            "credit", 3.0
        ),
        "for_notional": _wire(schedule.calculate_for_notional(1_000_000.0, "USD", "interest_rate", 5.0, "2025-01-15")),
        "ngr": _wire(
            schedule.calculate_netting_set_with_ngr(
                [(2e6, 1e8, "interest_rate", 5.0), (-1.5e6, 8e7, "credit", 3.0)], "USD", "2025-01-15"
            )
        ),
        "ngr_empty": schedule.calculate_netting_set_with_ngr([], "USD", "2025-01-15"),
    }

    haircut = HaircutImCalculator.bcbs_standard()
    out["haircut_im"] = {
        "haircut_for_cash": haircut.haircut_for("cash"),
        "default_asset_class": str(haircut.default_asset_class),
        "with_default_asset_class": str(haircut.with_default_asset_class("government_bonds").default_asset_class),
        "posted_collateral_currency": [
            haircut.posted_collateral_currency,
            haircut.with_posted_collateral_currency("EUR").posted_collateral_currency,
        ],
        "mpor_days": haircut.mpor_days,
        "treasuries_terms_haircut": HaircutImCalculator
        .us_treasuries()
        .with_collateral_terms(7.0, "AAA")
        .haircut_for("government_bonds"),
        "from_schedule_haircut": HaircutImCalculator.from_schedule(cash_only).haircut_for("cash"),
        "eligible_collateral": _wire(haircut.eligible_collateral),
        "for_collateral": _wire(haircut.calculate_for_collateral(10_000_000.0, "USD", "cash", True, "2025-01-15")),
    }

    vm = VmCalculator(usd)
    vm_result = vm.calculate(1_000_000.0, 0.0, "USD", "2024-06-17")
    calls = vm.generate_margin_calls(
        [("2024-06-17", 1_000_000.0), ("2024-06-18", 2_500_000.0), ("2024-06-19", 500_000.0)], 0.0
    )
    out["vm"] = {
        "csa_id": vm.csa.id,
        "result": _wire(vm_result),
        "net_margin": vm_result.net_margin,
        "requires_call": vm_result.requires_call,
        "calls": calls.to_dict("records"),
        "call_dates": [d.isoformat() for d in vm.margin_call_dates("2024-06-17", "2024-06-21")],
    }

    frtb = frtb_sensitivities()
    engine = FrtbSbaEngine(["low", "high"], ["girr", "fx"])
    out["frtb"] = {
        "wire": _wire(frtb),
        "base_currency": frtb.base_currency,
        "charge": _wire(frtb_sba_charge(frtb)),
        "charge_high": _wire(frtb_sba_charge(frtb, "high")),
        "engine": {"scenarios": engine.scenarios, "risk_classes": engine.risk_classes},
        "engine_default": {
            "scenarios": FrtbSbaEngine().scenarios,
            "risk_classes": FrtbSbaEngine().risk_classes,
        },
        "engine_charge": _wire(engine.calculate(frtb)),
    }

    netting_set = NettingSetId.bilateral("CPTY", "CSA")
    unmargined = SaCcrNettingSetConfig.unmargined(netting_set, 0.0, "2025-01-15")
    margined = SaCcrNettingSetConfig.margined(netting_set, 10_000.0, 50_000.0, 5_000.0, 0.0, 10, "2025-01-15")
    trades = saccr_trades()
    out["saccr"] = {
        "trades": [_wire(trade) for trade in trades],
        "unmargined": _wire(unmargined),
        "margined": _wire(margined),
        "ead_unmargined": _wire(saccr_ead(trades, unmargined)),
        "ead_margined": _wire(saccr_ead(trades, margined)),
        "ead_alpha": _wire(saccr_ead(trades, unmargined, 1.5)),
        "engine_alpha": [SaCcrEngine().alpha, SaCcrEngine(1.5).alpha],
        "engine_ead": _wire(SaCcrEngine(1.5).calculate_ead(unmargined, trades)),
    }
    return out


def _close(actual: Any, expected: Any, path: str) -> None:
    """Deep equality, with a relative tolerance on floats."""
    if isinstance(expected, dict):
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key, value in expected.items():
            _close(actual[key], value, f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, (list, tuple)), path
        assert len(actual) == len(expected), path
        for index, value in enumerate(expected):
            _close(actual[index], value, f"{path}[{index}]")
    elif isinstance(expected, float) and not isinstance(expected, bool):
        assert isinstance(actual, (int, float)), path
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), f"{path}: {actual} vs {expected}"
    else:
        assert actual == expected, f"{path}: {actual!r} vs {expected!r}"


def test_python_margin_matches_the_shared_golden() -> None:
    _close(json.loads(json.dumps(compute())), json.loads(GOLDEN.read_text()), "margin")


def test_validate_twins_reject_what_wasm_rejects() -> None:
    """The error cases the facade test asserts on the WASM side."""
    with pytest.raises(ValueError, match="strictly increasing"):
        ImProfile([2.0, 1.0], [10.0, 5.0]).validate()
    with pytest.raises(ValueError, match="vector lengths must be equal"):
        ExposureProfile([1.0, 2.0], [0.0], [0.0, 0.0], [0.0, 0.0]).validate()
    with pytest.raises(ValueError, match="maturity_years"):
        ImDecayProfile.linear_to_maturity(0.0)
    with pytest.raises(ValueError, match="cannot merge SIMM sensitivities in EUR into a USD container"):
        SimmSensitivities("USD").merge(SimmSensitivities("EUR"))
    with pytest.raises(ValueError, match="alpha"):
        saccr_ead(saccr_trades(), SaCcrNettingSetConfig.unmargined(NettingSetId.cleared("LCH"), 0.0, "2025-01-15"), 0.5)
    with pytest.raises(ValueError, match="unknown variant `extreme`"):
        frtb_sba_charge(frtb_sensitivities(), "extreme")
    with pytest.raises(ValueError, match="at least one correlation scenario"):
        FrtbSbaEngine([], None)
    SaCcrNettingSetConfig.unmargined(NettingSetId.cleared("LCH"), 0.0, "2025-01-15").validate()
    CsaSpec.usd_regulatory().validate()


if __name__ == "__main__":
    GOLDEN.write_text(json.dumps(compute(), indent=2, sort_keys=True) + "\n")
