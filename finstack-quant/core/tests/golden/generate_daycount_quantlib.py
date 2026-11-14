"""Regenerate ``data/daycount_quantlib.json`` by running QuantLib.

Run from the repository root::

    uv run --no-sync python finstack-quant/core/tests/golden/generate_daycount_quantlib.py

Every case whose convention QuantLib implements is computed by QuantLib itself:
``quantlib_convention`` names the exact QuantLib day counter and, for 30E/360
ISDA, ``end_is_termination_date`` records the termination input passed to it
(``Thirty360(ISDA, terminationDate)`` with ``terminationDate == end`` for
``true``). Day counts are QuantLib's ``dayCount`` and year fractions its
``yearFraction``. ACT/ACT ICMA (schedule-dependent in QuantLib) and ACT/365L
(not implemented by QuantLib) cases are hand-computed from the accepted
formulas and marked ``"source": "hand"``.
"""

from __future__ import annotations

import datetime as dt
import json
from pathlib import Path

import QuantLib as ql  # noqa: N813

OUT = Path(__file__).resolve().parent / "data" / "daycount_quantlib.json"

# Local convention -> (QuantLib label, day-counter factory taking the end date).
QUANTLIB = {
    "30_360": ("Thirty360(Thirty360.USA)", lambda _end, _term: ql.Thirty360(ql.Thirty360.USA)),
    "30_360_isda": (
        "Thirty360(Thirty360.BondBasis)",
        lambda _end, _term: ql.Thirty360(ql.Thirty360.BondBasis),
    ),
    "30e_360": ("Thirty360(Thirty360.European)", lambda _end, _term: ql.Thirty360(ql.Thirty360.European)),
    "30e_360_isda": (
        "Thirty360(Thirty360.ISDA, terminationDate)",
        lambda end, term: ql.Thirty360(ql.Thirty360.ISDA, end if term else ql.Date()),
    ),
    "30_360_it": ("Thirty360(Thirty360.Italian)", lambda _end, _term: ql.Thirty360(ql.Thirty360.Italian)),
    "act_act": ("ActualActual(ActualActual.ISDA)", lambda _end, _term: ql.ActualActual(ql.ActualActual.ISDA)),
    "act_act_afb": ("ActualActual(ActualActual.AFB)", lambda _end, _term: ql.ActualActual(ql.ActualActual.AFB)),
    "act_360": ("Actual360()", lambda _end, _term: ql.Actual360()),
    "act_365f": ("Actual365Fixed()", lambda _end, _term: ql.Actual365Fixed()),
}
SYNTHETIC = {"30_360", "30_360_isda", "30e_360", "30e_360_isda", "30_360_it"}

