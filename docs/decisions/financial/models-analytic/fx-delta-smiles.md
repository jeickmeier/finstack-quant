# MODELS-ANALYTIC-17: FX RR/BF conversion, delta strikes and smile interpolation

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Positive spot, positive ascending expiries, positive ATM vol, finite domestic/foreign rates, paired 25δ and optionally10δ risk-reversal/butterfly arrays.

## Choice and rationale

Use premium-unadjusted forward delta, delta-neutral-straddle ATM strike, and smile-butterfly arithmetic wing quotes. This does not solve a market-strangle premium equivalence problem.

## Algorithm

Forward=S exp[(rd−rf)T]; wing σcall=ATM+BF+RR/2,σput=ATM+BF−RR/2. K(δ)=F exp[−N⁻¹(δ)σ√T+σ²T/2]; put |δ| uses call-equivalent1−|δ|. ATM DNS strike=F exp(σATM²T/2). Build3/5 pillars per expiry, merge all strike grids (absolute dedup1e−10), interpolate each expiry using its own pillars linearly in strike with flat endpoints. ATM-only creates constant-vol rows. Direct delta-surface evaluation first linearly interpolates/clamps ATM,RR,BF in expiry, constructs query-expiry pillars using supplied forward, then linearly interpolates/clamps strike.

## Selection and controls

Builder 25δ presence determines wing mode; 10δ quotes are used only inside that branch. Public `delta_to_strike` and `strike_to_delta` are scalar unchecked transforms. Materialization derives forwards from constant rates; direct evaluation accepts current forward. Surface interpolation after materialization follows stored surface mode.

## Conventions

FX spot/strike in quote currency per base currency, rates continuous annual decimals, RR sign=call vol minus put vol, BF=(put+call)/2−ATM. Delta is not spot or premium-adjusted; vols are relative annual decimals.

## Limitations, failure and fallback

A paired10δ-only input passes validation but is ignored because `has_wings` tests25δ only. Extreme skew can produce crossed pillar strikes, with no general sorting in direct evaluation; materialization interpolator may reject nonmonotonic pillars. Direct expiry interpolation is not generally equal to interpolating materialized strike surfaces.

## Alternatives and tradeoffs

Premium-adjusted/spot delta and market-strangle BF conventions require other solvers and are not implemented choices here. Direct delta queries preserve quote-space evolution; materialization offers a reusable strike grid with interpolation changes.

## Implementation and validation evidence

[fx.rs](../../../../finstack-quant/models/src/volatility/fx.rs) and [fx_delta_vol_tests.rs](../../../../finstack-quant/models/tests/fx_delta_vol_tests.rs). The latter hand-builds RR/BF pillars but reuses public delta_to_strike, so it is partly shared-kernel evidence; short-expiry merged-grid and paired-quote tests cover regressions. Reference: [Garman–Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983) supports the underlying FX law, not all broker-quote conventions.

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
was accepted on **2026-11-08**, with reconciliation/qualification pending; this
record's full interpolation/tail model remains Draft.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
