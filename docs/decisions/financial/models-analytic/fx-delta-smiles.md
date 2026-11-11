# MODELS-ANALYTIC-17: FX RR/BF conversion, delta strikes and smile interpolation

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar arithmetic checks support the proposed distinctions. Production implementation, independent pricing and host checks remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX delta smiles and strike materialization

All 25 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Use the accepted FX quote profile.** Consume the canonical
   CORE-MARKET-15 source with explicit ordered pair and
   `forward_dns_smile` profile. Require ATM and paired 25-delta RR/BF
   at every expiry, with paired 10-delta RR/BF either present on all
   rows or absent. Retire the duplicate raw builder and its ATM-only,
   10-delta-only and spot/constant-rate paths. Spot or premium-adjusted
   delta, other ATM definitions and market-strangle premium conversion
   require their own qualified owner.

2. **Preserve the meaning of the quotes.** Forward and strike are in
   quote currency per unit of base, with option side on base. Vol is
   annual unshifted Black decimal sigma. At each supplied absolute
   delta label, RR is call sigma minus put sigma and BF is their mean
   minus ATM sigma. Recover call and put as ATM + BF ± RR/2 using
   the shared checked affine arithmetic. Preserve signed RR/BF and
   require finite strictly positive recovered sigmas.

3. **Keep expiry, delivery and spot settlement distinct.** Use positive
   option time on the resolved source expiry clock. The explicit
   forward refers to the actual underlying delivery contract in the
   source pair. The dated owner qualifies expiry, delivery, spot
   settlement, conventions and the corresponding forward, including
   clock and as-of agreement under ADR-0045. Numeric sources retain
   absent clock metadata when applicable. No implicit forward, clock
   or time-aging rule is selected here.

4. **Expose one checked transform in each direction.** Replace the
   unchecked scalar helpers with fallible `delta_to_strike` and
   `strike_to_delta` for explicitly selected call or put. Require
   finite positive F, sigma and T and, in the forward direction from
   strike, finite positive K. Strike inversion takes finite absolute
   delta p in (0,1). These are fixed-sigma premium-unadjusted forward
   delta magnitudes.

5. **Use checked Black delta geometry.** Set side eta=+1 for call or
   −1 for put, w=sigma sqrt(T), and
   `d1 = log(F/K)/w + w/2`. Return `Phi(eta d1)` using
   CORE-NUMERICS-12. Compute the log ratio from original F and K,
   using the stable close-level difference and scaled arithmetic for
   the complete result. Ordinary final probability rounding to 0 or 1
   is allowed after checked geometry. Propagate unresolved geometry or a
   failed computation as an error.

6. **Invert with the requested side and its own probability.** Use
   `z = Phi^-1(p)`, `e = w(w/2 - eta z)` and `K = F exp(e)`.
   The absolute put probability goes directly into the quantile.
   This preserves small p when a call-equivalent complement would
   round to 1. Delta-neutral straddle uses p=0.5 and ATM sigma,
   giving `K_DNS = F exp(w_ATM^2/2)`. Use the same checked kernel
   for DNS and every wing.

7. **Restore the actual positive represented strike.** Use checked
   scaled factors, retain cancellation in the exponent, and preserve
   the complete exponential product with F. Use close-level
   exponential differences when appropriate. A positive strike can
   be representable even when exp(e) alone overflows. Refuse a
   nonpositive, nonfinite or unresolved result without floors,
   clipping or endpoint nudges.

8. **Check the returned strike against the smaller delta tail.**
   Require a finite explicit `delta_rel_tol` in (0,1) for
   inversion and smile construction, shared by all pillars in that
   request. Recompute d1 from the actual returned K. For p≤0.5
   compare log Phi(eta d1) with log p. For p>0.5 compare
   log Phi(−eta d1) with log(1−p). For log(actual) minus log(target),
   the inclusive interval
   is `[log1p(-delta_rel_tol), log1p(delta_rel_tol)]`. Use the checked log CDF.
   A failed gate is a computation error, including rounded strikes
   that lose the intended delta.

