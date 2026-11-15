# VALUATIONS-FX-AND-EXOTICS-COMMODITY-ASIAN: Commodity Asian exact geometric and arithmetic moment matching

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-ASIAN`
- Decision status: **Accepted 2026-11-14** — complete commodity Asian terms, fixing, pricing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent exact geometric, arithmetic approximation, fixed cash and source-risk checks completed. Runtime, fixing and market data, approximation domain and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity Asian pricing and settlement

1. **Book the fixed-strike European cash option.** Require explicit holder
   long or short, call or put, arithmetic or geometric averaging, finite
   positive strike and quantity, and exact commodity benchmark, reference
   contract or roll, currency and units. Book a nonempty strictly ordered
   fixing schedule with equal weight for each slot. Specify the automatic
   payoff rule. Keep final observation, determination and payment on their
   actual dates and phases, including ordered events on one civil date.
   Any booked premium is a separate trade cashflow.
   [ICE Asian cash profile](https://idd.ice.com/CM/CMHelp/Content/FM/Asian.htm).

2. **Resolve exact authorized fixings.** Bind each slot's benchmark,
   reference, source version, session and publication cut. Consume the
   exact official fixing once eligible, including on the valuation date.
   A missing eligible fixing or unknown qualification errors. An explicitly
   unpublished slot remains pending. Require the elected matched source
   on the original price-curve clock and a qualified strictly positive
   model coordinate for each pending projection. A pending slot without
   an admitted forecast basis makes live pricing unavailable. Preserve all
   slots in the denominator. Arithmetic observed prices may be finite and
   signed or zero. Geometric observations and projected lognormal levels
   must be finite and positive.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [price curves](../core-market/price-curves.md).

3. **Prepare one shared law on the actual payment basis.** Each pending
   forward is the expected fixing under the common pricing law with
   deterministic discounting to the actual payment. Select one compatible
   Asian volatility quote for payoff, expiry and strike. Preserve total
   variance when mapping quote to model expiry time and use that constant
   annual volatility with one shared Brownian factor at the exact fixing
   times. Qualified model-time ties retain each slot and share the shock.
   Pass the prepared observed and projected fixings and dated payment factor
   to the checked shared kernel. Discount once and apply quantity and holder
   sign once. [Discrete Asian pricing](../models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing),
   [volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).

4. **Use separate model keys for exact geometric and fitted arithmetic.**
   Use `asian_geometric_bs` for the exact geometric law and
   `asian_turnbull_wakeman` for the arithmetic two-moment fit. Reject a
   conflicting model selection. In the arithmetic law preserve the known
   contribution `H=Σ known fixings / N` and fit only the future contribution
   against `K−H`. A nonpositive effective strike gives the exact linear call
   and zero put. Zero volatility or fully observed history gives exact
   deterministic intrinsic. A single distinct projected time gives exact
   shifted-lognormal arithmetic pricing. Distinct stochastic times generally
   require the approximation. Unresolved moments and detectable loss of
   positive variance error. Reconcile the duplicated local formulas to the
   shared checked law.
   [Turnbull and Wakeman](https://www.cambridge.org/core/journals/journal-of-financial-and-quantitative-analysis/article/abs/quick-algorithm-for-pricing-european-average-options/FDAE4E4B6A0FBE8556C94B6B85E44D07).

5. **Retain fixed cash until discharge.** Once complete authorized fixings
   and contractual determination establish the signed claim, value it to
   its actual payment date. Eligible same-day unpaid cash uses factor one.
   Partial, failed or overdue cash requires a qualified residual. Resolve
   known amounts before inactive forwards or volatility. Export one signed
   `DerivativeSettlement` for the fixed claim, with its observation
   provenance. Complete cash export is unavailable while the payoff is
   contingent. Confirmed zero payoff or discharge permits an empty residual.
   Use the same canonical Rust state through the existing Python and WASM
   instrument envelopes. [Cashflow export](../valuations-framework/cashflow-export.md).

6. **Report risk on the matched active sources.** `delta` is a directional
   secant under matched relative source-pillar shocks. Hold authorized
   fixings and other pricing inputs fixed, evaluate the same pending
   slots and divide the signed PV difference by the actual change in mean
   pending forward. Require resolved ordered moves on both sides. Vega
   reprices the active volatility source per `+0.01` native annual volatility.
   Parallel and bucketed DV01 use the actual payment discount curve. Fixed
   cash has zero commodity delta and vega and retains applicable discount
   and timing risk. Horizon theta uses qualified crossed observations and
   eligible signed cash. [Option sensitivities](../valuations-framework/option-sensitivities.md),
   [dated curve risk](../valuations-framework/finite-difference-rates.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- With observed fixing `68`, projected forwards `75` and `92` at `0.25`
  and `0.75` model years, annual volatility `0.35`, strike `80`, quantity
  `600` and payment factor `0.984`, the geometric call PV is USD
  `1,827.01479630`. The arithmetic fit gives USD `2,491.85791068` and
  direct conditional Gaussian integration under the same joint law gives
  USD `2,482.64105049`.
- Authorized fixings `64`, `80` and `100` against strike `80` on quantity
  `3,000` fix USD `4,000` arithmetic call cash and zero geometric call
  cash. The arithmetic cash is worth USD `3,964` with post-expiry payment
  factor `0.991`. An eligible unpaid due-day cut includes USD `4,000`.
- With observed `120`, pending forwards `90` and `110` at `0.25` and
  `0.75` years, strike `75`, annual volatility `0.35`, payment factor
  `0.985` and quantity `1,000`, a matched `±1%` curve scale moves the
  mean pending forward from `100` to `99` and `101`. Geometric and fitted
  arithmetic call deltas are USD `682.99251432` and `655.79428591` per
  unit of that mean, respectively.

## Observed behavior

### Choice, rationale and scope

The inspected commodity Asian uses a shared Brownian factor with a local
geometric formula or arithmetic lognormal moment fit.

### Algorithm and conventions

Validate every fixing on/before valuation has an exact realized value; future fixings are strictly after valuation. Let total count `n`, future count `m`, realized sum `H`, realized log-sum `L`, future pairs `(t_i,F_i)`, and expiry discount `D`. Fully fixed options use realized average intrinsic discounted to expiry; after expiry return zero. A live option queries one `σ` at expiry/strike and requires positive finite future forwards.

For geometric averaging, `μ=(L+Σ ln F_i-σ²Σt_i/2)/n` and `v=σ²Σ_iΣ_j min(t_i,t_j)/n²`. Expected geometric average is `G=exp(μ+v/2)`. Price `D[GΦ(d1)-KΦ(d2)]`, `d1=(μ+v-ln K)/√v`, `d2=d1-√v`; put is signed form. Degenerate variance/forward/time falls back to deterministic geometric-forward intrinsic.

For arithmetic averaging, effective future strike `K*=(nK-H)/m` and scale `m/n`. Future mean `M1=ΣF_i/m`; second moment `M2=Σ_iΣ_j F_i F_j exp(σ² min(t_i,t_j))/m²`. Match lognormal variance `v=ln(M2/M1²)`, price a Black-style option on mean `M1` against `K*`, multiply by scale and discount, and clamp final price nonnegative. Nonpositive `K*` makes a call linear in expected full average and a put zero. Nonpositive variance falls back to intrinsic.

Delta bumps the entire price curve multiplicatively and divides by the bump in the arithmetic mean future forward; realized fixings stay fixed. With no future fixings delta is zero. Vega uses active-volatility finite differences; generic discount DV01/key-rate risk delegates to shared methods.

### Selection and controls

The family/default model key is `AsianTurnbullWakeman` for the instrument, but the local kernel branches on `averaging_method` to exact geometric or arithmetic moment matching. This label does not imply the geometric branch is approximate.

Strike/quantity positive; dates nonempty, strictly increasing and no later than expiry; fixings unique and scheduled. Geometric realized fixings must be positive; arithmetic realized fixings need only be finite. Future lognormal prices must be positive. Default time Act365F.

### Limitations, failure and fallback

All future fixing correlations are induced by a single Brownian path with constant volatility: separate delivery forward dynamics are not fitted. Arithmetic moment matching is an approximation, including for skewed distributions, long windows and seasoned strikes. Same-day fixing is mandatory here, unlike the commodity future/swap convention. Deterministic fallbacks can hide ill-conditioned moments.

### Alternatives and tradeoffs

Full multi-factor commodity MC or conditional Asian approximations could improve dynamics/accuracy. Exact geometric pricing and arithmetic approximation are supported; neither establishes market calibration of the shared-forward covariance assumption.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/greeks.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/greeks.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/mod.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/metrics/mod.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated examples. Production qualification is pending.

Canonical reference index. [Kemna Vorst](../../../REFERENCES.md#kemna-vorst-1990), [Turnbull Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
