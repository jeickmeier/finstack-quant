# VALUATIONS-BONDS-09: Low-sample structural-parameter calibration to a cash-equivalent quote

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

Optional Merton calibration solves one structural parameter against a cash-pay version of the bond, then uses that parameter for full-budget PIK/toggle pricing. It is not a calibration directly to the PIK bond's price. Fixed and step-up coupon specs are accepted by the cash-equivalent helper; amortizers subsequently fail in the MC adapter. Supported targets are clean/dirty price, Japanese simple yield, YTM/YTW and Z-spread; DM/OAS/ASW/I-spread targets error.

## Algorithm and controls

Clone the bond and make its coupon type cash. Convert the target quote at settlement to dirty currency; multiply by settlement DF to obtain target valuation PV. YTM target conversion uses valuation-entitled flows, whereas other quote paths can use settlement entitlement. Build a trial config with a nondefault stepped all-cash PIK schedule so bond PIK defaults cannot override it. Preserve base asset value, payout, risk-free rate, barrier type and dynamics while replacing either debt barrier or asset volatility.

Default barrier bracket is `[0.001V0,0.999V0]`; volatility bracket `[0.01,2]`. Explicit brackets require finite `0<lo<hi`. Evaluate endpoints and require opposite residual signs (exact endpoint match returns immediately). Bisection uses common random numbers, default 2,000 low-path estimators (clamped to at least two), full-run antithetic setting and optional calibration seed. Default maximum is 40 iterations and absolute PV tolerance `1e−4` currency; implementation clamps iterations to at least one and tolerance to nonnegative. Stop only on PV residual, not parameter width. Failure to bracket or converge errors with residual/last parameter.

Return solved model, low-path calibrated PV, target, residual, iterations and parameter. The registry clears calibration and reprices at the ordinary full path budget. Its diagnostic residual is the low-path residual; no full-price revalidation is performed.

## Observed discrepancies, alternatives and evidence

The registry calibrates with `base_config` **before** populating term-structure cashflow DFs. With absent DF knots, calibration discounts at the supplied flat horizon-implied rate, while the final price uses curve DFs. Curved-term-structure quotes therefore need not be reproduced by final full-budget valuation even apart from sample noise. Target carry also omits detached-cash terms present in the canonical settlement bridge. These are source-supported mismatches; their numerical impact was not reproduced.

Alternative supported calibration parameter is volatility versus barrier; simultaneous fit, deterministic analytic calibration, CI-aware loss, automatic bracket expansion or full-budget acceptance are unimplemented alternatives. Tight currency tolerances against a discontinuous finite-sample barrier price can be unattainable; error is preferable to treating the last iterate as calibrated, but tolerance/budget suitability is not established.

Owning code: [calibration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/calibration.rs), [registry ordering](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/pricer.rs), [default calibration spec](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/merton_mc.rs). Inline pricer tests locate calibration failure/term-structure checks; [Merton tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/tests.rs) cover config decoding and shared simulation. These do not independently establish an identifiable structural parameter. [Merton](../../../REFERENCES.md#merton-1974) provides model background, not approval of this low-sample fitting policy.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