9. **Validate the complete represented pillar order.** Require
   put25 < DNS < call25, with put10 strictly to the left of put25
   and call10 strictly to the right of call25 when present.
   Check the whole requested smile before lookup, including exact
   strike requests. Each label uses its own sigma. Strict order
   refers to the DNS anchor and other pillars, so a put strike
   can exceed F. Crossings and within-row collisions error.
   No relabeling, sorting, coalescing or skew repair.

10. **Expose the complete labelled quote row.** The existing stored-row
    pillar-volatility access returns ATM and all supplied wing sigmas
    under their labels, including optional 10-delta wings. Bounds-check
    the row index and use canonical recovery. Volatility-only access
    needs no forward and makes no claim about strike order. Any
    strike-bearing result uses complete checked construction.

11. **Keep direct expiry lookup strict.** Accept the exact expiry knot
    or the actual adjacent bracket containing the request. A singleton
    supports its exact expiry. Refuse out-of-domain expiry. Remove
    the old tail that freezes raw quotes while recalculating strikes
    at a different expiry.

12. **Interpolate in quote space with one rounding of the wings.** At
    an exact expiry use that row only. Inside a bracket use the linear
    expiry weights for ATM and each RR and BF lane. Evaluate each
    complete interpolated affine wing before rounding its final sigma,
    equivalently applying the weights to the unrounded row expressions
    ATM + BF ± RR/2. Separately rounded lanes or node-wing intermediates
    can lose a valid result. Reuse checked scaled weight geometry and
    the same affine semantics as source admission.

13. **Build the direct smile at the requested expiry and forward.**
    Invert the reconstructed quote row with the actual requested T
    and its qualified F. Only the forward of this query's delivery
    is required. Do not construct neighboring expiry smiles or
    synthesize an interpolated forward. Validate the requested
    row's full strike geometry even when both stored endpoints
    admit their own smiles.

14. **Make direct strike tails explicit.** Require finite positive
    absolute strike before selecting strict strike bounds or flat
    strike wings. At an exact pillar return its native sigma. Use
    piecewise linear native sigma in absolute strike inside the
    ordered range. Flat wings return the corresponding boundary
    sigma and retain the requested and effective strikes. Strict
    wings refuse queries outside the represented range.

15. **Return the resolved quote together with its coordinates.**
    Use the accepted record 16 quote contract, with unshifted Black,
    source clock, source identity and requested/effective coordinates.
    Preserve the qualified forward association for the operation.
    Propagate invalid sources, missing inputs, unsupported domains
    and range or precision failures through existing typed errors.

16. **Share construction between actual consumers.** Use one checked
    row reconstruction and pillar inversion for direct evaluation and
    materialization. Apply canonical admission to the entire raw source.
    Check strike geometry on the reconstructed request smile in direct
    evaluation and on all original smiles in materialization. Retire
    duplicate formula paths and tolerance-based strike deduplication.

17. **Make FX strike materialization explicit.** Require an output ID,
    explicit output interpolation under record 16, the delta acceptance
    tolerance and one qualified forward for each exact source expiry,
    aligned to the original order and actual delivery basis. Reject
    missing, extra, misordered or mismatched rows. Build each original
    expiry smile on the resolved source clock with its supplied forward.

18. **Preserve the exact union of original pillars.** Form the ascending
    union of all represented absolute strikes. Deduplicate exact
    equality across different rows and keep every distinct strike.
    Each individual row must already pass strict order. Check union
    and rectangular allocation and work before publication. Never
    thin the grid or publish a partial output after a failed node.

19. **Define the rectangular row sampling rule.** At each union strike,
    sample that expiry's own checked smile. Use its exact pillar sigma
    at a knot, linear native sigma inside and flat boundary sigma
    outside. Other rows can supply strikes beyond this row's wings.
    This construction rule is fixed and recorded. Later grid
    interpolation and boundary selection use record 16.

20. **Publish the grid and construction provenance together.**
    Output an unshifted Black absolute-strike sigma grid with the
    original expiry axis and source clock. Preserve source identity,
    ordered pair, profile, raw quotes, exact expiry-to-forward
    association, acceptance control and sampling policy through
    the existing state and artifact paths. Dated provenance carries
    actual delivery dates, basis and forward dependencies. Treat
    the source and sampled grid as distinct selectable kinds.

