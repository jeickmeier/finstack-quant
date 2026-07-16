"""Example-only multi-asset instrument factories."""

from __future__ import annotations

from datetime import date

AS_OF = date(2025, 1, 15)
AS_OF_STR = AS_OF.isoformat()

def fixed_bond(idx: int) -> tuple[str, dict]:
    iid = f"BOND-FIXED-{idx}"
    coupon = 0.04 + 0.005 * (idx % 5)
    mat_year = 2028 + (idx % 8)
    return iid, {"type": "bond", "spec": {
        "id": iid, "notional": {"amount": "1000000", "currency": "USD"},
        "issue_date": "2024-01-15", "maturity": f"{mat_year}-01-15",
        "discount_curve_id": "USD-OIS", "accrual_method": "Linear",
        "settlement_days": 1, "ex_coupon_days": 0,
        "cashflow_spec": {"Fixed": {
            "coupon_type": "Cash", "freq": {"count": 6, "unit": "months"},
            "dc": "Thirty360", "bdc": "following", "calendar_id": "weekends_only",
            "end_of_month": False, "payment_lag_days": 0, "rate": str(coupon), "stub": "None",
        }},
        "call_put": None, "credit_curve_id": None,
        "attributes": {"tags": ["fixed-income"], "meta": {"sector": "IG"}},
        "pricing_overrides": {},
    }}


def floating_bond(idx: int) -> tuple[str, dict]:
    iid = f"BOND-FRN-{idx}"
    spread = 100 + 25 * (idx % 6)
    mat_year = 2027 + (idx % 5)
    return iid, {"type": "bond", "spec": {
        "id": iid, "notional": {"amount": "1000000", "currency": "USD"},
        "issue_date": "2024-01-15", "maturity": f"{mat_year}-01-15",
        "discount_curve_id": "USD-OIS", "accrual_method": "Linear",
        "settlement_days": 2, "ex_coupon_days": 0,
        "cashflow_spec": {"Floating": {
            "coupon_type": "Cash", "freq": {"count": 3, "unit": "months"}, "stub": "ShortFront",
            "dc": "Act360", "bdc": "modified_following", "calendar_id": "weekends_only",
            "payment_lag_days": 0, "end_of_month": False,
            "rate_spec": {
                "index_id": "USD-SOFR-3M", "spread_bp": str(spread), "gearing": "1",
                "gearing_includes_spread": True, "floor_bp": "0",
                "all_in_floor_bp": None, "cap_bp": None, "index_cap_bp": None,
                "fixing_calendar_id": None, "reset_freq": {"count": 3, "unit": "months"},
                "reset_lag_days": 2,
            },
        }},
        "call_put": None, "credit_curve_id": None,
        "attributes": {"tags": ["fixed-income", "frn"], "meta": {}}, "pricing_overrides": {},
    }}


def term_loan(idx: int) -> tuple[str, dict]:
    iid = f"TL-{idx}"
    # Institutional term loans are floating: SOFR + margin, often with a SOFR
    # (index) floor of 0 or 50bp. Vary both across the portfolio.
    spread = 250 + 50 * (idx % 4)          # SOFR + 250/300/350/400 bp
    index_floor_bp = "0" if idx % 2 == 0 else "50"
    mat_year = 2028 + (idx % 4)
    return iid, {"type": "term_loan", "spec": {
        "id": iid, "notional_limit": {"amount": "10000000", "currency": "USD"},
        "currency": "USD", "issue_date": "2024-01-01", "maturity": f"{mat_year}-01-01",
        "discount_curve_id": "USD-OIS",
        "rate": {"Floating": {
            "index_id": "USD-SOFR-3M", "spread_bp": str(spread), "gearing": "1",
            "gearing_includes_spread": True, "floor_bp": index_floor_bp,
            "all_in_floor_bp": None, "cap_bp": None, "index_cap_bp": None,
            "fixing_calendar_id": None, "reset_freq": {"count": 3, "unit": "months"},
            "reset_lag_days": 2,
        }},
        "day_count": "Act360", "frequency": {"count": 3, "unit": "months"},
        "bdc": "modified_following", "calendar_id": None, "stub": "None",
        "amortization": {"PercentPerPeriod": {"bp": 250}}, "coupon_type": "Cash",
        "settlement_days": 2, "attributes": {"tags": ["leveraged-loan"], "meta": {}},
        "pricing_overrides": {},
    }}


