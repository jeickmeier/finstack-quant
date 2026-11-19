# MARGIN-COLLATERAL-IM-017 — CSA selection and separate IM collateral terms

**Decision status:** Accepted 2026-11-17 for checked CSA elections and one-way IM account terms.

**Implementation status:** Existing behavior is described below. Agreement and account identity, qualified balances, exact transfer arithmetic, MTA policy, custody evidence and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary margin references and live caller paths inspected. Independent Decimal examples checked target and signed transfer arithmetic, exact MTA equality and cancellation at large amounts. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked CSA elections and one-way IM account terms

1. Bind the legal parties, dated agreement, authorized netting scope, exact account, posting or collecting side, currency and information cut. Elect explicit no IM with a reason, qualified bilateral SIMM or Schedule, or the exact cleared route from record 014. Require credit classification where the SIMM contract uses it. Retire the internal-model placeholder and generic haircut IM elections. Keep posting IM, collecting IM and VM in separate accounts. [Basel margin requirements](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15) · [Cleared IM](14-ccp-external-and-proxy.md#accepted-decision-for-elected-cleared-initial-margin-routes) · [Collateral valuation](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value).
2. Require actual dated agreement terms for the allocated group threshold, applicable minimum transfer amount (MTA), horizon, eligibility, custody, settlement and applicable curve and calendar. Apply each allocation once at its authorized scope. Group allocation belongs to the owner with complete group coverage. Retire defaults inferred from currency or methodology. Preserve other elected terms when amending a CSA. [Agreement election](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) · [Basel MGN20.5–20.6](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15).
3. Use qualified IM at the elected horizon. Convert directly into the account currency and sum only scopes authorized for that account. Require a dated current collateral balance credited under record 015 and explicit treatment of eligible pending transfers. An empty account requires an explicit zero. Missing balances, incompatible horizons and incomplete scope make the result unavailable. [Portfolio account scope](../portfolio-execution/17-margin-collateral-and-wire.md#accepted-decision-for-checked-portfolio-vm-and-collateral-reports).
4. In checked Money compute `target = max(pre-threshold IM − allocated threshold, 0)` and `difference = target − current collateral`. Positive differences add to the elected account and negative differences release excess. Use a qualified CCP report that already gives the account's target once. Preserve the indicative status of calculations based on a proxy. [Money arithmetic](../core-conventions/012-money-decimal-precision.md) · [Reported CCP amount](14-ccp-external-and-proxy.md#accepted-decision-for-elected-cleared-initial-margin-routes).
5. For a separately authorized IM MTA, compare the absolute unrounded difference with the elected threshold. Support both at least and strictly greater than, according to governing terms. Transfer the full difference when triggered. A shared IM and VM MTA requires a qualified joint decision. The supported standalone route requires an explicit no-rounding election. Required rounding needs its own supported treatment. [EU Article 25](https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A02016R2251-20230214) · [CFTC combined MTA rule](https://www.ecfr.gov/current/title-17/chapter-I/part-23/subpart-E/section-23.152).
6. Report source IM, target, current balance, raw difference, transfer outcome, side, coverage and required versus observed custody. Distinguish exact balance, MTA suppression, explicit no IM and unavailable. Use one checked Rust calculation and restoration path through portfolio, Python and WASM. Record 019 owns call and settlement timing. Record 022 owns the wire format. [Call and settlement timing](19-margin-dates-and-replay.md) · [Result transport](22-wire-tables-and-provenance.md).

USD 100 of pre-threshold IM with USD 20 of allocated threshold and USD 70 already credited gives a USD 80 target and a USD 10 increase. An MTA of USD 10 admits that difference under the at-least rule and suppresses it under the strictly-greater rule.

## Choice and rationale

The observed CSA operation applies a threshold and transfer filter to supplied model IM. The accepted calculation binds effective terms and credited collateral to an exact one-way account and keeps model amounts, custody requirements and transfers distinct.

## Input domain and conventions

Gross IM is nonnegative Money already calculated at the desired positive MPOR; current collateral is a nonnegative one-way IM account in the same currency. Threshold and MTA must be finite, nonnegative and in that currency. Gross model IM is supplied, not reconstructed from a pricing model here.

## Ordered algorithm

1. Validate CSA VM/IM monetary elections, positive IM MPOR, nonempty ID/curve ID and registered calendar. Validation does not validate the entire eligible-collateral schedule or implement call timing.
2. If no IM terms exist, `apply_im_terms` errors; otherwise target=`max(gross_IM-threshold,0)` and raw_transfer=target-current.
3. Return zero transfer iff abs(raw_transfer)<MTA; equality transfers. There is no IM rounding and no extra MPOR scaling. Positive transfer adds collateral to this one-way account; negative returns excess. Segregation is returned as a custody flag without discounting gross IM.
4. Bilateral SIMM/schedule constructors change only an existing IM methodology; they preserve no-IM agreements. Cleared constructors create a clearing-house CSA with registry defaults, currency-derived `<CCY>-OIS`, zero-lag/one-unit VM rounding and CCP timing.
5. OTC validation requires cleared status iff methodology is ClearingHouse when IM exists. Credit-specific validation additionally requires explicit SIMM CQ-sector or CNQ classification. InternalModel remains an enum and reuses SIMM defaults; it has no calculator in this slice.
6. NettingSetId distinguishes `(counterparty,csa)` bilateral keys from CCP-only cleared keys; this is identity, not an enforceability determination or numerical netting operation.

## Selection, controls and limitations

Default regulatory constructors use SIMM, same numeric50m threshold in any currency and supplied collateral curve string. `with_vm_threshold` preserves frequency/lag but omitted rounding/IA reset to 10,000/0, not previous custom values; `with_im` uses embedded methodology defaults for omitted segregation. Setters check currencies/MPOR but full validation occurs later. Portfolio dispatch/netting and CSA threshold allocation are covered by the portfolio owner.

## Alternatives and tradeoffs

The accepted account operation uses the qualified allocation supplied by the group owner. A shared IM and VM transfer gate requires a qualified joint decision under the elected agreement.

## Source and validation evidence

Inspected CSA/threshold/OTC tests cover defaults, currency mismatch, no-IM preservation and segregation defaults. Portfolio's existing production threshold test was located as cross-owner evidence, not audited or executed here.

Implementation and data: [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/types/netting.rs](../../../../finstack-quant/margin/src/types/netting.rs), [src/types/enums.rs](../../../../finstack-quant/margin/src/types/enums.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/defaults.v1.json](../../../../finstack-quant/margin/data/margin/defaults.v1.json).

Existing evidence: [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs), [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
