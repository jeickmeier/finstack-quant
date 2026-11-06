# CORE-MARKET-20: Market context selection, ordered shocks, curve rolls, and snapshot rebinding

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`MarketContext` manages typed market data with copy-on-write storage, canonical curve IDs and composite rebinding. Its dispatch and persistence rules determine which data a price/risk calculation actually sees. These are material calculation controls, not only storage plumbing.

## Algorithm and actual selection

Typed getters require the exact curve variant; Price versus VolIndex is additionally checked by kind. Missing or wrong-type curves error rather than substituting a nearby identifier. Generic `MarketBump::Curve` lookup instead uses ordered namespaces: curves, then volatility surfaces, then scalars, then scalar series. The same ID may exist across namespaces, so a collision selects the first. Inflation indices, FX-delta surfaces, cubes and dividends are not targeted by this generic dispatch.

Bump batches clone the context and apply changes in supplied order, so repeated targets compose. A failure discards the returned clone and leaves the original context unchanged. Discount/forward/hazard mutations retain IDs; rebuilt price/inflation/correlation curves are renamed back to their original IDs. Hazard generic shocks select the spread approximation. Basis-spread and parametric curve generic bumps error. Credit references are rebound once after batch shocks when relevant.

FX percentage batch shocks pass `pct/100` to `FxMatrix::with_bumped_rate(base,quote,relative_change,as_of)`. `convert_money` short-circuits same-currency conversion and otherwise queries the FX matrix with default query policy at the supplied date, applies the returned rate through checked Money conversion, and returns quote provenance. The FX resolution/triangulation kernel belongs to the money/FX audit; this context does not implement it. Scratch price `bump_pct` is a **decimal relative change** (0.01 for 1%), unlike batch FX/vol bucket percentage inputs (1 for 1%). Scratch operations return old state; reversion restores curve and affected credit-index snapshots or the selected surface node.

Context roll calls each stored curve's roll implementation. Base correlation is unchanged; discount, forward, hazard, CPI, price, basis and parametric curves use their own clocks. FX, surfaces/cubes and their expiry axes, scalars, observations, dividends and hierarchy are cloned unchanged. Roll does not age option-volatility expiry axes or create new realized fixings. A failure rolling any curve errors for the whole returned context. Credit indices are rebound afterward.

Credit-index insertion/updates resolve hazard/correlation references against canonical context entries. Rebinding failures remove the affected composite, emit a warning, and return invalidated IDs internally; ordinary insertion and roll discard that returned list, whereas `retain_curves_mut` exposes a sorted list. Replacing a curve with an incompatible variant can therefore remove a composite without making ordinary insertion fail.

## Snapshot semantics and limits

Snapshot serialization sorts IDs deterministically, serializes each validated curve representation, and stores credit-index references by IDs. Restore rejects duplicate IDs **within each namespace** and missing/wrong-type credit references before constructing the context; composite builders recheck count/recovery/weights. FX restore creates a quote-only snapshot from explicit serializable quotes; it does not recreate a live rate provider. The versioned byte loader performs contract decoding; direct typed state conversion performs semantic restore checks. Cross-namespace IDs are not globally unique.

`clear_market_scalars_mut` removes prices, series, inflation indices and dividends together. Surface/cube replacements directly replace their maps; references to missing ordinary data are not globally audited here. Hierarchy IDs are not checked for existence in market data on assignment/restoration.

## Alternatives, evidence

A globally unique typed market key, explicit target namespace, fail-fast composite updates, or full market-date roll are possible alternatives.

Owning code: [storage dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs), [getters](../../../../finstack-quant/core/src/market_data/context/getters.rs), [insertion](../../../../finstack-quant/core/src/market_data/context/insert.rs), [rebinding/conversion](../../../../finstack-quant/core/src/market_data/context/mod.rs), [ordered/scratch bumps](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs), [roll](../../../../finstack-quant/core/src/market_data/context/ops_roll.rs), [snapshot](../../../../finstack-quant/core/src/market_data/context/state_serde.rs), [retention/clearing](../../../../finstack-quant/core/src/market_data/context/stats.rs). Inspected `ordered_bumps_compose_repeated_scalar_targets` checks 100*1.1*1.2=132 and source preservation; [context](../../../../finstack-quant/core/tests/market_data/context.rs), [serde](../../../../finstack-quant/core/tests/market_data/serde.rs), and [lifecycle](../../../../finstack-quant/core/tests/market_data/lifecycle.rs) tests locate composite/roll/snapshot invariants. No external price or risk accuracy is claimed. The policy reference is [missing and invalid data](../conventions/0009-missing-and-invalid-data.md); these persistence choices have no standalone financial standard citation.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