def revolver(idx: int) -> tuple[str, dict]:
    iid = f"RCF-{idx}"
    spread = 200 + 50 * (idx % 4)
    mat_year = 2027 + (idx % 3)
    return iid, {"type": "revolving_credit", "spec": {
        "id": iid, "commitment_amount": {"amount": "50000000", "currency": "USD"},
        "drawn_amount": {"amount": "10000000", "currency": "USD"},
        "commitment_date": "2024-01-01", "maturity": f"{mat_year}-01-01",
        "discount_curve_id": "USD-OIS", "day_count": "Act360",
        "frequency": {"count": 3, "unit": "months"}, "stub": "ShortFront", "recovery_rate": 0.70,
        "base_rate_spec": {"Floating": {
            "index_id": "USD-SOFR-3M", "spread_bp": str(spread), "gearing": "1",
            "gearing_includes_spread": True, "floor_bp": "0",
            "all_in_floor_bp": None, "cap_bp": None, "index_cap_bp": None,
            "fixing_calendar_id": None, "reset_freq": {"count": 3, "unit": "months"},
            "reset_lag_days": 2,
        }},
        "draw_repay_spec": {"Deterministic": [
            {"date": "2024-06-01", "amount": {"amount": "5000000", "currency": "USD"}, "is_draw": True},
            {"date": "2025-06-01", "amount": {"amount": "3000000", "currency": "USD"}, "is_draw": False},
        ]},
        "fees": {"commitment_fee_tiers": [{"threshold": "0", "bps": "25"}],
                 "usage_fee_tiers": [{"threshold": "0", "bps": "10"}], "facility_fee_bp": 5.0},
        "attributes": {"tags": ["revolving"], "meta": {}}, "pricing_overrides": {},
    }}


def cds(idx: int) -> tuple[str, dict]:
    iid = f"CDS-{idx}"
    spread = 80 + 20 * (idx % 5)
    side = "pay" if idx % 2 == 0 else "receive"
    mat_year = 2028 + (idx % 5)
    return iid, {"type": "credit_default_swap", "spec": {
        "id": iid, "notional": {"amount": 10_000_000.0, "currency": "USD"},
        "side": side, "convention": "isda_na",
        "premium": {"start": "2025-03-20", "end": f"{mat_year}-03-20",
                    "frequency": {"count": 3, "unit": "months"}, "stub": "ShortFront",
                    "bdc": "following", "calendar_id": "usny", "day_count": "Act360",
                    "spread_bp": float(spread), "discount_curve_id": "USD-OIS"},
        "protection": {"credit_curve_id": "CORP-HAZARD", "recovery_rate": 0.4, "settlement_delay": 3},
        "pricing_overrides": {}, "attributes": {"tags": ["credit"], "meta": {}},
    }}


def cds_index(idx: int) -> tuple[str, dict]:
    iid = f"CDX-IDX-{idx}"
    spread = 50 + 10 * (idx % 5)
    side = "pay" if idx % 2 == 0 else "receive"
    mat_year = 2029 + (idx % 3)
    return iid, {"type": "cds_index", "spec": {
        "id": iid, "index_name": "CDX.NA.IG", "series": 42, "version": 1,
        "notional": {"amount": "10000000", "currency": "USD"}, "index_factor": 1.0,
        "side": side, "convention": "isda_na",
        "premium": {"start": "2025-03-20", "end": f"{mat_year}-12-20",
                    "frequency": {"count": 3, "unit": "months"}, "stub": "ShortFront",
                    "bdc": "following", "calendar_id": None, "day_count": "Act360",
                    "spread_bp": float(spread), "discount_curve_id": "USD-OIS"},
        "protection": {"credit_curve_id": "CDX-HAZ", "recovery_rate": 0.4, "settlement_delay": 3},
        "pricing": "SingleCurve", "constituents": [],
        "pricing_overrides": {}, "attributes": {"tags": ["credit-index"], "meta": {}},
    }}


