---
name: wdac-blocks-proc-macro-dll
description: Diagnose and work around Windows Application Control (WDAC) policy blocking Rust proc-macro DLLs during cargo build
source: auto-skill
extracted_at: '2026-06-23T14:47:43.348Z'
---

# Windows Application Control (WDAC) Blocking Proc-Macro DLLs

## When to use

When `cargo build` fails with **os error 4551** on Windows, indicating an Application Control policy is blocking a proc-macro or native DLL.

## Symptoms

Build fails with:
```
error: \\?\...\target\debug\deps\<crate>-<hash>.dll: LoadLibraryExW failed:
An Application Control policy has blocked this file. (os error 4551)
```

Common culprits: `webview2_com_macros`, `windows_*`, and other crates that ship native DLLs or proc-macro plugins.

## Diagnosis

### Step 1: Confirm it's a policy issue, not a code issue

Stash your changes and verify the error occurs on a known-good commit:
```bash
git stash && cargo build
```
If the same error appears, the problem is environmental, not caused by your code.

### Step 2: Identify the blocked file

The error message names the exact DLL path. Check if it exists:
```powershell
dir "<path-from-error>"
```
If the file exists but can't be loaded, WDAC is blocking it.

### Step 3: Check individual crates

Proc-macro crates and their dependent crates may still compile in isolation:
```bash
cargo check -p <blocked-crate-name>
cargo check -p tauri-runtime
cargo check -p tauri-runtime-wry
cargo check -p tauri
```
If these pass individually but the full build fails, the issue is at the DLL load stage during final linking, not at the code compilation stage.

## Solutions

### Option 1: Request WDAC policy exception (Enterprise)

If this is a managed/corporate machine:
1. Contact IT/security team with the DLL path and hash
2. Request an allow-listing for the crate's DLL
3. Provide business justification (development toolchain requirement)

### Option 2: Use a different machine for building

Move the build to a machine without WDAC enforcement:
- Personal development machine
- CI/CD pipeline (GitHub Actions, Azure DevOps, etc.)
- WSL2 (Linux subsystem has different security model)

### Option 3: Verify code correctness without full build

While blocked from a full `cargo build`, you can still verify your code:
```bash
cargo fmt --check          # Formatting check
cargo check -p <your-crate>  # Type-check your crate specifically
cargo clippy -p <your-crate>  # Lint your crate
```

### Option 4: Use `cargo check` for compilation verification

`cargo check` performs full type-checking and dependency resolution without producing final binaries. It often succeeds even when `cargo build` fails at the DLL load stage:
```bash
cargo check --lib
cargo check --lib --all-targets
```

## Why this happens

Windows Defender Application Control (WDAC) — formerly known as Device Guard — enforces code integrity policies that prevent unsigned or untrusted DLLs from loading. Proc-macro crates compile to DLLs that rustc loads at compile time. On enterprise-managed machines with strict WDAC policies, these build-artifact DLLs may not match allowed publishers or hashes.

## Project-specific notes (sapo-printing)

- **Affected crate**: `webview2_com_macros` (proc-macro for WebView2 COM bindings, dependency of `tauri`)
- **Root cause**: WDAC policy blocks `webview2_com_macros-*.dll` in `target/debug/deps/`
- **Impact**: Full `cargo build` and `cargo test` fail, but `cargo check -p tauri` and `cargo fmt` succeed
- **Workaround**: Use `cargo check -p <specific-crate>` for incremental verification; push to CI for full build validation
