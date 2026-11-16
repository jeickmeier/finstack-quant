# PORTFOLIO-EXECUTION-017 — VM perspective, CSA collateral terms and margin result validation

- **Decision status:** Accepted 2026-11-16 — complete VM call scope and perspective, explicit balances, IM accounts, checked totals and result validation package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent VM sign, account FX, gross outflow, exact Money and structured identity examples verified. Runtime, host and model qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio VM and collateral reports

1. **Calculate VM once per authorized call scope.** Apply signed holding
   size once to each agreement-qualified mark, convert directly into the
   agreement currency and sum in checked Money. Use the effective terms.
   Preserve valuation, call and settlement dates. A missing agreement or
   required mark makes the call unavailable under the accepted coverage
   policy. Remove the negative-MTM fallback. Repo margin uses its
   agreement-qualified route.
   [Margin valuation](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity)
   and [repo agreement rules](../valuations-rates/15-repo.md).

2. **Require explicit dated collateral balances.** Current VM is signed in
   agreement currency, positive when held and negative when posted. A
   zero-balance run requires explicit zero. Bind the exact account, cut and
   elected treatment of pending transfers. Use each admitted balance once
   within its authorized scope. Apply collateral eligibility and haircuts
   once under the owner's rules. The elected VM calculator owns threshold,
   independent amount, minimum transfer, rounding and settlement rules.
   [Variation margin](../margin-collateral-im/18-variation-margin.md).

3. **Report VM calls from the desk's cash perspective.** Signed VM outflow
   equals `post − collect`. A return of excess held collateral is an
   outflow to the desk. Translate the native call once on the elected
   reporting basis. Recognized VM and NICA for SA-CCR are separately
   prepared balance inputs.
   [ADR 0044 collateral components](../conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components).

4. **Apply IM collateral terms once per exact one-way account.** Group
   complete gross IM where the account terms authorize it and convert
   each source amount directly from its calculation currency into the
   account currency. Require explicit current collateral. Apply the
   allocated threshold and MTA once. Preserve gross IM, required and
   current collateral, transfer and custody status. Record whether this
   is a posting or collecting account so transfer direction is explicit.
   [CSA IM terms](../margin-collateral-im/17-csa-selection-and-im-terms.md).

5. **Publish totals on their stated basis.** Report gross model IM,
   signed net VM outflow and gross positive VM outflow separately.
   Identify the combined measure as gross IM plus VM outflow. Report
   required and segregated collateral separately from transfers and
   preserve the posting or collecting side. Apply strict coverage by
   default. Diagnostic mode may retain complete independent results.
   Dependent accounts and headlines remain unavailable if required
   coverage is incomplete.
   [Portfolio IM and coverage](16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).