def cds_tranche(idx: int) -> tuple[str, dict]:
    iid = f"CDX-TR-{idx}"
    attach_detach = [(0.0, 3.0), (3.0, 7.0), (7.0, 10.0)]
    a, d = attach_detach[idx % 3]
    coupon = 500.0 if a == 0.0 else 100.0
    return iid, {"type": "cds_tranche", "spec": {
        "id": iid, "index_name": "CDX.NA.IG", "series": 42,
        "attach_pct": a, "detach_pct": d,
        "notional": {"amount": "10000000", "currency": "USD"}, "maturity": "2029-12-20",
        "running_coupon_bp": coupon, "frequency": {"count": 3, "unit": "months"},
        "day_count": "Act360", "bdc": "following", "calendar_id": "weekends_only",
        "discount_curve_id": "USD-OIS", "credit_index_id": "CDX.NA.IG.HAZARD",
        "side": "buy_protection", "accumulated_loss": 0.0, "standard_imm_dates": False,
        "attributes": {"tags": ["structured-credit"], "meta": {}},
    }}


def cds_option(idx: int) -> tuple[str, dict]:
    iid = f"CDSOPT-{idx}"
    strike = 0.008 + 0.002 * (idx % 4)
    opt_type = "call" if idx % 2 == 0 else "put"
    return iid, {"type": "cds_option", "spec": {
        "id": iid, "strike": str(strike), "option_type": opt_type,
        "exercise_style": "european", "expiry": "2025-06-20", "cds_maturity": "2030-06-20",
        "notional": {"amount": "10000000", "currency": "USD"},
        "settlement": "cash", "recovery_rate": 0.4,
        "discount_curve_id": "USD-OIS", "credit_curve_id": "CORP-HAZARD",
        "vol_surface_id": "CDS-SPREAD-VOL",
        "underlying_is_index": False, "index_factor": None,
        "pricing_overrides": {}, "attributes": {"tags": ["credit-vol"], "meta": {}},
    }}


def _pool_assets(iid: str, n: int = 5) -> list[dict]:
    return [{
        "id": f"{iid}-LOAN-{j}",
        "asset_type": {"type": "FirstLienLoan", "industry": None},
        "balance": {"amount": "2000000", "currency": "USD"},
        "rate": 0.055 + 0.005 * (j % 3), "spread_bps": 300.0 + 50.0 * (j % 4),
        "index_id": None, "maturity": f"{2029 + (j % 3)}-01-01",
        "credit_quality": "BB", "industry": "Technology", "obligor_id": f"OBL-{j}",
        "is_defaulted": False, "recovery_amount": None, "purchase_price": None,
        "acquisition_date": None, "day_count": "Act360",
        "smm_override": None, "mdr_override": None,
    } for j in range(n)]


