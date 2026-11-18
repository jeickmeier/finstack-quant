# MARGIN-COLLATERAL-IM-015 — Collateral eligibility and credited value

**Decision status:** Accepted 2026-11-17 for checked collateral eligibility and credited value.

**Implementation status:** Existing behavior is described below. Qualified asset inputs, strict schedule selection, explicit currency rules, checked amounts and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary haircut references inspected. Independent Decimal examples checked reference rates, the one-year and five-year boundaries, additive haircuts, combined bounds and credited values. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked collateral eligibility and credited value

1. Elect one immutable dated schedule for the exact agreement, account, side and IM or VM purpose. Admit qualified asset identity, issuer where applicable, class, native currency and dated nonnegative gross Money value. Enforce the elected currency, issuer and related-party constraints. Require source-qualified maturity and rating evidence wherever the rules use it. An unrated security cannot satisfy a rating floor. [Schedule election](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) · [Rating conventions](../core-conventions/019-credit-ratings.md).
2. Use one strict lookup with nonoverlapping rules and exact interval endpoints. Apply the same validation to the selected schedule and its restored state. Remove vector-order precedence, the maturity tolerance, class-only eligibility, generic haircut fallback and the assumed government-bond class. Report known ineligibility with a reason, missing required evidence as a typed error and eligible zero credit as a distinct result.
3. The narrow bundled reference covers cash and qualified government or central-bank securities. Cash has a zero base haircut. Government and central-bank securities retain the profile's local AA− floor and use 0.5% for `[0, 1)` years, 2% for `[1, 5]` years and 4% for `(5, ∞)` years. Retain the earlier Treasury grid as reference evidence. An operative Treasury schedule needs qualified Treasury-issued security identity, dated rates, currency terms and any reuse authorization. Other asset classes need explicit elected entries. [Basel MGN20, Table 2](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15).
4. Compare native collateral currency with the governing currency or permitted currency set for the selected side and margin type. Apply the elected FX add-on, including an expressly authorized exemption or all-assets rule. Missing required currency terms error. Retain the native currency and mismatch decision through any spot conversion. [Currency treatment by margin type](https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX%3A32016R2251).
5. Convert gross value to account-currency Money `V` once using qualified FX. Add base and FX haircuts as exact decimal fractions and require the total `H` in `[0, 1]`. Compute credited value `V × (1 − H)` with checked Money arithmetic and derive the deduction as `V − credited`. A combined haircut above one errors. At exactly one the eligible asset receives zero credit. [Additive haircut formula](https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX%3A32016R2251) · [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
6. Replace generic haircut IM and its fixed two-day MPOR with one checked collateral valuation result. Carry eligibility, selected rule, valuation and FX evidence, haircuts and amounts through Rust, Python and WASM. Record 016 owns concentration, record 017 owns custody and transfer terms, record 020 owns repo margin flows and record 022 owns the wire format. [Concentration](16-collateral-concentration.md) · [Contractual terms](17-csa-selection-and-im-terms.md) · [Repo margin](20-repo-margin-cashflows.md#accepted-decision-for-checked-repo-cash-margin-and-interest) · [Result transport](22-wire-tables-and-provenance.md).

With a converted gross value of USD 1 million, an asset haircut of 2% and an FX add-on of 8%, the credited value is USD 900,000 and the deduction is USD 100,000. At exactly one year the qualified government security uses the 2% asset haircut.

## Choice and rationale

The observed calculator reports a haircut deduction as IM and selects the first entry satisfying its maturity and rating checks. The accepted calculation identifies eligible collateral and returns its credited value and deduction.

## Input domain and conventions

Strict calculation requires finite nonnegative collateral Money. Optional remaining maturity is finite/nonnegative years; an entry with maturity/rating restrictions needs corresponding supplied metadata. Ratings use the core CreditRating parser/order; NR never satisfies a minimum rating. Decimal base/FX haircuts are each required within[0,1].

## Ordered algorithm

1. Iterate schedule entries in vector order; match asset class, then optional maturity constraints, then minimum rating. Maturity comparison permits ±2.7e-6 years; minimum and maximum are inclusive with tolerance. Overlapping boundary entries therefore select the first.
2. A rating-restricted entry requires a non-NR rating no worse than its parsed floor. An invalid encountered rating label errors. The matching entry's base and FX haircuts must each be finite/in[0,1].
3. Total haircut is base+FX addon when mismatch=true, else base. No total cap is applied. IM=value×total_haircut, one class breakdown, fixed MPOR 2; no CSA thresholds are applied.
4. Trait calculation requires explicit exposure base. Mismatch is inferred only if posted-collateral currency was declared and differs from the amount currency; absent declaration means no FX addon.
5. Lightweight `haircut_for` selects the first class entry without maturity/rating checks, `haircut_for_maturity` checks only maturity, and both can return default_haircut. `is_eligible` treats any configured default haircut as class eligibility. Strict `HaircutImCalculator` never uses that fallback and does not infer metadata.

## Selection, controls and limitations

Embedded `bcbs_standard` permits cash 0% and AA- or better government bonds at 0.5%≤1y,2%≤5y,4%thereafter with8% FX addon; no default haircut/rehypothecation. `us_treasuries` has0.5%≤1y,1%≤5y,1.5%≤10y,2%thereafter, zeroFX addon and permits reuse; there is no issuer/country field enforcing actual US Treasury identity. Class conveniences use broader defaults (government 2%, corporate5%, equity/gold/funds15%, etc.) independently of these schedules.

## Alternatives and tradeoffs

The accepted lookup uses qualified security and issuer inputs and an exact schedule match. Explicit additive haircuts give the credited amount used at the agreement boundary.

## Source and validation evidence

Inspected hand/invariant tests cover maturity10y selecting1.5%, zero Treasury FX addon, rejected missing maturity/negative collateral/ineligible equity, and 8% cash FX addon. Tests establish the implemented cases, not general regulatory or legal eligibility.

Implementation and data: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [src/types/serde_validation.rs](../../../../finstack-quant/margin/src/types/serde_validation.rs), [src/calculators/im/haircut.rs](../../../../finstack-quant/margin/src/calculators/im/haircut.rs), [data/margin/collateral_schedules.v1.json](../../../../finstack-quant/margin/data/margin/collateral_schedules.v1.json).

Existing evidence: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [src/calculators/im/haircut.rs](../../../../finstack-quant/margin/src/calculators/im/haircut.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
