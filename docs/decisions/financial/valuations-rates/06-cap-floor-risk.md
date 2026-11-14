# VALUATIONS-RATES-06-CAP-FLOOR-RISK: Cap/floor inverse volatility and risk differentiation

- ID: VALUATIONS-RATES-06-CAP-FLOOR-RISK
- Decision status: **Accepted — complete checked cap and floor risk and implied volatility package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, model, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cap and floor risk and implied volatility

1. **Prepare one valuation and identify the risk experiment.** Use the
   [accepted pricing package](05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing)
   for signed holder cash, eligible dates, exact typed sources, fixing
   identities and authority, explicit dispatch, and the raw native-currency
   mark. Freeze the effective date, cuts, economics and selected model for
   local and curve risk. Each experiment declares its moved and held inputs;
   reuse the checked baseline and change only the permitted inputs.
   A fully fixed unpaid optionlet has no live forward or volatility exposure
   and may retain discount exposure, as may an outstanding premium. An
   entitled same-day cash amount has factor one. A settled or unentitled
   amount contributes no risk on this cut. Prove structural zeros without
   fetching inactive data. A required missing input, unsupported active
   basis, unavailable derivative or failed reprice cannot become zero,
   a partial total or a substitute model.
   [Shared option risk](../valuations-framework/option-sensitivities.md).

2. **Use a held-quote forward delta and gamma for Black, shifted Black
   and Normal.** Shift every distinct unresolved benchmark fixing
   quote by the same decimal rate `x`; retain any repeated use of that quote.
   Hold known observations, contractual spread, effective strike, eligible
   premium, discount, expiry and evaluated volatility and displacement fixed.
   For a live term optionlet the quote forward has `R′=1, R″=0`. For a
   compounded coupon use the exact accepted growth, including each use of a
   cutoff quote. For growth factor `g_i` set `b_i=w_i` for an unresolved use
   and zero for a known use. From `(P,P′,P″)=(1,0,0)` update simultaneously
   by `(P g_i, P′ g_i+P b_i, P″ g_i+2P′ b_i)`. Divide the final
   derivatives by contractual accrual `A` to get `R′,R″`. Sum
   `W U_F R′` for delta and `W [U_FF(R′)²+U_F R″]` for gamma,
   with `W=N A D(V,Pmt)` and the accepted unit kernel `U`.
   Report native currency per one decimal rate and per its square,
   respectively. This is a local partial through the coupon; a floor's
   compounded gamma can have either sign. Inherit the whole-bundle Greek
   availability of the kernel, including unavailable live zero-sigma
   Greeks. [Overnight projection](../valuations-framework/overnight-projection.md),
   [forward Greek bundles](../models-analytic/forward-normal-shifted.md) and
   [Strata unit and PV optionlet Greeks](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/capfloor/VolatilityIborCapletFloorletPeriodPricer.html).

3. **Declare the Hull–White rate axis.** Keep the qualified selected-model
   curve delta/gamma experiment under
   [framework 11](../valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).
   Require an exact active typed discount or projection evaluator and a
   represented decimal parallel zero-rate step. For an expressly shared
   evaluator the shock reaches both roles. Hold one coherent selected model
   block and its origin fixed; rebuild the permitted projections, discount
   factors and term basis or overnight moment match with the accepted
   route. Use the matched raw center and both full sides with the
   [framework 09](../valuations-framework/finite-difference-rates.md)
   base-local slope and curvature. Require qualified reuse of the block on
   the shocked curves. A required refit belongs to a separate authorized
   experiment. Identify this output as curve delta/gamma in decimal
   coordinates, with the actual scope and premium treatment. A
   projected-forward partial for Hull–White is unavailable without its own
   qualified provider. Remove the implicit forward-ID bump and provider
   switching.