def _structured_credit_spec(iid: str, deal_type: str, idx: int) -> dict:
    return {"type": "structured_credit", "spec": {
        "id": iid, "deal_type": deal_type,
        "pool": {
            "id": f"{iid}-POOL", "deal_type": deal_type, "base_currency": "USD", "assets": _pool_assets(iid),
            "cumulative_defaults": {"amount": "0", "currency": "USD"},
            "cumulative_recoveries": {"amount": "0", "currency": "USD"},
            "cumulative_prepayments": {"amount": "0", "currency": "USD"},
            "reinvestment_period": None,
            "collection_account": {"amount": "0", "currency": "USD"},
            "reserve_account": {"amount": "0", "currency": "USD"},
            "excess_spread_account": {"amount": "0", "currency": "USD"},
        },
        "tranches": {"tranches": [{
            "id": f"{iid}-A", "attachment_point": 0.0, "detachment_point": 100.0,
            "behavior_type": "Standard", "seniority": "Senior", "rating": None,
            "original_balance": {"amount": "10000000", "currency": "USD"},
            "current_balance": {"amount": "10000000", "currency": "USD"}, "target_balance": None,
            "coupon": {"Fixed": {"rate": 0.05 + 0.005 * (idx % 4)}},
            "oc_trigger": None, "ic_trigger": None,
            "credit_enhancement": {
                "subordination": {"amount": "0", "currency": "USD"},
                "overcollateralization": {"amount": "0", "currency": "USD"},
                "reserve_account": {"amount": "0", "currency": "USD"},
                "excess_spread": 0.0, "cash_trap_active": False,
            },
            "frequency": {"count": 3, "unit": "months"}, "day_count": "Act360",
            "deferred_interest": {"amount": "0", "currency": "USD"},
            "is_revolving": False, "can_reinvest": False,
            "maturity": "2034-01-01", "expected_maturity": None, "payment_priority": 1,
            "attributes": {"tags": [], "meta": {}},
        }], "total_size": {"amount": "10000000", "currency": "USD"}},
        "closing_date": "2024-01-01", "first_payment_date": "2025-04-01",
        "reinvestment_end_date": None, "maturity": "2034-01-01",
        "frequency": {"count": 3, "unit": "months"}, "discount_curve_id": "USD-OIS",
        "payment_calendar_id": "nyse",
        "attributes": {"tags": [deal_type.lower()], "meta": {}},
        "prepayment_spec": {"cpr": 0.15, "curve": None},
        "default_spec": {"cdr": 0.025, "curve": None},
        "recovery_spec": {"rate": 0.4, "recovery_lag": 18},
        "market_conditions": {"refi_rate": 0.04, "original_rate": None, "hpa": None,
                               "unemployment": None, "seasonal_factor": 1.0, "custom_factors": {}},
        "credit_factors": {"credit_score": None, "dti": None, "ltv": None,
                            "delinquency_days": 0, "unemployment_rate": None, "custom_factors": {}},
        "deal_metadata": {"manager_id": None, "servicer_id": None, "master_servicer_id": None,
                           "special_servicer_id": None, "trustee_id": None},
        "behavior_overrides": {"cpr_annual": None, "abs_speed": None, "psa_speed_multiplier": None,
                                "cdr_annual": None, "sda_speed_multiplier": None, "recovery_rate": None,
                                "recovery_lag_months": None, "reinvestment_price": None},
        "default_assumptions": {"base_cdr_annual": 0.02, "base_recovery_rate": 0.4, "base_cpr_annual": 0.15,
                                 "psa_speed": None, "sda_speed": None, "abs_speed_monthly": None,
                                 "cpr_by_asset_type": {}, "cdr_by_asset_type": {}, "recovery_by_asset_type": {}},
    }}


def clo_deal(idx: int) -> tuple[str, dict]:
    iid = f"CLO-{idx}"
    return iid, _structured_credit_spec(iid, "CLO", idx)


def abs_deal(idx: int) -> tuple[str, dict]:
    iid = f"ABS-{idx}"
    return iid, _structured_credit_spec(iid, "ABS", idx)


