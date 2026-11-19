# MARGIN-REGULATORY-XVA-016: SA-CCR interest-rate maturity buckets and hedging-set add-ons

- Decision status: **Accepted 2026-11-17 for the checked SA-CCR interest-rate add-on.**
- Implementation status: Existing behavior is described below. Typed hedging sets, prescribed factors, checked aggregation and applied result evidence remain pending.
- Validation status: Primary rules and Rust paths checked. Independent arithmetic covered bucket boundaries, exact within-set offsets, adjacent and outer bucket charges, separate sets, factor multipliers and large finite amounts. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for the checked SA-CCR interest-rate add-on

1. **Use qualified signed effective notionals.** Admit each eligible IR
   trade or prescribed component once within its legal netting set and
   applicable calculation group. Use the qualified signed
   `D = adjusted notional × supervisory delta × MF`. Retain trade and
   component identity, reporting currency and the qualified underlying
   end time `E`.
   [Record 014](14-saccr-inputs-and-supervisory-delta.md#accepted-decision-for-checked-sa-ccr-trade-inputs-and-supervisory-delta) ·
   [Record 015](15-saccr-maturity-and-duration.md#accepted-decision-for-checked-sa-ccr-maturity-factors-and-supervisory-duration) ·
   [Basel CRE52.57](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
2. **Derive hedging sets from typed risk identities.** Use one ordinary
   nominal set per risk currency. Elect the Basel inflation treatment with
   a separate inflation set per currency. Use a separate basis set for each
   qualified same-currency factor pair and express sign against its canonical
   spread direction. Use a separate volatility set per currency for
   contracts referencing volatility or variance. Route cross-currency
   swaps whose two floating legs reference different currencies to ordinary
   FX. Require an explicit rulebook treatment for unresolved assignments.
   [Basel CRE52.45–47 and 52.57 FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
3. **Bucket by the qualified reference end time.** Use bucket 1 for
   `E < 1`, bucket 2 for `1 ≤ E ≤ 5`, and bucket 3 for `E > 5`, in
   years. On the accepted ACT/365F date basis, 365 and 1,825 elapsed days
   both enter bucket 2. Sum signed effective notionals within each bucket
   to recognize full offset for eligible long and short positions there.
   [Basel CRE52.56–57](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
4. **Elect and check the Basel partial-offset formula.** For signed bucket
   totals `D1`, `D2` and `D3`, set the hedging-set effective notional `EN`
   to the square root of
   `D1² + D2² + D3² + 1.4(D1D2 + D2D3) + 0.6D1D3`.
   Use checked ordered sums. Evaluate the norm in scaled nonnegative form.
   With `u = D1 + D3` and `v = D1 − D3`, the equivalent squared norm is
   `(D2 + 0.7u)² + 0.16u² + 0.35v²`. An invalid or unrepresentable
   required result fails the calculation.
   [Basel CRE52.57](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27) ·
   [Accepted ordered sums](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).
5. **Apply one factor per set and sum the charges.** Multiply `EN` by
   `0.50%` for nominal and inflation sets, `0.25%` for basis sets and
   `2.50%` for volatility sets. Sum the nonnegative charges across sets.
   For opposite 100 million effective notionals at the ordinary weight,
   the add-on is about 387,298 in adjacent buckets, 591,608 in the outer
   buckets and 1 million in separate sets, in the same reporting currency.
   [Basel CRE52.57 and 52.72–73](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
6. **Preserve complete result evidence.** Carry the contribution-to-bucket
   assignments, signed totals, set identities, weights, charges, elected
   profile and coverage through Rust, wire, Python and WASM. A covered
   scope with no eligible IR trade or fully offset buckets yields zero.
   An unresolved required classification or failed calculation makes the
   IR add-on unavailable. [Record 017](17-saccr-non-ir-addons.md#accepted-decision)
   owns non-IR add-ons and [record 018](18-saccr-rc-pfe-ead.md#accepted-decision)
   owns PFE/EAD assembly.

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

IR trades use duration-adjusted signed effective notionals, three underlying-maturity buckets and fixed adjacent/nonadjacent correlations inside each caller-labelled hedging set. Hedging sets add without cross-set offset. No currency parsing checks that the label describes an economically valid set.

## Algorithm and conventions

1. For each IR trade compute `d=supervisory_delta×abs(notional)×SD×MF`, with duration and MF from record 15.
2. Use nonnegative remaining underlying end maturity `E` in years to select bucket 1 for `<1`, bucket 2 for `1<=E<=5`, or bucket 3 for `E>5`. The bucket uses underlying end date even when an option's MF uses its expiry.
3. Within each hedging-set string, sum d to D1/D2/D3 and calculate `EN=sqrt(max(D1²+D2²+D3²+1.4D1D2+1.4D2D3+0.6D1D3,0))`.
4. Multiply each EN by fixed supervisory factor `0.005`, then sum nonnegative set add-ons. IR has no category parameter branch.

## Selection and controls

The PFE router selects only IR trades for this path. Offsetting is based solely on equal hedging-set labels and the three maturity buckets; `underlier` does not split the IR calculation further. The SD formula handles forward starts; a matured end yields zero SD even though MF itself retains its floor.

## Limitations and failure behavior

An arbitrary same hedging-set label can create offset across unlike currencies because the engine does not decode or validate labels; distinct labels remove all offset even within one currency. No automatic treatment of basis/volatility subsets or multiple-currency trade decomposition is visible here.

## Alternatives and tradeoffs

The accepted partially offset method retains three maturity buckets and
assigns hedging eligibility from qualified risk identities. Basel also
prescribes a sum of absolute bucket totals for a profile electing zero
offset across maturity buckets.

## Evidence and role

Owning runtime: [src/regulatory/sa_ccr/add_on.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/add_on.rs); selection: [src/regulatory/sa_ccr/pfe.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs). Inspected add-on tests verify duration and per-trade maturity effects; engine tests locate simple IR RC/PFE/EAD examples. These are arithmetic/contract evidence. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

Implementation and qualification are tracked in the
[regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
