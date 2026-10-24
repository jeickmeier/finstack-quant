# Reporting contract

The report should be concise enough to act on, but specific enough that an unfamiliar developer can verify each finding without guessing. Don't pad the report to hit a quota. Use exact `path:line` references where possible; otherwise identify the symbol and explain the limitation. Describe proposed refactors as **recommendations**, not changes already made.

## Finding format

**[Priority] Clear, outcome-oriented title** — `path/to/file.ext:123` (and additional locations if needed)

- **Observed:** What the actual code currently does; name the relevant responsibilities, data flow, or symbols.
- **Human impact:** The concrete navigation, review, comprehension, or safe-change burden. Ground it in a plausible task.
- **Recommend:** The smallest improvement that addresses the observed root cause. Be explicit about which responsibility/interface would change.
- **Trade-off / confidence:** Acknowledge added coupling, compatibility, or migration risk for substantial changes; flag uncertainty rather than invent certainty.
- **Origin (uncommitted mode):** `introduced` / `pre-existing/contextual` / `uncertain`.

Don't use empty labels for every small observation if a tight paragraph communicates the point more clearly. Focus on demonstrated value rather than obligatory formatting.

## Short review (uncommitted or selected)

1. **Assessment:** 2–5 sentences on overall navigability, consistency, and biggest issue (or “no significant comprehension issues found”).
2. **Scope:** selected paths or change baseline; mention relevant exclusions and contextual files.
3. **Findings:** sorted Critical -> High -> Medium. Aim for up to ~3–7 distinct important findings for a typical small change; don't hide material findings to meet the guideline. Use exact source evidence and recommended action.
4. **What works:** 1–3 concrete strengths worth preserving.
5. **Reviewer/change notes:** diff cohesion for uncommitted; file responsibility map for selected when useful.
6. **Coverage:** what was fully examined, partially examined, and not examined; any blocked/unknown areas.

## Deep project review

1. **Executive assessment:** current architecture and its main comprehension bottleneck; strengths; overall review completeness.
2. **Architecture/navigation map:** concise table of major modules, owned responsibilities, public interfaces, and important dependencies (no invented components).
3. **Prioritized findings:** root causes with source evidence and reviewer/change impacts; avoid repeated symptoms across files.
4. **Developer-task traces:** 1–3 real, source-grounded navigation/change probes; cite key path/symbol transitions and the friction observed.
5. **Improvement roadmap:** `now / next / later` or phases with expected understanding benefit, effort (S/M/L relative), regression/compatibility risk, and ordering dependencies. If reorganizing: current vs. proposed responsibility ownership.
6. **Coverage & exclusions:** total relevant owned source files, fully reviewed, partially reviewed, unreviewed, generated/vendor exclusions, and major blind spots. Report **comprehensive / partial / sampled** honestly; explain counting basis.

## Anti-patterns to suppress

- “Use smaller functions,” without identifying confusing control flow or a safer decomposition.
- “Apply SOLID/DRY/Clean Architecture,” without showing what is easier to find, understand, or change.
- “Rename for clarity,” without a specific ambiguity and better name.
- “Extract a helper,” that adds a navigation hop and does not expose a useful concept.
- Redundant findings from the same root cause across multiple files.
- Speculative architectural claims based only on path names or search hits.
- Claims that tests/builds pass when they were not run; similarly, claims that code was refactored when the task was review-only.
- Overall numerical readability grades without a defined user-requested scoring framework.

## Example (illustrative, not a claim about the current repository)

**[High] Eligibility decisions have no clear owner** — `src/policy.py:86`, `src/api.py:140`

- **Observed:** Both the request handler and policy module decide eligibility using overlapping conditions.
- **Human impact:** Someone changing the eligibility policy must discover and reconcile two decision sites; updating one does not obviously cover the whole behavior.
- **Recommend:** Give the policy module one documented eligibility entry point, and make the handler responsible for input conversion and response handling only.
- **Trade-off:** Consolidation adds a dependency on the policy interface; confirm that API-specific constraints are genuinely policy decisions before moving them.
