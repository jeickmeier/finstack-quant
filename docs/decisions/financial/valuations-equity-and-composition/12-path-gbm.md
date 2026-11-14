# VALUATIONS-EQUITY-AND-COMPOSITION-12: ATM term-GBM, observation grids and pinned spot risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-12`.
- **Decision status:** Accepted 2026-11-14 — complete ATM term-GBM, exact observations and pinned spot risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent dated carry, variance, payment and spot-risk examples checked. Repository pricing, replay, risk and host qualification remain pending.

## Accepted decision for checked ATM term-GBM and pinned equity path risk

1. **Select one dated live model and resolve known claims first.** Keep
   `atm_term_gbm` under `monte_carlo_gbm` for single-underlying
   discretely observed autocallables and cliquets. Require a qualified
   same-currency deterministic discount source and an explicit choice
   of no distributions or signed continuous dividend yield. Resolve
   eligible history and known claims before collecting live model
   inputs. Price known unpaid claims on their actual payment dates
   without running paths. Retain the autocallable's dated coupon and
   principal amounts and the cliquet's single payment under their
   owners. End live simulation at the last required underlying fixing
   or reset.
   [Accepted autocallable payoff](../models-monte-carlo/20-autocallable.md),
   [accepted cliquet payoff](../models-monte-carlo/21-cliquet.md).

2. **Use dated forwards and explicit model and quote clocks.** At
   each live observation date `d_i` obtain `D_i=D(V,d_i)>0`
   on the curve's native date basis. Factors above one are valid.
   Set `D_0=1`, `t_0=W_0=0`. At model time `t_i`, evaluate
   the ATM strike `F_i=S_live exp(−q t_i)/D_i` on the elected
   current-origin unshifted Black absolute-strike surface. Use its
   native expiry time `τ_i`, declared interpolation and strict
   support, and preserve `W_i=σ_i²τ_i`. An explicitly elected
   annual model-clock scalar uses its checked `σ² Δt_i` directly
   with no surface dependency. Use checked integrated carry
   `A_i=log(D_(i−1))−log(D_i)−q Δt_i` and integrated variance
   `V_i=W_i−W_(i−1)` for the surface route. Admit exact zero
   variance and reject negative or unresolved increments. Publish
   the schedule after complete admission. The selected model matches
   those forward and ATM variance checkpoints. Smile dynamics and
   path-dependent price accuracy require separate evidence.
   [Accepted exact term-GBM](../models-monte-carlo/04-lognormal-diffusions.md),
   [accepted model resolution](../valuations-framework/model-resolution.md),
   [accepted volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).