# (id, description, convention, start, end, extra) — QuantLib-computed cases.
QL_CASES = [
    (
        "thirty360_us_eom_feb28_to_mar31_non_leap",
        "30U/360 EOM: February month-end start becomes 30.",
        "30_360",
        "2025-02-28",
        "2025-03-31",
        {},
    ),
    (
        "thirty360_us_eom_feb29_to_mar31_leap",
        "30U/360 EOM: leap February month-end start becomes 30.",
        "30_360",
        "2024-02-29",
        "2024-03-31",
        {},
    ),
    (
        "thirty360_us_not_eom_feb27_to_mar31",
        "30U/360 EOM: February 27 is not month-end and is kept.",
        "30_360",
        "2025-02-27",
        "2025-03-31",
        {},
    ),
    (
        "thirty360_us_leap_feb28_not_eom_to_feb28",
        "30U/360 EOM: 2024-02-28 is not leap-year February month-end; "
        "the 2025-02-28 month-end end is kept because the start is not February month-end.",
        "30_360",
        "2024-02-28",
        "2025-02-28",
        {},
    ),
    (
        "thirty360_us_eom_feb29_to_feb28_next_year",
        "30U/360 EOM: both ends February month-end, both become 30.",
        "30_360",
        "2024-02-29",
        "2025-02-28",
        {},
    ),
    (
        "thirty360_us_jan31_to_feb28",
        "30U/360 EOM: day-31 start becomes 30; February end kept (start not Feb EOM).",
        "30_360",
        "2025-01-31",
        "2025-02-28",
        {},
    ),
    (
        "thirty360_us_jan15_to_mar31",
        "30U/360 EOM: day-31 end kept after an unadjusted start.",
        "30_360",
        "2025-01-15",
        "2025-03-31",
        {},
    ),
    (
        "thirty360_isda_bond_basis_feb28_to_mar31",
        "30/360 ISDA bond basis: no February rule, day-31 end kept.",
        "30_360_isda",
        "2025-02-28",
        "2025-03-31",
        {},
    ),
    (
        "thirty360_isda_bond_basis_jan31_to_mar31",
        "30/360 ISDA bond basis: day-31 start and end become 30.",
        "30_360_isda",
        "2025-01-31",
        "2025-03-31",
        {},
    ),
    (
        "thirty360_isda_bond_basis_feb29_to_feb28",
        "30/360 ISDA bond basis: February month-ends are kept.",
        "30_360_isda",
        "2024-02-29",
        "2025-02-28",
        {},
    ),
    (
        "thirty360_e_feb28_not_adjusted",
        "30E/360: no February rule; day-31 end becomes 30.",
        "30e_360",
        "2025-02-28",
        "2025-03-31",
        {},
    ),
    ("thirty360_e_jan15_to_mar31", "30E/360: day-31 end always becomes 30.", "30e_360", "2025-01-15", "2025-03-31", {}),
    (
        "thirty_e_360_isda_feb29_ordinary_end",
        "30E/360 ISDA: February month-end end becomes 30 when not the termination date.",
        "30e_360_isda",
        "2011-08-31",
        "2012-02-29",
        {"end_is_termination_date": False},
    ),
    (
        "thirty_e_360_isda_feb29_termination_end",
        "30E/360 ISDA: a February month-end termination date is kept.",
        "30e_360_isda",
        "2011-08-31",
        "2012-02-29",
        {"end_is_termination_date": True},
    ),
    (
        "thirty_e_360_isda_feb28_termination_end",
        "30E/360 ISDA: non-leap February month-end termination kept.",
        "30e_360_isda",
        "2011-01-28",
        "2011-02-28",
        {"end_is_termination_date": True},
    ),
    (
        "thirty_e_360_isda_day31_termination_end",
        "30E/360 ISDA: a terminal day 31 still becomes 30.",
        "30e_360_isda",
        "2025-01-15",
        "2025-03-31",
        {"end_is_termination_date": True},
    ),
    (
        "thirty360_italian_jan31_to_feb28",
        "30/360 Italian: February day after the 27th becomes 30.",
        "30_360_it",
        "2025-01-31",
        "2025-02-28",
        {},
    ),
    (
        "thirty360_italian_leap_feb29_to_mar31",
        "30/360 Italian: leap February 29 start becomes 30.",
        "30_360_it",
        "2024-02-29",
        "2024-03-31",
        {},
    ),
    (
        "actact_isda_spanning_leap_year",
        "Act/Act ISDA: 184/366 + one complete year.",
        "act_act",
        "2024-07-01",
        "2026-01-01",
        {},
    ),
    (
        "actact_isda_within_leap_year",
        "Act/Act ISDA: 182/366 within a leap year.",
        "act_act",
        "2024-01-01",
        "2024-07-01",
        {},
    ),
    (
        "actact_isda_within_non_leap_year",
        "Act/Act ISDA: 181/365 within a common year.",
        "act_act",
        "2025-01-01",
        "2025-07-01",
        {},
    ),
    (
        "act_act_afb_short_leap_february",
        "Act/Act AFB: residual containing 29 February uses 366.",
        "act_act_afb",
        "2024-02-01",
        "2024-03-01",
        {},
    ),
    (
        "act_act_afb_short_non_leap_february",
        "Act/Act AFB: residual without 29 February uses 365.",
        "act_act_afb",
        "2025-02-01",
        "2025-03-01",
        {},
    ),
    (
        "act_act_afb_multi_year_whole_year_walk",
        "Act/Act AFB: two whole years walked back from the end.",
        "act_act_afb",
        "2023-03-15",
        "2025-09-15",
        {},
    ),
    ("act360_standard", "Act/360: 90 actual days.", "act_360", "2025-01-01", "2025-04-01", {}),
    ("act365f_standard", "Act/365F: 365 actual days.", "act_365f", "2025-01-01", "2026-01-01", {}),
]

