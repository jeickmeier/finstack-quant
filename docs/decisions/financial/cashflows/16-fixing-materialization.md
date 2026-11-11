# CASHFLOWS-016 — Projected fixing metadata and realized-forward rolls

- Decision status: **Accepted — complete qualified hypothetical fixing-materialization package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to `fixings.rs`, producer metadata and actual roll/valuation/host consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded date/binary64 examples and read-only financial/source/consumer reviews are reported below; they do not qualify publication data, a complete scenario or product pricing.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The observed implementation selects projected schedule metadata using an inclusive observation-date window and inserts values into a cloned market's FIXING: series. It uses exact presence as precedence, nullable projections, default series policies and a tolerance-sized last-winner conflict rule. The accepted replacement below selects qualified hypothetical observations explicitly, with exact conflicts and persistent model provenance. It is a scenario policy, not a claim that a forecast equals a published fixing. No runtime changes are authorized here.

## Accepted decision: qualified hypothetical fixing materialization

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[market12 observation identity, datasets and exact lookup](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[006 qualified raw term observations](06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures),
[007 actual overnight observations](07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
and [012 complete immutable schedule/source metadata](12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views).
Publication and projection-model qualification stay with actual owners; no generic
publication calendar, vintage database, index registry or scenario engine is added.

1. **Retain one explicitly hypothetical frozen-forward operation.** Select
   realization of owner-qualified raw pre-roll model observations as a named
   scenario assumption. It does not fetch publications, predict a realized path,
   calibrate a model or generate stochastic fixings. Actual corrected/published
   data remains a separately selected source. Advancing valuation dates or
   inserting a FIXING: key cannot convert model values into publication facts.

2. **Keep only successful genuine model projections.** Retain ProjectedFixing
   as a checked immutable record with required finite binary64 raw value;
   remove Option/None, fallback placeholders and Some-over-None repair. Observed
   values are not re-emitted as projections. Stage producer metadata until the
   whole actual observation/coupon operation succeeds, following006/007. Keep
   rates before spread/gearing/bounds, overnight raw index quotes and actual
   reference-swap CMS forwards before coupon convexity/constraints. Retain raw
   FX quotes directly, never reconstruct them from cash or notional ratios.

3. **Bind exact observation identity and financial meaning at the owner.** Use
   the configured observation IndexId, date and market12's single series-key
   helper; replace the caller-supplied authoritative series_id string. Verify
   resolved rate value interval/basis, CMS benchmark/tenor/run, currency and FX
   base/quote direction at their existing owners. A CurveId, prefix, date or
   unchecked IndexId wrapper is insufficient. Rates/CMS can be finite signed
   quotes; FX requires the selected owner's positive-rate domain. No global
   positivity rule, inferred CMS key or generic new family registry is added.

4. **Freeze the actual source immediately before the owning roll.** Prepare
   raw values from the SAME selected immutable pre-roll market, projection
   associations, information phase and source/version authority used by that
   operation. Preserve006/007's original observation and parent references;
   do not reconstruct or silently reproject a missing value after rolling.
   Separate later roll calls use their own pre-roll source and fresh required
   projections. A numeric match alone cannot validate stale provenance.

5. **Replace the observation-date window with an explicit selected set.** The
   roll/valuation owner determines exactly which observation keys it needs to
   treat as known under its selected old/new information phases and hypothetical
   policy. Remove old_date/new_date filtering and universal inclusive/exclusive
   endpoint decisions from materialize_fixings. A required value date may be
   before old_date yet only become available later; an observation on new_date
   may still be unpublished. Equal dates can advance knowledge. Economic roll
   date/range rules remain at the roll owner, without a new timestamp engine.

6. **Require complete owner dependency preparation.** For every required key,
   supply either a qualified retained exact observation or a genuine qualified
   pre-roll projection; otherwise error. Do not fill earlier gaps, interpolate
   exact fixings, use rolled curves or invent zero/None substitutes. Collect
   fixing dependencies independently of full payoff/cashflow decomposition:
   an explicitly known empty dependency set is valid, whereas unsupported or
   failed dependency discovery is not an empty schedule. This preserves012's
   removal of successful Placeholder schedules.

7. **Return and retain the market with its model-observation provenance.**
   Replace the schedule/date-window API by three explicit inputs: the qualified
   frozen source, required observation keys and candidate projections. Keep
   required keys separate from successful projections: a key may already have
   qualified retained data, or may be missing entirely. Reject unrequested
   candidates; canonicalize duplicate consistent required keys without losing
   their actual owner association. Use one small immutable paired source/result
   holding the market plus its retained model-observation records; return the
   complete retained records, including earlier realizations and new additions,
   in canonical order. Reuse actual006/007 owner observation/source references
   and retain their origin/information/scenario authority. An initially genuine
   source has no model records only after owner qualification, not by a default
   assertion. A bare MarketContext clone or additions-only log loses the role
   of earlier model values and is not this result. No new projection framework
   or historical-data registry is selected.

8. **Make that provenance operational at every boundary.** Actual roll/metric/
   horizon owners retain the paired state, expose it through existing reports/
   application envelopes and explicitly pass its model-key qualification to
   006/007/CMS/FX consumers. Model-realized exact values remain hypothetical
   through subsequent rolls, serialization and reload. Missing, inconsistent or
   discarded origin/role is unknown/error, never implicit Published status.
   Validate private construction/restoration against the market values and
   retained observation identities; reject duplicate/conflicting records,
   unknown fields and contradictory source/scenario lineage. Shape validation
   alone does not prove external provenance.

9. **Validate the complete supplied batch before precedence or identity.**
   Admit every required key and candidate IndexId/date/value, resolved association and source qualification
   before lookup, duplicate elimination, empty/no-change results or existing-
   value shortcuts. Reject NaN/infinity in constructors, wire restoration and
   serialization as well as use; null is not a missing model value. Normalize
   numeric -0 to+0. Raw observations remain binary64 model quantities, with no
   Money conversion, tolerance rounding or decimal currency quantization.

10. **Use exact deterministic projection conflicts.** Canonicalize by resolved
    observation identity and date. Collapse repeated identical qualified records
    only when their source authority agrees and finite numeric values are equal
    after zero normalization. Reject every distinct value, including adjacent
    binary64 values, and contradictory provenance even if values coincide.
    Remove1e-12 tolerance, chained candidate replacement, last-winner selection
    and averaging. Make schedule/batch order and exact duplicate multiplicity
    irrelevant to the admitted result.

11. **Give only qualified retained exact values precedence.** The owner must
    resolve the selected dataset/version, identity, units/currency and actual or
    hypothetical eligibility for this operation before lookup. A qualified
    existing value can legitimately differ from a forecast; preserve it without
    overwrite. Internally conflicting/invalid supplied projections still error
    under points9–10. An ineligible stored future/revised value or contradictory
    scenario history cannot win merely by exact presence; select the correct
    source or error. No actual-history revision is silently performed.

12. **Require explicitly configured destination series.** Every targeted fixing
    series must already be bound by its owner with explicit market12 sampling/
    right-tail policies and correct identity/currency/source role. Empty history
    is valid. Preserve these policies and metadata when adding samples; do not
    create a missing series with default Step, tail, units or currency. Missing
    configuration errors until the owner supplies it through the existing
    series/context path. Exact fixing lookup never uses interpolation or tails.

13. **Preserve genuine lookup and qualification failures.** Use canonical
    get_value_exact with the expected series identity, and distinguish an absent
    exact date from wrong identity, source outage, invalid state or failed
    publication/scenario qualification. Remove `.ok()`-based missing conversion.
    Only a qualified missing exact date may be filled by a selected projection;
    do not repair a bad configured source by replacing it.

14. **Stage one strict atomic merge.** Build every targeted series with ordered
    unique dates and finite samples, then the complete paired result, before
    publishing any output. Preserve all qualified existing observations, series
    policies and unrelated market state. Validate the complete retained model
    records against the result. Failures leave input market, actual datasets and
    source provenance unchanged; no partially inserted series or orphaned model
    role is published. A new model insertion produces a distinct hypothetical
    dataset/version under the actual owner, not a publication revision.

15. **Define empty, repeated and partitioned calls narrowly.** After complete
    admission, an explicitly empty required set with no candidates, or a set
    fully satisfied by qualified existing data, can return unchanged state.
    An empty candidate list with an unsatisfied required key errors; do not
    invent a new realization. Dates being
    equal is not an independent shortcut when an explicit batch was selected.
    Reapplying the same batch to a compatible extension of the SAME frozen
    authority is idempotent. Within-batch order invariance applies to its full
    admitted union. Partitioned calls agree only for a globally consistent
    projection union under that fixed source and precedence; conflicting unions
    have no equivalent success/failure guarantee across partitions. No guarantee
    extends across fresh projections, different revisions or successive
    changed-market rolls. Do not add a log of ignored forecasts to promise more.

16. **Preserve earlier hypotheses across successive operations.** Keep prior
    model records and their original source/scenario roles when a later roll
    retains exact values. They do not become actual because the next market
    contains them. Owners distinguish retained realized scenario history from
    fresh forecasts and actual later publications; conflicting lineage or stale
    required forecasts error rather than being silently merged or relabelled.

17. **Reconcile the actual dependency producers and consumers.** Align builder,
    overnight, CMS, XCCY/revolver, bond-future/theta/metric and scenario/horizon
    paths with the same qualified observation contract. Include required initial
    FX resets as well as later resets, preserve raw FX at zero notional and
    remove structured-credit's date-only first-match observation lookup or
    missing-data-to-projected-cash fallbacks. Share source preparation at the
    actual roll boundary rather than infer it independently in horizon code.
    The observed engine phases its sole time roll before shocks and rejects
    multiple rolls in one spec; no bump-before-roll bug or new general operation
    order is established here. Broader scenario choices retain their owners.

18. **Limit the atomicity and financial invariance claim.** Point14 covers
    materialization output and its provenance. Curve/hazard rolling, carry
    pricing, later shocks, external sources and report/application commit rules
    retain their actual transaction owners. Do not claim full-scenario rollback
    from a cloned market or that successful materialization guarantees unchanged
    product value, correct theta/carry or partition-invariant pricing. Pricing
    qualification and missing-dependency errors remain visible.

19. **Update existing exact Rust/host/state contracts together.** Reconcile
    ProjectedFixing construction/getters, schedule metadata, result/source shape,
    materialize_fixings, Python/WASM bindings, stubs/declarations, schemas,
    parity and actual examples/benchmarks. Required values and policies have no
    defaults. Remove series_id/nullable/old-new-window aliases and mutation
    bypasses; hosts only convert types and map canonical errors. Use the small
    paired value through existing surfaces, without a separate host state
    machine, copied financial logic or compatibility path.

20. **Separate primary evidence from this house policy.** Verified NY Fed SOFR
    publication/revision and ECB reference-FX publication procedures demonstrate
    distinct observation, publication and revision phases and source-specific
    quotation roles. They do not prescribe frozen-forward realization, an
    observation-date window or a conflict tolerance. Independently qualify raw
    rate/CMS/FX identity, phase/key selection, exact conflicts, retained source
    roles, atomic failure, strict state/hosts, actual consumers and complete
    product roll behavior. This acceptance changes documentation only.

## Independent bounded reference checks

Independent Python binary64/date/set calculations on2026-11-09, without importing
production code, checked these definitions and observed-expression counterexamples:

- Values0,0.75e-12,1.5e-12 for one key pass current-style consecutive tolerance
  replacement in ascending order and select1.5e-12; another order fails because
  a nonadjacent pair differs by more than1e-12. The exact selected rule rejects
  all orders. Adjacent binary64 values around0.05 also conflict; -0/+0 normalize
  to one value. Exact duplicate order/multiplicity does not alter a valid set.
- An illustrative source has value date Monday, publication Tuesday08:00 and
  selected old cutoff Tuesday07:00/new cutoff Tuesday09:00. Its newly needed key
  predates old_date; a date-window filter misses it. A Tuesday value published
  Wednesday is not newly available Tuesday09:00. Both knowledge cutoffs can share
  one Date, so equal dates do not establish an empty selected set. These are
  owner-phase examples, not an embedded universal publication calendar.
- A qualified existing0.051 and forecast0.05 preserve0.051; two different
  forecasts for that key still conflict before precedence. Two distinct indices
  sharing one date stay separate keys. Unavailable source/identity is distinct
  from an absent exact sample.
- At zero constant notional, reconstructing FX as constant/reset notional yields
  0/0 although a directly projected positive quote remains valid. With nonzero
  values, divide-then-divide can change the original binary64 quote; retain the
  actual projection. Keeping model-role records across a second operation is
  necessary even when all existing numeric samples remain unchanged.

These checks support the selected semantics and removal of shortcuts; they do
not execute production materialization, certify live publication schedules or
prove complete scenario/product valuation invariance.

## Observed algorithm and controls

1. Store `ProjectedFixing {series_id,date,value}` per needed observation. Rate values are decimal index quotes **before** spread/gearing/caps/floors; FX consumers may use the same structure for quote-currency-per-base-currency rates, with orientation carried by the series identifier. `None` marks an observation whose missing dependency was masked by coupon fallback.
2. Normalize by series ID, date and optional value; sort None before finite values. Collapse duplicate identical values and let a Some value replace None. Distinct conflicting Some values remain present for later conflict checking rather than being averaged.
3. `materialize_fixings` requires `new_date>=old_date`. Equal dates return an unchanged market clone and perform no materialization. An advancing window includes observations at **both** old_date and new_date; earlier dates are not invented.
4. Require nonempty `FIXING:`-prefixed series IDs for crossed observations. If the input market already has an exact value on that date, retain it and skip projected-value validation for that observation.
5. Group missing observations across all schedules. A finite projected value must exist; None-only observations error. Multiple projections must agree within `1e-12×max(abs(values),1)`. Larger differences or nonfinite values error. Values within tolerance replace the accumulated candidate in iteration order; there is no averaging rule.
6. Merge additions into existing observations without overwriting existing dates; retain existing currency and interpolation metadata, or default interpolation for a new series. Build all replacement series first, then return a market clone containing them. Input market is immutable; failures do not partially mutate it.

## Observed selection and limitations

The method is explicitly called by roll/scenario consumers with schedules projected on the pre-roll market at the old date. It does not rebuild schedules or verify that their metadata truly came from that market/date. Raw fixing generation follows [CASHFLOWS-006](06-floating-rate-projection.md) and [CASHFLOWS-007](07-overnight-compounding.md). SpreadOnly/fixed fallback is not silently promoted to a realized fixing when value is None. An existing exact fixing can satisfy such an observation.

Only observations needed by supplied schedules are materialized. No missing historical gap before old_date is interpolated. SeriesInterpolation is preserved but exact-date lookup still governs precedence. Near-equal conflicting values can produce a tolerance-sized order effect; exact conflicting values are deliberately retained during normalization.

## Alternatives and evidence

Authoritative observations selected under their actual source knowledge remain an alternative to hypothetical model realization. Holding fixings unchanged while rolling curves, stochastic scenarios and unrelated forecasts are different owner policies, not extra modes of this operation. The accepted package removes observation-date selection, nullable fallback repair, tolerance conflicts and unlabelled history insertion rather than importing those choices from the implementation.

- [fixings.rs](../../../../finstack-quant/cashflows/src/fixings.rs), [producer metadata](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [final metadata normalization](../../../../finstack-quant/cashflows/src/builder/pipeline.rs), [merge normalization](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Located invariant/regression evidence in [production_fixing_roll.rs](../../../../finstack-quant/cashflows/tests/production_fixing_roll.rs): preservation of existing values/metadata, conflict and missing-projection failures, inclusion of both advancing endpoints and no-op equal dates. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locates one raw fixing under weekend balance splits.
- Actual consumers include [time-roll adapter](../../../../finstack-quant/scenarios/src/adapters/time_roll.rs), [horizon preparation](../../../../finstack-quant/scenarios/src/horizon.rs), [engine phase0](../../../../finstack-quant/scenarios/src/engine/mod.rs), [scenario admission](../../../../finstack-quant/scenarios/src/spec.rs), [XCCY raw FX metadata](../../../../finstack-quant/valuations/rates/src/xccy_swap/pricing_mtm.rs) and [structured-credit observation handoff](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs). Source inspection distinguishes actual phase0 behavior from an unproven spec-order discrepancy.
- [Verified publication/quotation examples](../../../REFERENCES.md#fixing-publication-timing) were checked on2026-11-09. Their clock/revision details are source-specific evidence, not adopted generic fixings rules. Read-only independent financial, exact-conflict, date/provenance and consumer reviews challenged the full proposal; no production code was executed.
- No external standard mandates realized-forward scenario rolls or the conflict tolerance. No tests executed.

## Relationship to the accepted fixing dataset boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) binds exact rate/CMS
observation identities and separates published fixing qualification from date
matching. Materialized pre-roll model values require explicitly hypothetical
dataset provenance; namespace insertion or a later valuation date cannot confer
actual publication authority. Reconcile strict series construction/policies
without retaining default Step or ordinary generic bumps of fixing history.
The old conflict/window/realization policy remains observed; the complete
replacement above was accepted separately as CASHFLOWS-016 on2026-11-09;
record12 does not independently adopt its realization policy. Record12 was accepted on **2026-11-08**, with
reconciliation and independent qualification pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
