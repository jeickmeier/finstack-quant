# VALUATIONS-OTHER-FIXED-INCOME-02: Convertible defaultable-equity lattice and exercise sequencing

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

The native/default `Tree` route calls a defaultable-equity CRR lattice in [credit_tree.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/credit_tree.rs), with cashflow/exercise mapping in [valuator.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/valuator.rs). Equity jumps to zero at default and surviving equity drift includes hazard. Both total value and the diagnostic cash component use the same survival-weighted discounting plus fractional principal recovery. This is not a Tsiveriotis–Fernandes split-discount implementation, despite cash/equity bookkeeping and older terminology in surrounding documentation.

## Scope and algorithm

1. Validate contract, ratio/price consistency, dated conversion events, positive tree steps, spot currency and recovery. Build the coupon/principal schedule. Process time is Actual/365 Fixed; default grid has 200 steps. Volatility comes first from explicit implied-vol override, otherwise a Black-lognormal strike surface at effective conversion strike/maturity or a unitless flat scalar. Dividend yield is zero only when no identifier is configured; configured missing/wrong-type data errors.
2. Use constant CRR up/down factors for recombination. At each step infer conditional growth from dated risk-free DF and conditional survival: `r_step+lambda_step=−ln(DF×S_step)/dt`; branch probability is `(exp((r_step+lambda_step−q)dt)−d)/(u−d)`. Invalid probabilities fail through the shared evolution helper.
3. Dated flows map to the nearest Act/365F step, with discount-and-survival timing correction; post-maturity principal gets a recovery tail adjustment. Recovery within a step is split at actual balance events and integrated through `continuous_frp_weight` on outstanding principal. Explicit recovery in `[0,1]` is required whenever credit curve is supplied.
4. At maturity choose optional conversion versus cash principal/put, or force mandatory conversion; always retain scheduled coupon. Only the diagnostic cash/equity split is smoothed across one terminal log-spot cell; total payoff remains the maximum.
5. Backward step: `continuation = DF×S_step×E[next_total]+recovery`. Apply allowed voluntary conversion, then issuer call with the holder's allowed conversion response, then holder put; finally add coupon/principal due at that node. Mandatory conversion takes precedence and extinguishes call/put alternatives.
6. Conversion windows map dates to grid steps. Price-trigger conversion uses current node spot; IPO/change-of-control conversion remains inactive without a realized settlement state. Mandatory-variable payoff is `face×S/lower` below lower strike, face between bounds, and `face×S/upper` above upper strike. Scale conversion by outstanding/face.
7. Dividend protection accretes conversion ratio as `exp(max(q,0)t)` while leaving stock drift at `r−q+lambda`. Soft-call days-above conditions are approximated by a trigger uplift `1+β_BGK × required_days/observation_days × sigma × sqrt(1/252)`; no observation history is counted.

## Contract adjustments and controls

[Types](../../../../finstack-quant/valuations/fixed-income/src/convertible/types.rs) apply all ordered dilution events before pricing: full ratchet uses the minimum issue price; weighted average updates `CP←CP×(old_shares+new_money/CP)/(old_shares+new_shares)`. The resulting ratio is face/CP. Events are not filtered by `as_of`; their dates are validated but the full recorded list affects today's conversion ratio. CP below `1e-10` errors. Make-whole calls use deterministic reference-curve PV floored by clean call percentage plus accrued. Call/put overlaps resolve to minimum call and maximum put strikes per grid node.

## Limitations and alternatives

A one-factor equity lattice has deterministic rate, hazard and volatility term inputs, no equity-credit correlation factor, no default recovery in stock, and no true soft-call path memory. Future dilution events can affect current value before their event dates; the input contract must specify whether records represent already-effective events or future schedules. Grid mapping, terminal split smoothing and dividend protection are distinct choices requiring validation. A TF split model, two-factor credit model, PDE or path-dependent simulation would be alternatives, not currently registered substitutes.

## Evidence

[Pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs) inspect exercise ordering, martingale/default behavior and numerical regressions; [type tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/types.rs) inspect dilution ordering and collapsed price rejection. [TF](../../../REFERENCES.md#tsiveriotis-fernandes-1998), [Ayache–Forsyth–Vetzal](../../../REFERENCES.md#ayache-forsyth-vetzal-2003) and [BGK](../../../REFERENCES.md#broadie-glasserman-kou-1997) are comparison references, not proof of equivalence.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
