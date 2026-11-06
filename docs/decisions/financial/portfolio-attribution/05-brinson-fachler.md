# PORTFOLIO-ATTRIBUTION-005: Single-period Brinson–Fachler attribution

- ID: PORTFOLIO-ATTRIBUTION-005
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

The function decomposes portfolio active return into benchmark-centered allocation, selection and interaction. Inputs are sector labels and decimal portfolio/benchmark weights and sector returns. Require a nonempty list, all numerical inputs finite, and each side's weight sum within `1e-6` of one. Short weights are permitted; sector labels need not be unique or nonempty. There is no automatic cash bucket or weight normalization.

## Formula and selection

Compute `R_P=sum wP_i*rP_i` and `R_B=sum wB_i*rB_i` using compensated sums. When a sector has exactly zero benchmark weight, replace its benchmark sector return with the whole-benchmark return `R_B`; otherwise use supplied `rB_i`. Then:

- `A_i=(wP_i-wB_i)*(rB_i-R_B)`;
- `S_i=wB_i*(rP_i-rB_i)`;
- `I_i=(wP_i-wB_i)*(rP_i-rB_i)`.

The sector total is their direct sum; overall effects are compensated sums. The reference-centered allocation is Brinson–Fachler rather than the uncentered Brinson–Hood–Beebower expression. The exact-zero-benchmark substitution makes an out-of-benchmark sector's allocation zero and puts its active contribution into interaction. The active-return identity is numerical algebra, not independent evidence that a sector taxonomy or benchmark is suitable.

Single-period inputs can contain returns at or below `-100%`; the [Carino linker](17-carino-linking-and-reconciliation.md) has the stricter logarithmic domain. Labels and input ordering are preserved and matter to linked-period row matching.

## Alternatives and tradeoffs

Two-effect attribution combining selection and interaction, uncentered allocation, geometric attribution and alternate out-of-benchmark conventions are distinct methods. This implementation exposes three effects and a specific zero-benchmark convention without a selection flag. Signed weights preserve algebraic identities but may produce economically unintuitive effects.

## Evidence

[Source and inline attribution/reconciliation tests](../../../../finstack-quant/portfolio/src/brinson.rs) supply formula and algebraic evidence. The current [Brinson reference entry](../../../REFERENCES.md#brinson-fachler-1985) is titled Brinson–Fachler but lists Hood/Beebower's *Determinants of Portfolio Performance*: that entry does not establish bibliographic provenance for this exact centered formula. No external standard or independent benchmark was verified in this audit.

Multi-period validation is specified separately; no runtime change or methodology approval is implied.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
