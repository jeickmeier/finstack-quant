# CORE-MARKET-05: Stored CDS quote interpolation and synthetic hazard shocks

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Core exposes an interpolated stored CDS quote or a local credit-triangle approximation; its generic hazard bump shifts hazard by spread divided by loss-given-default. It does **not** replay CDS calibration in this path. That distinction is material to CS01 and market-change measurements.

## Algorithm, selection, and conventions

`cds_quote_bp(t,method)` uses stored par quotes only when at least two aligned tenor/quote entries exist. Linear is the default metadata choice. Explicit LogLinear attempts a log-linear quote strategy; if construction fails it silently uses Linear. Both request FlatForward, which extends the linear spread slope or log-spread slope respectively, not a flat endpoint spread. With insufficient quotes the output is `max(lambda(max(t,0))*(1-R)*10000,0)`. It uses the **local segment hazard**, not a fitted CDS par spread or average hazard.

Generic `Bumpable::apply_bump` and context curve bumps select `ParSpread`: additive decimal spread shift `s` becomes `delta_lambda=s/(1-R)`. Recovery must satisfy the stricter loss-given-default validation before division. Explicit direct-hazard APIs instead add basis points/10,000 to hazard. Parallel or triangular key-rate weights are sampled at the stored hazard ending knots, so bucket shapes are piecewise constant in hazard after rebuilding, unlike continuous discount/forward shocks.

Every resulting hazard must be finite, nonnegative, and within its configured cap. The mutation builds new survival before committing, so failure leaves source values intact. A successful shock clears stored par quotes and calibration recipe, even a zero-size generic shock. `with_recovery_rate` changes recovery and clears the recipe while preserving hazards and stored par quotes.

Direct tenor hazard bumps select a knot within 1e-6 years of requested tenor, otherwise the next ending knot (containing segment). Requests beyond last knot plus 1e-6 are ignored. Repeated targets accumulate sequentially and each intermediate negative result is rejected. For a single-point hazard curve all requested finite bumps are summed into one parallel shock regardless of target tenor.

## Domain, failures, and alternatives

Par-tenor ingestion sorts but only checks finite nonnegative tenors and finite spreads. It does not reject duplicate quote tenors or negative quote spreads; log-linear failure can therefore change the requested method silently. Stored quotes are not verified against hazards/recovery. A recovery override can consequently retain par quotes inconsistent with the changed recovery. These behaviors require explicit policy decisions.

Quote bump and full recalibration, supported by downstream recipe consumers, is a different alternative with more faithful instrument repricing but additional dependencies and solver costs. Direct hazard bump is explicitly supported and measures another risk. The credit-triangle approximation requires a stated intended range; code/tests alone do not establish its accuracy for term-structured or distressed names.

## Evidence

Owning code: [quote evaluation](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [quote ingestion](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [shock/recovery/tenor dispatch](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), [context dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) verify lambda shift equals `spread/(1-R)`, fallback quote changes from 60 to 70 bp, stale quote removal, direct-hazard additivity, segment targeting, atomic failed bumps and partition of knot shocks. Those validate the approximation's implementation, not CDS-repricing accuracy. [O'Kane](../../../REFERENCES.md#o-kane-2008) and [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model) are comparison references.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