21. **Qualify what materialization preserves.** Original pillar
    sigmas are retained at the original expiry and strike. At the
    original expiry a Vol-mode grid with the same flat wing policy
    refines the source row's piecewise linear function, subject
    to rounding. TotalVariance uses RMS between strike knots at a
    fixed expiry. Off-expiry interpolation and forward sensitivity
    differ in general. Neither route establishes static arbitrage
    freedom, quote fit to a market strangle, sticky-smile dynamics
    or equivalence of product risk.

22. **Bind the actual dated FX consumers.** Reconcile vanilla, digital,
    touch and barrier adapters that currently forward expiry-time
    inputs to generic sigma lookup. Active FX delta use requires
    their actual compatible quote delivery basis. The pricing owner
    qualifies its payoff forward and discounting and applies any
    supported ADR-0045 sigma clock bridge. Explicit scalar
    overrides and inactive or terminal branches need only their own
    inputs. Keep dependencies source-conditional and visible to the
    pricing and portfolio owners. An aged snapshot requires a
    separately qualified source roll.

23. **Keep pricing and risk on the same active source.** Reconcile
    vanna/volga and shared risk paths with the source selected for PV
    and its quote clock and forward. Generic strike-grid bumps apply
    only to eligible strike grids. RR/BF shocks and forward-smile
    dynamics require explicit owners. Product reporting delta and
    fixed-sigma partials retain their own meanings. A checked sigma
    does not select digital smile derivatives, touch or barrier laws.

24. **Reconcile existing transport and host routes.** Use the same
    checked Rust transforms, labelled quote access, query and
    materialization in Python, WASM and the existing FX chart.
    Carry complete query and forward identity, errors and sampled
    metadata. Update the paired stubs, facade, declarations, parity,
    schemas, examples and existing context, calibration MarketDatum
    and attribution snapshot transport with canonical restoration.

