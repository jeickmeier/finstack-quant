# VALUATIONS-FX-AND-EXOTICS-COMMODITY-ASIAN: Commodity Asian exact geometric and arithmetic moment matching

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-ASIAN`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Discrete commodity Asian options use the full forward-price fixing grid. Geometric averages have an exact lognormal formula under a shared Brownian factor; arithmetic averages use a two-moment lognormal approximation with seasoning adjustment.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Validate every fixing on/before valuation has an exact realized value; future fixings are strictly after valuation. Let total count `n`, future count `m`, realized sum `H`, realized log-sum `L`, future pairs `(t_i,F_i)`, and expiry discount `D`. Fully fixed options use realized average intrinsic discounted to expiry; after expiry return zero. A live option queries one `σ` at expiry/strike and requires positive finite future forwards.

For geometric averaging, `μ=(L+Σ ln F_i-σ²Σt_i/2)/n` and `v=σ²Σ_iΣ_j min(t_i,t_j)/n²`. Expected geometric average is `G=exp(μ+v/2)`. Price `D[GΦ(d1)-KΦ(d2)]`, `d1=(μ+v-ln K)/√v`, `d2=d1-√v`; put is signed form. Degenerate variance/forward/time falls back to deterministic geometric-forward intrinsic.

For arithmetic averaging, effective future strike `K*=(nK-H)/m` and scale `m/n`. Future mean `M1=ΣF_i/m`; second moment `M2=Σ_iΣ_j F_i F_j exp(σ² min(t_i,t_j))/m²`. Match lognormal variance `v=ln(M2/M1²)`, price a Black-style option on mean `M1` against `K*`, multiply by scale and discount, and clamp final price nonnegative. Nonpositive `K*` makes a call linear in expected full average and a put zero. Nonpositive variance falls back to intrinsic.

Delta bumps the entire price curve multiplicatively and divides by the bump in the arithmetic mean future forward; realized fixings stay fixed. With no future fixings delta is zero. Vega uses active-volatility finite differences; generic discount DV01/key-rate risk delegates to shared methods.

## Selection and controls

The family/default model key is `AsianTurnbullWakeman` for the instrument, but the local kernel branches on `averaging_method` to exact geometric or arithmetic moment matching. This label does not imply the geometric branch is approximate.

Strike/quantity positive; dates nonempty, strictly increasing and no later than expiry; fixings unique and scheduled. Geometric realized fixings must be positive; arithmetic realized fixings need only be finite. Future lognormal prices must be positive. Default time Act365F.

## Limitations, failure and fallback

All future fixing correlations are induced by a single Brownian path with constant volatility: separate delivery forward dynamics are not fitted. Arithmetic moment matching is an approximation, including for skewed distributions, long windows and seasoned strikes. Same-day fixing is mandatory here, unlike the commodity future/swap convention. Deterministic fallbacks can hide ill-conditioned moments.

## Alternatives and tradeoffs

Full multi-factor commodity MC or conditional Asian approximations could improve dynamics/accuracy. Exact geometric pricing and arithmetic approximation are supported; neither establishes market calibration of the shared-forward covariance assumption.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/greeks.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/greeks.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/mod.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/mod.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Kemna Vorst](../../../REFERENCES.md#kemna-vorst-1990); [Turnbull Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
