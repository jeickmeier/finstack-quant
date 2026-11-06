# MODELS-AND-VALUATIONS-CREDIT-03: Rating transition, generator recovery and path simulation

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Finite-state homogeneous Markov migration is implemented using matrix logarithms, generator repair, matrix exponentials and Gillespie simulation. The repair and roundtrip tolerance are explicit approximation policies, not exact embeddability guarantees.

## Scope and input domain

Positive finite horizons, square row-stochastic matrices on matching rating scales, and absorbing configured default state. Custom scales assume their last label is default unless an explicit default label is supplied.

## Algorithm

1. Transition construction checks entries within a 1e−12 edge allowance, row sum within 1e−8 and absorption, then clips tiny probability excursions. Composition multiplies matrices and adds horizons.
2. Recover `Q=log(P)/T`: real Schur decomposition must have real positive eigenvalues. Repeated triangular square roots (at most 64) reduce distance from identity to .25; a 64-term bounded log series is rescaled. Negative off-diagonal intensities are floored, diagonal becomes minus their sum, and the default row is zeroed. Report removed-negative L1 mass and `||exp(QT)−P||∞`; reject error above default .01 or caller tolerance.
3. Projection uses order-13 Padé scaling-and-squaring, threshold 5.371920351148152, LU then full-pivot LU solve; post-processing floors negative probabilities and normalizes rows before validation.
4. Simulation draws exponential holding times `−ln(U)/λ` and categorical off-diagonal transitions from one seeded PCG64 stream. Rates below 1e−15 stop the path; the final positive destination handles cumulative rounding. Empirical matrices count terminal states; empty path default-rate aggregation is zero.

## Selection and controls

Caller supplies generator directly or requests transition-to-generator conversion and its tolerance; simulation uses explicit path count and seed. No time-varying generator or correlated multi-obligor migration is dispatched here.

## Conventions

Horizons and intensities must share a time unit (documented in years); row i is source, column j destination. Default is absorbing, and path lookup is piecewise constant.

## Limitations, failure and fallback

Complex/nonpositive eigenvalues, excessive reconstruction error, invalid rows and singular solves fail. Generator and transition constructors permit tiny numerical repairs. Projection normalization is not accompanied by a repair-size diagnostic. The generator recovery rejects valid cases outside its real-positive-spectrum algorithmic domain.

## Alternatives and tradeoffs

Direct generator estimation avoids logarithm embeddability issues; constrained optimization or more general matrix logarithms are unimplemented. Discrete multiplication is supported through compose for integral successive horizons.

## Evidence

Owners: [matrix.rs](../../../../finstack-quant/models/src/credit/migration/matrix.rs), [generator.rs](../../../../finstack-quant/models/src/credit/migration/generator.rs), [projection.rs](../../../../finstack-quant/models/src/credit/migration/projection.rs), [simulation.rs](../../../../finstack-quant/models/src/credit/migration/simulation.rs), [scale.rs](../../../../finstack-quant/models/src/credit/migration/scale.rs). [Migration tests](../../../../finstack-quant/models/src/credit/migration/tests.rs) includes matrix invariants, reconstruction and simulation evidence; simulation/analytic agreement is internal model consistency. References: [Generator recovery](../../../REFERENCES.md#israel-rosenthal-wei-2001), [CreditMetrics](../../../REFERENCES.md#creditmetrics-1997), [Gillespie](../../../REFERENCES.md#gillespie-1977).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
