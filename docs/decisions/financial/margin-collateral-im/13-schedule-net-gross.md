# MARGIN-COLLATERAL-IM-013 — Schedule IM net-to-gross reduction

**Decision status:** Accepted 2026-11-17 for checked schedule netting and one net-to-gross reduction.

**Implementation status:** Existing behavior is described below. Qualified scope, complete inputs, zero-denominator handling, checked net amounts and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary references inspected. Independent Decimal and rational examples checked trade-rate aggregation, signed replacement costs, zero gross replacement cost, a repeating ratio and cancellation. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked schedule netting and reduction

1. Bind one legally enforceable netting set to its elected gross schedule, complete eligible trade population, valuation date and information cut, calculation currency and side receiving IM. Evaluate separate scopes separately. Where the elected treatment permits gross-only calculation, use record 012. Withhold net IM when required coverage is missing. [Basel MGN20.17](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn?allChapters=true) · [Gross schedule contract](12-schedule-notional.md#accepted-decision-for-checked-gross-schedule-initial-margin).
2. Use record 012's checked gross calculation for each qualified trade's effective notional, class and maturity. Apply absolute holding size to notional and signed holding size to replacement marks, once each. Normalize applicable amounts onto the common calculation basis through qualified FX. A positive mark represents exposure owed to the receiving side before collateral and thresholds. Include every eligible mark in the replacement-cost sums, including a mark on a trade with zero gross charge. [Portfolio input and netting contract](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
3. Set gross replacement cost `P = Σ max(mark, 0)` and net replacement cost `N = max(Σ mark, 0)`. Validate `0 ≤ N ≤ P`. When `P > 0`, use `NGR = N/P`. When `P = 0` for a nonempty set, use `NGR = 1` and retain the full gross charge. Preserve which rule produced the ratio. [EU Annex IV replacement-cost definitions](https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX%3A32016R2251) · [CFTC zero-denominator rule](https://www.ecfr.gov/current/title-17/chapter-I/part-23/subpart-E/section-23.154).
4. Sum the checked gross trade charges to `G` and evaluate one net charge `G × (0.4 + 0.6 × NGR)`. For positive `P`, evaluate the equivalent `G × (2P + 3N)/(5P)` from the admitted monetary amounts with one final bounded Decimal rounding. Use checked deterministic Money sums and retain `N` and `P` as ratio evidence. Publish class charges as gross and return one authoritative net charge for the set. [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
5. Return currency-bearing zero for a complete validated empty set with an empty-set reason and no applied ratio. A nonempty set with zero gross IM still validates all trades and computes the replacement costs and NGR. Missing required inputs or unusable arithmetic produce typed errors.
6. Carry gross and net Money amounts, replacement costs, ratio and zero reason, profile, scope, receiving side, valuation cut, currency, reference horizon and coverage through the same Rust result in portfolio, Python and WASM. Apply the elected profile's reference horizon. Record 017 owns contractual collateral and transfers. Record 022 owns the wire format. [Contractual IM](17-csa-selection-and-im-terms.md) · [Result transport](22-wire-tables-and-provenance.md).

Two USD 1 million notionals, short interest rate and equity, give gross IM of USD 160,000. Marks of `+100` and `−60` give a ratio of 0.4 and net IM of USD 102,400. Marks of `+100` and `−100` give net IM of USD 64,000. When every mark is zero or negative, the net charge is USD 160,000. An independent example with `G = 700`, `N = 1` and `P = 7` gives exactly 340.

## Choice and rationale

The observed calculation applies one net-to-gross adjustment across the supplied trades. The accepted contract binds the eligible scope and applies the set-wide factor to the total of individually rated trades.

## Input domain and conventions

Rows are `(signed_mtm, notional, class, maturity_years)` in one currency. Amounts must be finite and maturity nonnegative. The caller must supply an economically/legal valid netting set; the calculator does not inspect agreements or counterparty IDs.

## Ordered algorithm

1. Empty input returns None. Use the first MTM currency; reject any row whose MTM or notional differs.
2. Sum `abs(notional)*trade_rate` by class and gross absolute notional across rows. If gross notional equals zero, return None even if MTM is nonzero.
3. Use Neumaier sums for total signed MTM V and positive MTM P. If P=0, NGR=0; otherwise `NGR=clamp(max(V,0)/P,0,1)`.
4. Multiply every class gross subtotal by `0.4+0.6*NGR`. Fully directional positive MTM gives100% gross IM; fully offset or all-negative MTM gives40%.
5. Store `<class>_ngr` components and Neumaier-sum their monetary amounts for total. Gross-by-class and notional accumulation themselves use ordinary sums. MPOR is not rescaled here.

## Selection, controls and limitations

The same reduction is applied across all included classes. Notional signs do not offset gross IM; MTM signs determine NGR. There is no instrument-product or regulatory offset partition beyond the rows the caller provides. Missing rate cells use the single-trade schedule fallback.

## Alternatives and tradeoffs

The observed zero-denominator rule applies a 40% multiplier without positive replacement marks. The accepted rule retains the whole gross charge in that case. The accepted result reports class totals at the gross step so the net charge is computed from one set-level product.

## Source and validation evidence

Inspected unit tests cover full/partial/no offset, all-negative MTM, empty/mixed currency, and matching constructors. Independent hand regression uses 1m IR at 1% and 1m equity at 15%, offsetting±100 MTM, for `(10k+150k)*0.4` total.

Implementation and data: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [data/margin/schedule_im.v1.json](../../../../finstack-quant/margin/data/margin/schedule_im.v1.json).

Existing evidence: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
