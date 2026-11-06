# ATM term-GBM, observation grids and pinned spot risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-12`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Autocallable and cliquet contracts explicitly declare `AtmTermGbm`. Their shared adapter preserves rates/ATM total variance between product checkpoints using piecewise-constant GBM, then simulates exact lognormal steps within each interval.

## Scope and input domain

Single-asset path-dependent equity products with deterministic curves and continuous dividend yield. This is a term-volatility model, not local volatility or a full-smile calibration. Recorded fixings must be finite and positive.

## Algorithm

At checkpoint ti obtain rebased Di, forward `Fi=S exp(-q ti)/Di`, implied σi at `(ti,Fi)`, and total variance `Wi=σi²ti`. Interval rate is `log(Dprev/Di)/(ti−tprev)` and interval volatility `sqrt((Wi−Wprev)/(ti−tprev))`. Negative variance increments, invalid DFs and intervals at most `1e-6` years error; nonincreasing checkpoints are skipped by the shared bootstrap. Exact interval steps multiply spot by `exp((r−q−σ²/2)dt+σ√dt Z)`.

Product grids insert every future observation/reset time; checkpoints are sorted and deduplicated at `1e-10`. The adapter passes merged engine configuration, target CI half-width and Philox seed to `McEngine`. Spot delta/gamma/vanna first pin an omitted initial level to unbumped spot, preventing the contractual anchor from moving with each spot shock. Cliquet on a same-day reset pins that fixing instead. Shared fixing lookup returns the first exact-date entry; its helper does not reject duplicate dates.

## Selection, controls and conventions

Base path configuration reads [MC defaults](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json): 100,000 paths, parallel, 252 steps/year, minimum eight, chunk size 1,000. These products enable antithetics; path/antithetic overrides are merged. Path count zero means use default and count above 5,000,000 errors. Actual seed comes from model configuration/instrument identity, not the base config's example seed.

## Limitations, failures and alternatives

Required observation times avoid interpolating barrier observations, but the model omits smile-state dynamics and stochastic rates. Identical seeded bumps reduce simulation noise but do not remove discontinuous-payoff bias. Local-vol/stochastic-vol path models are possible alternatives, not implemented variants of the declared path model.

## Evidence

[Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo); shared [piecewise_gbm.rs](../../../../finstack-quant/models/src/monte_carlo/process/piecewise_gbm.rs) contains exact-boundary and deterministic-step identities. Product seasoning/grid evidence is linked in records 13–14. No new simulation or independent path benchmark was run.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [piecewise_gbm.rs](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs)
- [spot_pinned.rs](../../../../finstack-quant/valuations/equity/src/spot_pinned.rs)
- [past_fixings.rs](../../../../finstack-quant/valuations/equity/src/past_fixings.rs)
- [lib.rs](../../../../finstack-quant/valuations/equity/src/lib.rs)
