# Mode: Deep project / package / Rust crate review

**Purpose:** Examine system-wide discoverability, conceptual boundaries, public interfaces, local readability, and how maintainers would safely evolve the package. This is **architecture-first and evidence-driven**, not a bulk list of style suggestions.

## Pass 1 — Inventory and architecture map

1. Resolve the explicit project/package/crate boundary. Record source roots, project manifests, entry points, public APIs, language boundaries, tests, config, and relevant design docs/`AGENTS.md` guidance.
2. Enumerate the **in-scope owned handwritten source files** using Git tracked-file listings and/or repository-aware search (`rg --files` when available), adding relevant untracked owned files if the user wants the current worktree. Exclude vendored, generated, lock/build artifacts, caches, and outputs by default; document exclusions.
3. Sketch the dependency and responsibility map: each major module -> concept owned -> public surface -> upstream/downstream collaborators -> relevant tests. Identify important cross-boundary data and control flow.
4. State what the package appears to optimize for. Do not assume the current directory hierarchy reflects the best domain decomposition.

## Pass 2 — Module reviews

1. Work through *every relevant owned source module* when feasible, in manageable logical groups. Read source, interfaces and associated tests deeply enough to understand behavior and maintenance implications; a search hit, path inventory, or symbol skim is **not** a deep file review.
2. For each module, assess cohesion, discoverability, surprising dependencies, public/private API clarity, internal control flow, naming, side effects, contracts, and significant complexity. Record strengths and evidence-backed candidate issues.
3. Trace representative real developer tasks: locate a named behavior, follow an end-to-end path, add a variant/feature, and locate tests. Identify unnecessary jumps or hidden assumptions. Vary tasks across major functionality.
4. Keep a coverage ledger: total in-scope owned source files, fully examined, partially examined, context-only, and excluded. If time/context prevents complete inspection, **stop short of claiming an exhaustive review** and clearly limit conclusions.

## Pass 3 — Cross-module synthesis

Look beyond what can be seen within a single file:

- **Concept ownership:** Are policy, validation, transformation, persistence, and orchestration responsibilities placed predictably?
- **Boundary fitness:** Do modules map to stable domain concepts? Could moving one responsibility simplify multiple developer tasks?
- **Indirection tax:** How many layers/files/traits/wrappers/hooks must be traversed for one common operation? Are they justified?
- **Interface quality:** Does the public surface reveal a coherent model while hiding incidental mechanics? Are exports, naming, and errors consistent?
- **Dependency coherence:** Are there cycles, global service-locator patterns, broad “common/utils” hubs, or cross-cutting changes that routinely touch many modules?
- **Policy duplication vs. coupling:** Are duplicate rules truly the same concept? Would centralizing them improve clarity more than it increases coupling?
- **Human onboarding:** Where would a new contributor begin? Can they find entry points, conventions, and representative tests without reverse-engineering the repository?
- **Multi-language seams:** Are data contracts and authority of rules clear across TS/Python/Rust/SQL/Airflow or other components?

Consolidate candidate findings by root cause; don't repeat the same architecture concern in ten module-specific comments. Revisit contradictory observations before finalizing recommendations.

## Pass 4 — Improvement plan

1. Summarize the **current mental model** of the architecture with a compact module/responsibility map.
2. Prioritize 3–10 high-leverage changes, where supported, by reviewer benefit, breadth, implementation effort and regression risk. Smaller changes should precede major reshaping when feasible.
3. Where a major reorganization is warranted, give a proposed **target responsibility map** (current -> recommended owner), why it helps developer navigation, compatibility/API impacts, and a safe sequence: create boundaries -> migrate call sites -> tighten exports -> remove redundant layers -> update docs/tests.
4. Identify design choices and strengths that should remain unchanged. Do not recommend a rewrite because an architecture looks unfashionable.
5. State coverage in quantified terms with named major gaps. Say whether the review is **comprehensive**, **partial**, or **sampled**; a directory inventory alone does not establish completeness.

## Scaling and delegation

For a large repository, partition by cohesive package/module and retain a concise coverage ledger and inter-module notes across passes. If parallel review agents are available, give them **non-overlapping scopes and the same rubric**, require file/line evidence and explicit coverage, and reconcile findings centrally. Parallelism is optional; don't assume agents/tools exist. Never trade away architecture synthesis or truthfully reporting missing coverage just to claim a “deep” review.

## Output

Use the **deep review** format in `reporting.md`: executive assessment, architecture/navigation map, positives, prioritized findings, developer-task trace examples, phased improvement roadmap, and detailed coverage/limitations. Avoid numeric scores unless explicitly requested.