3. **Bind the exact event and coefficient grid.** Use the origin and
   the exact remaining observation or reset times, which also serve
   as the coefficient checkpoints. Bind contractual event identities
   to the actual stored nodes and retain identities where the product
   admits a shared coordinate. Resolve an eligible origin event
   separately. Reject a collapsed live horizon or geometry that
   conflates distinct required states. Remove tolerant matching,
   skipped checkpoints, the 252-steps-per-year default and the
   eight-step minimum. Over each interval use the checked exact
   GBM transition with log increment `A_i−V_i/2+√V_i Z_i`.
   Subsequent payment dates supply discount queries and no new
   underlying observations or coefficient checkpoints.
   [Accepted time grids](../core-numerics/time-grids.md),
   [Glasserman](https://doi.org/10.1007/978-0-387-21617-1).

4. **Prepare the contractual anchor and eligible fixing state once.**
   Require a finite positive fixing per required contractual
   observation identity, with source and publication qualification.
   Reject duplicate entries, including identical duplicates. A strike
   already set requires its qualified level even before the first
   payoff observation. Live spot may establish an anchor at an
   explicitly qualified current-cut strike-setting event. Remove
   generic same-day live spot substitution for consumed resets.
   If a product owner admits a modeled origin proxy, prepare it
   once at the unbumped cut, keep its scenario provenance and hold
   it fixed across the whole stencil. A future observation remains
   live. Records 13 and 14 own exact coupon, reset and horizon
   scenarios. Reconcile affected fixing-helper callers on their
   own product terms.
   [Accepted fixing authority](../core-market/scalar-series-and-fixings.md),
   [dated autocallable](13-autocallable.md),
   [dated cliquet](14-cliquet.md).

5. **Require explicit fixed-budget Monte Carlo and truthful results.**
   Require 2 through 10,000,000 independent groups, pairing, exact
   Philox seed or explicitly elected derivation, execution mode and
   a positive trajectory-step budget. Preflight actual groups times
   legs times intervals, including aggregate work for the requested
   risk stencil, before drawing. Check the actual draw and allocation
   requirements. Run the full budget or fail. Compute fixed-sample
   95% Student-t intervals from complete signed native-PV groups.
   Preserve literal zero dispersion and leave sampling uncertainty
   unavailable where a claim is resolved without paths. Apply an
   elected positive currency half-width threshold after the full
   price run. Retain actual grid, group and trajectory counts, exact
   seed and stream and failures through the existing Rust and host
   instrument routes.
   [Accepted estimation](../models-monte-carlo/01-engine-and-estimation.md),
   [accepted Philox streams](../models-monte-carlo/02-random-streams.md).

6. **Rebuild responsive ATM inputs while holding contractual state
   fixed.** On the surface route, same-cut spot or carry shocks
   recompute forwards and ATM checkpoint variance on the held
   surface. Keep the anchor and history fixed on the complete
   stencil. Treat frozen variance as a separately elected partial.
   Volatility shocks move only the elected scalar or declared
   exact native source nodes and rebuild the checked schedule.
   Require checked actual stencil coordinates and qualified common
   logical groups over all legs, with uncertainty on the complete
   within-group stencil. Qualify any changed-grid or dated coupling
   independently. Study finite-bump and sampling effects separately,
   including call and coupon thresholds.
   [Accepted selected-model risk](08-selected-risk-and-implied-vol.md),
   [paired Monte Carlo risk](../models-monte-carlo/25-finite-difference-greeks.md).

## Observed behavior

### Choice and rationale

Autocallable and cliquet contracts explicitly declare `AtmTermGbm`. Their shared adapter preserves rates/ATM total variance between product checkpoints using piecewise-constant GBM, then simulates exact lognormal steps within each interval.

### Scope and input domain

Single-asset path-dependent equity products with deterministic curves and continuous dividend yield. This is a term-volatility model, not local volatility or a full-smile calibration. Recorded fixings must be finite and positive.

### Algorithm

At checkpoint ti obtain rebased Di, forward `Fi=S exp(-q ti)/Di`, implied σi at `(ti,Fi)`, and total variance `Wi=σi²ti`. Interval rate is `log(Dprev/Di)/(ti−tprev)` and interval volatility `sqrt((Wi−Wprev)/(ti−tprev))`. Negative variance increments, invalid DFs and intervals at most `1e-6` years error; nonincreasing checkpoints are skipped by the shared bootstrap. Exact interval steps multiply spot by `exp((r−q−σ²/2)dt+σ√dt Z)`.

Product grids insert every future observation/reset time; checkpoints are sorted and deduplicated at `1e-10`. The adapter passes merged engine configuration, target CI half-width and Philox seed to `McEngine`. Spot delta/gamma/vanna first pin an omitted initial level to unbumped spot, preventing the contractual anchor from moving with each spot shock. Cliquet on a same-day reset pins that fixing instead. Shared fixing lookup returns the first exact-date entry; its helper does not reject duplicate dates.

### Selection, controls and conventions

Base path configuration reads [MC defaults](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json): 100,000 paths, parallel, 252 steps/year, minimum eight, chunk size 1,000. These products enable antithetics; path/antithetic overrides are merged. Path count zero means use default and count above 5,000,000 errors. Actual seed comes from model configuration/instrument identity, not the base config's example seed.

### Limitations, failures and alternatives

Required observation times avoid interpolating barrier observations, but the model omits smile-state dynamics and stochastic rates. Identical seeded bumps reduce simulation noise but do not remove discontinuous-payoff bias. Local-vol/stochastic-vol path models are possible alternatives, not implemented variants of the declared path model.

### Evidence

[Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo); shared [piecewise_gbm.rs](../../../../finstack-quant/models/src/monte_carlo/process/piecewise_gbm.rs) contains exact-boundary and deterministic-step identities. Product seasoning/grid evidence is linked in records 13–14. No new simulation or independent path benchmark was run.

## Independent numerical checks

- With spot 100, signed yield −.02, ACT/360 model time and
  ACT/365F quote time, the 90-, 360- and 540-day checkpoints use
  `D=(1.01,.96,.91)` and ATM sigma `(.30,.15,.18)`. The middle
  integrated variance increment is exactly zero. The zero-normal
  path ends at 110.5550671608. An illustrative strike-104 terminal
  claim paid at discount .88 is worth 5.7684591015 on that path.
- For an already-consumed cliquet reset with base level 110,
  contractual anchor 100, notional 100, unit payment factor and
  inactive caps, a moving reset proxy on live spots 109, 110 and
  111 gives PVs 9, 10 and 11. Holding the consumed reset fixed
  gives PV 10 across the stencil and zero spot delta.
- For a live one-reset cliquet with notional and anchor 100,
  unit horizon, fixing factor .97, yield .02 and payment factor
  .95, the spot stencil 99, 100 and 101 gives delta .57624710
  under responsive ATM sigma with center .20 and strike slope
  .001. Holding sigma fixed at .20 with the same anchor gives
  .53804030. The illustrative payoff is the positive part of
  the simple return and the independent price uses the Black law.

Production pricing, Philox replay, product risk and host suites
remain unrun for this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [piecewise_gbm.rs](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs)
- [spot_pinned.rs](../../../../finstack-quant/valuations/equity/src/spot_pinned.rs)
- [past_fixings.rs](../../../../finstack-quant/valuations/equity/src/past_fixings.rs)
- [lib.rs](../../../../finstack-quant/valuations/equity/src/lib.rs)
