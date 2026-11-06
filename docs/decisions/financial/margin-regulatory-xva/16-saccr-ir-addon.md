# MARGIN-REGULATORY-XVA-016: SA-CCR interest-rate maturity buckets and hedging-set add-ons

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

IR trades use duration-adjusted signed effective notionals, three underlying-maturity buckets and fixed adjacent/nonadjacent correlations inside each caller-labelled hedging set. Hedging sets add without cross-set offset. No currency parsing checks that the label describes an economically valid set.

## Algorithm and conventions

1. For each IR trade compute `d=supervisory_delta×abs(notional)×SD×MF`, with duration and MF from record 15.
2. Use nonnegative remaining underlying end maturity in years to select bucket 1 for `<1`, bucket 2 for `1<=M<=5`, or bucket 3 for `M>5`. The bucket uses underlying end date even when an option's MF uses its expiry.
3. Within each hedging-set string, sum d to D1/D2/D3 and calculate `EN=sqrt(max(D1²+D2²+D3²+1.4D1D2+1.4D2D3+0.6D1D3,0))`.
4. Multiply each EN by fixed supervisory factor `0.005`, then sum nonnegative set add-ons. IR has no category parameter branch.

## Selection and controls

The PFE router selects only IR trades for this path. Offsetting is based solely on equal hedging-set labels and the three maturity buckets; `underlier` does not split the IR calculation further. The SD formula handles forward starts; a matured end yields zero SD even though MF itself retains its floor.

## Limitations and failure behavior

An arbitrary same hedging-set label can create offset across unlike currencies because the engine does not decode or validate labels; distinct labels remove all offset even within one currency. No automatic treatment of basis/volatility subsets or multiple-currency trade decomposition is visible here.

## Alternatives and tradeoffs

Explicit typed currency and basis hedging-set keys could prevent accidental offsets, at API cost. A flat absolute notional sum would discard the observed maturity diversification; full covariance modelling is a different methodology from this fixed three-bucket formula.

## Evidence and role

Owning runtime: [src/regulatory/sa_ccr/add_on.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/add_on.rs); selection: [src/regulatory/sa_ccr/pfe.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/pfe.rs). Inspected add-on tests verify duration and per-trade maturity effects; engine tests locate simple IR RC/PFE/EAD examples. These are arithmetic/contract evidence. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
