# MARGIN-COLLATERAL-IM-016 — Collateral concentration reporting

**Decision status:** Accepted 2026-11-17 for checked collateral pool concentration.

**Implementation status:** Existing behavior is described below. Qualified pool inputs, value basis, complete grouping, checked results and host reconciliation with the accepted contract are pending.

**Validation status:** Primary concentration references inspected. Independent arithmetic checked repeated-class aggregation, gross and credited bases, equality and a large-value comparison. Runtime and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked collateral pool concentration

1. Bind one dated collateral pool to the agreement, account, side, margin purpose and valuation cut. Use qualified lot values in one account currency under record 015. Count each lot once. Report known ineligible holdings as exclusions and return a typed error when required evidence is missing. [Collateral valuation](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value).
2. Elect either gross market value before haircuts or credited value after haircuts. Use the same basis for each group and the eligible pool total. Include eligible cash and uncapped holdings in the denominator.
3. Use immutable dated rules with explicit asset class, security, issuer or issuer-group scope and exact Decimal caps in `[0, 1]`. Sum all eligible holdings in each applicable group and evaluate overlapping rules independently. Retire the first-entry lookup and inherited 30% corporate cap. This operation covers fractional pool caps. Rules involving additional monetary thresholds require their own qualified calculation. [Basel MGN20.34](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15) · [EU Article 8](https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX%3A32016R2251).
4. Compare `group value > limit × pool value` using checked exact arithmetic on admitted amounts before rounding. Equality passes. Report each group's share, limit and any excess in percentage points at the observed pool size. [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
5. For zero pool value report that the share is unavailable and give the reason. Distinguish an unevaluated share, an explicit election of no limits, an evaluated pass and an input error.
6. Return one checked diagnostic report through Rust, Python and WASM. Preserve scope, cut, value basis, pool total, coverage, exclusions and each rule result. Allocation changes and transfers belong to their respective owners. [Contractual terms](17-csa-selection-and-im-terms.md) · [Result transport](22-wire-tables-and-provenance.md).

Two USD 20 corporate lots and USD 60 cash give a 40% gross corporate share. An expressly elected 30% cap is breached by ten percentage points on that basis. If the corporate lots each receive USD 10 of credit and cash receives USD 60, the credited share is 25%.

## Choice and rationale

The observed concentration check evaluates each supplied row. The accepted report evaluates the full group defined by each elected cap and preserves the value basis used for the comparison.

## Input domain and conventions

Input is a slice of `(CollateralAssetClass,f64 amount)` without currency tags, requiring the caller to normalize units. The API does not validate nonnegative/finite amounts or aggregate repeated classes before computing fractions.

## Ordered algorithm

1. Sum all row amounts with ordinary floating addition. If total≤0, return no breaches.
2. For each original row compute fraction=amount/total.
3. Find only the first schedule entry matching its asset class. If that entry has a concentration_limit and fraction>limit, emit class, fraction, limit and excess=fraction-limit.
4. Equality does not breach. Missing class, missing limit or a later constrained entry produce no breach through this path. Default haircut eligibility does not create a concentration limit.

## Selection, controls and limitations

Limits are decimal fractions. The corporate class convenience inherits30% from embedded class defaults; the default BCBS/treasury schedule entries themselves carry no concentration limits. Output is row-level: two separately supplied20% corporate rows each pass a30% limit even though their combined class amount is40%. This is a direct consequence of the inspected loop, not an executed regression result.

## Alternatives and tradeoffs

The accepted report supports separate cap groups that can share members. A lot enters the pool total once. The chosen allocation and any corrective transfers belong to their respective owners.

## Source and validation evidence

Inline tests cover one corporate row at 20% passing30% and one row at 40% producing a10pp breach. Those assertions do not cover repeated classes, invalid values, first-entry ambiguity or automatic enforcement.

Implementation and data: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [data/margin/collateral_schedules.v1.json](../../../../finstack-quant/margin/data/margin/collateral_schedules.v1.json).

Existing evidence: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
