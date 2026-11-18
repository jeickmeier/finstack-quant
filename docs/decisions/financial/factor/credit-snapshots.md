# FACTOR-017: Credit anchor and dated spread decomposition

**Decision status:** Accepted 2026-11-17.

**Implementation status:** The shared peel is implemented. Checked proxy and DTS admission, frozen model comparison, and population reporting require reconciliation.

**Validation status:** Exact arithmetic checked a bucket membership change and reconstruction. Current reconstruction and anchor parity fixtures were inspected. The accepted proxy, DTS, and population rules still require qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Use one shared weighted peel for calibration anchors and dated snapshots. Validate the model and preserve its operative folds. Known issuers use their calibrated tags and loadings. New issuers require explicitly authorized proxies with canonical tags, finite loadings, and recorded sources. Every contributing DTS issuer requires sourced positive duration and a finite positive duration-times-current-spread weight. Fail on missing required inputs. [Panel and clock](credit-panel.md) · [Credit matching](credit-matching.md) · [Bucket folds](credit-buckets.md).
2. Compare ordered snapshots on one frozen model basis. Require consistent loadings, tags, and folds for continuing issuers. Difference common issuer and bucket keys. Report issuer and bucket entries and exits explicitly and flag continuing buckets whose contributing membership changed.
3. Report spread decomposition in basis points. With equal weights and an unchanged generic factor, replacing a 120 bp peer with a 160 bp peer raises the bucket by 20 bp in a two-issuer example. The continuing issuer's adder falls by 20 bp and reconciles its unchanged spread.

## Observed choice and rationale

Use one shared single-observation peel for calibration anchors and dated observed-spread snapshots, then difference only keys present at both dates. This supplies a reconstructable spread attribution in bp; it does not itself price P&L.

## Input domain and conventions

Observed spreads/generic are finite decimal spreads in (-0.5,2), converted to bp. Known model issuer tags/betas are authoritative; absent observed model issuers are skipped. A new observed issuer requires runtime tags and uses unit betas. Snapshot date is supplied by the caller and is not constrained to the calibration window.

## Algorithm stages

1. Check all model beta-level lengths and fold-record level indices, resolve each observed issuer and complete hierarchy paths. Known tags ignore runtime overrides.
2. Set residual r_i=s_i-beta_pc_i*g. At each level exclude folded issuer/level pairs and compute weighted mean residual by bucket. Equal uses weight 1; DTS uses model duration times current spread for known issuers.
3. For every bucket lacking any usable positive weight, fall back to an equal mean of observed residuals. New DTS issuers have no model duration, so they do not contribute to a weighted mean when known weighted names exist, but they can participate in this all-unweighted fallback.
4. Subtract beta_level times the bucket value from each active residual. Output current bucket values and final issuer adders; absent buckets are omitted, not zero-filled. Calibration anchors use the same kernel with calibrated weights/folds.
5. `decompose_period` requires from<=to and identical level count/index/dimension. It subtracts generic values and only intersection bucket/issuer keys, silently omitting entries/exits. It does not call snapshot finite validation.
6. Snapshot/period JSON convenience loaders validate finite values; direct serde and public struct construction can bypass those methods.

## Selection and controls

Fold-up behavior is recovered from `diagnostics.fold_ups`. The stored anchor state is not the source of current bucket values; observations are re-peeled. A zero folded beta alone does not exclude that issuer from a bucket mean unless the fold record is retained. Period changes use fixed supplied snapshots; there is no automatic model-roll or taxonomy reconciliation.

## Limitations, failures, and fallbacks

The snapshot function does not call full model validation and does not reject dotted runtime tags itself, unlike the credit matcher. Duplicate model issuer rows are collapsed by a BTreeMap index if unvalidated. Finite beta/derived output checking is incomplete. Period validation methods check values, not date ordering or correspondence between level index and vector position; the decomposition function adds some shape/date checks but does not validate all values.

## Alternatives and tradeoffs

A frozen-anchor attribution, an equal-only new-issuer policy, caller-supplied new issuer durations, or explicit universe entry/exit residuals would change meanings. Requiring exact key equality would prevent silent omissions but reject legitimate evolving universes.

## Implementation and evidence

- [src/credit/decomposition.rs](../../../../finstack-quant/factor/src/credit/decomposition.rs) — inspected implementation.
- [src/credit/peel.rs](../../../../finstack-quant/factor/src/credit/peel.rs) — inspected implementation.
- [src/credit/calibration/assemble.rs](../../../../finstack-quant/factor/src/credit/calibration/assemble.rs) — inspected implementation.

[tests/credit_peel_parity.rs](../../../../finstack-quant/factor/tests/credit_peel_parity.rs) was inspected: anchor and snapshot agree on a single-level example because they share the peel kernel; this is integration consistency, not an independent formula oracle. [tests/credit_decomposition.rs](../../../../finstack-quant/factor/tests/credit_decomposition.rs) locates reconstruction, folded-issuer, unknown-issuer and period-shape checks. Reconstruction algebra establishes conservation, not the economic merit of the bucket construction.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