# Hand-computed cases: (id, description, convention, start, end, frequency, coupon_period, year_fraction, notes).
HAND_CASES = [
    (
        "actact_icma_full_semi_annual_period",
        "Act/Act ICMA: one regular semi-annual coupon.",
        "act_act_isma",
        "2025-01-15",
        "2025-07-15",
        "6M",
        ("2025-01-15", "2025-07-15"),
        0.5,
        "1 coupon x 1/2",
    ),
    (
        "actact_icma_full_quarterly_period",
        "Act/Act ICMA: one regular quarterly coupon.",
        "act_act_isma",
        "2025-01-01",
        "2025-04-01",
        "3M",
        ("2025-01-01", "2025-04-01"),
        0.25,
        "1 coupon x 1/4",
    ),
    (
        "actact_icma_full_annual_period",
        "Act/Act ICMA: one regular annual coupon.",
        "act_act_isma",
        "2025-01-01",
        "2026-01-01",
        "1Y",
        ("2025-01-01", "2026-01-01"),
        1.0,
        "1 coupon x 1",
    ),
    (
        "actact_icma_short_front_stub",
        "Act/Act ICMA: short stub inside its regular reference coupon.",
        "act_act_isma",
        "2025-03-15",
        "2025-07-15",
        "6M",
        ("2025-01-15", "2025-07-15"),
        122 / 181 * 0.5,
        "122 of 181 reference days x 1/2",
    ),
    (
        "actact_icma_short_back_stub",
        "Act/Act ICMA: short back stub inside its regular reference coupon.",
        "act_act_isma",
        "2025-01-15",
        "2025-04-15",
        "6M",
        ("2025-01-15", "2025-07-15"),
        90 / 181 * 0.5,
        "90 of 181 reference days x 1/2",
    ),
    (
        "actact_icma_spanning_leap_year",
        "Act/Act ICMA: one semi-annual coupon across 29 February.",
        "act_act_isma",
        "2024-01-15",
        "2024-07-15",
        "6M",
        ("2024-01-15", "2024-07-15"),
        0.5,
        "1 coupon x 1/2",
    ),
    (
        "actact_icma_monthly_frequency",
        "Act/Act ICMA: one monthly coupon.",
        "act_act_isma",
        "2025-01-01",
        "2025-02-01",
        "1M",
        ("2025-01-01", "2025-02-01"),
        1 / 12,
        "1 coupon x 1/12",
    ),
    (
        "act365l_contains_feb29",
        "Act/365L: period ending in leap year 2024 uses 366.",
        "act_365l",
        "2024-01-01",
        "2024-03-01",
        None,
        ("2024-01-01", "2024-03-01"),
        60 / 366,
        "60 actual days / 366",
    ),
    (
        "act365l_leap_end_year_without_feb29",
        "Act/365L: a period ending in a leap year uses 366 even without 29 February.",
        "act_365l",
        "2024-03-01",
        "2024-06-01",
        None,
        ("2024-03-01", "2024-06-01"),
        92 / 366,
        "92 actual days / 366 (ISDA coupon-end-year rule)",
    ),
    (
        "act365l_period_before_feb29_in_leap_year",
        "Act/365L: a period before 29 February ending in a leap year uses 366.",
        "act_365l",
        "2024-01-01",
        "2024-02-28",
        None,
        ("2024-01-01", "2024-02-28"),
        58 / 366,
        "58 actual days / 366 (ISDA coupon-end-year rule)",
    ),
    (
        "act365l_common_end_year",
        "Act/365L: a period ending in a common year uses 365.",
        "act_365l",
        "2024-07-01",
        "2025-01-01",
        None,
        ("2024-07-01", "2025-01-01"),
        184 / 365,
        "184 actual days / 365",
    ),
]


