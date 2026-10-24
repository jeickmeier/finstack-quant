# Mode: Selected files or directories

**Purpose:** Evaluate the intrinsic comprehension and organization of named files or a narrow directory, not just differences from a Git baseline.

## Select and inspect

1. Resolve the user's literal targets (individual files, glob patterns, or directories) and list the resolved files. Treat a directory as recursive within that directory, unless the user indicates otherwise. Avoid silently expanding to sibling modules or other packages.
2. Identify handwritten source, tests, docs, generated/vendor material, and unreadable/binary files. Review generated code only when explicitly relevant. Do not follow symlinks outside the intended project scope unless authorized.
3. Read **complete selected files** where feasible, not just search hits. Where a file is too large, inspect it systematically by sections, record the limitation, and do not imply full coverage.
4. Examine the nearest parent package/module, relevant interfaces, exports, representative callers, callees, and tests to establish the design context. These are **context-only files**, unless specifically included in the requested target list.
5. Build a concise responsibility map: file -> principal responsibility -> public surface -> main collaborators. Question whether the conceptual ownership is where a new developer would expect.
6. Use the rubric and concrete developer-task probes to find verified obstacles. Confirm each finding against surrounding conventions and constraints.

## Questions specific to this mode

- Does this file have an intelligible, cohesive responsibility? If it has several responsibilities, is their co-location useful or confusing?
- Do the file name, exported symbols, and internal names make the code's behavior discoverable?
- Is there a clear path through the major operation, or do tiny helpers/traits/hooks/wrappers obscure it?
- Are side effects, cache/state mutation, default behaviors, retries, and exception/error boundaries easy to locate?
- Are public contracts and non-obvious invariants apparent to callers?
- Do tests give a human maintainer an understandable model of behavior and a way to verify a change?
- Is splitting or consolidating code actually an improvement, once the cost of navigation and dependencies is included?
- Is the apparent issue an established local convention or a meaningful violation of it?

## Scope management

- Primary findings should concern selected targets. When a selected file depends on a confusing out-of-scope interface, explain that dependency as context rather than quietly broadening the audit.
- A finding that necessarily involves two files may cite both, but distinguish selected vs. contextual locations.
- If the user names a directory that is effectively a whole crate/project yet requests only “review these files,” stay in selected-file mode; offer or describe further architectural analysis only if justified by evidence.

## Output

Use `reporting.md`: short overall assessment, responsibility map when it helps, prioritized findings, positives, and a file coverage statement listing selected vs. context-only scope. Omit generic line-size or naming pedantry. If a cohesive 500-line file is easier to follow than the proposed split, say so.
