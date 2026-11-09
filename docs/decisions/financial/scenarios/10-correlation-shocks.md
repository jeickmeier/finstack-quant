# SCENARIOS-010: Base-correlation and structured-credit correlation shocks

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: apply additive decimal correlation changes, with bounds enforced by clipping rather than rejection. Surface base correlations and portfolio-model correlations are separate targets with different clipping intervals and semantics. A change of `0.01` is one correlation percentage point; the scenario spec allows finite changes in [-2,2].

## Base-correlation algorithm and confirmed unit discrepancy

`BaseCorrParallelPts` passes no detachment filter; bucket operations convert each integer `detachment_bp` to `f64/10,000`, then emit a typed base-correlation bucket bump. Core selects exact detachment knots within absolute `1e-10`, adds points and clamps correlations to [0,1]. The adapter previews matching knots to warn on clipping, and warns when a **nonempty** requested filter matches nothing. Empty filters silently select nothing. A missing surface is ultimately an application error, not a price-scalar fallback.

**Confirmed static unit mismatch:** core `BaseCorrelationCurve` documents detachment coordinates as **percent of pool notional**, supports [0,100], and tranche pricing queries `attach_pct`/`detach_pct`. A 7% tranche therefore queries 7.0. The scenario adapter maps 700 basis points to 0.07 rather than 7.0. Scenario bucket tests construct knots at 0.03/0.07/0.10 and label them 3%/7%/10%, so they demonstrate internally consistent fractional fixtures while missing the production convention. A bucket request can therefore warn/no-op against conventionally scaled curves. This is a reconciliation defect, not a new optional model convention; runtime impact has not been executed here.

## Instrument correlation algorithm

Scan the supplied top-level inventory and downcast to `StructuredCredit`; ignore other instruments and structures absent from `credit_model.correlation_structure`. Unlike generic price/spread shocks, this path does not descend into composites. Delegate to the model's structure bump:

- Flat asset correlation adds delta and clamps [0,0.99].
- Sectored asset correlation adds delta to intra-sector and **half delta** to inter-sector, each clamped [0,0.99].
- Matrix asset correlation adds delta to every off-diagonal entry and clamps [-0.99,0.99], retaining diagonal/labels. The adapter performs no post-bump PSD test or projection.
- Prepay/default correlation adds delta and clamps [-0.99,0.99] for flat/sectored structures. A matrix has no such field: the model returns it unchanged with a no-op message.

Every matching instrument is assigned the returned structure and counted as changed, including zero shocks and matrix prepay/default no-ops. All returned messages are wrapped as `CorrelationClamped`, even the matrix no-op explanation. If no matching instruments exist, emit `CorrelationShockNoMatch`. Applied-shock reports retain requested points, not effective clipped changes.

## Alternatives and evidence

Owning source: [adapters/basecorr.rs](../../../../finstack-quant/scenarios/src/adapters/basecorr.rs), [engine/instrument_shocks.rs](../../../../finstack-quant/scenarios/src/engine/instrument_shocks.rs). Inspected dependencies: [core base correlation](../../../../finstack-quant/core/src/market_data/term_structures/base_correlation.rs), [model correlation structure](../../../../finstack-quant/models/src/credit/pool/correlation.rs), [tranche query](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs). [Core methodology](../core-market/base-correlation.md) states the percentage coordinate. Inspected [bucket fixture](../../../../finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs) shows the conflicting fractional convention; inspected [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs) assert flat clamping and report units. No independent tranche-price or matrix-validity benchmark was executed.

Loss-space/no-arbitrage recalibration and nearest-correlation projection are alternatives, not behavior implemented by these adapters.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires detachment-bp conversion by /100 into the core percent coordinate,
exact unique stored bucket targets and one checked selection shared by previews
and application. Native correlation changes are decimal amounts; invalid final
[0,1] parameters error atomically without clipping or no-match warning success.
Preserve full empty/identity validation, source ID and actual applied quantities.
Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's distinct portfolio-model
correlation domains, matrix/PSD policies or wider scenario reporting choices.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
