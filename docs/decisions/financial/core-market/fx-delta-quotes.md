# CORE-MARKET-15: Explicit FX delta-quote identity and checked wing interpretation

- Decision status: **Accepted — complete FX quote-profile/source package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted identity, shared arithmetic and forward-qualified adapters are unimplemented.
- Validation status: **Primary/source evidence and independent quote/numerical, model/forward and consumer/host reviews inspected; standalone rational/high-precision/normal-transform examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Examples do not certify production smiles, broker conversion, risk or arbitrage freedom.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns FxDeltaVolSurface's pair/profile, ATM/RR/BF meaning, raw-quote
admission and source/evaluator compatibility. It preserves accepted
[ADR0045 volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility),
[FX pair/date semantics](../core-conventions/011-fx-date-and-quote-conventions.md)
and [record 14's explicit strike-volatility grid](volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
Quote-space smiles and absolute-strike grids remain distinct sources.

[FX smile interpolation/materialization](../models-analytic/fx-delta-smiles.md),
[generic volatility dispatch](../models-analytic/volatility-source-interpolation.md),
[delivery forwards](../valuations-fx-and-exotics/fx-delivery-cip.md),
and [option pricing/reporting deltas](../valuations-fx-and-exotics/fx-vanilla-delta.md)
retain their remaining Draft models/policies. This source contract does not adopt
their complete interpolation, tail, delivery/collateral, sticky-smile, pricing or
aging choices. Accepted [context lifecycle20](context-lifecycle.md) owns typed
selection, atomic mutation and state capture.

## Foundational findings

The current container and model use one fixed quote convention, without a pair
or explicit profile declaration in state. Arithmetic smile-vol BF is also called
broker strangle in model documentation, although market-strangle quotes require
a different price-equivalence calculation. Other delta/ATM conventions cannot
be inferred from pair names or a product's hedge-delta reporting selection.

Source and model duplicate wing arithmetic. ATM = 1, BF = 1e-20, RR = -2 has a positive
call wing approximately 1e-20, but adding ATM+BF first loses it and produces 0.
Conversely, an underflowed half-RR can hide a final unavailable positive wing.

The model also overloads a tenor argument as FX forward. Source inspection finds
an actual FX option route supplying tenor 0 through the generic resolver, which
reaches a positive-forward check. A real delivery forward is a required financial
input; no zero/one sentinel, flat-rate default or unrelated numeric tenor supplies it.

## Accepted decision: one explicit FX quote profile and qualified forward inputs

Accepted on **2026-11-08** after independent quote/numerical, model/forward
and consumer/host reviews. Runtime/caller/host reconciliation and independent
financial qualification remain pending.

1. **Retain one deliberately fixed supported quote profile.** Require a closed
   explicit `quote_profile` declaration, `forward_dns_smile`, meaning unshifted
   Black annual decimal volatility, premium-unadjusted forward delta,
   delta-neutral-straddle ATM and arithmetic smile-vol butterfly. It is one
   complete input assertion, not independently selectable flags for unsupported
   conventions. Reject missing/unknown/incompatible profiles without defaults
   or inference from IDs, currency pairs, expiry or broker labels. Ingestion
   verifies the actual supplied quote definitions and retains source/convention
   provenance; the tag alone does not attest vendor compliance.

   Spot delta, premium-adjusted delta, ATM-forward/spot and price-defined market-
   strangle BF need a separately qualified conversion or an explicit refusal.
   Do not add ignored premium-currency metadata to this unadjusted source;
   conversion involving premium adjustment needs its real premium/discount
   context at that owner. Product reporting/hedge deltas are separate: reporting
   a premium-adjusted delta does not retag or invalidate an otherwise correctly
   interpreted unadjusted quote source. No general quote-convention framework
   or valuation-layer dependency is introduced into core.

2. **Require the ordered currency pair.** Reuse `CurrencyPair` with distinct
   explicit base and quote currencies. Spot, forward and strike use quote
   currency per one unit of base; calls/puts are on base currency. RR is base-
   call volatility minus base-put volatility. Require exact source/product/
   forward-pair agreement, with no ID parsing or reversed-pair fallback. Merely
   swapping put/call labels or negating RR does not preserve the same forward-
   delta/DNS coordinates under reciprocal FX. Pair reversal requires actual
   repricing/relabeling at a qualified owner, otherwise error. Source and
   transport retain the pair/profile together, not just a display ticker.

3. **Admit one canonical three/five-point raw quote source.** Require nonempty
   finite strictly positive increasing expiry coordinates, aligned positive
   finite ATM sigma, and aligned signed finite 25d RR/BF. Optional 10d RR/BF are
   both present with full matching length or both absent. No sorting, missing-
   quote fill, shape repair, skew cap or nonnegative-BF restriction. Single
   expiry is valid. Units are decimal relative annual sigma or sigma differences:
   0.12 means 12%, and RR = 0.01 means one absolute vol percentage point.

   For each supplied absolute delta p = 0.25 or 0.10, define

   ```text
   RR_p = sigma_call,p - sigma_put,p
   BF_p = (sigma_call,p + sigma_put,p)/2 - ATM
   sigma_call,p = ATM + BF_p + RR_p/2
   sigma_put,p  = ATM + BF_p - RR_p/2.
   ```

   Both recovered wings must be representable finite and strictly positive.
   Positive ATM/wing sigma is required for nondegenerate delta-strike geometry;
   record 14's zero-sigma absolute-strike grid remains valid under its own contract.
   The arithmetic BF is not a market-strangle premium consistency condition.

4. **Share one checked wing reconstruction owner.** Core admission and model
   access use the same private checked affine arithmetic for the complete
   ATM+BF±RR/2 expressions. Use scaling/compensated sums as required; do not lose
   a small valid wing by rounded intermediate addition or require an unused
   half-RR to be representable separately. Judge the final wing values, retaining
   genuine cancellation and ordinary final rounding. A true nonpositive wing
   is a domain failure; an unrepresentable positive wing is a range/computation
   failure, without a tiny-vol floor, alternate BF, dropped wing or clipping.
   Revalidate complete input before identities. Keep raw ATM/RR/BF canonical;
   no independently persisted derived-wing arrays or public unchecked twin.

5. **Preserve optional source clocks and actual date roles.** Add/preserve
   accepted `expiry_clock: Option<VolatilityClock>`. Pure numeric sources may omit
   it; dated producers record the actual expiry mapping and dated active use
   requires complete source-clock resolution/agreement under ADR0045. Preserve
   metadata through clones, context state and derived outputs. Ordinary direct
   dated use requires source origin=valuation as_of unless an explicit owned
   seasoned-source model applies; cloning or recomputing remaining time does
   not qualify an older smile as current data.

   Option expiry E, FX delivery L and market spot settlement P are distinct.
   Sigma time concerns expiry on the actual source clock; strike inversion uses
   the positive forward for the actual delivery contract. Dated input owners
   retain those dates/conventions and their dependencies, not a numerical expiry
   substituted for delivery. No universal delivery lag, holiday calendar, cash-
   spot assumption or borrowing of a discount/coupon clock is selected here.

6. **Pass the actual delivery forward explicitly.** Active evaluation requires
   finite positive F in the source pair/units from an explicitly qualified
   numerical input or the owning dated FX forward resolver. Materialization
   requires the corresponding explicit forward for every source expiry and its
   actual delivery basis. Replace the overloaded tenor-as-forward route with a
   clearly named forward input in the existing request/adapter; do not invent 0,
   1, spot or a rate tenor when it is missing. Select the actual compatible
   source before resolving dependencies, and reject wrong/ambiguous financial
   association rather than silently substitute another same-ID source kind.
   Explicit sigma overrides and inactive lifecycle branches do not inspect an
   unused FX quote source or require its forward.

   In particular, `collect_fx_option_inputs_at_spot` must obtain and bind the
   actual delivery forward before active FX-delta resolution, or return an
   error when it is unavailable. Its current expiry-based rate inputs and
   missing delivery/spot-settlement contract cannot be repaired by merely
   renaming the generic tenor argument.

   Remove the duplicate raw `FxVolSurfaceBuilder` quote/spot/constant-rate bypass.
   Model construction consumes the canonical validated source and qualified
   forwards. A deliberately selected flat-rate numerical model can compute its
   own explicit forwards through its owner; there is no local default
   `S*exp((r_quote-r_base)*T_expiry)` substitute. ATM-only or 10d-only inputs are
   not this profile and must not be accepted then silently ignored. An actual
   flat-volatility model/strike grid retains its existing explicit owner.

7. **Require checked transforms and supported strike geometry.** The profile's
   mathematical interpretation is call forward delta Phi(d1), put absolute
   delta Phi(-d1), with d1=[log(F/K)+sigma^2*T/2]/(sigma*sqrt(T)). Thus

   ```text
   K_call(p) = F*exp(-Phi_inverse(p)*sigma_call*sqrt(T) + sigma_call^2*T/2)
   K_put(p)  = F*exp(-Phi_inverse(1-p)*sigma_put*sqrt(T) + sigma_put^2*T/2)
   K_DNS     = F*exp(sigma_ATM^2*T/2).
   ```

   Existing model delta/strike transforms become checked `Result` operations:
   finite positive F,K,sigma,T where required and interior normalized delta,
   with no endpoint/zero-vol proxy or error-to-NaN. Use the accepted checked
   normal functions and stable complete-result log/scaled arithmetic rather
   than an overflowing unrequested exp or ratio. A nonrepresentable positive
   strike or lost required geometry errors; no arbitrary tiny-time/vol repair.
   Raw wing positivity does not prove that derived strikes are distinct or in
   the assumed put10/put25/DNS/call25/call10 order. The actual interpolation
   owner must verify its supported ordering/range before lookup and refuse an
   unsupported smile, not silently relabel, cap quotes or merge collisions.
   This defines quote interpretation, not a new core smile interpolation engine.

8. **Keep direct quote evaluation and strike materialization distinct.**
   Quote-space expiry interpolation then strike construction differs from
   constructing each original-expiry smile and interpolating its sampled strike
   grid. The model owner selects and qualifies the actual off-expiry/wing/tail
   operation; neither route is automatically interchangeable or lossless.
   Materialization must retain every original supported pillar exactly, without
   the current absolute 1e-10 strike coalescing. Deduplicating exactly identical
   coordinates across expiries is different from merging distinct pillars;
   unrepresentable within-smile collisions error. Preserve source pair/profile,
   raw quotes, clock and actual forward dependencies as construction provenance.
   The new `VolSurface` output carries explicit Black/absolute-strike convention,
   chosen interpolation metadata and truthful clock under accepted record 14. Successful
   sampled repricing does not certify the whole source function or its risk.

9. **Keep shocks, dynamics and fit claims at their owners.** A stored-strike
   sigma-grid shock is not an ATM/RR/BF quote change or a recalibrated broker
   smile. This record adds no generic FX-delta bump fallback; an explicitly new
   raw quote source goes through full canonical admission. A changing forward
   with fixed delta quotes and a fixed materialized strike grid need not have
   the same smile movement or hedge risk. Data presence, profile metadata and
   product reporting-delta choice do not select sticky-delta/sticky-strike or
   quote recalibration. Positivity certifies neither calendar/butterfly freedom
   nor market-strangle fit, delivery/collateral assumptions or quote quality.
   Those models and financial acceptance budgets remain separately owned.

10. **Restore one strict source and reconcile actual exposed paths.** Require
    nonempty source ID, ordered pair, explicit supported profile, ordered expiry/
    raw quote arrays, paired optional 10d data and accepted optional clock. Reject
    missing required/unknown/incompatible state without defaults, repairs,
    convention aliases, migrations or old untagged readers. Reuse the canonical
    source validation at direct builders, model materialization and all restore
    routes. Reconcile core context/calibration MarketDatum/attribution transport,
    generic VolSource and FX-forward adapters, existing Python/WASM source/model
    classes/helpers, stubs/declarations, schemas/parity and examples together.
    Use a builder/params object rather than extending the seven-position raw
    constructor. Rust owns financial behavior; no parallel host conversion,
    new quote framework or unused premium-currency dependency is introduced.

## Observed implementation and discrepancies

Core stores ID and ATM/25d/optional 10d arrays, with positive expiries/ATM/wings,
signed finite RR/BF and paired 10d validation. It has no pair, profile or clock.
Source restore correctly reuses constructor validation, but ordinary staged
wing addition and half-RR arithmetic can misclassify admitted values.

Model construction has a second raw builder with spot/rate defaults, ATM-only
and paired 10d-only paths; 10d-only quotes are ignored because 25d presence selects
wing construction. It recovers wings independently and calls arithmetic BF
smile (broker) strangle. Direct evaluation blends/clamps quotes in expiry then
uses the supplied forward; materialization builds original-expiry smiles with
flat-rate forwards and tolerance-coalesces a common strike grid. Derived
strike ordering is not consistently admitted before direct lookup.

Generic source dispatch uses tenor for the FX forward. The actual shared
strike-only resolver supplies tenor 0; an active FX-delta route therefore reaches
the model's invalid-forward error. This is a source/control-flow finding, not
an executed production pricing result. Four reported product delta conventions
already exist but do not select quote-source conventions. Context aging clones
the original FX quote map; that is not an approved seasoned-smile model.

## Independent examples and qualification limits

On **2026-11-08**, thirteen standalone rational/200-digit Decimal and standard-
library normal-transform cases were evaluated without production imports.
Normal CDF/quantile contrasts are approximate binary64 references, not 200-digit
normal-function or production accuracy certification:

- ATM = 0.12, BF = 0.005, RR = 0.02 gives call 0.135/put 0.115 and reproduces the signed RR/BF
  definitions. Separately, ATM = 0.12, BF = -0.01, RR = 0.02 gives positive call 0.12/put 0.10;
  negative arithmetic BF is not automatically invalid.
- ATM = 1, BF = 1e-20, RR = -2 has positive call wing approximately 1e-20; staged ordinary
  addition returns 0. With ATM/RR both minimum positive binary64 and BF = 0,
  underflowed RR/2 instead makes both old wings positive, although final exact
  half-wing rounding gives call 2min/put 0 and the put is unavailable. These were
  separate cancellation and final-range cases over represented input values.
- At sigma = 0.2, T = 1, DNS K/F = exp(0.02), approximately 1.02020134, whereas K=F has
  call delta approximately 0.53982784. DNS is not ATM-forward. Separately, foreign
  discount exp(-0.04) turns forward delta 0.25 into spot delta approximately 0.24019736.
- Reciprocal coordinates map an original absolute put forward delta 0.25 with
  sigma*sqrt(T) = 0.2 to inverse call delta approximately 0.31757535. The reciprocal
  of the original DNS strike has inverse K/F = exp(-0.02), approximately 0.98019867,
  while inverse-pair DNS is exp(.02). These were separate reversal examples.
- ATM = 0.2, BF = 1.4, RR = -2.8 gives positive call 0.2/put 3 at T = 1, yet put/DNS/call strike
  ratios are approximately 11.89991/1.02020/1.16754: source positivity does not
  establish the current model's assumed strike order.
- Stipulated cash spot 1.2 with expiry factors (base 0.99, quote 0.98) gives forward
  approximately 1.21224490; delivery factors (0.985, 0.97) give 1.21855670. This is a
  stated cash-spot/factor example, not adoption of the Draft FX delivery model.
- Setting sigma = 0 in the formal inversion collapses every delta strike to F,
  so it cannot identify a nondegenerate 25d/10d smile. Separately, distinct
  strike coordinates 1 and 1+5e-11 are coalesced by the old 1e-10 threshold.
- With F = 1e-300 and log-strike offset 1000, complete K=exp(log(F)+1000) is finite
  although exp(1000) overflows binary64. This thirteenth case checks expression
  range, not a particular recovered binary64 sigma/expiry pair.

Independent production qualification must cover pair/profile/source provenance,
exact units/25d/10d shape and strict source/host restore; signed BF/RR, cancellation,
subnormal/final-wing range and shared reconstruction; actual clock and expiry/
spot-settlement/delivery-forward association; active-source dispatch/overrides,
checked delta/strike transforms, geometric collisions/order and materialized
pillar fidelity; off-expiry representation, risk and quote-recalibration behavior
at their actual owners. Reconcile all Rust/host/schema/parity paths and compare
the chosen models with independent market fixtures. Algebra, quote identities,
inverse round trips and same-kernel pillar checks do not establish those results.

## Evidence and alternatives

Inspected [core FX quote source](../../../../finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs),
[model builder/evaluator](../../../../finstack-quant/models/src/volatility/fx.rs),
[VolSource dispatch](../../../../finstack-quant/models/src/volatility/source.rs),
[active source resolution](../../../../finstack-quant/valuations/core/src/market/volatility.rs),
[shared volatility request](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs),
[FX pricing adapter](../../../../finstack-quant/valuations/fx/src/shared.rs),
[product delta conventions](../../../../finstack-quant/valuations/fx/src/fx_option/types.rs)
and [CurrencyPair](../../../../finstack-quant/core/src/money/fx/types.rs).
Existing constructor, FX smile, conversion, model and host tests were inspected
or located, not executed.

[Reiswich–Wystup, version 2, sections 1.2–1.4 and 2.3, equations 31–32](https://www.econstor.eu/bitstream/10419/40186/1/613825101.pdf)
distinguishes pair/delta/ATM definitions and arithmetic smile-vol versus market-
strangle information. Its historical pair-default tables are not adopted as
current universal market policy. [Clark](../../../REFERENCES.md#clark-fx-options)
and [Wystup](../../../REFERENCES.md#wystup-fx-options) provide broader context.
The one-profile source tag, canonical data ownership and explicit adapter/error
boundaries are this record's architectural choices, not publisher requirements.

Additional broker profiles, premium-adjusted inversion, market-strangle solves,
pair conversion and smile dynamics retain explicit financial owners rather
than becoming unchecked modes or fallbacks for this fixed source.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
