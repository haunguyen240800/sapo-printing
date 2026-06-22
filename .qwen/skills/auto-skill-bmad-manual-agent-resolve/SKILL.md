---
name: bmad-manual-agent-resolve
description: Manually resolve BMad agent or workflow customization block when the Python resolver script is unavailable (e.g. Windows without Python).
source: auto-skill
extracted_at: '2026-06-23T08:44:20.967Z'
---

# Manual BMad Customization Resolver

## When to use

When activating any BMad agent or workflow skill and the standard resolver command fails:

```
python3 _bmad/scripts/resolve_customization.py --skill <skill-root> --key <agent|workflow>
```

This commonly happens on Windows systems without Python installed. Applies to:
- **Agent skills** (`bmad-agent-dev`, `bmad-agent-pm`, etc.) — use `--key agent`
- **Workflow skills** (`bmad-code-review`, `bmad-create-story`, etc.) — use `--key workflow`

## Procedure

### Step 1: Read the three customization files in order

Read these files from the project working directory and skill root:

1. **Base defaults**: `{skill-root}/customize.toml` — always exists
2. **Team overrides**: `{project-root}/_bmad/custom/{skill-name}.toml` — may not exist, skip if missing
3. **User overrides**: `{project-root}/_bmad/custom/{skill-name}.user.toml` — may not exist, skip if missing

Where `{skill-name}` is the skill directory basename (e.g. `bmad-agent-dev`).

### Step 2: Apply structural merge rules

Merge the files in order (1 → 2 → 3), applying these rules:

- **Scalars**: later value overrides earlier (e.g. `icon`, `role`, `communication_style`)
- **Tables (nested objects)**: deep-merge — child keys from later files override, missing keys preserved
- **Arrays of tables with `code` or `id` key**: replace matching entries by key, append new entries
  - e.g. `[[agent.menu]]` items: if `code = "DS"` exists in both, the later file's version replaces it
- **All other arrays**: append — items from later files are added to the end

### Step 3: Extract key fields for activation

**For `[agent]` blocks** (agent skills), extract:

- `icon` — emoji prefix for messages (e.g. `💻`)
- `role` — the agent's role description
- `identity` — the agent's identity/persona
- `communication_style` — how the agent speaks
- `principles` — array of value system entries
- `persistent_facts` — array of static context (entries prefixed `file:` are file paths/globs under `{project-root}`)
- `activation_steps_prepend` — steps to run before standard activation
- `activation_steps_append` — steps to run after greet
- `menu` — array of `[[agent.menu]]` entries, each with `code`, `description`, and either `skill` or `prompt`

**For `[workflow]` blocks** (workflow skills like `bmad-code-review`), extract:

- `persistent_facts` — array of static context
- `activation_steps_prepend` — steps to run before standard activation
- `activation_steps_append` — steps to run after greet
- `on_complete` — custom post-completion behavior (empty = none)

### Step 4: Load config

Read `{project-root}/_bmad/bmm/config.yaml` for:

- `user_name` — for greeting
- `communication_language` — language for all communications
- `document_output_language` — language for output documents
- `planning_artifacts` — output location for artifact scanning
- `project_knowledge` — additional context directory

### Step 5: Load persistent facts

For each entry in `persistent_facts`:
- If prefixed `file:` — resolve the path/glob under `{project-root}` and read the file contents as facts
- Otherwise — treat the entry as a verbatim fact

### Step 6: Adopt persona

Layer the customized persona: fill the role of `{agent.role}`, embody `{agent.identity}`, speak in the style of `{agent.communication_style}`, and follow `{agent.principles}`.

### Step 7: Greet and dispatch

- **Agent skills**: Greet `{user_name}` with `{agent.icon}` prefix in `{communication_language}`. If the user's initial message maps to a menu item, dispatch it directly. Otherwise present the menu.
- **Workflow skills**: Greet `{user_name}` in `{communication_language}`, then proceed directly to the workflow's first step file.

## Example merge

Given base `customize.toml`:
```toml
icon = "💻"
principles = ["Red, green, refactor"]
activation_steps_prepend = []
[[agent.menu]]
code = "DS"
description = "Dev story"
skill = "bmad-dev-story"
```

And team `bmad-agent-dev.toml`:
```toml
principles = ["No task complete without passing tests"]
[[agent.menu]]
code = "QD"
description = "Quick dev"
skill = "bmad-quick-dev"
```

Result:
- `icon` = "💻" (from base, not overridden)
- `principles` = ["Red, green, refactor", "No task complete without passing tests"] (arrays append)
- `menu` = [{code: "DS", ...}, {code: "QD", ...}] (arrays of tables with `code` key: new code appended)
