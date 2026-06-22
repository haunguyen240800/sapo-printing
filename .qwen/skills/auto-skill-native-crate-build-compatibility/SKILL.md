---
name: native-crate-build-compatibility
description: Handle MSVC toolset version mismatches when adding native C/C++ Rust crates on Windows
source: auto-skill
extracted_at: '2026-06-23T08:41:19.763Z'
---

# Native Crate Build Compatibility on Windows

## When to use

When adding a Rust crate that compiles C/C++ code from source (e.g., `mupdf-sys`, `openssl-sys`, `libsqlite3-sys`) and encountering MSVC toolset version errors on Windows.

## Symptoms

Build fails with error **MSB8020**:
```
error MSB8020: The build tools for Visual Studio 2019 (Platform Toolset = 'v142') cannot be found.
```

Or similar toolset mismatch errors indicating the crate requires a specific Visual Studio version that doesn't match the installed version.

## Diagnosis

### Step 1: Check installed MSVC toolset

```powershell
dir "C:\Program Files (x86)\Microsoft Visual Studio\<version>\BuildTools\VC\Tools\MSVC\" /B
```

Look for version folders like `14.51.36231` (VS 2025 = v145) or `14.29.xxxxx` (VS 2019 = v142).

### Step 2: Identify the required toolset

The error message will specify the required Platform Toolset (e.g., `v142` for VS 2019, `v143` for VS 2022, `v145` for VS 2025).

## Solution Options

### Option 1: Install matching Visual Studio Build Tools (Recommended for production)

Download and install the specific Visual Studio version with the required toolset:
- VS 2019: https://visualstudio.microsoft.com/vs/older-downloads/
- VS 2022: https://visualstudio.microsoft.com/downloads/

During installation, select "Desktop development with C++" and ensure the required Windows SDK version is included.

### Option 2: Pivot to alternative crate with pre-built binaries (Quick workaround)

Many native crates have alternatives that use pre-built binaries instead of compiling from source:

| Original Crate | Alternative | Notes |
|----------------|-------------|-------|
| `mupdf-sys` | `pdfium-render` | PDF rendering, requires runtime DLL |
| `openssl-sys` | `rustls` | TLS/SSL, pure Rust |
| `libsqlite3-sys` (with `bundled`) | `rusqlite` (with `bundled`) | Already bundled in this project |

**Trade-offs**:
- ✅ No build toolchain dependencies
- ✅ Faster compilation
- ⚠️ May require runtime libraries (DLLs, shared objects)
- ⚠️ Slightly different API

### Option 3: Attempt toolset override (Unreliable)

**Warning**: This rarely works because build scripts regenerate files from scratch.

If the crate uses CMake and generates `.vcxproj` files in `target/debug/build/<crate>-<hash>/out/build/`:

1. Replace `<PlatformToolset>v142</PlatformToolset>` with your version (e.g., `v145`) in all `.vcxproj` files
2. Create `Directory.Build.props` in the build directory:
   ```xml
   <?xml version="1.0" encoding="utf-8"?>
   <Project ToolsVersion="4.0" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
     <PropertyGroup>
       <PlatformToolset>v145</PlatformToolset>
     </PropertyGroup>
   </Project>
   ```

**Why this fails**: Cargo build scripts regenerate the build files on each compilation, overwriting your changes.

## Runtime Dependency Setup (for pre-built binary alternatives)

When using crates with pre-built binaries (e.g., `pdfium-render`), you must distribute the runtime library:

### Step 1: Download the binary

For PDFium on Windows x64:
```bash
curl -L -o pdfium-win-x64.tgz "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F<version>/pdfium-win-x64.tgz"
```

Replace `<version>` with the latest tag (check with `curl -s "https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest" | findstr "tag_name"`).

### Step 2: Extract and place DLL

```bash
tar -xzf pdfium-win-x64.tgz bin/pdfium.dll
copy bin\pdfium.dll target\debug\
copy bin\pdfium.dll target\debug\deps\
```

**Critical**: The DLL must be in both `target/debug/` (for `cargo run`) and `target/debug/deps/` (for `cargo test`).

### Step 3: Verify at runtime

```rust
use pdfium_render::prelude::*;

let pdfium = Pdfium::default(); // Attempts to load pdfium.dll
```

If the DLL is missing, you'll get a panic or error when initializing the library.

## Example: Switching from mupdf-sys to pdfium-render

### Original (mupdf-sys)

```toml
# Cargo.toml
[dependencies]
mupdf-sys = "0.8"
```

```rust
use mupdf::{Document, Matrix, Colorspace};

let doc = Document::open("file.pdf")?;
let page = doc.load_page(0)?;
let pixmap = page.to_pixmap(&Matrix::new_scale(4.0, 4.0), &Colorspace::device_rgb(), false, true)?;
let bytes = pixmap.samples();
```

**Problem**: Requires VS 2019 build tools, fails on VS 2025 systems.

### Alternative (pdfium-render)

```toml
# Cargo.toml
[dependencies]
pdfium-render = { version = "0.9", features = ["thread_safe"] }
```

```rust
use pdfium_render::prelude::*;

let pdfium = Pdfium::default();
let document = pdfium.load_pdf_from_file("file.pdf", None)?;
let page = &document.pages().iter().next().unwrap();
let bitmap = page.render_with_config(&PdfRenderConfig::new().set_fixed_size(2480, 3508))?;
let bytes = bitmap.as_raw_bytes();
```

**Key differences**:
- PDFium renders in BGRx format (4 bytes/pixel), manual conversion needed for RGB/GRAY/BINARY
- Use `set_fixed_size()` to force exact dimensions (vs `set_target_width` + `set_maximum_height` which maintains aspect ratio)
- Runtime DLL required: `pdfium.dll` must be in PATH or working directory

## Decision Framework

When facing native crate build failures:

1. **Is this a production deployment?** → Install matching Visual Studio (Option 1)
2. **Is this development/testing?** → Try alternative crate (Option 2)
3. **Does the story/architecture allow alternatives?** → Check Dev Notes for approved alternatives
4. **Are runtime dependencies acceptable?** → Pre-built binaries require DLL distribution
5. **Is compilation speed critical?** → Pre-built binaries are much faster

## Project-Specific Notes

### This project (sapo-printing)

- **PDF rendering**: Use `pdfium-render` instead of `mupdf-sys` (implemented in Story 3.2)
- **Database**: Already using `rusqlite` with `bundled` feature (no external SQLite dependency)
- **TLS/HTTP**: Using `reqwest` with default TLS backend (no manual OpenSSL setup needed)
- **Windows APIs**: Using `windows` crate (pure Rust bindings, no C compilation)

### Build environment

- **Current MSVC**: VS 2025 Build Tools (v145, version 14.51.36231)
- **Rust version**: 1.96.0 (as of 2026-06-23)
- **Target**: Windows x64

## References

- Story 3.2 implementation: `_bmad-output/implementation-artifacts/3-2-implement-mupdf-renderer-with-color-mode-support.md`
- PDFium binaries: https://github.com/bblanchon/pdfium-binaries/releases
- pdfium-render docs: https://docs.rs/pdfium-render/latest/
