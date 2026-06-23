---
name: bmad-code-review-manual-fallback
description: Manual adversarial code review fallback when bmad-code-review subagent pipeline is unavailable (no python, subagents fail, or subagents return empty)
source: auto-skill
extracted_at: '2026-06-23T14:54:27.161Z'
---

# BMad Code Review — Manual Fallback

**When to use:** When the bmad-code-review workflow cannot launch its parallel subagents (python unavailable for customization resolver, subagents return empty findings, or subagents time out/fail).

**Goal:** Perform a thorough adversarial code review manually by reading source files directly, cross-referencing against the spec, and producing structured findings.

## Procedure

### 1. Gather Context

- Read the story spec file (typically `_bmad-output/implementation-artifacts/<story-key>.md`)
- Extract `baseline_commit` from frontmatter if present
- Read `sprint-status.yaml` to confirm story is in `review` status
- Get the full diff: `git diff <baseline> HEAD -- src-tauri/` (or appropriate source directory)
- Get `git status --short` to identify untracked new files
- Read all new (untracked) files in full
- Read the diff output for modified files

### 2. Load Affected Source Files

For every file mentioned in the diff, read the **current full content** (not just the diff hunk). This is critical because:
- Diff context is limited and may miss interactions between distant parts of a file
- You need to verify imports, trait implementations, and cross-file references
- Test files need full context to verify coverage

Also read any related files the diff touches indirectly (e.g., `mod.rs` exports, trait definitions, parent structs).

### 3. Adversarial Review Layers (Sequential)

Since parallel subagents aren't available, run these three review passes sequentially:

#### Layer 1 — Blind Review (diff-only mindset)
Mentally ignore the spec. Look at the code changes only. Ask:
- Does this code handle all error paths?
- Are there silent failures (functions returning `Ok(())` when nothing happened)?
- Are there resource leaks (files, connections, temp dirs)?
- Is the error handling consistent across similar functions?
- Are there race conditions or thread-safety issues?
- Is there code duplication that violates DRY?

#### Layer 2 — Edge Case Review
Focus on boundary conditions and unusual inputs:
- What happens with empty inputs, zero values, maximum values?
- What happens on duplicate operations (double-save, double-update)?
- What happens if the underlying resource disappears mid-operation?
- What happens with corrupt or malformed data from the database?
- Are there missing test cases for error-returning code paths?
- Are timestamps consistent (computed once vs. multiple calls to `now()`)?

#### Layer 3 — Spec Compliance Review
Cross-reference each acceptance criterion against the implementation:
- For each AC in the spec, verify the code implements it exactly
- Check SQL statements match spec exactly (column names, parameter order)
- Verify error types match spec (correct enum variants, correct error messages)
- Check that all specified tests exist and cover the right scenarios
- Verify file structure matches the spec's "What This Story Does NOT Do" exclusions

### 4. Produce Findings

Format each finding as:

```
### Finding N: <one-line title>
**Severity:** High | Medium | Low | Info
**File:** `path/to/file.rs`, <specific location>

<Description of the issue, why it matters, and evidence from code>

**Recommendation:** <specific fix suggestion, with code snippet if applicable>
```

**Severity guide:**
- **High:** Bug that causes incorrect behavior, data loss, or silent failure
- **Medium:** Missing error handling, unclear semantics, potential future bug
- **Low:** Missing test coverage, defensive improvement, code smell
- **Info:** Style nit, documentation gap, or cosmetic change

### 5. Summary Table

Conclude with a severity count table and overall assessment:

```
| Severity | Count |
|----------|-------|
| High     | N     |
| Medium   | N     |
| Low      | N     |
| Info     | N     |
```

Add 1-2 sentences on overall code quality and which findings are blockers vs. optional improvements.

## Key Principles

- **Always read full files**, not just diffs — diff context is insufficient for thorough review
- **Verify before asserting** — grep for functions, check imports, confirm trait implementations exist
- **Cross-reference the spec** — ACs are the contract; deviations are findings even if the code "works"
- **Be specific** — cite exact line numbers, method names, and SQL statements
- **Distinguish spec violations from improvements** — label clearly so the dev knows what's mandatory vs. optional