4. **Define quote vega on a compatible native volatility direction.**
   For `vega` on quote models move the evaluated annual quote sigma in its
   declared convention and clock, with the forward, strike, expiry,
   displacement and discount held. Use `W U_σ` for a model-clock quote
   derivative, multiplying by `sqrt(T_source/T_model)` for a supported
   source-clock bridge. Sum only a compatible declared direction; report
   currency per `q=.01` annual relative sigma for Black and shifted Black
   or per required explicit absolute annual-rate increment for Normal
   (e.g. `.0001` for one Normal volatility bp). Apply `q` once. A live
   unavailable kernel result remains unavailable. Grid risk, when
   supported, is a separately identified direct shock of exact active
   stored native sigmas, with explicit step and held quote-mapping policy.
   Use framework 11's sparse source-qualified nodes, matched union
   comparator and raw residual for any published bucketed vega. Replaying
   market quotes, including fitted SABR or Hull–White quote risk, requires
   a separate qualified center and full side replay of the actual fit and
   quote mapping. Do not infer it from data presence or treat it as an
   evaluated-sigma partial.
   [Native grids](../core-market/volatility-grids-and-shocks.md),
   [Strata distinguishes volatility parameter and rates risk](https://strata.opengamma.io/apidocs/com/opengamma/strata/measure/capfloor/IborCapFloorTradeCalculations.html).

5. **Keep Hull–White short-rate sigma vega separate.** Remove the
   automatic Normal-surface bump as `vega` under Hull–White. Retain
   `hw_sigma_vega` for the active scalar or one parallel move of every
   ordinate in the admitted active schedule, with mean reversion, clock,
   schedule knots, curves and the fit held. Require a finite positive
   calculation step and verified represented scalar widths or common
   schedule amplitudes. Report signed native currency per `.01` absolute
   annual short-rate sigma under the checked framework 11 vega stencil.
   Use both full sides when admitted, including a literal zero lower
   endpoint. Use the full up move and matched center only when the full
   lower move would leave the nonnegative sigma domain. No flooring,
   step reduction, or fallback to one-sided pricing when a required side
   is missing, ineffective or fails. Fixed eligible cash and premium have
   no direct sigma exposure.

6. **Separate direct curve risk from market quote replay.** Use `dv01`
   for combined parallel direct risk, `pv01` for typed per-source
   parallel rows and their checked sum, and `bucketed_dv01` for exact
   triangular direct key-rate experiments. Report signed native currency
   per positive bp using the represented central difference. A shared OIS
   evaluator receives one shock through all bound roles. Retire the
   cap/floor `forward_pv01` and discount-DV01-as-`rho` registrations;
   `pv01` rows identify the source and roles, including any joint role.
   Reprice the same holder mark, retaining active quote sources and fitted
   parameters, with any qualified forward-dependent quote coordinate
   reevaluated on the declared mapping. Held evaluated quote risk is the
   separate local experiment in item 2. Report actual source, scope,
   stencil and quote behavior. Replay needs an explicit separate basis,
   exact authorized targets and native quote units, an admitted original
   center and all required refits on both sides. Recipes do not select
   replay, and a failed replay cannot fall back to direct risk.
   [Shared rate risk](../valuations-framework/finite-difference-rates.md),
   [calibration replay](../core-market/calibration-provenance.md) and
   [Strata calibrated and market-quote PV01](https://strata.opengamma.io/apidocs/com/opengamma/strata/measure/Measures.html).

7. **Measure theta across the requested holder horizon.** Require an
   explicit closing date or positive calendar tenor and both cuts under
   [framework 13](../valuations-framework/carry-theta-breakeven.md).
   Remove the 1D default and fixing/payment clamp. Freeze eligible opening
   history. On the declared opening-forward scenario only, materialize
   admitted unresolved term or unique overnight observations privately
   when needed at the closing cut, retaining provenance and repeated uses.
   Fail missing required history. Condition the active dated market at
   the actual closing date and require qualified quote aging or conditional
   model use where live exposure remains. Derive closing PV and period cash
   from the same scenario. Include signed eligible optionlet and premium
   settlements exactly once, including the opening/closing phase rules;
   outstanding cash stays in PV until its eligible settlement cut.
   Report `theta=C+V_close−V_open`, `theta_carry=C`,
   `theta_roll_down=V_close−V_open` and the exact elapsed civil days under
   `theta_period_days`. Cash is nominal signed cash; local diffusion
   theta is a separate measure. Return zero closing PV after final
   settlement only when no claim remains.
   [Hypothetical fixing materialization](../cashflows/16-fixing-materialization.md).

8. **Invert one live caplet or floorlet premium explicitly.** Provide a
   standalone `implied_vol` product operation, with its Python and WASM
   twins. Require exactly one included, unresolved Caplet or Floorlet,
   a supported positive quote-model horizon, positive finite `N,A,D`,
   an explicit Black, shifted Black or Normal convention with its clock
   and applicable shift, and a finite nonnegative same-currency `Money`
   target for gross as-of option PV. Use record 05's quote coordinates,
   including the spread translation before any displacement. Keep those inputs
   fixed and solve for annual sigma on the declared admitted quote-model
   clock. Pricing an opening net mark or resolving its market sigma is
   unnecessary. Fixed, settled, unentitled, date-only unresolved same-day
   and multi-optionlet cases are unavailable. A supported intraday live
   quote still requires a positive qualified clock.
   Remove the cap/floor generic premium-override metric that first prices
   a baseline. Preserve other products' independent quote routes and
   keep aggregate cap-vol fit and stripping with
   [calibration](../calibration-rate-models/02-cap-schedules-and-quote-inversion.md).

9. **Qualify inverse bounds, controls and the actual result in cash.** Use
   the accepted [bounded model inverse](../models-analytic/implied-volatility-solvers.md)
   and [supplied-bracket Brent](../core-numerics/brent-root.md).
   With `W=N A D(V,Pmt)`, compute the exact gross intrinsic bound in the
   original coordinates. Check Black's exclusive upper bound first:
   `W(F+shift)` for a caplet and `W(K_quote+shift)` for a floorlet
   (zero shift for unshifted Black). Normal has no finite model price
   upper bound. Reject below intrinsic; exact intrinsic on an eligible
   live case is the sole zero-volatility result. Require all five
   `IvControls` fields: `max_vol`, `vol_tol`, `max_iterations`,
   `price_abs_tol` in the target currency, and dimensionless
   `price_rel_tol`. Solve only in `[0,max_vol]` on the shared finite
   log-OTM objective. Use original-coordinate, checked stable cash
   comparisons before output rounding. Preserve the supplied cash target
   under [Money arithmetic](../core-conventions/012-money-decimal-precision.md)
   and positive time value independently of rounded intrinsic.
   On every exit check the actual returned annual sigma,
   gross PV and independently evaluated same-law OTM time value against
   their paired absolute-plus-relative gates in original currency.
   Preserve typed failures for unavailable results, invalid domains,
   unresolved precision, search limits and failed reprices. Remove
   automatic ceilings, notional-only residuals, NaN transport and fixed
   or settled zero sentinels. Solver acceptance certifies the stated
   residuals within the admitted domain, without guaranteeing a
   well-conditioned volatility estimate.
   [Strata Black inversion](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/BlackFormulaRepository.html),
   [Strata Normal inversion](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/NormalFormulaRepository.html)
   and [White and Iwashita on caplet stripping](https://quant.opengamma.io/Caplet-Stripping-OpenGamma.pdf).

10. **Reconcile existing producers, reports and hosts together.** Align
    Rust registration, effective dependencies, raw risk providers,
    scenario/attribution consumers and the existing Python and WASM
    routes, named units, metadata, declarations, parity ledger and
    generated schemas. Carry the actual selected coordinate, source
    roles, held inputs, quote family/clock, reporting unit, represented
    steps, cuts and availability through reporting; do not alias model
    sigma, quote and curve directions. Keep the standalone inverse
    separate from horizon volatility breakeven. Qualify checked raw
    differences against independent Greeks, repeated cutoff/seasoned
    compounding, fixed and paid transitions, shared-curve versus
    distinct-curve roles, sparse grids and any admitted replay,
    zero-boundary sigma stencils, expiry/payment/premium crossings,
    exact inverse bounds and tiny time values. Qualify the overnight
    Hull–White approximation and its risk range separately. Roundtrips
    through one kernel do not establish independent pricing accuracy.
    [Metric orchestration](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice, scope and rationale

Analytic quote-model Greeks use the same resolved optionlet inputs as PV; overnight delta/gamma include the compound projection's chain rule. Curve risk, quote volatility, model sigma and date roll are separate perturbations. Their differing units and data-selection paths are material methodology choices.

### Algorithm, selection and conventions

- Delta sums kernel delta times coupon first parallel-forward sensitivity, weighted by N, accrual and payment DF. Gamma uses `kernel_gamma*sensitivity^2 + kernel_delta*second_sensitivity`. Both skip fixed/valuation-date optionlets; HullWhite1F dispatches to the shared full-repricing rate-Greek helper instead.
- Quote-model vega sums Black/Bachelier vega times N, accrual, DF and divides by 100, reporting currency per absolute 0.01 volatility increment. HW surface vega centrally bumps the **Normal surface** by `1e-4` and reprices, scaled to 0.01; it does not itself recalibrate explicit HW parameters. HwSigmaVega separately bumps resolved scalar sigma or every sigma-schedule ordinate with the shared HW bump. It is central if every ordinate exceeds the bump; otherwise forward-sided, scaled to 0.01.
- DV01 uses central quote-replay when required calibration recipes exist; same-ID discount-only OIS has its own eligibility path. Without recipes it uses direct combined rate curves (discount-only for single-curve OIS). ForwardPv01 explicitly requires and bumps a forward object; Rho is generic discount-only DV01; bucketed DV01 uses triangular bumps.
- Theta rolls the date with the shared theta-period rule, capped at the next unfixed optionlet fixing, otherwise final payment, and reprices on the unchanged market. It returns rolled PV minus base PV; it does not invent newly realized fixing data.
- ImpliedVol is supported only for Caplet/Floorlet. Read required quoted premium, obtain the convention using a zero-sigma quote override, and solve `(gross_option_PV(sigma)-target)/N=0` with Brent tolerance `1e-12`, 100 iterations and bracket `[0,1]` Normal or `[0,5]` lognormal. Settled/fixed optionlets return zero; negative/nonfinite target errors. Errors inside the objective become NaN and are handled by the solver. Multi-period caps must use stripping elsewhere.

### Limitations and alternatives

Local analytic Greeks hold quote inputs/discount weights fixed except the specified coupon chain rule. They are not smile-recalibrated total derivatives. Surface vega can be zero with fixed HW sigma and requires a Normal surface even though explicit-parameter pricing need not consume it. Implied vol solves gross option value, while instrument PV may be premium-net; target premium semantics must distinguish those. A flat cap-vol inverse solve is deliberately unsupported here.

## Evidence

[Metric selection and chain rule](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/mod.rs), [shared aggregation](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/common.rs), [vegas](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/vega.rs), [DV01](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/dv01.rs), [theta](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/theta.rs), [inverse solve](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/implied_vol.rs). [Implied-vol tests](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/implied_vol.rs), [normal Greeks](../../../../finstack-quant/valuations/rates/tests/cap_floor/validation/normal_greeks.rs), [forward PV01](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/forward_pv01.rs) provide roundtrip/finite-difference evidence. [Brent](../../../REFERENCES.md#brent-1973) supplies solver provenance.

[Hull–White Greek dispatch](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/option_greeks.rs)
currently delegates to the shared rate-Greek helper. The
[generic registry](../../../../finstack-quant/valuations/core/src/pricer/registry.rs)
builds a center PV before computing metrics, which currently couples the
inverse request to a baseline pricing sigma. The existing
[vega](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/vega.rs),
[DV01](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/dv01.rs)
and [theta](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/theta.rs)
suites were located, not executed.

### Independent review evidence

- For a compounded Normal caplet, take `N=1m,A=.25,D=.97,T=1`,
  one recognized rate `.02` with weight `.06`, and two uses of the same
  unresolved `.03` quote with weights `.08,.12`. The coupon rate and first
  two derivatives are `.028863401472,.8032667648,.07689216`. With
  `K=R+.002` and `σ=.01`, raw delta is about `81,956.92` and
  gamma is `6,126,493.93`. A `.0001` reporting increment yields quote
  vega `9.482785329`. Through the same growth rule, a floor with
  `K=R+.05` has negative compound gamma despite the convex unit
  Normal floor kernel.
- A known optionlet receipt of `1,500` at `D=.99` and a premium payment
  of `500` at `D=.98` give opening PV `995`. At respective discount
  source coordinates two and one year, a direct ±1 bp shock gives
  `dv01≈−.248000002`. If both cash amounts settle within the requested
  horizon, `C=1,000`, closing PV is zero and theta is `+5`.
- An ATM Normal caplet with `N=1m,A=.25,D=.98,T=1,σ=.005` has a gross
  target of about `488.704293492`; using the PV net of a `100` premium
  would imply about `.003976886419`. An exact shifted Black boundary
  with `W=128,F=1/64,K_quote=−1/64,shift=1/32` has caplet bounds
  `[4,6)` and floorlet bounds `[0,2)` in currency.
- Near intrinsic, let a Normal caplet have `W=128,F=2^54,K_quote=1`
  and quoted `P=2^61`. Exact intrinsic is `P−128`, which rounds to
  `P` alone in binary64. For a candidate with OTM time value `256`,
  relative-only tolerance `1e−8` accepts the gross PV residual but fails
  the time-value gate against the exact target `128`. A rounded lower
  equality cannot authorize zero IV.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