6. **Rebuild and verify results through one bounded Rust contract.** Use
   exact structured identities, a deterministic scope and account order,
   and holding order within each scope. Retain native and reporting
   Money with directed dated FX evidence sufficient to reproduce mixed-
   currency totals. Validate dates, terms, scope links, checked counts,
   coverage and method-specific breakdowns. Compare derived claims in
   checked Money with exact equality, replacing the binary64 `1e-9`
   tolerance. Preserve exact Money and typed status through Python,
   WASM and UI with an explicit float display conversion. Structural
   restore checks arithmetic and relationships. Economic replay requires
   original qualified inputs.
   [Bounded contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

## Independent calculation checks

- Exposure `+100` with held collateral `+80` produces `−20` desk
  outflow under zero threshold, MTA and rounding. Current collateral
  `+120` produces `+20` outflow. Calls of `+30` and `−20` give net VM
  outflow of 10 and gross positive VM outflow of 30.
- EUR 100 gross less EUR 20 allocated threshold leaves EUR 80 required.
  EUR 70 current gives an account increase of EUR 10. At `1.25 USD/EUR`,
  the required balance and transfer are USD 100 and USD 12.50.
- Exact Money distinguishes `10000000000000000.01` from
  `10000000000000000.00`. Their binary64 projections agree. Likewise,
  distinct structural identities `(A:B, C)` and `(A, B:C)` share the
  same delimiter-joined display label.

## Observed choice and scope at the audit revision

The adapter reports initial margin and signed variation-margin calls in portfolio base currency, with a collateral-outflow total computed separately from net signed VM. It applies CSA IM account terms once to grouped gross IM, while VM is calculated per netting set. The portfolio API accepts current IM collateral by CSA but no current VM collateral input.

## VM and collateral algorithm

Obtain each member instrument's VM MTM, multiply by signed position scale, and spot-convert into base currency. A `Notional(Some(currency))` mismatch is a position error here, unlike the warning-only ordinary scale path. MTM/FX failures are logged as degraded and excluded from the netting-set sum; successful MTM entries determine `position_count`.

For a netting set with a CSA, convert gross signed MTM to CSA base currency and invoke `VmCalculator` with **zero current VM collateral**, then convert its signed net margin back to portfolio currency. The margin crate owns threshold, minimum-transfer and perspective logic. Without a spec, return negative gross MTM; bilateral positions are explicitly degraded because no threshold/MTA/haircut terms were applied. Repo instruments can reach this path: their repo haircut specification is not consumed by this OTC adapter. Cleared no-spec paths do not receive that bilateral degradation marker.

For IM collateral, collect netting-set IM by CSA only where IM parameters exist. Convert each gross amount to CSA currency and sum, then reject any supplied current-collateral CSA absent from that collection. Missing current IM collateral defaults to zero in CSA currency. Apply the CSA's own collateral terms to gross/current balances. Convert required collateral and transfer back to portfolio currency; separately add required balances whose account is segregated. These account fields are distinct from gross model IM.

Per-set `total_margin = IM + max(VM,0)`. Portfolio total is the sum of those per-set totals, while `total_variation_margin` separately sums signed VM. It is therefore **not** `total_IM + max(sum(VM),0)`: a receipt at one counterparty does not cancel another set's positive outflow in that field. Result construction checks IM/VM currency equality; it does not require nonnegative IM at the standalone constructor boundary.

## Wire validation and ordering

Serialization sorts netting sets by display ID and emits CSA/breakdown maps as ordered maps. Deserialization checks matching set currencies and the above per-set total identity using absolute tolerance `1e-9`; portfolio Money currencies must equal declared base. It rejects duplicate netting IDs, checks set sums against all three portfolio margin totals and verifies summed position count.

Each IM account must reference at least one result set with that CSA. Gross/required/current balances must be finite, nonnegative and in one currency. Transfer must be finite in that currency and either zero or equal required minus current within tolerance; required cannot exceed gross. Portfolio collateral totals are reconciled to accounts **only when all account currencies equal the portfolio base**. Cross-currency accounts lack an FX proof in this payload, so no corresponding sum check is made.

The wire adapters do not check all nested consistency: set dates against portfolio date, breakdown currency/sum, sensitivity calculation currency, every degradation ID/count relationship or model/version provenance. Their helper wire structs do not deny unknown fields. Public typed results can also be mutated and serialized without re-running deserialize validation. Thus accepted JSON means the implemented subset of checks passed, not that a margin run was replayed or certified.

## Alternatives and limitations

Actual current VM collateral is needed for a live incremental call; the present zero-balance calculation is an observed scope limit. A strict completeness policy, repo-specific collateral bridge and validated result constructors are possible changes, not existing capabilities. Plain f64/hash-map reductions differ from the [required accumulation policy](../../../../INVARIANTS.md#12-arithmetic-and-aggregation). The `1e-9` wire tolerance is absolute, and canonical wire order differs from runtime map traversal. No bad collateral transfer or round-trip failure was numerically reproduced.

## Evidence

[aggregator.rs](../../../../finstack-quant/portfolio/src/margin/aggregator.rs), [results.rs](../../../../finstack-quant/portfolio/src/margin/results.rs) and [wire.rs](../../../../finstack-quant/portfolio/src/margin/wire.rs). Inspected [margin aggregation tests](../../../../finstack-quant/portfolio/tests/margin_aggregation.rs) distinguish net signed VM from positive outflow totals. [Serialization tests](../../../../finstack-quant/portfolio/tests/margin_serialization.rs) check mixed currencies, inconsistent totals, duplicate IDs and sensitivity bucket round trips. These are algebraic/contract evidence. Underlying VM/IM formulas and reference-version qualification belong to the [margin scope](../margin-collateral-im/README.md); [BCBS/IOSCO](../../../REFERENCES.md#bcbs-iosco-uncleared-margin) is conceptual context only.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
