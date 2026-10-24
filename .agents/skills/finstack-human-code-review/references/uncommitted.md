# Mode: Uncommitted work

**Purpose:** Make the current work easy for another developer to review and maintain *before committing*. Prioritize problems introduced by the change and the readability of the diff. Do not turn this into a historical audit of the containing package.

## Define the exact change set

1. Work from the Git repository root, respecting any explicit narrower scope. Identify the Git root (`git rev-parse --show-toplevel`) and the working-tree state.
2. Run the bundled helper when available:
   ```bash
   python3 <skill-directory>/scripts/worktree_inventory.py --repo .
   ```
   This enumerates **staged, unstaged, untracked, renames, deletions, and conflicts** without trusting a truncated `git diff` as a complete scope list. It does not read source contents. It deliberately omits ignored files; inspect ignored files only if the user explicitly targets them.
3. On repositories with a `HEAD` commit, read `git diff --no-ext-diff --no-textconv HEAD -- <scoped paths>` for tracked changes, and separately inspect the index/worktree when that distinction matters:
   ```bash
   git diff --no-ext-diff --no-textconv --cached -- <scoped paths>
   git diff --no-ext-diff --no-textconv -- <scoped paths>
   ```
   The combined diff **does not include untracked files**. Open each relevant untracked source file directly (and read its full content when feasible). If `HEAD` does not exist yet, inspect staged and unstaged diffs without a `HEAD` comparison, plus all untracked sources.
4. Include added files, modifications, renames, deletions, and interface changes. For deletions/renames, inspect affected callers and exports. Identify binary, generated, submodule, and non-source entries and say how they were treated. A merge conflict makes some behavior indeterminate; report the conflict instead of assuming the intended code.
5. If changes are very large, partition by feature or module, mark reviewed vs. unreviewed portions, and avoid claiming complete coverage.

## Context without scope creep

- Read enough surrounding code to establish local conventions, target behavior, existing abstractions, related tests and public interfaces.
- Attribute **newly introduced** issues to the diff when possible. Report pre-existing issues only if they materially affect the changed behavior and clearly label them **pre-existing/contextual**.
- Do not recommend broad unrelated rewrites as a condition for a small change.

## Questions specific to this mode

- Can a reviewer grasp the intent, boundaries, and side effects of this change from the code and available context?
- Are unrelated formatting, renames, generated output, incidental cleanup, or independent behaviors mixed into the same change?
- Did the change introduce a new helper, wrapper, type, or layer that costs more navigation than it saves?
- Does new code match existing module ownership, naming, error handling, and state conventions?
- Are business decisions, validation, and side effects now located where a future maintainer would expect?
- Are contracts and non-obvious invariants discoverable through comments, types, documentation, or focused tests?
- Is the diff reviewable in a coherent sequence, or does it force the reviewer to repeatedly jump between unrelated concerns?
- Would a *smaller* restructuring solve the issue more safely than a sweeping cleanup?

## Output

Use the **short review** format in `reporting.md`: verdict, change summary, up to ~3–7 significant findings (unless more are materially distinct), strengths, reviewability notes, and precise coverage. For each finding, say whether it is **introduced** or **pre-existing**. If there is nothing actionable, report that and do not invent minor style issues.

**Boundary case:** If the user explicitly asks to review only staged changes, ignore unstaged/untracked as primary targets (but disclose their existence if relevant); if they ask to review the entire uncommitted worktree, include all three.
