# MODELS-AND-VALUATIONS-CREDIT-01: Financial distress scores and classification

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Fixed published coefficient models are exposed as separate functions. Their formulas are observed; adoption of the sample-era coefficients and classification thresholds for a present portfolio is unresolved.

## Scope and input domain

Altman public manufacturing, private manufacturing, non-manufacturing and emerging-market variants; Ohlson nine-factor logistic and Zmijewski three-factor probit scores. Inputs are caller-computed financial ratios, not raw statements. All inputs must be finite; Ohlson indicators must be exactly zero or one.

## Algorithm

1. Altman Z uses coefficients (1.2, 1.4, 3.3, 0.6, 1.0), with safe above 2.99 and distress below 1.81. Z′ uses (0.717, 0.847, 3.107, 0.420, 0.998), thresholds 2.90/1.23. Z″ uses (6.56, 3.26, 6.72, 1.05), thresholds 2.60/1.10. EM adds 3.25 to Z″ and uses 5.85/4.35. Boundary equality is Grey; Altman outputs no implied PD.
2. Ohlson calculates `O = −1.32 − .407 SIZE + 6.03 TLTA − 1.43 WCTA + .0757 CLCA − 1.72 OENEG − 2.37 NITA − 1.83 FUTL + .285 INTWO − .521 CHIN`. A sign-stable logistic gives PD. Safe is below .019, distress above .038, otherwise Grey.
3. Zmijewski computes `Y = −4.336 − 4.513 NITA + 5.679 TLTA + .004 CAL`; PD is Φ(Y). Safe is below .10, distress above .50, otherwise Grey.
4. Reporting carries the score, model label, zone and optional PD; if a PD exists it maps through the registry-default master scale, while Altman reports no grade.

## Selection and controls

Caller explicitly chooses the scoring function; there is no universal score-model selector or automatic industry inference.

## Conventions

Ratios are decimal multiples; the Ohlson size variable is already log- and price-level-adjusted by the caller. A score-implied PD is not automatically risk-neutral or a CDS hazard probability.

## Limitations, failure and fallback

Finite inputs can produce extreme outputs. No denominator repair or statement normalization occurs here. Fixed score zones, especially the grey-zone constructions, are implementation choices rather than validation of predictive performance.

## Alternatives and tradeoffs

Locally re-estimated logit/probit, survival models and score-to-PD calibration are unimplemented alternatives. Altman variants are supported alternatives with different economic domains, not interchangeable defaults.

## Evidence

Owners: [altman.rs](../../../../finstack-quant/models/src/credit/scoring/altman.rs), [ohlson.rs](../../../../finstack-quant/models/src/credit/scoring/ohlson.rs), [zmijewski.rs](../../../../finstack-quant/models/src/credit/scoring/zmijewski.rs), [types.rs](../../../../finstack-quant/models/src/credit/scoring/types.rs), [report.rs](../../../../finstack-quant/models/src/credit/scoring/report.rs). [Scoring tests](../../../../finstack-quant/models/src/credit/scoring/tests.rs) contains coefficient/zone/invalid-input cases; these are formula and contract checks, not out-of-sample credit validation. Canonical sources: [Altman](../../../REFERENCES.md#altman-1968), [Ohlson](../../../REFERENCES.md#ohlson-1980), [Zmijewski](../../../REFERENCES.md#zmijewski-1984).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
