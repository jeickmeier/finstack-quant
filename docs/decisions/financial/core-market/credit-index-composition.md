# CORE-MARKET-19: Explicit credit-pool composition and authoritative constituent inputs

- Decision status: **Accepted — complete credit-pool composition/source package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted composition, lookup, weight/recovery and consumer/state reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent source/composition, financial-consumer and context/host reviews inspected; standalone rational/high-precision/binary64 examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify portfolio loss, calibration, pricing, risk or arbitrage freedom.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns `CreditIndexData`'s pool composition, member identity, effective
future cash-recovery/weight inputs and canonical reference/state contracts.
It preserves accepted [hazard probabilities and clocks](hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
[recovery/quote/risk role separation](hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
[lossless construction history](calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification)
and [base-correlation financial association](base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks).
A complete input bundle does not compute or qualify a pool loss distribution.

[Loss-family selection](../models-and-valuations-credit/0031-tranche-loss-model-selection.md),
[tranche loss](../models-and-valuations-credit/17-tranche-loss-distribution.md),
[projection and realized state](../models-and-valuations-credit/18-tranche-projection.md),
[credit risk](../models-and-valuations-credit/19-tranche-risk.md),
[base-correlation calibration](../calibration-targets/15-base-correlation.md)
and [context lifecycle](context-lifecycle.md) retain their remaining Draft
methods. This source contract does not adopt their complete copula, projection,
realized-event approximation, recovery model, fit, risk or invalidation policies.

## Foundational findings

Current state uses a separately stored count and three optional issuer maps.
Complete curve coverage does not make lookup strict: unknown IDs receive an
index hazard, common recovery and equal weight. Iterator collection silently
keeps the last duplicate issuer, and missing recovery/weight maps select implicit
lookup defaults. Source identity and effective cash inputs should be explicit.

Weights lack individual upper bounds and use naive summed mass within 1e-9.
A complete named roster is not an equal-exposure homogeneous pool. Shared hazard
sources do not merge distinct issuer default events, and zero weights do not
create extra equally weighted exposures.

Recovery has distinct roles: hazard construction metadata, future pool cash
recovery and observed historical default/recovery state. Current realized-state
logic uses current index recovery to infer historical defaulted/recovered
notional. Index hazard also remains an actual timing dependency in named-pool
pricing, even though issuer hazards supply its expected losses. Neither role
can be inferred from map presence or the word index.

## Accepted decision: explicit credit-pool composition and exact issuer contracts

Accepted on **2026-11-08** after independent source/composition, financial-
consumer and context/host reviews. Runtime/caller/host reconciliation and
independent financial qualification remain pending.

1. **Require an explicit composition mode.** Retain one immutable validated
   `CreditIndexData` with a required Homogeneous or Named composition. Keep the
   existing common index-level hazard and base-correlation references, with
   their actual declared financial roles. Do not infer mode from absent maps,
   an empty roster or curve equality.

   Homogeneous contains a positive u16 count and explicit finite future cash
   recovery in [0,1]. It declares equal 1/N exposures, a shared marginal from the
   index hazard and that common recovery: an anonymous model pool, not a hidden
   roster of identified constituents. Named contains a nonempty complete roster
   of atomic issuer entries, each with its hazard reference, explicit future
   cash recovery and weight. Derive its count from the roster, checked against
   the existing u16 bound; no independent mutable Named count or optional
   parallel curve/recovery/weight overlays.

   In Named mode the common index hazard remains a distinct reference/auxiliary
   source for operations that actually use it, including the current timing
   model. It is not a missing-member replacement or automatic weighted issuer
   aggregate. The base-correlation source requires accepted18's actual maturity/
   series/pool/hazard/recovery/loss-model association before financial use.

2. **Resolve complete member records and exact identity.** Require stable
   nonblank issuer IDs, exact case-sensitive lookup and duplicate rejection
   before iterator/entry-list collection overwrites information. Canonical
   lexicographic ordering is appropriate for this keyed roster; it is different
   from repairing unordered financial knots. Do not trim, case-fold, parse IDs
   or silently alias names. Issuer ID, hazard CurveId and external context key
   are distinct roles. Several names may deliberately share one hazard source
   under a qualified proxy, without becoming one member or default event.

   Named lookup uses one checked member resolver; existing curve/recovery/weight
   accessors return the selected record's values or NotFound. Unknown/blank IDs
   never receive index/equal-weight data. Homogeneous named lookup is unsupported
   and returns a validation error, rather than accepting any string as a member.
   Explicit common-recovery/equal-weight construction may resolve caller-selected
   settings into complete Named entries at the canonical Rust owner; those are
   effective stored inputs, not runtime fallback precedence or host calculations.

3. **Bound each weight and state the unit-mass precision.** Named weights are
   fractions of the declared pool notional, individually finite in [0,1]; zero
   is valid. Accumulate them in canonical issuer order with the existing Neumaier
   compensated sum, requiring finite computation and
   abs(total-1)<=f64::EPSILON, where EPSILON=2^-52, approximately 2.22e-16.
   This fixed representation budget replaces 1e-9; it is not an economic fit
   tolerance or a claim that the exact rational sum of represented inputs is 1.

   Retain every supplied weight unchanged. Do not normalize, redistribute a
   residual, delete small positives or silently repair exposure. The same gate
   applies at direct construction, state restore and replacement. It admits
   ordinary rounded fractions, such as 49 copies of 1/49, while rejecting mass
   beyond the stated budget. It does not authorize approximate homogeneity,
   exposure pruning, loss-grid tolerance or an arbitrage acceptance budget.
   All-zero Named exposure fails the mass gate; zero-weight identities remain
   part of the roster/count even when an actual operation omits their exposure.

4. **Make effective future cash recovery authoritative for its role.** Require
   one explicit finite recovery in [0,1] for the Homogeneous pool or each Named
   member. Future default loss uses that selected cash recovery; no absent
   override or unrelated hazard metadata silently replaces it. Preserve genuine
   recovery endpoints, subject to the actual operation's supported limit or
   explicit refusal, without a generic positive-LGD floor.

   Hazard-curve recovery metadata describes its source/calibration construction;
   it need not equal a separately selected payoff recovery in every operation.
   Blanket equality or automatic synchronization would conflict with accepted05's
   explicit frozen-intensity changes. Qualify actual quote/contract/fit consistency
   and label the chosen operation. Future cash-recovery replacement, hazard
   metadata replacement and quote-preserving recalibration remain distinct.
   Rebinding a hazard must not rewrite stored pool/member payoff recovery.

5. **Separate historical state from future recovery assumptions.** A future
   recovery field does not establish observed defaulted or recovered notional.
   Resolve actual realized events/state at their financial owner. An explicitly
   selected historical inference/approximation must retain its own assumption
   and provenance; if required state/assumptions are missing, refuse unsupported
   seasoned use rather than infer it from current future recovery by default.

   Ordinary future-recovery shocks must not rewrite historical state. Any
   deliberately selected joint/counterfactual operation is a separate request,
   not implicit Recovery01. In particular, loss/(1-R_current), a near-zero LGD
   floor and clipping do not make prior defaults identifiable. At R=1, zero loss
   can coexist with many defaulted-notional amounts. This record does not create
   a core event ledger or adopt the Draft tranche-state/erosion model.

6. **Consume the actual probability sources and financial basis.** Preserve
   each hazard's validated source clock/context and accepted04 dated conditional
   probability semantics. Do not reconstruct named PD as clipped 1-survival
   using default day-count context, or infer complete clock compatibility from
   matching base dates. Source/valuation conditioning and observed credit events
   remain distinct; named identity alone is not a probability-domain certificate.

   Index and issuer hazards have declared roles, not a forced equality relation.
   Averaging hazards does not generally produce weighted survival, a portfolio
   loss law or a market index CDS quote. No automatic index-from-members fit,
   issuer fallback or consistency repair is selected here. Actual dated use
   qualifies reference index/series or portfolio, maturity, composition/notional
   basis, hazard/recovery assumptions, correlation model and construction history.
   Numeric proxy inputs need explicit role declarations without invented fit
   records. A context key or successful builder is insufficient financial authority.

7. **Keep distinct identities, zero exposure and model choices intact.** Loss
   calculations may omit zero exposure where mathematically inactive, without
   deleting its source identity, redistributing weight or using total roster count
   as an active equal-weight count. Shared hazard sources do not merge issuer
   default variables. Retain actual exposure sizes and recoveries around tranche
   payoff kinks; count and an aggregate expected loss do not identify concentration.

   A homogeneous computational shortcut must preserve the actual canonical
   probability/exposure/recovery law, or be a separately selected and qualified
   approximation. Near equality within 1e-9 is not exact equivalence or source
   permission to replace the pool. Bounded subset/grid/binomial/resource policies
   remain with the model; u16 source admission does not promise affordable exact
   pricing or authorize a large-pool/Monte Carlo fallback.

8. **Trace every active dependency and recovery target.** Named expected loss
   uses member hazards, but current protection/accrued-premium timing also uses
   index hazard. Actual pricing/risk operations must resolve every consumed role;
   selecting only issuer IDs is not evidence of complete hazard risk coverage.
   Conversely, do not evaluate an unused source merely because it is stored.
   Qualify zero-weight dependencies at the selected operation, rather than
   assuming all stored names are active or universally irrelevant.

   Deduplicate shared CurveIds for a source shock/replay where appropriate,
   replacing all corresponding references coherently; retain separate issuer
   events/exposures. Future recovery risk must target the actual selected pool/
   member cash inputs, not only a common field or hazard metadata that named
   loss does not consume. Preserve the historical-state boundary above and
   accepted05's frozen-versus-recalibrated risk meaning. JTD averaging/count,
   quote replay and broader metric accuracy remain separately owned Draft work.

9. **Preserve canonical state and validated reference replacement.** Keep the
   existing `CreditIndexState`/context restore owner, with required composition
   tag, common curve IDs and complete mode-specific data. Named wire entries
   retain issuer identity, hazard CurveId, recovery and weight together; derive
   count instead of restoring a redundant count/parallel-map/default combination.
   Resolve all referenced sources by exact ID/type and reuse canonical composition
   admission before committing the composite. Reject unknown/missing/incompatible
   fields and duplicate preserved entries, without old readers, migrations,
   aliases, defaults or last-wins reconstruction.

   Context registration's key remains the composite identity; `CreditIndexData`
   needs no duplicate standalone ID. Canonical rebinding replaces curve handles
   while preserving mode, member identity, recovery, weight and role/provenance
   associations. A same-ID correct-type replacement is not by itself new maturity/
   series/pool/model qualification or current fit. Existing raw-handle rebinding
   leaves structural fields unchanged; this is not a claim of a reproduced count
   defect. New invariants must survive all actual replacement/restore paths.
   The broader policy for invalidated composites and reported IDs stays Draft20.

10. **Reconcile existing hosts and financial owners together.** Update canonical
    builder/lookup/state, context/calibration-envelope/attribution transport,
    tranche probability/loss/timing/risk adapters, existing Python/WASM classes,
    stubs/generated declarations/schemas/parity and examples in the same slice.
    Reuse existing state restoration after source loading rather than add a second
    standalone serde path. Named input remains plain structured params/entries;
    no new host-side calculation or unrelated class/framework is needed.

    Python missing Named issuer maps NotFound to KeyError; unsupported mode or
    malformed composition maps validation to ValueError, and computation failures
    retain RuntimeError. Existing WASM methods propagate the same Rust-owned
    failure rather than return defaults. Reject duplicates where input still
    preserves pairs/entries; a host dictionary that already erased duplicate
    history cannot recover it. Construction or replacement success is structural
    evidence, not loss-model/fit/risk certification.

## Observed implementation and discrepancies

Current builder requires count/common recovery/index hazard/base correlation and
stores optional parallel issuer maps. Curve coverage is complete when supplied,
but sparse recovery overlays and omitted weights retain lookup defaults. Map
setters collect iterators with last-wins duplicates; issuer keys are not checked
for blank identity. Unknown IDs receive aggregate/equal-weight values. The
internal heterogeneous loss loop enumerates known members, so this lookup gap
is not a claim that that loop currently introduces unknown-name fallbacks.

Weights are finite/nonnegative but individually may exceed 1 within the naive
1e-9 mass budget. Named loss correctly skips zero-weight exposure, then its first
homogeneous fast path tolerates near-equal PD/recovery/weights. Named PD uses
default day-count context and clipped 1-sp. Index hazard still supplies timing;
active-hazard CS01 selects named hazards in Named mode, while other metrics
include all roster names in their count/averaging. These are actual dependency/
model choices requiring qualification, not executed risk-impact measurements.

Current recovery fields serve several roles; historical erosion uses current
index recovery even for named pools. State restore already resolves typed curve
IDs and builds validated data. Rebinding replaces handles by ID/type without
financial maturity/model qualification. These are source/control-flow findings;
full projection, recovery, loss, calibration, risk and invalidation methods remain
with their Draft owners. Python accepts issuer inputs directly; the current WASM
constructor is homogeneous, with Named data available through context JSON.
Reconcile those existing routes; no runtime or production tests were executed here.

## Independent examples and qualification limits

On **2026-11-08**, sixteen standalone rational/200-digit Decimal/binary64 cases
were evaluated without production imports. The small probability/loss examples
state independent-name assumptions explicitly; none is a production benchmark:

- Duplicate pairs A→HA,A→HB become one last-wins map entry. Separately, unknown
  issuer fallback weight 0.5 is neither a 0.7 nor a 0.3 member weight: at LGD=0.6,
  replacing the 0.7 exposure gives loss 0.30 instead of 0.42. A third identity case
  distinguishes a three-name roster's derived count from an independent count 2.
- Weight 1+5e-10 passes the old mass budget but violates an individual unit bound.
  Separately, adding twelve 2^-54 weights sequentially after 1 loses all additions;
  accurate represented-input total is 1+3*2^-52, beyond the new budget. A sixth
  case has 49 unchanged 1/49 weights total approximately 0.99999999999999989,
  admitted within the explicit 2^-52 representation allowance without normalization.
- Two independent names, PD=0.2 and recovery=0, give capped EL 0.046 at cap 0.15
  with weights 0.9/0.1, versus 0.054 with equal weights. Both full-pool ELs are
  0.2. Separately, weights 0.75/0.25, PD=0.1 and recoveries 0.2/0.8 give EL 0.065,
  versus 0.06 with a common recovery 0.4.
- Frozen lambda=0.1 at time 1 gives unchanged PD, but selected recoveries 0.4/0.2
  give losses approximately 0.05709754918/0.07613006557. Separately, equal-weight
  issuer hazards 0.01/0.2 at time 5 give weighted survival 0.65955443284, whereas
  survival at average hazard 0.105 is 0.59155536437.
- Adding a zero-weight name to the concentrated example leaves EL 0.046, while
  preserving its roster identity. Separately, two independent equal-weight
  names sharing PD=0.5 have capped EL 0.375 at cap 0.5; merging their default
  events into one weight-1 name gives 0.25. Sharing a hazard source is not merging.
- Fixed realized loss 0.06 implies defaulted fraction 0.1 under historical R=0.4,
  but 0.075 under R=0.2. Separately, at R=1 both defaulted fractions 0 and 0.5
  produce zero loss, so loss alone cannot identify historical defaults.
- Two independent PD=0.5/recovery-0 names with weights 0.5±2e-10 have capped EL
  0.375-5e-11 at cap 0.5, rather than 0.375: near homogeneity is not exact.
  Separately, for lambda*time=1e-20, ordinary 1-exp(-lambda*time) returns 0 while
  the stable direct interval probability remains approximately 1e-20. This
  sixteenth case checks accepted probability-expression ownership.

Independent production qualification must cover mode/roster/count/duplicates and
strict lookup; complete recovery/weight inputs, bounds/compensated mass and zero
identity; distinct hazard/payoff/historical recovery roles; accepted dated PDs,
actual index/member timing dependencies, shared-source versus issuer identity;
active-exposure/model shortcut/resource/risk semantics; canonical state/rebinding,
mode-specific host failures and financial maturity/pool/fit association. Reconcile
all exposed paths and compare actual selected models with independent references.
Unit bounds, algebra, same-kernel round trips and complete source coverage do not
establish pricing, fit, risk precision or absence of arbitrage.

## Evidence and alternatives

Owners: [CreditIndexData and builder](../../../../finstack-quant/core/src/market_data/term_structures/credit_index.rs),
[source tests](../../../../finstack-quant/core/tests/market_data/credit_index.rs),
[existing compensated sum](../../../../finstack-quant/core/src/math/summation.rs),
[context binding](../../../../finstack-quant/core/src/market_data/context/mod.rs),
[state](../../../../finstack-quant/core/src/market_data/context/state_serde.rs)
and [restore](../../../../finstack-quant/core/src/market_data/context/insert.rs),
[named loss](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/heterogeneous.rs),
[loss composition](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs),
[timing](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs),
[recovery/history](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs)
and [active-hazard risk](../../../../finstack-quant/valuations/credit/src/cds_tranche/metrics/cs01.rs).
Existing source/context/state and loss/risk tests were inspected or located,
not executed. Existing [Python class](../../../../finstack-quant-py/src/bindings/core/market_data/curves/credit.rs),
[WASM class](../../../../finstack-quant-wasm/src/api/core/market_curves.rs)
and [Python error mapping](../../../../finstack-quant-py/src/errors.rs) carry the
actual exposed contract.

Primary [Gibson, Federal Reserve, section 3](https://www.federalreserve.gov/pubs/feds/2004/200436/200436pap.pdf)
identifies per-name notionals, risk-neutral marginal probabilities and recoveries,
with homogeneous simplification under common exposures/assumptions. Canonical
[Gibson](../../../REFERENCES.md#gibson-2004-synthetic-cdos) and
[Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas)
provide model provenance. They do not choose this record's tagged source/state
architecture, epsilon mass budget or host/error policy, or certify the repository's
loss/risk implementation.

Alternative pool aggregation, recovery/event inference, approximate homogeneity,
normalization and index/member calibration remain explicit financial operations
rather than hidden lookup defaults or source repairs.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
