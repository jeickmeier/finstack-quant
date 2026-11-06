# MARGIN-COLLATERAL-IM-020 — Repo margin balance and interest cashflows

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: all active repo margin types share one deterministic collateral-ratio projection. Proposed rationale: track previous agreed top-ups so a persistent valuation shortfall is not called repeatedly; separate margin-interest cashflows from valuation calls.

## Input domain and conventions

Cash principal is Money; collateral observations are same-unit raw f64 values with dates. Margin ratio and trigger discount are caller-provided decimals, with no validating constructor/domain check here. MarkToMarket, NetExposure and Triparty are labels sharing the active calculation; None suppresses calls.

## Ordered algorithm

1. Defaults: no margining, ratio 1.02, trigger threshold 0.01, daily, T+1, no interest, no substitution. MarkToMarket enables interest/substitution with treasury schedule and caller ratio/threshold; Triparty chooses threshold 0.005, T+0 and BCBS collateral schedule.
2. Helpers compute required=principal×ratio; trigger=required×(1-threshold); margin call iff active and collateral<trigger. Deficit/excess helpers measure distance from required without applying active/trigger checks.
3. Projection converts settlement lag to i32 before None short-circuit. Start margin_balance=0. At each observation in input order, effective collateral=valuation+margin_balance. If below trigger, emit positive VariationMarginPay of required-effective and add to balance immediately.
4. Else if effective>required and observation index>0, emit positive VariationMarginReceive of effective-required and subtract from balance. There is no cap by prior cash margin, and no return at the first observation. Settlement is observation date plus business-day lag, without the VM calculator's explicit Following call-date roll.
5. Interest generation returns empty unless pays_margin_interest and an explicit rate are both present. For each adjacent balance pair accrue prior balance×annual decimal rate×caller day-count fraction, paid on the latter date; emit only if abs(interest)>0.01 major currency units. No end balance close-out or reinvestment is synthesized.
6. Margin-call conversion uses type direction: initial→InitialMarginPost; VM-post/top-up→VariationMarginPay; VM-collect→VariationMarginReceive; substitution→CollateralSubstitutionOut, retaining positive event amount and settlement date.

## Selection, controls and limitations

call_frequency, substitution_allowed and eligible_substitutes are not consulted by the projection. The caller owns observation cadence, eligibility and return/cash balance interpretation. No chronology/sorting, rate/ratio domain check, initial margin balance input or implicit market-rate fallback is implemented. Interest uses supplied balance dates, not automatically the generated calls.

## Alternatives and tradeoffs

A full repo contract engine could distinguish net exposure, tri-party operational timing, securities returns and cash margin, with settled balances and final unwind. A strictly top-up-only model would avoid excess returns but change current behavior. Both require agreed financial requirements before changing code.

## Source and validation evidence

Inspected tests check persistent-deficit no-repeat, threshold suppression,3m excess return with no prior top-up, and 2m×5%×7/360≈1944.44 versus Act365F≈1917.81. A weekend settlement regression was located. These are hand/invariant tests, not GMRA compliance evidence.

Implementation and data: [src/types/repo_margin.rs](../../../../finstack-quant/margin/src/types/repo_margin.rs), [src/types/repo_cashflows.rs](../../../../finstack-quant/margin/src/types/repo_cashflows.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs).

Existing evidence: [src/types/repo_margin.rs](../../../../finstack-quant/margin/src/types/repo_margin.rs), [src/types/repo_cashflows.rs](../../../../finstack-quant/margin/src/types/repo_cashflows.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
