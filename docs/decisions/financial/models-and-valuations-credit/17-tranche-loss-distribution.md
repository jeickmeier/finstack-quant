# MODELS-AND-VALUATIONS-CREDIT-17: Finite-pool tranche losses and base correlation integration

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Base-correlation equity-loss differences use finite binomial pools or conditional heterogeneous convolution, rather than an automatic large homogeneous pool approximation. Exact small-pool state enumeration retains individual losses around tranche payoff kinks. Alternative copulas are explicitly constructible; adoption remains pending.

## Scope and input domain

CDS tranches with percent attachment/detachment, positive tranche notional, complete issuer curves when heterogeneous, issuer weights summing to one within 1e−9, and credit curves based on valuation date for live pricing. Detachment at or below 1 percent is rejected as fraction-like input, which also rejects legitimate sub-1-percent tranches.

## Algorithm

1. Infer historical defaulted pool fraction from realized loss divided by floored LGD. Erode the original pool bottom by losses and top by recoveries, divide surviving attachment/detachment by surviving pool factor.
2. Read base correlations at original attachment/detachment. Exponential boundary smoothing operates around .01/.99 with width .005; probabilities are clipped at 1e−12 before copula thresholds.
3. Conditional homogeneous defaults follow a finite binomial PMF. Sum capped loss `min(k/N × LGD,K)` and separately capped recovered amounts. Heterogeneous pools with equal PD/recovery/weights within 1e−9 take the homogeneous path; otherwise enumerate all 2^N states when within 200,000 states, else convolve a .001 portfolio-fraction loss grid with linear split of each name’s exact loss between adjacent bins. Reject grids at the size limit or above 5,000,000 issuer-grid visits.
4. Gaussian factors use adaptive Simpson over [−10,10], segmented at unit widths, absolute portfolio-fraction tolerance 1e−10 and depth 20. RFL and multi-factor use nested adaptive normal integration with that budget. Student-t delegates to its product Gauss rule; the adaptive tolerance is not its error guarantee. These tranche-specific choices override generic Gaussian/RFL/multi-factor quadrature in [copula kernels](09-copula-kernels.md).
5. Subtract capped equity expectations at detachment and attachment, scale by surviving pool and original width. At maturity negative loss differences beyond the integration error budget produce a base-correlation arbitrage error; smaller numerical residuals clamp to zero. Interim negative loss differences are floored. Recovery-side negative differences are floored without the strict maturity loss-side contract. Successive expected losses and recovery writedowns are made nondecreasing, then writedown is capped by remaining loss capacity.

## Selection and controls

Default/new pricer is Gaussian; `with_copula` explicitly builds an alternative. Presence of issuer curves selects heterogeneous calculation. Grid and integration controls are constants, not public overrides. The .001 convolution grid has discretization error separate from the factor-quadrature budget.

## Conventions

Attachment and detachment are percent (3 means 3%); recovery, default probability and correlation are decimal fractions. Realized loss is a fraction of original pool, not tranche notional. Hazards use each curve's own clock subject to the live base-date restriction; discounting uses relative factors on the discount curve's own axis. EL fractions used for cashflows include prior loss; the separate expected-loss metric reports modeled future loss amount.

## Limitations, failure and fallback

Grid convolution is discretized even though some diagnostics call it exact. Clamping and monotonicity repairs can hide interim base-correlation inconsistency; only maturity loss receives the strict tranchelet arbitrage test. Recovered-principal top-down erosion assumes index-level recovery for realized state. Smoothing also prevents direct pricing of exact independent/perfect-correlation limits.

Invalid copula parameters, incomplete issuer coverage, resource limits, nonfinite integrands and failed adaptive convergence return errors. No large-pool or Monte Carlo fallback is selected. Loss/recovery repairs and clipping do not prove general base-correlation arbitrage freedom.

## Alternatives and tradeoffs

Large-pool asymptotic approximation, FFT convolution and Monte Carlo are possible but unselected here. Explicit finite pools preserve granularity; the bounded grid controls work at a measurable discretization cost.

## Evidence

Owners: [expected_loss.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs), [heterogeneous.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/heterogeneous.rs), [integration.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/integration.rs), [sensitivities.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs), [engine.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs), [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/pricer.rs). [Pricing tests](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/tests.rs) and [tranche integration tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) are invariant/regression evidence, not a complete external accuracy grid. See [family selection](0031-tranche-loss-model-selection.md).

- [Instrument dispatch](../../../../finstack-quant/valuations/credit/src/cds_tranche/types.rs) and [family registration](../../../../finstack-quant/valuations/credit/src/family.rs) establish default-pricer reachability. Pricing tests include homogeneous/heterogeneous agreement, maturity arbitrage errors, survival-weighted timing, recovery writedown, curve rebasing and explicit state-enumeration/factor-grid comparison for heterogeneous tail loss.
- The [loss reference fixture](../../../../finstack-quant/valuations/credit/tests/fixtures/production_tranche_loss_reference.json) identifies SciPy binomial CDF, QUADPACK and composite Gauss–Legendre values. Its located evidence does not qualify provenance or the full copula/grid domain under [ADR 0014](../../project/0014-financial-reference-evidence.md); no generators were rerun.
- Technical sources: [Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas), [Gaussian copula](../../../REFERENCES.md#li-2000-gaussian-copula), [Student-t](../../../REFERENCES.md#demarta-mcneil-2005-t-copula), [random factor loadings](../../../REFERENCES.md#andersen-sidenius-2005-rfl) and [finite-pool/multi-factor loss methods](../../../REFERENCES.md#andersen-sidenius-basu-2003).

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires checked detachment-percent queries without magnitude-based fraction
inference, actual maturity/pool/model/recovery association, and exact admitted
source parameters. Implement supported endpoint limits or refuse them rather
than smoothing into an interior band. Common pool-basis capped equity losses
must be subtracted before tranche-width normalization; increasing rho alone
does not establish consistent losses. Interim-date reuse and original-versus-
effective strikes remain explicit model choices. Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's complete loss/recovery floors, quadrature,
convolution, realized-state mapping or arbitrage acceptance methodology.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires explicit pool mode and authoritative member probability/cash-recovery/
weight inputs, strict lookup and the stated compensated mass gate. Preserve
zero-weight roster identity and distinct default events even when hazards are
shared. An approximate-homogeneity shortcut is not exact source equivalence;
qualify an approximation explicitly or preserve the actual law. Reconcile named
PDs with accepted hazard date/context/conditional-probability semantics rather
than clipped 1-sp with default context. Record 19 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's full copula/convolution/quadrature/loss-recovery model.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
