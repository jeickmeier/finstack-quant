# VALUATIONS-EQUITY-AND-COMPOSITION-09: Equity Black–Scholes and Heston PDE adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-09`.
- **Decision status:** Accepted 2026-11-14 — complete equity Black–Scholes and Heston PDE package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent deterministic, 1D grid and Heston Fourier reference checks completed. Production PDE pricing, risk and host qualification remain pending.

## Accepted decision for checked equity Black–Scholes and Heston PDE pricing

1. **Select the live model explicitly and resolve the contract first.**
   Keep `pde_crank_nicolson_1d` for European and qualified American
   exercise and `pde_adi_2d` for European Heston. The 1D route
   excludes live Bermudan exercise and American cash-dividend
   detachments during the remaining exercise window. The Heston
   route excludes future cash-dividend detachments through fixing.
   Resolve exercise, expiry and outstanding settlement before
   collecting live inputs. Unsupported terms, unresolved arithmetic
   and solve failures propagate through the selected model.
   [Accepted equity lifecycle](06-option-lifecycle.md),
   [accepted coefficient bridges](../models-numerical/black-scholes-and-heston-pde-bridges.md).

2. **Separate fixing carry, dividend entitlement and payment.**
   Require positive finite fixing time `T` on the declared model
   clock. Use the original dated discount `D(V,E)`, positive and
   finite, and its equivalent continuous rate `r=−log(D(V,E))/T`.
   Use the elected signed continuous yield on that clock, or the
   admitted no-distribution basis. The constant-rate approximation
   matches fixing discounting. Qualify intermediate carry and
   American exercise. The 1D European route also admits record 07's
   expiry-window cash escrow with exact entitlement phases, actual
   dividend payment dates and a positive escrow-adjusted spot. Use
   volatility qualified for that underlying and preserve total
   variance when mapping its admitted source clock. For qualified
   European cash payment at `P`, apply `D(V,P)/D(V,E)` once to the
   fixing-discounted unit premium and both bounds, using checked
   complete arithmetic. Admit physical terms only on qualified
   equivalence to the modeled exercise payoff.
   [Accepted European carry and settlement](07-black-tree-and-direct-greeks.md),
   [accepted clock and fixing rules](../conventions/0045-financial-clocks-and-date-boundaries.md).

3. **Use the 1D damped Crank–Nicolson profile and its wider domain.**
   Use 200 total sinh nodes centered at log strike, with intensity
   `.1`, and 100 uniform time intervals. The log-spot domain spans
   both actual log spot and log strike with margin
   `5w+|(r−q)T|+w²/2` on each side, where `w=σ√T`. Check that
   both coordinates lie strictly inside the actual represented grid.
   Run four full backward-Euler intervals before Crank–Nicolson.
   Admit the fixed immediate intrinsic exercise profile only where
   the prepared receiving knots represent the American eligibility
   phases. Apply post-step projection at those knots and check an
   eligible immediate exercise against the interpolated value at
   actual spot. This is a discrete stopping approximation. At
   exactly zero Black volatility, use the deterministic European
   discounted payoff or the American maximum over admitted
   endpoints and any eligible interior stationary time.
   [Accepted time march](../models-numerical/theta-time-march-and-thomas.md),
   [accepted exercise projection](../models-numerical/pde-exercise-projection.md).