25. **Require independent and complete-path qualification.** Test
    primary-reference RR/BF interpretation, absolute side delta,
    DNS versus ATM-forward, tiny probabilities, cancellation and
    full-result range, actual returned-strike checks, crossed
    3/5-pillar geometry, exact and interior expiry, both strike
    boundaries, exact union and row coverage, wrong clocks and
    forwards, publication failure and host parity. Qualify complete
    dated pricing and risk separately. See
    [FX delta smile references](../../../REFERENCES.md#fx-delta-smile-construction-and-materialization).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| F=1, sigma=0.2, T=1 | DNS K≈1.02020134 and delta magnitude 0.5 on both sides. At K=F, call delta≈0.53982784. |
| F=1, w=0.2, absolute put delta 1e−20 | K≈0.16001815. Direct lower-tail inversion recovers the target. The binary64 complement rounds to 1. |
| F=1, w=1e−17, absolute call delta 0.25 | K rounds to 1 and represented-strike delta to 0.5. This fails the relative tail check with tolerance 0.001. |
| F=1e−300, w=40, p=0.5 | K≈2.7263745721e47 even though exp(800) alone overflows binary64. |
| Midpoint with ATM=(1,nextUp(1)), BF=(nextUp(0.25),0.25), RR=−2.5 in both rows | Complete call wing is 5×2⁻⁵⁵. Separately rounding ATM and BF first gives zero. |
| F=1, T=(1,3), ATM/call sigma=(0.1,0.2), put sigma=(1.2,0.8) | Both original smiles are ordered. At T=2, K_put≈1.047208 exceeds K_DNS≈1.022755 and the direct query must fail. |
| F=1 at T=(1,3), ATM 0.2, RR 0 and BF 0.05 in both rows, query T=2 at K=exp(0.04) | Direct quote sigma is 0.2. The sampled Vol grid returns approximately 0.20473735. |
| F=T=1, ATM 0.2, RR25 0 and BF25 0.5 | Pillars are ordered. Unit Black call premiums rise from about 0.07076019 at DNS to 0.07659896 at the 25-delta call, violating call monotonicity. |

These are standalone scalar and exact-rational or high-precision arithmetic
checks. They do not exercise the production code or establish market-strangle
fit, whole-smile arbitrage freedom, pricing behavior or host parity.

## Observed implementation and evidence

### Scope and input domain

Positive spot, positive ascending expiries, positive ATM vol, finite domestic/foreign rates, paired 25δ and optionally10δ risk-reversal/butterfly arrays.

### Choice and rationale

Use premium-unadjusted forward delta, delta-neutral-straddle ATM strike, and smile-butterfly arithmetic wing quotes. This does not solve a market-strangle premium equivalence problem.

### Algorithm

Forward=S exp[(rd−rf)T]; wing σcall=ATM+BF+RR/2,σput=ATM+BF−RR/2. K(δ)=F exp[−N⁻¹(δ)σ√T+σ²T/2]; put |δ| uses call-equivalent1−|δ|. ATM DNS strike=F exp(σATM²T/2). Build3/5 pillars per expiry, merge all strike grids (absolute dedup1e−10), interpolate each expiry using its own pillars linearly in strike with flat endpoints. ATM-only creates constant-vol rows. Direct delta-surface evaluation first linearly interpolates/clamps ATM,RR,BF in expiry, constructs query-expiry pillars using supplied forward, then linearly interpolates/clamps strike.

### Selection and controls

Builder 25δ presence determines wing mode; 10δ quotes are used only inside that branch. Public `delta_to_strike` and `strike_to_delta` are scalar unchecked transforms. Materialization derives forwards from constant rates; direct evaluation accepts current forward. Surface interpolation after materialization follows stored surface mode.

### Conventions

FX spot/strike in quote currency per base currency, rates continuous annual decimals, RR sign=call vol minus put vol, BF=(put+call)/2−ATM. Delta is not spot or premium-adjusted; vols are relative annual decimals.

### Limitations, failure and fallback

A paired10δ-only input passes validation but is ignored because `has_wings` tests25δ only. Extreme skew can produce crossed pillar strikes, with no general sorting in direct evaluation; materialization interpolator may reject nonmonotonic pillars. Direct expiry interpolation is not generally equal to interpolating materialized strike surfaces.

### Alternatives and tradeoffs

Premium-adjusted/spot delta and market-strangle BF conventions require other solvers and are not implemented choices here. Direct delta queries preserve quote-space evolution; materialization offers a reusable strike grid with interpolation changes.

### Implementation and validation evidence

[fx.rs](../../../../finstack-quant/models/src/volatility/fx.rs) and [fx_delta_vol_tests.rs](../../../../finstack-quant/models/tests/fx_delta_vol_tests.rs). The latter hand-builds RR/BF pillars but reuses public delta_to_strike, so it is partly shared-kernel evidence; short-expiry merged-grid and paired-quote tests cover regressions. Reference: [Garman–Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983) supports the underlying FX law, not all broker-quote conventions.

The shared FX valuation adapter currently uses expiry-date rates and generic
sigma lookup. Vanilla vanna and volga independently fetch a strike surface.
The existing FX chart uses the scalar query forward and spot-and-rates
materializer through WASM. See the [shared FX adapter](../../../../finstack-quant/valuations/fx/src/shared.rs),
[vanilla risk methods](../../../../finstack-quant/valuations/fx/src/fx_option/types.rs),
[Python binding](../../../../finstack-quant-py/src/bindings/models/volatility.rs),
[WASM binding](../../../../finstack-quant-wasm/src/api/models/volatility.rs) and
[FX surface chart](../../../../finstack-quant-ui/registry/models/components/fx-surface-chart/fx-surface-chart.tsx).

## Relationship to the accepted FX quote-source boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
requires one canonical validated raw source with an explicit ordered pair,
forward-delta/DNS/arithmetic-smile-BF profile and optional source clock. Model
adapters consume that source and qualified delivery forwards instead of the
duplicate raw quote/spot/rate builder. Share checked wing reconstruction, make
delta/strike transforms fallible and admit the model's supported strike order
before interpolation; positive raw wings alone do not establish that order.
Preserve every original supported strike pillar exactly through materialization,
without tolerance coalescing, and retain source/forward construction provenance.
Quote-space expiry evaluation and a sampled strike grid are distinct model
operations with independently qualified off-grid behavior and risk. Record 15
was accepted on **2026-11-08**, with reconciliation and qualification pending.
[MODELS-ANALYTIC-16](volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
owns the accepted generic resolution and derived strike-grid evaluation
contracts. This record owns the accepted FX quote-space construction and
sampling rules.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
