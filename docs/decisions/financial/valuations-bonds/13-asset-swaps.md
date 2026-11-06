# VALUATIONS-BONDS-13: Asset-swap annuities, projection and market/par spread conventions

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

Par ASW measures coupon versus swap return without a price adjustment. Market ASW includes bond purchase price and normalizes by the floating spread annuity. An optional ASW forward curve selects dual-curve projection; otherwise floating PV is the discount-factor difference. Ordinary quote inversion accepts only plain fixed bonds without custom cashflows or exercise; optional forward market-ASW reporting can use a clean-quoted workout. Custom calculator support is a distinct floating-spec equivalent-coupon route.

## Algorithms and conventions

Fixed annuity is `A_fix=Σα_fix D(payment)` on supplied dates, with ACT/365L per-period context and ICMA frequency context. Single-curve par rate is `(D_start−D_end)/A_fix`; helper returns zero rate/annuity for fewer than two dates or tiny annuity. Calculator boundaries reject unusable schedules/annuities.

Forward components compute `PV_float=Σ(F_i+spread_bp/10000)α_float D_i` and `A_float=Σα_float D_i`. For curve tenor at most one week, projection uses the forward curve's period rate and rescales its accrual to the floating-leg basis; otherwise use term `rate_between_dates`. Seasoned workout floating periods require exact historical reset fixings when reset precedes valuation; missing fixing errors. Other routes construct a fresh settlement-start schedule and project without that seasoned branch.

Ordinary par ASW is `c−par_rate` in the single-curve route or `(c A_fix−PV_float)/A_float` with a forward curve. Market ASW computes settlement-normalized coupon PV, adds principal-PV minus bullet maturity DF where principal rows exist, then `(coupon_PV+1−dirty/face−PV_float)/A_float`. Inversion rearranges that equation to dirty currency. Ex-coupon coupons are excluded from ordinary settlement coupon PV.

The clean-quoted early-workout market route builds a short-back schedule from the prior contractual fixed date, business-day adjusts fixed dates and uses clean price in `(c A_fix+1−clean/100−PV_float)/A_float`. It infers special float conventions from ID text: SOFR→ACT/360 annual, EURIBOR-6M→ACT/360 semiannual; otherwise forward metadata. The ordinary settlement route instead derives frequency from forward tenor. This difference is material for overnight IDs.

Custom forward helpers use future holder dates starting at `as_of`; custom coupon PV includes only Fixed/Stub rows. Equivalent coupon is coupon PV/(face·fixed annuity). Market helper requires explicit dirty currency and adds `(1−dirty/face)/A_float` to par ASW. Calculator dispatch requires a custom bond with Floating spec, while `from_cashflows` creates a Fixed placeholder by default; generic custom-bond ASW is therefore not automatically supported.

## Limits, alternatives and evidence

Several schedule conventions differ between plain, workout and custom paths; custom helpers do not apply the canonical settlement carry/ex-coupon adjustment consistently with ordinary settlement calculations. These observed differences are not accepted market exceptions. Explicit index conventions would avoid name-derived rules but are not implemented in these helpers.

Owning code: [ASW calculators](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/asw.rs), [annuity/projection/settlement components](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs), [inverse domain](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs). [Quote tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/tests.rs) inspect forward-DF implied rates; inline annuity tests locate actual formula checks. General references: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income), [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook). Independent market-convention benchmarks for these distinct paths were not established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
