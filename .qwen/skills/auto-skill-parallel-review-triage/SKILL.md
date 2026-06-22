---
name: parallel-review-triage
description: Execute parallel adversarial code reviews (Blind Hunter, Edge Case Hunter, Acceptance Auditor), triage overlapping findings into a deduplicated priority list, and batch-fix all issues efficiently.
source: auto-skill
extracted_at: '2026-06-24T14:52:49.500Z'
---

# Parallel Review Execution & Batch-Fix Methodology

## When to use

After completing a code review via `bmad-code-review` or when you have multiple adversarial review layers that need to be consolidated and their findings fixed systematically.

## Step 1: Run Parallel Reviews

Launch three `agent` subagents simultaneously — each with a different context boundary:

1. **Blind Hunter** — receives diff only. No spec, no project context. Focus: correctness, security, resource management.
   ```
   Invoke via: agent with bmad-review-adversarial-general skill, passing full diff as context
   ```

2. **Edge Case Hunter** — receives diff + read access to project. Focus: boundary conditions, unhandled paths, race conditions.
   ```
   Invoke via: agent with bmad-review-edge-case-hunter skill, passing diff + project context
   ```

3. **Acceptance Auditor** — receives diff + spec file + context docs. Focus: AC violations, spec deviations, missing tests.
   ```
   Invoke via: agent with custom prompt that references the spec and acceptance criteria
   ```

**Important:** If diff output is large (>3000 lines), it may be truncated in tool output. Save to a temp file first and reference it, or use `read_file` with the saved path to ensure subagents receive the complete diff.

## Step 2: Triage Findings

Each reviewer returns overlapping findings. Triage follows this process:

### 2a. Collapse duplicates
- Same finding from multiple reviewers → merge into one entry, credit all layers
- Example: `list_printers omits merge` found by Blind Hunter + Acceptance Auditor → single finding with both credited

### 2b. Assign severity levels
| Level | Criteria |
|-------|----------|
| **Critical** | Functionality broken, spec violation that causes runtime failure, security hole |
| **High** | Error handling gap that masks failures, missing validation with security impact, untested critical path |
| **Medium** | Code quality issue, missing test for specified behavior, architectural deviation |
| **Low** | Cosmetic inconsistency, minor naming issue, future-maintenance concern |

### 2c. Build actionable table
Present findings in a table format:
- Column 1: Severity + ID (C-1, H-2, M-3, L-1)
- Column 2: Finding title
- Column 3: Which reviewer layer(s) found it
- Column 4: One-line detail

## Step 3: Plan Batch Fixes

### 3a. Group by file
Cluster all fixes by the source file they affect. This minimizes file read/write cycles.

### 3b. Identify independent edits
Within each file, order edits so they don't conflict:
- Imports/uses declarations first
- Type/struct changes before consumers
- Logic changes before tests
- Test additions last

### 3c. Build a todo list
Create a todo list tracking every finding with status. Group related fixes under one todo item when they affect the same code location.

## Step 4: Execute Batch Fixes

For each file:

1. **Read the current file** before every edit — never rely on cached content
2. **Make multiple independent edits** using `edit` tool — one edit per logical change
3. **Verify after each file** — run `cargo check` (or equivalent) to catch compilation errors early
4. **Handle cascading changes** — if a type change breaks downstream code, fix those in the same edit cycle

### Edit ordering rules
- Always fix the **most upstream** change first (e.g., fix a struct before fixing its consumers)
- When a single edit introduces a new import, add it to the import block in the same edit
- When fixing tests, update test mocks/fixtures before adding new test cases

## Step 5: Final Verification

1. Run full compilation check: `cargo check` + `cargo check --tests`
2. Distinguish pre-existing failures from new ones by testing against baseline (git stash + build + git stash pop)
3. Run tests if compilation passes: `cargo test`
4. Report: what passed, what was pre-existing failures, what still needs attention

## Anti-patterns to avoid

- **Don't** batch all edits into a single massive `write_file` — lose the ability to isolate which change broke things
- **Don't** fix findings in the order they were reported — fix in dependency order (upstream first)
- **Don't** assume compilation errors are new — verify against baseline before investigating
- **Don't** skip reading the file before each `edit` — concurrent edits may have invalidated cached line numbers