4. **Use Heston's own parameter block, geometry and variance limit.**
   Require the five coherent risk-neutral scalars `v0`, `κ`, `θv`,
   `σv`, `ρ` from the qualified authored or fitted block on
   the declared model clock. Admit `v0=0` and Feller
   violations. Remove the Black-volatility lookup. Exactly
   zero vol-of-vol uses the checked Black expectation with
   deterministic integrated variance. Positive vol-of-vol uses
   200 log-spot nodes, 80 variance nodes and 100 uniform parent
   intervals, with Modified Craig–Sneyd `θ=1/3` and two split
   backward-Euler startup parent intervals. From the span of actual
   log spot and log strike, use lower and upper margins
   `max(log 20,6√I)` and `max(log 10,6√I)`, respectively, plus
   `|(r−q)T|+I/2` on each side, where `I` is mean integrated
   variance. Keep the log-spot sinh center at log spot with intensity
   `.1 log(200)/(x_max−x_min)`, and variance center at
   `min(v0,θv)` with intensity `.025`. Start the variance axis at
   exact zero and end at
   `max(1.5,5θv,1.5v0,max(v0,θv)+6s_v)`, where `s_v` is the
   checked CIR standard-deviation bound from the separate maxima of
   the two variance terms over the horizon. With
   `d=−expm1(−κT)` and `a=d(1−d)` for `d≤.5` or `.25`
   otherwise, its square is `σv²/κ × (v0 a+θv d²/2)`.
   Evaluate the complete geometry and require `v_max>v0`.
   Use Linear reconstruction at both variance edges and the accepted
   corner rule. These margins are finite-domain heuristics.
   [Accepted Heston coefficient and variance-edge laws](../models-numerical/black-scholes-and-heston-pde-bridges.md),
   [accepted Heston input resolution](../valuations-framework/model-resolution.md),
   [accepted ADI profile](../models-numerical/mcs-adi-time-march.md),
   [Heston](https://doi.org/10.1093/rfs/6.2.327).

5. **Check the unit price before applying signed quantity.** Use
   the checked grid extraction at actual log spot and, for Heston,
   actual `v0`. Use zero on the out-of-the-money spot edge and
   LinearInExp on the opposite edge. For prepaid equity `A` and
   `B=K D(V,E)`, require a European call in `[(A−B)⁺,A]` or a
   put in `[(B−A)⁺,B]` on the appropriate payment basis. For the
   admitted American route take the greater of the European
   lower bound and eligible immediate intrinsic. The signed-carry
   upper bound is `S exp(max(0,−qT))` for calls and
   `K exp(max(0,−rT))` for puts. An unresolved comparison or
   resolved violation fails pricing. Remove the Heston epsilon
   clamp. Scale once by signed quantity and convert to Money.
   [Accepted boundaries](../models-numerical/pde-boundary-elimination.md),
   [accepted interpolation](../models-numerical/pde-grids-interpolation-and-greeks.md).

6. **Qualify price and risk separately on the selected model.**
   Use the checked record 08 reprices on the admitted risk axis,
   retaining actual bump widths and grid geometry. A defined
   Black-only shock can report structural zero with fixed Heston
   inputs. A missing or unsupported direction is unavailable.
   Heston parameter and recalibration directions are separate.
   Qualify time, spot and variance spacing, finite spot boundaries,
   upper variance truncation and actual bump widths separately,
   including short times, wings, signed carry, starting at and
   reaching zero variance, both correlation signs and eligible
   American stopping. Compare to independent same-law Black,
   Heston and American references with matching clocks, payoffs
   and carry. Retain checked controls, provenance and failures in
   Rust and propagate them through existing Python and WASM routes.
   [Accepted selected-model risk](08-selected-risk-and-implied-vol.md),
   [published Heston ADI benchmark](https://doi.org/10.4208/ijnam.v7.n2.p303).

## Observed behavior

### Choice and rationale

PDE valuation is explicitly selected. The adapters choose concrete domains/grids and interpolation points before calling shared 1D theta or 2D modified Craig–Sneyd solvers; these domain choices materially affect instrument values.

### Scope and input domain

1D Black–Scholes supports European and American options; Bermudan is rejected, as is American exercise with future cash dividends. European discrete dividends use escrowed spot. 2D Heston supports European exercise without future cash dividends and requires strict market Heston parameters.

### Algorithm

For 1D, use a log-spot sinh grid with 200 points concentrated at log strike. The domain spans the lesser/greater log spot/strike plus/minus `5σ√T+|(r−q)T|`. Solve with four Rannacher startup intervals and 100 time steps; American exercise supplies interior intrinsic payoff constraints. Interpolate at log spot and scale by quantity. At zero volatility, bypass the PDE: maximize discounted deterministic payoff at maturity, also at zero and any interior stationary time for American exercise.

For 2D, use 200 log-spot and 80 variance points, 100 time steps and two startup damping intervals. Log tails span at least spot factors 0.05 to 10 and widen to six standard deviations of mean integrated variance. The variance upper bound is the maximum of 1.5, five times long-run variance, 1.5 times initial variance and a six-sigma variance-tail bound; grids concentrate around log spot and min(initial,long-run variance). Solve Heston ADI and interpolate at `(log S,v0)`. Reject results outside `[0,S exp(-qT)]` for calls or `[0,K exp(-rT)]` for puts except `128×epsilon×max(bound,1)` roundoff, which is clamped.

### Selection, controls and conventions

Registry keys are `PdeCrankNicolson1D` and `PdeAdi2D`. Counts are adapter defaults; ordinary instrument model overrides do not expose these grid sizes. Both use expiry-equivalent constant r/q; lifecycle claims resolve before PDE setup.

### Limitations, failures and alternatives

The 1D adapter does not apply the explicit European arbitrage-bound check used by 2D. Fixed default grids can fail or be inaccurate in extreme tails and short expiries. A bounded result is not convergence evidence. Black/tree, Fourier and MC engines are implemented alternatives; no automatic fallback substitutes them on a PDE failure.

### Evidence

[ADI reference](../../../REFERENCES.md#in-t-hout-welfert-2009); inline cases in [pde.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/pde.rs) and [pde2d.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/pde2d.rs) and [test_alt_models.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_alt_models.rs) locate deterministic-limit, grid-bound and cross-engine comparisons. Cross-engine agreement can share input assumptions; production convergence budgets remain unqualified.

## Independent numerical checks

- A European unit call with spot 100, strike 105, half-year horizon,
  volatility .20, a dividend of 3 discounted at .94, fixing discount
  .96 and payment discount .93 gives adjusted spot 97.18 and price
  3.8318908408 analytically. A separate 200-node, 100-interval
  calculation using the accepted 1D margin and startup gives
  3.8322777910. The matching put gives 7.3387658408 analytically and
  7.3389937581 on that grid.
- With spot and strike 100, rate .05, zero continuous yield,
  one-year maturity and volatility .20, a standalone American put
  projection gives 6.0828486268 at 200 by 100 and 6.0896907601 at
  1,200 by 1,000. A separate 3,000-step CRR tree gives 6.0901170825.
  At exactly zero volatility, the American call with spot 180,
  strike 100, rate .10, yield .05 and a four-year horizon has
  interior stationary time 2.1072103132 years with value 81.
- Independent Fourier quadrature reproduces the published Heston
  Case 1 call at 8.8948693601. This is a comparator for the
  stochastic law. The selected 2D domain and edge closure require
  independent PDE refinement.

Repository production PDE, risk and host suites remain unrun for this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [pde.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/pde.rs)
- [pde2d.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/pde2d.rs)
