# Human-comprehension review rubric

Use these seven dimensions as **questions**, not independent quotas for findings. A symptom can span dimensions; report its root cause once. Source-code conventions and domain complexity matter.

| Dimension | Ask | High-value evidence |
| --- | --- | --- |
| **Readability & clarity** | Can a reader tell what each important operation means without mentally decoding it? | Ambiguous identifiers; compressed expressions; unclear branches; implicit assumptions; distracting implementation detail. |
| **Organization & discoverability** | Does a developer know where a behavior lives and what each module owns? | Misplaced domain policy; broad utility buckets; circular relationships; unclear public API; related behavior scattered across modules. |
| **Simplicity & abstraction cost** | Is unavoidable complexity expressed directly, or hidden behind unnecessary layers? | One-use frameworks; pass-through wrappers; unnecessary genericity; over-split helpers; excessive configuration for a fixed behavior. |
| **Consistency & predictability** | Do equivalent concerns follow coherent local patterns? | Inconsistent names, error conventions, state ownership, validation locations, or API styles across similar features. |
| **Behavioral transparency** | Can a reader identify inputs, effects, invariants, state changes, and failure paths? | Hidden I/O, mutation, transaction boundaries, concurrency, retries, defaults, magic transformations, or overloaded responsibilities. |
| **Explanations & contracts** | Is *why* a non-obvious choice exists discoverable? | Undocumented trade-offs; algorithm assumptions; public pre/postconditions; opaque domain terminology; comments that contradict code. |
| **Changeability & reviewer experience** | Can a new maintainer make and assess a safe, targeted change? | Large ripple effects; hard-to-locate tests; tight coupling; nonlocal dependencies; unrelated changes mixed into a diff. |

## Calibrate judgments

- A large file is not automatically a problem. Ask whether it has a cohesive purpose and clear internal navigation.
- A short function is not automatically clear. Include the cost of following helpers, wrappers, dependency injection, generics, and indirection.
- Duplication is not automatically worse than coupling. Similar-looking code may implement distinct concepts or be more understandable separately.
- Abstraction is warranted when it captures a stable concept, eliminates meaningful repeated decisions, or creates a useful public boundary. Do not ask to “DRY” incidental similarities.
- Domain difficulty is real. A difficult algorithm or transactional protocol may be best served by explicit, well-documented complexity rather than by further layers.
- Tests are review evidence and developer documentation. Check whether their names, fixtures, and locations expose intended behavior; do not turn the exercise into a test-coverage audit.
- Comments and type annotations are useful when they encode information not obvious from names, types, or local code.
- For AI-generated or heavily agent-edited code, look especially for locally tidy but globally fragmented structures, unused or duplicated helpers, redundant wrappers, and inconsistent patterns. Verify, don't presume authorship.

## Reviewer-task probes

Use actual symbols and paths; describe the route a developer would take.

1. **Find:** Where would a newcomer look to change a named externally visible behavior? Is the owning module apparent?
2. **Trace:** Follow a real value or request from entry point through transformation and side effects. Which jumps are conceptually necessary?
3. **Change:** What code, contracts, and tests would a small plausible behavior change affect? Are assumptions and blast radius discoverable?
4. **Debug (optional):** Where would a developer start after seeing a particular error/response? Are boundaries and errors visible?

A probe supports a finding only when you can point to avoidable friction. Do not fabricate probe results, source paths, or developer durations.

## Priority classification

- **Critical:** Essential behavior/invariants are so obscured or dispersed that ordinary changes have substantial, demonstrated risk. Rare in a readability-focused review.
- **High:** Material structural/comprehension issue likely to impede maintainers or reviewers repeatedly.
- **Medium:** Meaningful, localized clarity or maintainability problem; reasonable to address when working in the area.
- **Low:** Optional polish. Usually omit unless user wants exhaustive feedback.

Consider reviewer impact, frequency of encountering the code, size of affected surface, and cost/risk of the proposed fix. Do not automatically elevate an issue because the code is long, contains a macro, or differs from a favorite style.

## Findings: evidence threshold

For each finding, require:

- Actual location (specific file:line or symbol; multiple locations for distributed concerns).
- Concise description of *what the code does now* and *what makes understanding/changing it difficult*.
- Concrete consequence for a likely reviewer or maintenance task.
- Actionable recommendation at an appropriate scope, with trade-off if substantial.
- Verification against context so the recommendation does not ignore an existing reason for the design.

If material context is missing, label an **investigation question** separately rather than reporting a high-confidence defect.
