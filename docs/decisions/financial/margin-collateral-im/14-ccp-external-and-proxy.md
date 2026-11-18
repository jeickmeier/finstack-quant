# MARGIN-COLLATERAL-IM-014 — Reported CCP initial margin and indicative exposure proxy

**Decision status:** Accepted 2026-11-17 for elected cleared IM routes and qualified scope.

**Implementation status:** Existing behavior is described below. Route selection, source and profile admission, horizon and coverage checks, calculation and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary CCP disclosures and caller paths inspected. Independent Decimal arithmetic checked gross sizing of offsetting positions under a selected proxy rate. Runtime, portfolio, provider and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for elected cleared initial margin routes

1. Elect one route for the exact clearing entity, service, member or client account, covered position set, valuation cut and currency. Choose a qualified reported amount or a named indicative proxy. Retire substring routing, ambiguous aliases and automatic switching between routes. [Parameter and agreement election](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) · [Portfolio scope](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
2. Admit a single checked source record of the reported IM requirement. Require nonnegative Money, exact account and position coverage, source authority, effective time, availability at the information cut, currency and horizon basis. The reported requirement covers applicable IM add-ons. Keep VM, collateral balances and transfers as separate amounts. Accept an explicit zero. Return typed errors for missing, stale, invalid or mismatched data. Consume the account amount once with its original sizing and offsets. [Clearing model and add-ons](https://www.lseg.com/en/post-trade/clearing/lch-services/swapclear/risk-management).
3. Retire the bundled venue-labelled rates and generic 5% default from calculation. An expressly elected local proxy requires a complete immutable profile with source, effective dates, eligible exposures, currency, reference horizon and an exact nonnegative Decimal multiplier. Admit custom and restored profiles through the same validation path. Label the result indicative and retire the conservative claim. [LCH risk methodology](https://www.lseg.com/en/post-trade/clearing/lch-services/swapclear/risk-management) · [CME SPAN 2 methodology](https://www.cmegroup.com/clearing/files/cme-span-2-margin-framework.pdf).
4. Require a qualified nonnegative exposure base for each eligible proxy position. Apply absolute holding size once, multiply by the elected rate using checked Money arithmetic and sum gross charges in deterministic order without offsets. Missing required exposure makes the affected account result unavailable. [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
5. Preserve the source or profile horizon basis with its day unit and applicable calendar. A different required horizon needs a separately qualified amount or profile. Remove the provider's separate horizon override and portfolio square-root rescaling. [Published service and account horizons](https://www.lseg.com/en/post-trade/clearing/risk-management/ltd-risk-management).
6. Return one checked result carrying native Money, source or profile, exact scope, cut, horizon, coverage and qualification. Distinguish reported requirements, indicative estimates, computed zero and unavailable amounts. Preserve the result through portfolio and its existing Python and WASM surfaces. Keep direct CCP providers and calculators Rust-only. Record 017 owns contractual collateral and transfers. Record 022 owns the wire format. [Contractual IM](17-csa-selection-and-im-terms.md) · [Result transport](22-wire-tables-and-provenance.md).

An independent Decimal example with opposing two-lot positions, USD 1 million of qualified exposure per lot and an elected 3% rate gives USD 120,000 of indicative initial margin. A supplied account requirement enters the calculation once for the covered position set.

## Choice and rationale

The observed calculator tries a supplied amount and otherwise uses an exposure-rate proxy. The accepted contract elects one route and binds the amount and horizon to the same clearing scope.

## Input domain and conventions

External source returns `Option<Money>` and optional MPOR; it has no error-return channel. Fallback requires an explicit `im_exposure_base`, not signed current MTM. CCP names select a methodology by trimmed lowercase substring matching.

## Ordered algorithm

1. Map names in order: LCH (credit/CDS→CDSClear, otherwise SwapClear), ICE (credit/CDS→ClearCredit, otherwise ClearUS), CME, JSCC, Eurex; unmatched names→GenericProxy.
2. Resolve direct parameter override or embedded named params, falling back to the flagged registry default if the key is absent.
3. When an external source exists, apply its MPOR override before asking for its amount. `Some(amount)` wins; `None` falls back. Thus source MPOR can still label a proxy result when no external amount is supplied.
4. Proxy IM=`abs(exposure_base)*conservative_rate`. Embedded rates: LCH SwapClear 2%, LCH CDSClear 8%, CME 3%, ICE Credit 10%, ICE US 5%, JSCC 3%, Eurex 3%, generic 5%; all MPOR 5.
5. Return a single methodology-label breakdown; set approximation=true only for the proxy and log its status. External data returns approximation=false, which means supplied rather than independently certified.

## Selection, controls and limitations

No time scaling is performed here. External nonnegative amount, expected currency and positive MPOR are not checked by this calculator. Direct `CcpParams` overrides also bypass registry scalar validation. `calculate_conservative` returns only Money and cannot itself carry approximation metadata. The local data README explicitly describes these rates as heuristics with no published CCP source.

The name `ICE` selects ClearUS, while registry alias `ice` identifies ClearCredit. The heuristic rates have no established conservative bound or CCP-reference accuracy.

## Alternatives and tradeoffs

The accepted source route preserves the qualified account requirement and its offsets. An elected local proxy gives a reproducible gross estimate from a documented exposure base.

## Source and validation evidence

Inspected tests distinguish external amount/proxy flags, named rates, explicit exposure requirements and absence of MTM substitution. They do not establish any rate as conservative for a portfolio or validate the external source's quality.

Implementation and data: [src/calculators/im/clearing.rs](../../../../finstack-quant/margin/src/calculators/im/clearing.rs), [src/calculators/im/mod.rs](../../../../finstack-quant/margin/src/calculators/im/mod.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [data/margin/ccp_methodologies.v1.json](../../../../finstack-quant/margin/data/margin/ccp_methodologies.v1.json), [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

Existing evidence: [src/calculators/im/clearing.rs](../../../../finstack-quant/margin/src/calculators/im/clearing.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
