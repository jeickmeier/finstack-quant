# STATEMENTS-ANALYTICS-CREDIT-18: WARM exposure-years loss-rate measurement

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

WARM uses a historical annual loss rate applied to the integral of expected exposure over remaining life. It is a separate dispatch path from PD/LGD/EAD and therefore does not use the PD scenarios or impaired-credit branch.

## Scope and input domain

Exposure validated under the common ECL contract; CECL configuration selects `Warm` and requires an explicit finite annual loss rate in [0,1]. Common bucket/forecast/reversion/history/recovery controls are still validated even when they do not affect WARM arithmetic.

## Algorithm

1. Engine construction allows empty PD sources for WARM and does not validate supplied scenario weights or LGD overrides. Measurement validates exposure, then dispatches to WARM before PD-source lookup and impairment classification.
2. Cover remaining maturity with ceil(T/bucket_width) buckets, at least one. Clip the final endpoint; evaluate converted drawn-plus-undrawn EAD at each midpoint.
3. Sum `EAD(midpoint)*(end−start)` to obtain exposure-years. Multiply by the configured annual historical loss rate. Reject a non-finite final amount and return contractual remaining maturity as horizon.
4. Zero maturity produces a zero-duration bucket and zero allowance. Constant EAD gives rate × EAD × maturity; an amortizing schedule changes the exposure-years integral. No allowance cap at current EAD is applied.

## Selection and controls

`CeclMethodology::Warm` and explicit `warm_annual_loss_rate` are required. Supplying that rate under PD/LGD/EAD errors. Bucket width affects numerical integration; PD sources, scenario weights, LGD, EIR, discount toggle, stage/default flags and forecast/historical PD reversion do not enter WARM arithmetic.

## Conventions

Rate is an annual loss fraction already containing loss severity; multiplying LGD again would double-count severity. Exposure-years retains the exposure's implicit base currency times years. This implementation is undiscounted even when the shared config's discount toggle is true.

## Limitations, failure and fallback

Generic exposure validation still rejects invalid LGD/EIR even though WARM does not read them. An impaired exposure uses the same pooled historical loss-rate method as a performing exposure. No separate macro/Q-factor adjustment, vintage schedule or individual assessment is inferred. Midpoint integration does not split at EAD knots, and multi-year loss-rate products can exceed current EAD.

## Alternatives and tradeoffs

Exact trapezoidal integration of piecewise-linear EAD with knot splitting removes avoidable schedule-grid error. Explicit exclusion or a separately defined impaired-asset route can limit WARM's economic scope. Time-varying loss rates or documented qualitative adjustments broaden the method but require their own methodology controls.

## Evidence

[cecl.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs) owns WARM and dispatch. Inline `warm_constant_ead_golden`, `warm_amortizing_ead_golden`, loss-rate validation and retired-vintage-wire tests are located; the golden cases provide hand-formula evidence for simple schedules. [ASC 326 reference](../../../REFERENCES.md#asc-326-cecl) supplies framework context, not approval of every exposure/method combination.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
