# VALUATIONS-OTHER-FIXED-INCOME-09: Term-loan workout yields, quote-pinned risk and default metrics

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

The [metric registry](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/mod.rs) separates yield conventions from model risk. Risk to a quoted price first fits and holds a spread/OAS, so curve bumps change model value instead of leaving a fixed quote unchanged. This economic distinction is observable; method adoption remains draft.

## Scope and algorithm

- [Yield helpers](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/irr_helpers.rs) turn clean percent into dirty settlement money using funded outstanding plus principal already removed from balance but still unpaid, and cash accrued. Without a price quote, remove pre-settlement cash from model PV and divide by the settlement DF.
- YTM is dated XIRR on all purchased future cash settlements. YT2Y/YT3Y/YT4Y truncate at `as_of + n years`, capped at the last cash payment/maturity, and add exercise principal plus cash accrued. These are hypothetical redemption yields, not embedded-call optimal exercise.
- [YTC](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/mod.rs) takes the earliest candidate call; no candidate falls back to YTM. Candidates include future call start dates and, for an already active provision, the first coupon after settlement. [YTW](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/ytw.rs) is the minimum XIRR over these calls plus final maturity. Each candidate includes cash settled through exercise and an additional dirty redemption claim; make-whole claims use the same helper as pricing. This finite candidate set is narrower than every exercise date on the tree.
- [Discount margin](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/discount_margin.rs) is floating-only and solves on projected contractual flows. Callable loans require a clean quote. Without a quote on an otherwise eligible loan, the target is zero-margin PV, so the result is zero; contractual margin is the solver guess.
- [Risk view](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/risk_view.rs) clears price drivers and holds explicit OAS or solves OAS from the clean quote for Tree; Discounting holds explicit/fitted z-spread. OAS on Discounting is rejected. Registered parallel/bucketed DV01 use shared repricing. CS01 uses hazard repricing only for Tree plus credit curve; otherwise it uses periodic z-spread bumps on contractual flows. Thus explicit Discounting can ignore a supplied hazard curve for these metrics.
- Embedded option value is `callable − straight` using the pinned model. [Default risk](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/default_risk.rs) requires Tree plus hazard: Recovery01 delegates to hazard-recovery repricing; jump-to-default is `recovery × outstanding(as_of) − base_value`.

## Controls, conventions and failure behavior

Yield outputs are annual-effective decimal XIRRs, OAS and DM decimal spreads, and sensitivity outputs cash per specified shared bump. Workout date selection uses `as_of`; the price leg is at settlement. A candidate occurring inside the settlement lag can therefore precede the purchase date; that boundary is not adopted here. Solver and fixing errors propagate; missing required quotes error. [Record 06](06-loan-quotes-eir.md) covers shared cost/EIR arithmetic; [record 08](08-term-loan-pricing.md) covers model restrictions.

## Alternatives and tradeoffs

A dense all-dates YTW search would align more closely with standing calls but increase work and require a redemption-date convention. Quote-fixed sensitivities describe mark carry only; spread-pinned sensitivities describe model risk at the mark. Both are meaningful outputs but are not interchangeable. Hazard CS01 and z-spread CS01 measure different credit perturbations.

## Evidence

[Registered metrics](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/registered_metrics.rs), [YTC](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/ytc.rs), [YTW](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/ytw.rs), [DM](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/discount_margin.rs), and [default risk](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) are located regression/invariant evidence, not rerun. [Fixed-income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is contextual.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