def irs(idx: int) -> tuple[str, dict]:
    iid = f"IRS-{idx}"
    rate = 0.040 + 0.005 * (idx % 4)
    side = "pay" if idx % 2 == 0 else "receive"
    mat_year = 2028 + (idx % 7)
    return iid, {"type": "interest_rate_swap", "spec": {
        "id": iid, "notional": {"amount": 10_000_000.0, "currency": "USD"}, "side": side,
        "fixed": {"discount_curve_id": "USD-OIS", "rate": rate,
                  "frequency": {"count": 6, "unit": "months"}, "day_count": "Thirty360",
                  "bdc": "modified_following", "calendar_id": None, "stub": "None",
                  "start": "2025-04-15", "end": f"{mat_year}-04-15",
                  "par_method": None, "compounding_simple": True},
        "float": {"discount_curve_id": "USD-OIS", "forward_curve_id": "USD-SOFR-3M",
                  "spread_bp": 0.0, "frequency": {"count": 3, "unit": "months"},
                  "day_count": "Act360", "bdc": "modified_following", "calendar_id": None,
                  "stub": "None", "reset_lag_days": 2,
                  "start": "2025-04-15", "end": f"{mat_year}-04-15", "compounding": "Simple"},
        "attributes": {"tags": ["rates"], "meta": {}},
    }}


def swaption(idx: int) -> tuple[str, dict]:
    iid = f"SWPN-{idx}"
    strike = 0.035 + 0.005 * (idx % 4)
    opt_type = "call" if idx % 2 == 0 else "put"
    swap_end_year = 2030 + (idx % 5)
    return iid, {"type": "swaption", "spec": {
        "id": iid, "option_type": opt_type,
        "notional": {"amount": "10000000", "currency": "USD"}, "strike": strike,
        "expiry": "2025-07-15", "swap_start": "2025-07-17", "swap_end": f"{swap_end_year}-07-17",
        "fixed_freq": {"count": 6, "unit": "months"}, "float_freq": {"count": 3, "unit": "months"},
        "day_count": "Thirty360", "exercise_style": "european", "settlement": "cash",
        "vol_model": "black", "discount_curve_id": "USD-OIS",
        "forward_curve_id": "USD-SOFR-3M", "vol_surface_id": "USD-SWPNVOL",
        "pricing_overrides": {}, "sabr_params": None,
        "attributes": {"tags": ["rates-vol"], "meta": {}},
    }}


def ir_future(idx: int) -> tuple[str, dict]:
    iid = f"IRF-{idx}"
    expiry_offsets = [(2025, "06"), (2025, "09"), (2025, "12"), (2026, "03"),
                      (2026, "06"), (2026, "09"), (2026, "12"), (2027, "03")]
    end_offsets = [(2025, "09"), (2025, "12"), (2026, "03"), (2026, "06"),
                    (2026, "09"), (2026, "12"), (2027, "03"), (2027, "06")]
    y, m = expiry_offsets[idx % len(expiry_offsets)]
    ey, em = end_offsets[idx % len(end_offsets)]
    price = 95.0 + 0.25 * (idx % 8)
    return iid, {"type": "interest_rate_future", "spec": {
        "id": iid, "notional": {"amount": "1000000", "currency": "USD"},
        "expiry": f"{y}-{m}-17", "fixing_date": f"{y}-{m}-17",
        "period_start": f"{y}-{m}-19",
        "period_end": f"{ey}-{em}-18",
        "quoted_price": price, "day_count": "Act360",
        "position": "long" if idx % 2 == 0 else "short",
        "contract_specs": {"face_value": 1000000.0, "tick_size": 0.0025,
                           "tick_value": 6.25, "delivery_months": 3, "convexity_adjustment": 0.0002},
        "discount_curve_id": "USD-OIS", "forward_curve_id": "USD-SOFR-3M",
        "vol_surface_id": None,
        "attributes": {"tags": ["rates-futures"], "meta": {}},
    }}


def spot_equity(idx: int) -> tuple[str, dict]:
    iid = f"EQ-{idx}"
    return iid, {"type": "equity", "spec": {
        "id": iid, "ticker": "AAPL", "currency": "USD", "shares": 100.0,
        "price_quote": None, "price_id": "AAPL-SPOT", "div_yield_id": "AAPL-DIV",
        "discount_curve_id": "USD-OIS",
        "attributes": {"tags": ["equity"], "meta": {}},
    }}


