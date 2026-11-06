# MARGIN-REGULATORY-XVA-014: SA-CCR trade domain, caller-supplied deltas and netting-set validation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

SA-CCR starts from one externally assembled netting set and caller-classified trades. It consumes one consistent reporting currency, signed mark-to-market and supervisory deltas; it does not price trades, derive option deltas, perform FX conversion, or establish legal netting eligibility. The observed validation aims to prevent sign/category inconsistencies, but is not a complete lifecycle or option-input validator.

## Algorithm and conventions

1. Require finite notional, direction, supervisory delta and MTM. Direction must be nonzero, while supervisory delta lies in `[-1,1]`. Effective signed notionals use `delta×abs(notional)`: negative notionals do not reverse exposure.
2. For linear trades, `abs(abs(delta)−1)<=1e-6` and `delta×direction>0`. The direction magnitude is otherwise unused. For options require nonzero delta and an option type: CallLong/PutShort require positive delta; CallShort/PutLong require negative delta.
3. `is_option` must equal whether `option_maturity_date` is supplied, and option expiry must not exceed the underlying end date. IR/FX require no supervisory category; credit/equity/commodity require a category whose asset class agrees.
4. Across the whole supplied book, enforce the same supervisory category for the same `(asset_class,underlier)`, even when trades use different hedging-set labels.
5. Netting configuration requires finite signed collateral/NICA and finite nonnegative threshold/MTA. For margined sets MPOR must be at least 10 business days for bilateral IDs or 5 for cleared IDs. Unmargined configurations do not apply an MPOR floor. No external CCP check supplements the ID's enum tag.
6. Engine alpha defaults to `1.4`; overrides must be finite and at least one. Constructors populate fields but the calculation invokes validation. The unmargined constructor sets threshold/MTA/NICA to zero and MPOR to 10; fields may subsequently be changed.

## Selection and controls

Every trade is validated before calculation; validation is not restricted to nonzero notionals or currently live trades. `SaCcrTrade.notional` is pre-duration notional for IR/credit and already-adjusted notional for FX/equity/commodity. An option's supervisory delta magnitude must therefore be supplied under the intended market/regulatory convention. Config `as_of` controls the calendar-day offsets used by duration and maturity factors.

## Limitations and failure behavior

Option direction is checked only for finiteness/nonzero, not cross-checked with the long/short option type; option type determines the delta sign. Linear trades may carry an unused `option_type`. Blank IDs/underliers/hedging sets, duplicate trades, zero notionals, reversed underlying dates and already-matured trades are not rejected. This record does not reopen the held-by-bank positive collateral direction already recorded in [financial perspectives ADR](../conventions/0044-financial-perspectives-and-boundary-conversions.md); record 18 describes the distinct field scope.

## Alternatives and tradeoffs

Typed validated trades could move errors earlier; automatically deriving supervisory deltas would require strike/forward/volatility conventions absent from this input type. Silently taking a direction field over an option type would introduce a new precedence rule. Current runtime gives the option type precedence.

## Evidence and role

Runtime validation and fixed category mapping: [src/regulatory/sa_ccr/types.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs); book-level category and alpha checks: [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs). Netting ID variants are declared in [src/types/netting.rs](../../../../finstack-quant/margin/src/types/netting.rs). Located type/engine tests cover wrong delta signs, absent option type/expiry, category mismatch, nonfinite fields and invalid MPOR; inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) checks MPOR rejection. These are contract tests, not independently calibrated option deltas. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
