# VALUATIONS-RATES-09-LMM-BERMUDANS: Bermudan LMM displacement, terminal numeraire and regression fallback

- ID: VALUATIONS-RATES-09-LMM-BERMUDANS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

LmmMonteCarlo is an explicit Bermudan alternative to Hull–White. It builds a displaced two-factor forward model from the single discount curve and an explicitly supplied positive `lmm_base_vol`; it does not calibrate that scale from the swaption vol surface. The low-level process/predictor-corrector algorithm is documented in [models-monte-carlo](../models-monte-carlo/10-lmm.md). The product adapter's factor shape, displacement, contract domain and stopping rule are additional choices.

## Contract gate and parameter construction

1. Require physical CoTerminal settlement, identical fixed/float discount and forward IDs, zero float spread/reset lag and simple compounding. Fixed and float start/end/frequency/day-count/stub/EOM/BDC/calendar must match; payment lags must be zero. Every actual payment must equal accrual end without business-day displacement. Effective exercises must equal accrual starts. Other contracts error.
2. Drop forward periods before the next strictly future exercise; retain original period index for loading shape. Tenor times use ACT/365F from valuation; accrual factors use actual contractual schedule alpha. Bootstrap `L_i=(D(start_i)/D(end_i)-1)/alpha_i`, requiring finite positive DFs/alphas and finite L.
3. Set a common displacement `max(0,0.01-min_i L_i)`, so the minimum shifted initial rate is at least1%. Use two constant factors with row `base_vol*[1-0.4*u,0.4*u]`, where `u=original_period_index/original_period_count`. There are no volatility breakpoints. This is a prescribed loading/correlation shape, not a fitted full covariance matrix. Changing curves can change the displacement; rates risk repricing therefore includes that parameter reconstruction.
4. Defaults from the embedded product JSON are 25,000 independent streams, seed42, antithetic=true, degree3 and minimum4 steps/event gap; OOS=false. Unlike HW Bermudan orchestration, this adapter passes its pricer config unchanged and does not apply the instrument mc_paths override. The scalar lmm_base_vol override is required.

## Simulation, payoff and continuation

Build a grid containing exercises, tenor starts and vol breakpoints, plus maturity and the shared monthly subdivision floor. Exercise times must be finite, strictly increasing, more than `1e-10` apart, strictly inside maturity and within `1e-10` of a positive tenor start. Use Philox substreams, negated-shock antithetics and the LMM predictor-corrector. Reject excessive simulated paths and fewer than two independent pricing estimators after any OOS split.

At exercise let j be the first live forward (tenor start plus `1e-8` must not be before t). Reconstruct `P(t,T_k)=product_{i=j}^{k-1}1/(1+alpha_i*L_i)`, `A=sum(alpha_i*P(t,T_{i+1}))`, `S=(1-P(t,T_end))/A`; return S=0 for annuity magnitude at most `1e-15`. Exercise amount is `sign*(S-K)*A*N`. Deflate by the path's terminal bond `P(t,T_end)`; nonpositive terminal numeraire gives zero in the local fallback. Terminal exercise retains positive values.

For earlier exercise regress future chosen **terminal-deflated** cashflow against ITM training states. Basis always includes `[1,S,A]`; degree>=2 adds `[S²,S*A]`, degree>=3 adds `S³`. Higher requested degrees add no further terms but increase the regression count threshold. Fit when ITM training count exceeds `basis_degree+3`, using the shared least-squares solver. Apply exercise if positive deflated immediate value exceeds fitted continuation. **If the ITM count is insufficient, compare immediate value against each path's realized future chosen cashflow and select the larger.** This fallback uses future path information and can introduce foresight bias, including when OOS mode is selected; it is materially different from HW's skip-exercise fallback.

Mean terminal-deflated payoff is multiplied by the time-zero terminal discount factor. Antithetic pairs count as one estimator; optional OOS trains even base streams and prices odd ones. Return PV and positive standard error. The public wrapper finally compares PV with deterministic exercise today, using the shared single-curve HW cashflow helper at zero volatility. Matured/no-future-exercise paths short-circuit before full LMM contract validation, so an exercise-today-only request can bypass the physical/matching-schedule gate.

## Alternatives and risk

Independent fitted vol/correlation term structures, different displaced-rate floors and alternate stopping bases are unimplemented adapter alternatives, even though lower-level LmmParams supports richer inputs. The common Bermudan Delta/Gamma remain discount-curve bump/reprice channels; HwSigmaVega registration is not an LMM volatility sensitivity, and ExpectedExerciseTime is not published by LMM. No dedicated lmm_base_vol sensitivity is published.

Independent market/covariance and out-of-sample regression validation was not established.

## Evidence

- [LMM product adapter](../../../../finstack-quant/valuations/rates/src/swaption/lmm_pricer.rs), [LSMC/numeraire kernel](../../../../finstack-quant/valuations/rates/src/swaption/pricing/lmm_bermudan.rs), [product defaults](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json), [shared metric registration](../../../../finstack-quant/valuations/rates/src/swaption/metrics/mod.rs).
- [Numeraire tests](../../../../finstack-quant/valuations/rates/src/swaption/pricing/numeraire_tests.rs) compare single-exercise terminal-measure estimates and a co-terminal lower bound using shared process primitives. Inline adapter/kernel tests locate contract gates, parameter shape, grid and sampling checks. They were not executed and do not qualify the low-ITM fallback as unbiased.
- [Andersen/Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) give general context. A product-specific factor-shape rationale/adoption record and canonical LMM-paper citation were not established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
