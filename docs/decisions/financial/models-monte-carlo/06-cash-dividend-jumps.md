# MODELS-MONTE-CARLO-06: Exact GBM subintervals and discrete dividend jumps

- Decision status: **Accepted — complete discrete-dividend jump package, 2026-11-11**.
- Implementation status: Existing dividend process and subinterval stepper observed. The accepted checked jump values, exact knot placement and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent jump, claim-value, grid and forward examples checked. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked discrete-dividend jumps on exact grid knots

1. **Select the dedicated full-spot jump model.** Use one typed exact
   dividend scheme for the single-asset flat GBM process. The caller
   explicitly supplies the clock, measure, finite nonnegative annual
   relative volatility of full spot, finite signed rate and finite
   signed residual continuous yield `q`.
   The carry `r-q` excludes the modeled discrete distributions from
   the continuous yield. An explicit zero yield is valid. Preserve
   plain flat GBM on the same grid for an empty jump schedule.
   Require a finite strictly positive opening spot, including
   representable subnormals. A depleted origin needs an owner-qualified
   lifecycle treatment outside this fresh-path route. Generic Euler,
   plain GBM dispatch that omits the jumps and escrow substitution
   are unsupported.
   [Accepted exact GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [separately owned escrowed lattice](../models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise).

2. **Use the capped cash jump with an absorbing zero.** For pre-jump
   spot `S_minus` and finite nonnegative modeled ex-value `J` per
   share, set modeled drop `A=min(J,S_minus)` and post-jump spot
   `S_plus=S_minus-A`. For a synthetic proportional cash fraction
   `p in [0,1]`, the modeled drop is `p*S_minus` and post-jump
   spot is `(1-p)*S_minus`. Use checked complete arithmetic and
   preserve zero-jump identities. Exact depletion, including
   `J>=S_minus` or `p=1`, makes the spot zero and absorbing.
   A positive surviving value that rounds to zero is a range failure.
   Keep the requested claim value distinct from the modeled drop,
   using the same checked evaluation for any reported drop. Retire
   the ambiguous amount helper. Proportional cash fractions are
   explicit numerical inputs. Stock distributions and annual yield
   are outside this event type.
   [Liquidator policy and ex-dividend jump references](../../../REFERENCES.md#discrete-dividend-jumps-and-modeled-ex-values).

3. **Require a qualified dated cash adapter.** A dated owner resolves
   the actual underlying, share/quote basis, same currency, source
   snapshot, cutoff and original event IDs and ex/pay dates. Preserve
   exact cash-per-share Money and announcement or forecast provenance.
   For a supported ordinary event with ex date `e`, pay date `p`
   and scheduled cash `D`, compute requested ex-value
   `J=D*P(e,p)` using a qualified deterministic same-currency
   dated relative factor and accepted Money arithmetic. Require
   finite positive factors. Same-date uses exact one after validation,
   and factors above one are valid. Reject unsupported entitlement,
   share-basis or corporate-action cases, including positive cash
   paid before ex and due-bill regimes. Require the owner to
   qualify the capped stock-drop economics when using the model
   for actual cash claims. Preserve the source payment date and
   amount. The modeled drop supplies no guarantee of full payment,
   realized cash settlement or allocation of a shortfall.
   [Cash event authority](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates),
   [Money](../core-conventions/012-money-decimal-precision.md),
   [dated discounting](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting).

4. **Prepare one validated jump per exact ex boundary.** At one qualified
   ex boundary, value compatible fixed claims individually at their actual
   pay dates, then combine the ex-values through checked Money aggregation
   before model conversion. Retain contributing IDs and dates. Reject
   mixed cash/proportional jumps or duplicate synthetic jumps at a
   boundary because order can change the result. Require strictly
   increasing finite positive numeric event times, one admitted
   jump at each, and whole-schedule validation before publication.
   The dated owner prepares only the eligible events for the run
   horizon while preserving all original source records.
   Reject unordered or duplicate numeric state and distinct dated
   ex boundaries that collapse onto the same model time. Literal
   zero model jumps are valid and bind to their supplied knots.

5. **Bind jumps to the exact effective simulation grid.** The dedicated
   price route and a supplied explicit time grid require every
   admitted jump time to match an actual knot. Reject off-grid times
   and times beyond the terminal knot. A raw uniform base-grid
   request explicitly uses the exact union with admitted jump times
   and returns the augmented grid. Validate the base and effective
   grids, event binding and final draw, work and storage sizes before
   drawing. Preserve actual adjacent knots however close. Inserted
   raw nodes grant no contractual observation rights. Remove
   epsilon comparisons, reconstruction of endpoints as `t+dt`,
   and hidden in-step splitting in the numerical stepper.
   [Accepted time grids](../core-numerics/time-grids.md),
   [raw simulation owner](28-capture-and-simulation-contracts.md).

6. **Use explicit post-ex phases on this route.** The opening spot
   is already resolved for origin events. Reject a model jump at
   time zero, including a literal zero amount there. At each
   receiving knot, first evolve to its pre-jump spot, then apply
   its jump and publish the post-jump projection to the single
   ordinary payoff or raw-node observation. This order includes
   the terminal knot. Require the dated owner to qualify spot
   phase, entitlement and observation eligibility before binding.
   Cum-phase payoff, monitoring and exercise rights need a
   separately qualified owner route. A past ex event with future
   payment remains with the receivable owner. Keep any equivalent
   payoff settlement and its positive discount with the accepted
   pricing contract.
   [Accepted engine events and discounting](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

7. **Consume one checked Gaussian innovation per actual interval.**
   Use the shared checked exact GBM transition on each positive
   adjacent interval, including widths below the old cutoff.
   Use one normal in chronological order from the declared
   path substream. The jump consumes no draw. Preserve this
   layout at zero volatility and after depletion. Validate the
   assigned draw even on a deterministic interval. Negate the
   same ordered draws for an eligible antithetic partner. Remove
   the schedule-length factor budget, unused normal slots and
   tiny-interval skip. Propagate range or event errors with path
   and node context and publish only a complete checked step.
   Changing the effective grid can change common-random-number
   alignment. [Accepted Philox and draw provenance](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

8. **Use the realized modeled drop in forward checks.** With flat
   carry `b=r-q`, jumps at `t_k` and their modeled drops `A_k`,
   the ideal transition law gives
   `E[S_T]=S0*exp(b*T)-sum_k exp(b*(T-t_k))*E[A_k]`
   for the admitted post-event horizon. For capped cash,
   `A_k=min(J_k,S_k_minus)`. A deterministic subtraction of
   the full ex-value represents the case where the cap never
   binds. Independently qualify depletion probability, the
   resulting spot atom and forward moments on floor-sensitive
   cases. The finite-grid Gaussian transition and event law
   supply no product-wide price, continuous monitoring or
   exercise-error guarantee.
   [Dividend policy references](../../../REFERENCES.md#discrete-dividend-jumps-and-modeled-ex-values).

9. **Reconcile existing simulation and state transport.** Use the same
   checked Rust jump model in the existing numerical and serialized
   simulation routes. Preserve explicit dedicated scheme identity,
   effective grid, event layout and replay provenance, with validated
   state restoration and existing error categories. Update existing
   Python/WASM simulation, schemas, declarations and parity in the
   same implementation slice. Low-level process and jump types
   remain Rust-owned. Existing equity product dispatch, tree and PDE
   economics retain their owners. A future dated product caller
   needs the adapter above.

## Observed choice and domain

GBM with explicit cash/proportional dividends requires the dedicated exact-dividend scheme. Parameters retain continuous yield q as well; callers determine whether that represents residual yield. Cash dividends are nonnegative amounts per share; proportional dividends are fractions in [0,1]. The process sorts finite nonnegative times and rejects duplicates; `simulate_paths` more strictly rejects time-zero events.

## Observed algorithm and selection

At each outer step select dividends in (t,t+dt], using four machine epsilons of relative endpoint tolerance. Split into diffusion subintervals and zero-duration jumps. Each positive subinterval above 1e-10 consumes a distinct normal, evolves exact GBM, then applies its jump: cash `max(S-D,0)` or proportional `S*max(1-p,0)`. Allocate `number_of_dividends+1` factor slots per outer step as the maximum random budget. With no dividends in the interval, consume z[0] for the whole step. Cash depletion makes zero absorbing.

## Observed limitations

The low-level interval selector excludes origin events, while raw
simulation rejects them. The selector uses an epsilon around both
boundaries, and the stepper skips tiny positive subintervals. Global
factor allocation can consume unused normals. The nominal cash amount
helper returns the full input even when the stock drop is capped.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| Same ex boundary, cash claims `3` and `2`, pay factors from valuation `0.95` and `0.91`, and ex factor `0.98` | Requested combined ex-value is `467/98` before numerical rounding. At pre-jump spot `4` the modeled drop is `4` and the stock is depleted. At `6` the post-jump value is `121/98`. |
| Same-boundary cash `10` and proportional fraction `0.1`, pre-jump spot `100` | Cash first gives `81`, proportional first gives `80`. The accepted input contract rejects the mixture. |
| Knots `[0,1,nextUp(1),2]`, zero carry and volatility, initial spot `100`, cash jumps `10` at `1` and `5` at `nextUp(1)` | Post-event path is `[100,90,85,85]`. The old endpoint tolerance can include the second jump at time `1`. |
| Positive width `2^-52`, `r=2^52,q=0,sigma=0`, initial spot `100`, receiving cash jump `10` | Checked carry is one. The post-jump result is `100*e-10≈261.82818285`. Skipping diffusion would give `90`. |
| One cash jump with pre-jump lognormal mean `100`, total log SD `0.2` and `J=100` | Depletion probability is about `0.53982784` and expected post-jump spot is about `7.96556746`. Full nominal subtraction from the mean gives zero. |

Qualify exact knots, phase and grid admission, draw order, cash and
proportional identities, absorbing zero, claim coverage and underflow.
Check forward and distributional limits independently, then qualify
any product selection and existing host path output on the effective
grid.

## Evidence

Existing tests cover cash, proportional and multiple jumps, ordinary
grid-knot placement, and deterministic carried-dividend expectations.
The located production route is raw path simulation through the
existing Rust, Python and WASM surfaces.
[Process](../../../../finstack-quant/models/src/monte_carlo/process/gbm_dividends.rs),
[stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/exact_gbm_dividends.rs),
[simulation dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[Python simulation](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM simulation](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
