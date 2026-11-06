# CORE-MARKET-15: FX ATM, risk-reversal, and butterfly quote admissibility

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`FxDeltaVolSurface` stores ATM, 25-delta risk reversal/butterfly and an optional paired 10-delta set. Constructor validation interprets each wing through the simple arithmetic smile-volatility decomposition. The documented convention is premium-unadjusted forward delta and delta-neutral-straddle ATM. The core container does not convert deltas to strikes, interpolate expiries, or calibrate a market-strangle butterfly.

## Algorithm, domain, and conventions

Expiries must be nonempty, finite, strictly positive and strictly increasing; all required arrays must match their count. ATM vols must be finite and positive. Risk reversals and butterflies may be signed but must be finite. For each expiry and each supplied delta pair, construct `sigma_call=ATM+BF+RR/2` and `sigma_put=ATM+BF-RR/2`; both must be finite and strictly positive. Optional 10-delta RR and BF must either both exist with matching length or both be absent. Deserialization reruns these checks.

The arithmetic means that RR is call volatility minus put volatility and BF is their average minus ATM, all in decimal volatility units. It is a **smile-vol butterfly** identity. A market-strangle quote requiring price-equivalent wing reconstruction is not validated by this identity. The structure carries no pair currencies, selectable delta/premium-adjustment/ATM-convention flags, premium currency, or discount curves. The linked evaluator implements the fixed documented convention; other broker conventions are not interchangeable inputs.

## Selection, limitations, and alternatives

There is no fallback from invalid wing quotes and no clipping: invalid inputs error. Single expiry is accepted. Calendar arbitrage, convexity, monotone total variance, smile-wing extrapolation and quote quality are not certified by positivity checks. `MarketContext` stores these quotes in a separate namespace; generic curve bump dispatch does not target them.

The downstream [FX delta smile evaluator](../models-analytic/fx-delta-smiles.md) owns interpolation and strike inversion under the fixed convention. A market-strangle quote representation or selectable convention metadata here would be additional design choices.

## Evidence

Owning code and inline constructor test: [fx_delta_vol_surface.rs](../../../../finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs). Evidence validates array/domain consistency and arithmetic wing positivity; it is not vendor-smile accuracy validation. References: [Clark](../../../REFERENCES.md#clark-fx-options), [Wystup](../../../REFERENCES.md#wystup-fx-options). Full model evaluation belongs to the linked evaluator record; no algorithm inspection in this owned quote-storage file is deferred.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
