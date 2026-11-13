# MODELS-MONTE-CARLO-08: CIR/CIR++ dynamics and quadratic-exponential sampling

- Decision status: **Accepted — complete CIR and CIR++ quadratic-exponential package, 2026-11-11**.
- Implementation status: Existing CIR and CIR++ transitions and shared QE kernel observed. The accepted checked sampling, projection and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent moment, tail, boundary, log-moment and shift examples checked. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked CIR and CIR++ quadratic-exponential paths

1. **Select checked QE for CIR and CIR++ paths.** Model the unshifted
   factor by `dx=kappa*(theta-x)*dt+sigma*sqrt(x)*dW` on a declared
   model clock. Require immutable checked finite `kappa>0` in
   inverse model years, `theta>=0` in factor units,
   `sigma>=0` in square-root factor units per square root model year,
   and `x>=0` in factor units. Exact zero sigma is the deterministic
   limit of the same law. Reject invalid or negative incoming state.
   Select the typed QE transition in existing raw simulation and
   retire the raw Euler override on these routes. No implicit
   truncation, alternate transition or parameter repair.
   [CIR and QE references](../../../REFERENCES.md#checked-cir-qe-and-deterministic-shifts).

2. **Use Feller as an inclusive diagnostic.** Reuse the accepted
   range-safe exact comparison `2*kappa*theta>=sigma^2` of
   represented inputs after CIR admission. Keep Feller-failing
   parameters admissible and remove constructor warnings. In the
   stochastic law with positive immigration, Feller concerns whether
   zero can be hit from a positive start. At `x=0`, positive
   `kappa*theta` supplies immigration. With `theta=0`, zero is
   absorbing. The QE atom is a feature of the approximating
   transition. A positive-degree exact stochastic CIR endpoint
   has no atom at a fixed positive time, even when zero is
   accessible. With zero immigration the true transition can
   have an absorbing atom that QE need not match.
   [Accepted shared Feller diagnostic](../models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform),
   [exact transition references](../../../REFERENCES.md#checked-cir-qe-and-deterministic-shifts).

3. **Compute the exact conditional moments in scale.** For an actual
   positive width `h`, set `d=exp(-kappa*h)`,
   `g=-expm1(-kappa*h)`, `L=g/kappa`, `U=x*d`, and `I=theta*g`.
   The conditional mean is `m=U+I` and variance is
   `v=sigma^2*L*(U+I/2)`. Evaluate the complete terms, using the
   stable dimensionless continuation of `L=h*E(kappa*h)`.
   Do not drop positive immigration or variance because a detached
   exponential, product, square or ratio underflows or overflows.
   On a positive step, choose the deterministic branch for genuine
   zero variance only, including zero sigma and the absorbing state.
   Return the checked mean in that branch. Validate any separately
   admitted zero-duration identity before using it.

4. **Pin the CIR branch switch and quadratic law.** For positive
   variance compare `v` with `(3/2)*m^2` in scale. Equality
   selects quadratic. Fix the CIR and CIR++ threshold at `3/2`
   and retire the unused threshold override. With
   `psi=v/m^2`, the quadratic branch uses
   `a=m*psi/[2*(1+sqrt(1-psi/2))]` and
   `(sqrt(m-a)+sqrt(a)*z)^2`. Use
   `m-a=m*sqrt(1-psi/2)` directly to preserve the companion
   term near the shared domain boundary. Keep needed scale through the
   roots, signed sum and final square. A genuine cancellation
   can give zero. A required positive result rounded to zero
   or unresolved arithmetic is a computation error.
   The shared kernel validates any owner-supplied threshold in
   `[1,2]`. Record 09 owns the Heston threshold selection.
   [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe).

5. **Invert the atom-and-exponential branch in the normal log tail.**
   Let the positive mixture weight be `q=2/(1+psi)` and the
   exponential scale be `B=m/q`. Preserve `log(q)` and the
   required scale without forming a rounded atom probability
   `1-q` as the sampling threshold. With
   `ell=log_norm_cdf(-z)=log(1-Phi(z))`, return zero if
   `ell>=log(q)`, including equality. Otherwise return
   `B*(log(q)-ell)` with a checked complete comparison and
   result. Remove the CDF cap and the final zero floor. Propagate
   unresolved tail comparison or range errors. No retry, redraw
   or substitute positive tail. Finite-grid normal draws can make
   an ideal positive mixture component unreachable.
   [Accepted normal log tail](../core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains).

6. **Bind the transition to actual steps and fixed draw positions.**
   Use the exact stored endpoints and positive adjacent widths,
   with the same checked calculation for direct and prepared paths.
   Reject invalid shape, range, coefficient and draw requests with
   context. On the standalone path consume one assigned finite
   normal for each positive actual interval, including deterministic
   or absorbing steps. An eligible antithetic leg uses its
   negation. Preserve the normal-to-branch coupling without an
   additional uniform or rejection draws. An admitted origin-only
   grid consumes no draw. Publish only checked factor and projection
   results. Coupled products retain their draw layout, qualify the
   innovation's unit marginal and propagate shared failures.
   [Time grid](../core-numerics/time-grids.md),
   [accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

7. **Share a checked log exponential moment for the selected QE law.**
   For finite coefficient `c`, use the same validated regime as
   sampling. The deterministic log moment is `c*m`. In the
   quadratic branch set `D=1-2*c*a` and, for `D>0`, evaluate
   `c*(m-a)/D-log(D)/2`. In the mixture branch set
   `D=1-c*B` and, for `D>0`, evaluate
   `log((1-q)+q/D)` with stable log arithmetic. Return exact
   zero at `c=0` after admission. Distinguish mathematical
   divergence at or beyond a pole from numerical failure for a
   finite moment. An out-of-range exponential need not invalidate
   a representable logarithm. These moments describe the QE
   approximation. Record 09 owns when and how Heston applies its
   correction, including the action on a divergent moment.
   [Heston QE owner](09-heston-qe.md).

8. **Keep the CIR++ factor unshifted and project a checked signed rate.**
   Use the same factor transition and an explicit immutable signed
   shift `phi(t)` on the common model clock. Require nonempty
   equal lengths, finite values, origin-zero strictly increasing
   start knots, right-continuous lookup and a documented flat tail.
   Change raw CIR++ initial input to the unshifted `x0`. Publish
   `short_rate=x+phi(t)` at the exact stored nodes, including the
   origin, and reject unresolved or nonfinite projections. Negative
   shifted rates are valid. Remove the subtract-and-restore origin
   path and generic spot alias. A shift knot consumes no draw and
   leaves factor dynamics unchanged. Plain CIR reports its factor.
   Curve fitting, calibration and an integrated rate or stochastic
   discount law require separate product qualification.
   [Deterministic shift reference](../../../REFERENCES.md#checked-cir-qe-and-deterministic-shifts).

9. **Reconcile live shared callers and existing host transport.**
   Use one checked Rust regime across CIR, Heston and credit
   consumers. Remove nominal zero clamps that currently conceal
   invalid structured-credit and revolving-credit factor inputs or
   results. Preserve admitted equal-time behavior and reject reversed
   periods, with credit law, coupling and estimator controls at the
   owner. Carry the canonical CIR/CIR++ input, actual grid, scheme
   identity, factor/rate meaning and error categories through
   existing Rust, Python and WASM simulation, validated state,
   schemas, declarations and parity. Keep low-level QE and regime
   operations Rust-owned. The endpoint route supplies no implied
   bank-account integral or scalar-discount authorization.
   [Credit-line owner](11-credit-line.md),
   [raw simulation owner](28-capture-and-simulation-contracts.md).

10. **Qualify the approximation independently.** QE matches the first
    two conditional moments in its ideal law. Qualify endpoint
    distributions, zero mass, weak error and grid refinement against
    independent exact CIR transitions, including Feller-failing,
    zero-immigration, small-variance and tail-sensitive cases.
    Keep approximation and finite random-grid effects distinct.
    Pricing, risk, integrated rates and any joint Heston or credit
    law require the respective owner's benchmarks.
    [CIR benchmark references](../../../REFERENCES.md#checked-cir-qe-and-deterministic-shifts).

## Observed choice and domain

CIR uses `dx=kappa(theta-max(x,0))dt+sigma sqrt(max(x,0))dW`, with checked kappa/sigma>0 and theta≥0. Feller failure logs a warning and is allowed. CIR++ adds a deterministic piecewise constant shift to the published rate, while the internal state remains the unshifted factor. Default simulation uses QE; Euler is an explicit nonpositivity-preserving alternative.

## Observed algorithm and controls

Compute exact conditional mean m and variance s² with `exp(-kappa dt)` and `expm1`-stable immigration. If m or s² equals zero, return m. Otherwise psi=s²/m². For psi≤psi_c use `(sqrt(m-a)+sqrt(a)z)²`, where `a=m psi/[2(1+sqrt(1-psi/2))]`. Otherwise use a point mass p at zero plus exponential tail: `beta=2/(s²/m+m)`, `p=1-m beta`; u=normal-CDF(z), capped below one; return zero if u≤p, else `ln((1-p)/(1-u))/beta`, floored at zero. Default threshold=1.5; override must be in [1,2]. Shared code also exposes exact exponential moments of this approximating conditional distribution for Heston.

CIR++ rate is x+shift(t), using the knot effective on equality and flat extrapolation. `simulate_paths` accepts the actual starting rate, subtracts initial shift, simulates the factor, and adds the shift back to every reported time. Low-level process stepping expects the unshifted factor.

## Observed limitations

The kernel truncates negative incoming state. Detached moments
can round to zero and select the deterministic branch. The
exponential branch forms a rounded atom probability and caps
the CDF, so it can erase a positive tail. The shared exponential
moment result merges divergent and out-of-range cases. Raw CIR++
adds the shift after the factor finiteness check and can publish
a nonfinite rate. Shift calibration is outside this implementation.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| Conditional moments `m=1,v=1.5` | Equality selects quadratic. `a=0.5`, the sample at `z=0` is `0.5`, and `z=-1` gives genuine cancellation to zero. |
| `x=0,kappa=ln(2),theta=5e-19,sigma=0.2,h=1,z=8.5` | Positive weight is about `3.46573590e-17`. The rounded atom probability becomes one in the old arithmetic. The checked log-tail sample is about `0.00935136776`. |
| `kappa=sigma=1,h=ln(2)`, with `theta=0,x=1` or `theta=0.25,x=0` | In the first case the exact endpoint has atom `exp(-2)` and quadratic QE has no ideal atom. In the second case the exact endpoint has no atom and QE gives `1/3` at zero. |
| Deterministic value `m=1`, exponential-moment coefficient `c=1000` | The log moment is `1000` even though its exponential overflows. In a quadratic regime with `a=0.5`, `c=1` is a true pole. |
| CIR++ `x0=theta=1/32,sigma=0`, shift `-1/16` before time 1 and `1/8` from time 1 | The factor remains `1/32`, the signed reported rate changes from `-1/32` to `5/32`, and there is no extra draw at the shift knot. Reject a projected sum of finite inputs that overflows. |

## Evidence

Existing tests cover conditional moments, the numerical atom,
small-scale examples and shared Heston/CIR kernel consistency.
The independent examples check particular numerical and model
boundaries. [Process](../../../../finstack-quant/models/src/monte_carlo/process/cir.rs),
[QE moments and branches](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_common.rs),
[CIR adapter](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_cir.rs),
[raw simulation](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[structured-credit paths](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_paths.rs),
[credit-line step](../../../../finstack-quant/models/src/monte_carlo/discretization/credit_line.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
