# FACTOR-004: Credit issuer matching and hierarchy factor identifiers

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Map a credit dependency to a generic factor plus nonzero hierarchy betas. Known issuer rows take precedence over runtime attributes. Unknown issuers can use unit-beta runtime tags; generated calibration configs require an issuer-ID attribute.

## Input domain and conventions

Only dependencies matching Credit are handled after the configured dependency filter. Issuer identity comes from `credit::issuer_id`; runtime hierarchy tags come from `credit::<dimension_key>`. Built-in keys are lower-case rating/region/sector; custom keys are literal. A bucket ID is `credit::level{k}::{dimension_path}::{value_path}`, with dot-separated prefixes and the generic ID `credit::generic`.

## Algorithm stages

1. Sort issuer rows lexically and binary-search the issuer attribute. If `require_issuer_id` is set, an absent issuer attribute is an error; an unknown supplied ID can still enter the runtime-tag path.
2. For a known issuer, require the beta-level count to match the hierarchy and every hierarchy tag to exist. Emit the generic beta even if zero; emit only nonzero level betas with cached bucket IDs. Runtime tag overrides are ignored.
3. For an unknown issuer, start with generic beta 1. If no hierarchy tag is supplied, return generic-only. If any is supplied, require all hierarchy dimensions and reject dot-containing values; emit all corresponding level factors at beta 1.
4. Enumerate known nonzero level factors plus generic, deduplicated/sorted. Dynamic unknown-issuer buckets are not part of that static enumeration.
5. The public parser extracts level index and value path but does not verify the dimension path against a hierarchy.

## Selection and controls

Ad hoc `CreditHierarchicalConfig` defaults `require_issuer_id=false`; the calibrator sets true. Zero level betas denote folded levels and do not invent undeclared factor IDs. Shape and missing-tag failures propagate through cascades rather than trigger broad fallback.

## Limitations, failures, and fallbacks

Standalone matcher construction does not validate finite betas, duplicate issuer rows, dotted tags in known rows, or hierarchy uniqueness; callers must establish the intended validated config. Runtime unknown-issuer IDs can emit factors outside the static enumeration, leaving downstream universe handling material. Required issuer identity is not a requirement that the issuer already be calibrated.

## Alternatives and tradeoffs

Rejecting unknown issuers gives a closed universe but precludes controlled onboarding. Mapping unknown issuers only to generic, or to the nearest existing bucket, would avoid dynamic factors but changes risk allocation. Unit beta is an observed fallback, not evidence of predictive accuracy.

## Implementation and evidence

- [src/matching/credit.rs](../../../../finstack-quant/factor/src/matching/credit.rs) — inspected implementation.
- [src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) — inspected implementation.

[src/matching/credit.rs](../../../../finstack-quant/factor/src/matching/credit.rs) includes known/unknown issuer, beta-shape, missing-tag, folded-level and ID-stability checks. [src/config.rs](../../../../finstack-quant/factor/src/config.rs) supplies duplicate-row/universe checks. These establish branching semantics; they do not establish held-out issuer accuracy for unit betas.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
