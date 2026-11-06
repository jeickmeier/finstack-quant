# VALUATIONS-BONDS-14: Quoted-yield duration, convexity and signed yield DV01

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and selection

Bond risk defaults to `Workout`; model-based `CallableOas` is an explicit metric override. The default uses a clean-quoted optional bond's cached worst-workout yield and cashflows; otherwise it uses YTM and cached settlement-entitled cashflows. This reproduces the chosen quotation basis, rather than reoptimizing exercise after yield moves. Source comments associate the default and convexity scale with Bloomberg YAS; that is implementation rationale, not an independently verified market mandate.

## Formulation and conventions

Time is the bond yield clock from settlement (record 02), nominal yield `y` and frequency `m=max(payments_per_year,1)`. Recompute `P=Σ CF_i (1+y/m)^(−m t_i)` on positive-time flows. Macaulay duration is `Σ t_i PV_i/P`. Modified duration is `D_mac/(1+y/m)`, with near-zero denominator (`1e−12`) an error. Convexity is `Σ PV_i (m t_i)(m t_i+1)/(m+y)^2 /P/100`. The `/100` is a reporting convention: Taylor price change uses `0.5·C_reported·100·dy²`. Weighted sums use compensated accumulation. Near-zero yield price returns zero Macaulay duration and convexity.

`YieldDv01=−P·D_mod·0.0001`, in currency for an upward one-bp quoted-yield move. Long ordinary bonds normally produce negative DV01. Under `CallableOas`, modified duration and convexity of optional bonds instead use the curve-bump method (record 15), but `YieldDv01` still multiplies that effective duration by the yield-basis price. Macaulay duration never switches to a model-based measure. This mixed basis is observed behavior, not an adopted equivalence between yield and curve risk.

## Limitations, alternatives and evidence

Workout is selected once, and its switches are not differentiated. Signed/nonstandard cashflows can yield unusual duration/convexity and nonunique yields. With no explicit clean quote an optional bond's yield-risk basis remains maturity cashflows, even if model PV reflects optimal exercise. Supported alternative is `CallableOas`; continuously compounded yield derivatives or finite differences on a freshly selected workout are different unimplemented conventions.

Owning code: [shared risk basis/cache](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/mod.rs), [Macaulay](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/duration_macaulay.rs), [modified duration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/duration_modified.rs), [convexity](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/convexity.rs), [yield DV01](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/yield_dv01.rs). [Duration tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/duration.rs) inspect zero-coupon maturity, the modified/Macaulay identity, finite-difference convexity against the same yield kernel, and workout-versus-OAS selection; these are analytic/internal-consistency checks, not universal vendor agreement. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
