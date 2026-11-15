# STATEMENTS-ANALYTICS-CREDIT-18: WARM exposure-years loss-rate measurement

- Decision status: **Accepted for the full WARM exposure-years measurement contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for WARM exposure-years and annual loss rates

1. Elect WARM for verified performing exposures in a comparable loss-experience pool. Use a separately selected recovery assessment for impaired exposures. Missing eligibility or impairment evidence makes the assessment incomplete. [Impaired recovery assessment](17-cecl-forecast-reversion.md#accepted-decision-for-lifetime-cecl-forecast-and-reversion).
2. Use a sourced annual net loss fraction in `[0,1]` matched to the exposure basis. Record the cohort, observation window, recovery treatment and explicit adjustments. The rate incorporates loss severity.
3. Verify the exposure path, remaining life and reporting clock. Split at every exposure knot and at the exact horizon. For each segment, multiply the average endpoint exposure by the duration in years and sum the exposure-years with checked arithmetic. An eligible exposure with zero remaining life has zero exposure-years and zero allowance. [Exposure inputs](12-ecl-exposure-and-pd.md#accepted-decision-for-exposure-and-cumulative-default-inputs) · [Numerical accumulation](../../standards/0042-numerical-accumulation-and-comparisons.md).
4. Multiply exposure-years by the annual rate once to obtain the undiscounted allowance. Reject incompatible PD, LGD, scenario and discount settings. Report the currency, exposure-years, annual loss rate and policy provenance. Retain the calculated allowance and flag it when it exceeds opening exposure. [Resolved impairment policy snapshots](14-ecl-policy-defaults.md#accepted-decision-for-resolved-impairment-policy-snapshots).

For example, an exposure path of USD 100 today, USD 50 at year one and zero at year three gives 75 USD-years in the first segment and 50 USD-years in the second. Apply a 2% annual net loss rate to the total of 125 USD-years to obtain USD 2.50 of allowance.

## Audited choice and rationale

WARM uses a historical annual loss rate applied to the integral of expected exposure over remaining life. It is a separate dispatch path from PD/LGD/EAD and therefore does not use the PD scenarios or impaired-credit branch.

## Audited scope and input domain

Exposure validated under the common ECL contract; CECL configuration selects `Warm` and requires an explicit finite annual loss rate in [0,1]. Common bucket/forecast/reversion/history/recovery controls are still validated even when they do not affect WARM arithmetic.

## Audited algorithm

1. Engine construction allows empty PD sources for WARM and does not validate supplied scenario weights or LGD overrides. Measurement validates exposure, then dispatches to WARM before PD-source lookup and impairment classification.
2. Cover remaining maturity with ceil(T/bucket_width) buckets, at least one. Clip the final endpoint; evaluate converted drawn-plus-undrawn EAD at each midpoint.
3. Sum `EAD(midpoint)*(end−start)` to obtain exposure-years. Multiply by the configured annual historical loss rate. Reject a non-finite final amount and return contractual remaining maturity as horizon.
4. Zero maturity produces a zero-duration bucket and zero allowance. Constant EAD gives rate × EAD × maturity; an amortizing schedule changes the exposure-years integral. No allowance cap at current EAD is applied.

## Audited selection and controls

`CeclMethodology::Warm` and explicit `warm_annual_loss_rate` are required. Supplying that rate under PD/LGD/EAD errors. Bucket width affects numerical integration; PD sources, scenario weights, LGD, EIR, discount toggle, stage/default flags and forecast/historical PD reversion do not enter WARM arithmetic.

## Audited conventions

Rate is an annual loss fraction already containing loss severity; multiplying LGD again would double-count severity. Exposure-years retains the exposure's implicit base currency times years. This implementation is undiscounted even when the shared config's discount toggle is true.

## Audited limitations, failure and fallback

Generic exposure validation still rejects invalid LGD/EIR even though WARM does not read them. An impaired exposure uses the same pooled historical loss-rate method as a performing exposure. No separate macro/Q-factor adjustment, vintage schedule or individual assessment is inferred. Midpoint integration does not split at EAD knots, and multi-year loss-rate products can exceed current EAD.

## Audited alternatives and tradeoffs

Exact trapezoidal integration of piecewise-linear EAD with knot splitting removes avoidable schedule-grid error. Explicit exclusion or a separately defined impaired-asset route can limit WARM's economic scope. Time-varying loss rates or documented qualitative adjustments broaden the method but require their own methodology controls.

## Evidence

[cecl.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/cecl.rs) owns WARM and dispatch. Inline `warm_constant_ead_golden`, `warm_amortizing_ead_golden`, loss-rate validation and retired-vintage-wire tests are located; the golden cases provide hand-formula evidence for simple schedules. [ASC 326 reference](../../../REFERENCES.md#asc-326-cecl) supplies framework context, not approval of every exposure/method combination.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
