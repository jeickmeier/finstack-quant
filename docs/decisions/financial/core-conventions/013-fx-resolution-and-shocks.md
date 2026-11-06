# CORE-CONVENTIONS-013: FX source authority, single-pivot conversion, state and relative shocks

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to all six `money/fx/` implementation files.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

FX resolution is an explicit hierarchy over pair-global overrides, date/policy-specific overrides and a provider. Optional triangulation uses one configured pivot, USD by default. Revision-aware caching must preserve these semantics. Rate selection is separate from Money conversion/rounding (012). The code documents deterministic source precedence; economic adoption of global overrides and average-rate inversion remains pending.

## Algorithm: rate resolution

Identity queries return 1. For other pairs, try up to three complete resolutions while checking matrix/provider revisions before and after. Within each resolution: pair-global direct or reciprocal first; matrix pinned `(pair,date,policy)` direct or reciprocal second; a current observed LRU cache entry third; provider fourth. Direct orientation wins over inverse **within** each authority layer, while a higher-authority inverse wins over a lower-authority direct. All served rates must be positive finite and inverted rates must have representable reciprocals.

If provider returns `InvalidFxRate`, propagate it. Other provider errors trigger triangulation when enabled, even if not specifically a missing-quote error. Resolve from->pivot and pivot->to by the same authoritative layers/provider, without recursively searching other pivots, multiply, and validate the product. A failed leg is surfaced as `FxTriangulationFailed` naming the leg, potentially losing its original error type. Identity pivot legs are 1. Triangulated provenance is retained through cached repeat queries.

Observed quotes use a canonical currency ordering distinct from market-quotation rank, with reciprocal storage and reciprocal serving as needed; this preserves cold/warm bit consistency. LRU capacity defaults 256 and must be positive. Cache keys include date/policy; values include matrix/provider revision. Providers with no revision do not have observations cached. Explicit global/pinned quotes are never evicted by LRU pressure; updates invalidate observations.

`SimpleFxProvider` itself serves identity 1, scoped pinned direct/inverse, then global direct/inverse. Global quotes ignore query date and policy. `FxConversionPolicy` defaults CashflowDate; PeriodEnd and PeriodAverage are carried as keys to providers. There is **no averaging or period-end-date calculation** in this layer: PeriodAverage reciprocates/triangulates supplied rates like other policies.

## Algorithm: persistence and risk shocks

Snapshots preserve config, matrix globals/pinned quotes and provider global/pinned quote snapshots, with deterministic ordering; they omit transient observed cache entries. Capture retries up to three revision-stable attempts and rejects nonempty provider snapshots without a revision. Reconstruction builds a SimpleFxProvider and restores authority layers. `load_from_state` merges matrix global/pinned values into an existing object; it does not replace provider/config, and a later invalid pinned quote can leave earlier writes applied. It is distinct from fresh `try_from_state`.

`with_bumped_rate(from,to,b,on)` validates the effective rate at `on`/CashflowDate and requires a nonidentity pair and positive finite bumped rates. Direct target rates multiply by `1+b`; reverse rates divide by it; unrelated pairs remain unchanged. A wrapper resolves a derived target cross before shocking, preserves the original provider's date/policy structure, and normalizes snapshots of the shocked orientation. Static global/pinned scopes are checked for shock overflow, fresh observations are built, and future live provider changes can still affect rates. The bump is fractional relative change, not percentage points or basis points.

## Limitations, alternatives and controls

Caller may disable triangulation/change pivot/cache or explicitly pin/global-override quotes. No bid/ask spread, arbitrage reconciliation, multi-hop graph search or stochastic FX projection is done here. Conflicting direct/inverse authoritative inputs can remain direction-specific; direct wins. Average of reciprocals generally differs from reciprocal of average, and average cross rates generally differ from product of averages; the code chooses the latter operations on supplied aggregates. Revisionless mutable providers cannot provide the same consistency guarantee as versioned snapshots.

## Evidence

[Matrix](../../../../finstack-quant/core/src/money/fx/matrix.rs), [provider contracts](../../../../finstack-quant/core/src/money/fx/provider.rs), [simple/bumped providers](../../../../finstack-quant/core/src/money/fx/providers.rs), [policy/config](../../../../finstack-quant/core/src/money/fx/types.rs). Inspected inline test `period_average_quotes_invert_and_triangulate_like_other_policies` explicitly encodes aggregate inversion/crossing. [FX integration tests](../../../../finstack-quant/core/tests/money/money_fx.rs) locate source precedence, scope retention, revision races, shock snapshots, cache pressure and overflow cases; these are deterministic invariants, not independently observed FX-market prices. [Clark](../../../REFERENCES.md#clark-fx-options) is general financial background.

No independent accounting-rate or market-data provenance certification was found.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