def qdate(text: str) -> ql.Date:
    """Convert an ISO date string to a QuantLib date."""
    d = dt.date.fromisoformat(text)
    return ql.Date(d.day, d.month, d.year)


def quantlib_case(case_id: str, description: str, convention: str, start: str, end: str, extra: dict) -> dict:
    """Build one fixture case from QuantLib's own day count and year fraction."""
    label, factory = QUANTLIB[convention]
    terminal = extra.get("end_is_termination_date", False)
    counter = factory(qdate(end), terminal)
    days = counter.dayCount(qdate(start), qdate(end))
    expected: dict = {"year_fraction": counter.yearFraction(qdate(start), qdate(end)), "tolerance": 1e-14}
    expected = {"days": days, **expected} if convention in SYNTHETIC else {"actual_days": days, **expected}
    case = {
        "id": case_id,
        "description": description,
        "source": "quantlib",
        "convention": convention,
        "quantlib_convention": label,
        "start_date": start,
        "end_date": end,
        **extra,
        "expected": expected,
    }
    return case


def hand_case(row: tuple) -> dict:
    """Build one hand-computed fixture case."""
    case_id, description, convention, start, end, frequency, coupon_period, year_fraction, notes = row
    case = {
        "id": case_id,
        "description": description,
        "source": "hand",
        "convention": convention,
        "start_date": start,
        "end_date": end,
    }
    if frequency is not None:
        case["frequency"] = frequency
    if coupon_period is not None:
        case["coupon_period"] = list(coupon_period)
    case["expected"] = {"year_fraction": year_fraction, "tolerance": 1e-14}
    case["notes"] = notes
    return case


def main() -> None:
    """Write the regenerated fixture."""
    suite = {
        "meta": {
            "suite_id": "daycount_quantlib_parity",
            "description": (
                "Day-count cases. Cases with source 'quantlib' were computed by running QuantLib "
                "(quantlib_convention names the exact day counter; 30E/360 ISDA records its "
                "termination input). Cases with source 'hand' (ACT/ACT ICMA, ACT/365L) are "
                "hand-computed from the accepted formulas and are not vendor output."
            ),
            "reference_source": {
                "name": "QuantLib",
                "version": ql.__version__,
                "vendor": "QuantLib Project",
                "url": "https://www.quantlib.org/",
                "notes": "Generated by generate_daycount_quantlib.py in this directory.",
            },
            "extra": {
                "isda_reference": {
                    "document": "2006 ISDA Definitions",
                    "sections": [
                        "4.16(b) Actual/Actual (ISDA)",
                        "4.16(d) Actual/365 (Fixed)",
                        "4.16(e) Actual/360",
                        "4.16(f) 30/360 (bond basis)",
                        "4.16(g) 30E/360",
                        "4.16(h) 30E/360 (ISDA)",
                    ],
                }
            },
            "generated": {
                "at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
                "by": "finstack-quant/core/tests/golden/generate_daycount_quantlib.py",
                "command": "uv run --no-sync python finstack-quant/core/tests/golden/generate_daycount_quantlib.py",
            },
            "status": "provisional",
            "schema_version": 1,
        },
        "cases": [quantlib_case(*row) for row in QL_CASES] + [hand_case(row) for row in HAND_CASES],
    }
    OUT.write_text(json.dumps(suite, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