def variance_swap(idx: int) -> tuple[str, dict]:
    iid = f"VARSWAP-{idx}"
    strike_var = 0.04 + 0.005 * (idx % 4)
    side = "receive" if idx % 2 == 0 else "pay"
    mat_year = 2026 + (idx % 3)
    return iid, {"type": "variance_swap", "spec": {
        "id": iid, "underlying_ticker": "SPX",
        "notional": {"amount": "100000", "currency": "USD"},
        "strike_variance": strike_var, "start_date": AS_OF_STR,
        "maturity": f"{mat_year}-01-15",
        "observation_freq": {"count": 1, "unit": "days"},
        "observation_calendar_id": "weekends_only",
        "realized_var_method": "CloseToClose", "side": side,
        "discount_curve_id": "USD-OIS", "day_count": "Act365F",
        "attributes": {"tags": ["equity-vol"], "meta": {}},
    }}


def fx_swap(idx: int) -> tuple[str, dict]:
    iid = f"FXSWAP-{idx}"
    far_year = 2025 + (idx % 2)
    far_rate = 1.085 + 0.005 * (idx % 4)
    return iid, {"type": "fx_swap", "spec": {
        "id": iid, "base_currency": "EUR", "quote_currency": "USD",
        "near_date": "2025-01-17", "far_date": f"{far_year}-07-17",
        "base_notional": {"amount": "1000000", "currency": "EUR"},
        "domestic_discount_curve_id": "USD-OIS", "foreign_discount_curve_id": "EUR-OIS",
        "near_rate": 1.08, "far_rate": far_rate,
        "attributes": {"tags": ["fx"], "meta": {}},
    }}


def convertible_bond(idx: int) -> tuple[str, dict]:
    iid = f"CB-{idx}"
    coupon = 0.015 + 0.005 * (idx % 4)
    ratio = 20.0 + 5.0 * (idx % 3)
    mat_year = 2028 + (idx % 4)
    return iid, {"type": "convertible_bond", "spec": {
        "id": iid, "notional": {"amount": "1000000", "currency": "USD"},
        "issue_date": "2024-01-15", "maturity": f"{mat_year}-01-15",
        "discount_curve_id": "USD-IG", "credit_curve_id": "USD-CREDIT-BBB",
        "conversion": {"ratio": ratio, "policy": "Voluntary",
                       "anti_dilution": "None", "dividend_adjustment": "None"},
        "underlying_equity_id": "TECH",
        "fixed_coupon": {"coupon_type": "Cash", "rate": coupon,
                         "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360",
                         "bdc": "following", "calendar_id": "weekends_only",
                         "end_of_month": False, "payment_lag_days": 0, "stub": "None"},
        "attributes": {"tags": ["convertible"], "meta": {}}, "pricing_overrides": {},
    }}


def instrument_description(instrument_spec: dict) -> str:
    """One-line analyst-readable label for a portfolio position."""
    itype = instrument_spec["type"]
    s = instrument_spec["spec"]
    ref_year = AS_OF.year

    if itype == "bond":
        tenor = int(s["maturity"][:4]) - ref_year
        cf = s["cashflow_spec"]
        if "Fixed" in cf:
            cpn = float(cf["Fixed"]["rate"]) * 100
            return f"Fixed {tenor}Y {cpn:.1f}%"
        sp = cf["Floating"]["rate_spec"]["spread_bp"]
        return f"FRN {tenor}Y SOFR+{sp}bp"

    if itype == "term_loan":
        tenor = int(s["maturity"][:4]) - ref_year
        rate = s["rate"]
        if "Fixed" in rate:
            return f"TL Fixed {tenor}Y {rate['Fixed']['rate_bp']}bp"
        flt = rate["Floating"]
        sp = flt["spread_bp"]
        floor = flt.get("floor_bp", "0")
        return f"TL Flt {tenor}Y SOFR+{sp}bp (fl {floor}bp)"

    if itype == "revolving_credit":
        tenor = int(s["maturity"][:4]) - ref_year
        drawn = float(s["drawn_amount"]["amount"]) / 1e6
        commit = float(s["commitment_amount"]["amount"]) / 1e6
        return f"Revolver {tenor}Y {drawn:.0f}/{commit:.0f}M"

    if itype == "credit_default_swap":
        tenor = int(s["premium"]["end"][:4]) - ref_year
        spread = float(s["premium"]["spread_bp"])
        side = s["side"][:3].title()
        return f"CDS {side} {tenor}Y {spread:.0f}bp"

    if itype == "cds_index":
        tenor = int(s["premium"]["end"][:4]) - ref_year
        spread = float(s["premium"]["spread_bp"])
        side = s["side"][:3].title()
        return f"CDX IG {side} {tenor}Y {spread:.0f}bp"

    if itype == "cds_tranche":
        a, d = s["attach_pct"], s["detach_pct"]
        return f"CDX Tr {a:.0f}-{d:.0f}%"

    if itype == "cds_option":
        strike_bp = float(s["strike"]) * 10000
        opt = s["option_type"].title()
        return f"CDS Opt {opt} K={strike_bp:.0f}bp"

    if itype == "structured_credit":
        deal = s["deal_type"]
        tr = s["tranches"]["tranches"][0]
        cpn = tr["coupon"]
        if "Fixed" in cpn:
            rate = cpn["Fixed"]["rate"] * 100
            return f"{deal} Snr {rate:.1f}%"
        return f"{deal} Snr"

    if itype == "interest_rate_swap":
        tenor = int(s["fixed"]["end"][:4]) - ref_year
        rate = s["fixed"]["rate"] * 100
        side = s["side"].title()
        return f"IRS {side} {tenor}Y {rate:.1f}%"

    if itype == "swaption":
        swap_tenor = int(s["swap_end"][:4]) - ref_year
        strike = s["strike"] * 100
        opt = s["option_type"].title()
        return f"Swpn {opt} {swap_tenor}Y K={strike:.1f}%"

    if itype == "interest_rate_future":
        exp = s["expiry"][:7]
        pos = s["position"][:1].upper()
        price = s["quoted_price"]
        return f"SOFR Fut {exp} {pos} @{price:.2f}"

    if itype == "equity":
        return f"{s['ticker']} {s['shares']:.0f}sh"

    if itype == "variance_swap":
        tenor = int(s["maturity"][:4]) - ref_year
        side = s["side"][:3].title()
        strike_vol = (s["strike_variance"] ** 0.5) * 100
        return f"VarSwap {s['underlying_ticker']} {side} {tenor}Y σ={strike_vol:.0f}%"

    if itype == "fx_swap":
        return f"FX {s['base_currency']}/{s['quote_currency']} {s['far_date'][:7]}"

    if itype == "convertible_bond":
        tenor = int(s["maturity"][:4]) - ref_year
        cpn = s["fixed_coupon"]["rate"] * 100
        return f"CB {s['underlying_equity_id']} {tenor}Y {cpn:.1f}%"

    return itype


INSTRUMENT_FACTORIES = [
    ("fixed_bond", fixed_bond),
    ("floating_bond", floating_bond),
    ("term_loan", term_loan),
    ("revolver", revolver),
    ("cds", cds),
    ("cds_index", cds_index),
    ("cds_tranche", cds_tranche),
    ("cds_option", cds_option),
    ("clo_deal", clo_deal),
    ("abs_deal", abs_deal),
    ("irs", irs),
    ("swaption", swaption),
    ("ir_future", ir_future),
    ("spot_equity", spot_equity),
    ("variance_swap", variance_swap),
    ("fx_swap", fx_swap),
    ("convertible_bond", convertible_bond),
]
