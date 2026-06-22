---
baseline_commit: 9538156
parent_story: 2-5-create-printer-configuration-ui-basic-settings-3-sections
---

# Story 2.6: Create Printer Configuration UI - Advanced Settings (2 Sections)

Status: in-progress

## Story

As a **nhân viên kho**,
I want **advanced printer settings in a separate section**,
So that **I can configure print mode and buffer settings when needed without cluttering the basic UI**.

## Acceptance Criteria

**Given** Story 2.5 (Basic Config UI) is complete
**When** I add advanced configuration sections

**AC-1: Extend PrinterConfigForm with Print Mode section (Section 4)**
- Add Section 4 titled "In ảnh" (Print Mode)
- Add "In ảnh" checkbox (print_as_image field)
- Add "Loại ảnh in" dropdown (color_mode field) - shown only when "In ảnh" is checked
- Color mode options: RGB/ARGB/BGR/GRAY/BINARY
- Default: print_as_image = false, color_mode = 'RGB'
- Use @sapo/ui-components (Checkbox, Select)

**AC-2: Extend PrinterConfigForm with Advanced section (Section 5)**
- Add Section 5 titled "Cài đặt nâng cao" (Advanced)
- Add "Bật Printing Buffer" checkbox (enable_buffer field)
- Add "Kích thước Buffer (KB)" number input (buffer_size_kb field) - shown only when buffer enabled
- Default: enable_buffer = false, buffer_size_kb = null
- Use @sapo/ui-components (Checkbox, TextField)

**AC-3: Update form validation schema**
- Add print_as_image: boolean, not required
- Add color_mode: string, not required (only validated when print_as_image = true)
- Add enable_buffer: boolean, not required
- Add buffer_size_kb: number, nullable
  - When enable_buffer = true: required, range 1-1024 KB
  - When enable_buffer = false: can be null
- Update yup schema with conditional validation

**AC-4: Update PrinterConfigDto backend**
- Extend `src-tauri/src/interface/tauri/dtos/printer_dto.rs`
- Add fields to PrinterConfigDto:
  - print_as_image: bool
  - color_mode: String
  - enable_buffer: bool
  - buffer_size_kb: Option<u32>
- All fields must be Serialize + Deserialize

**AC-5: Update save_printer_config command validation**
- In `src-tauri/src/interface/tauri/commands/printer.rs`
- Add validation for buffer_size_kb: 1-1024 KB if enable_buffer = true
- Validate buffer_size_kb is None when enable_buffer = false (reject inconsistent state)
- Validate color_mode is one of: RGB, ARGB, BGR, GRAY, BINARY
- Return Vietnamese error messages for validation failures

**AC-6: Update database persistence**
- No schema changes needed (columns already exist from Story 2.1)
- Update `save_printer_config` to persist new fields to printer_configs table
- Fields: print_as_image (BOOLEAN), color_mode (TEXT), enable_buffer (BOOLEAN), buffer_size_kb (INTEGER NULL)

**AC-7: Visual separation between sections**
- Use visual headers to distinguish "Cài đặt cơ bản" (Sections 1-3) vs "Cài đặt nâng cao" (Sections 4-5)
- Consider collapsible sections or clear visual dividers
- Maintain visual consistency with Sections 1-3 (same font, spacing, border radius)
- Section headers should use same styling as existing sections

**AC-8: Manual testing verification**
- All 5 sections render correctly
- Print Mode section:
  - "In ảnh" checkbox toggles color_mode dropdown visibility
  - Color mode dropdown shows 5 options
  - Default RGB selected when checkbox enabled
  - Uncheck "In ảnh" clears color_mode value (verify dropdown resets)
- Advanced section:
  - "Bật Printing Buffer" checkbox toggles buffer_size_kb input visibility
  - Buffer size validates 1-1024 KB range
  - Validation error shows in Vietnamese
  - Uncheck "Bật Buffer" clears buffer_size_kb value (verify input clears)
- Form saves all fields correctly (verify in database)
- Existing functionality (Sections 1-3) remains unchanged

## Tasks / Subtasks

- [ ] **Task 1: Update DTOs with advanced fields** (AC: #4)
  - [ ] Update PrinterConfigDto in `printer_dto.rs` (currently ~26 lines, add 4 fields after line 25)
  - [ ] Add print_as_image: bool
  - [ ] Add color_mode: String
  - [ ] Add enable_buffer: bool
  - [ ] Add buffer_size_kb: Option<u32>
  - [ ] Ensure all fields are Serialize + Deserialize
  - [ ] **IMPORTANT:** After updating Rust DTO, run `cargo build` BEFORE updating TypeScript
  - [ ] Update TypeScript PrinterConfigFormData interface to match exactly

- [ ] **Task 2: Update backend validation** (AC: #5, #6)
  - [ ] Update save_printer_config command in `commands/printer.rs`
  - [ ] Add buffer_size_kb validation: 1-1024 KB if enabled
  - [ ] Add color_mode validation: must be RGB|ARGB|BGR|GRAY|BINARY
  - [ ] Add Vietnamese error messages
  - [ ] Update database INSERT/UPDATE to include new fields
  - [ ] Use existing printer_configs table columns (no migration needed)

- [ ] **Task 3: Extend frontend form state** (AC: #1, #2)
  - [ ] Update PrinterConfigFormData interface in PrinterConfigForm.tsx
  - [ ] Add print_as_image: boolean
  - [ ] Add color_mode: string
  - [ ] Add enable_buffer: boolean
  - [ ] Add buffer_size_kb: number | null | undefined
  - [ ] Update defaultValues in useForm

- [ ] **Task 4: Extend yup validation schema** (AC: #3)
  - [ ] Add print_as_image: yup.boolean()
  - [ ] Add color_mode: yup.string() with conditional validation
    - Required when print_as_image = true
    - Must be one of: RGB, ARGB, BGR, GRAY, BINARY
  - [ ] Add enable_buffer: yup.boolean()
  - [ ] Add buffer_size_kb: yup.number().nullable() with conditional validation
    - When enable_buffer = true: required, min(1), max(1024)
    - When enable_buffer = false: nullable
  - [ ] Add Vietnamese error messages

- [ ] **Task 5: Add Section 4 - Print Mode UI** (AC: #1, #7)
  - [ ] Add Section 4 container with title "4. In ảnh"
  - [ ] Add Checkbox for "In ảnh" (print_as_image)
  - [ ] Use Controller for print_as_image field
  - [ ] Add conditional Select for color_mode (shown when print_as_image = true)
  - [ ] Color mode options: RGB (24-bit), ARGB (32-bit với alpha), BGR (Windows default), GRAY (8-bit grayscale), BINARY (1-bit monochrome)
  - [ ] Use @sapo/ui-components (Checkbox, Select)

- [ ] **Task 6: Add Section 5 - Advanced UI** (AC: #2, #7)
  - [ ] Add Section 5 container with title "5. Cài đặt nâng cao"
  - [ ] Add Checkbox for "Bật Printing Buffer" (enable_buffer)
  - [ ] Use Controller for enable_buffer field
  - [ ] Add conditional TextField for buffer_size_kb (shown when enable_buffer = true)
  - [ ] Use type="number" with KB suffix or helper text
  - [ ] Use @sapo/ui-components (Checkbox, TextField)

- [ ] **Task 7: Add visual separation** (AC: #7)
  - [ ] Add visual header before Section 1: "Cài đặt cơ bản"
  - [ ] Add visual header before Section 4: "Cài đặt nâng cao"
  - [ ] Use consistent styling (font size, color, spacing)
  - [ ] Ensure clear distinction between basic and advanced settings

- [ ] **Task 8: Test conditional rendering** (AC: #8)
  - [ ] Test print_as_image checkbox toggles color_mode dropdown
  - [ ] Test enable_buffer checkbox toggles buffer_size_kb input
  - [ ] Test unchecking print_as_image clears color_mode field
  - [ ] Test unchecking enable_buffer clears buffer_size_kb field
  - [ ] Test validation errors display correctly
  - [ ] Test form submission with all fields
  - [ ] Verify database persistence (check printer_configs table)

- [ ] **Task 9: Cargo verification**
  - [ ] `cargo build` — zero errors
  - [ ] `cargo test` — all tests pass
  - [ ] `cargo clippy` — zero warnings

- [ ] **Task 10: TypeScript verification**
  - [ ] `pnpm exec tsc --noEmit` — zero errors

## Dev Notes

### ⚠️ CRITICAL: Verify Backend State Before Starting

**Before implementing, verify in `commands/printer.rs`:**

1. **Check current SQL query:** Does `save_printer_config` INSERT/UPDATE statement already include these 4 columns?
   ```rust
   // Look for: ...print_as_image, color_mode, enable_buffer, buffer_size_kb...
   ```

2. **If columns are present:** Just add DTO fields and validation (Tasks 1-2)
3. **If columns are missing:** Add them to both INSERT and UPDATE parts of UPSERT query

**Why critical:** Prevents wasting time checking if backend is incomplete when it's actually ready (or vice versa).

---

### 🎯 Story Scope

**This story:** Extends Story 2.5 UI with 2 additional sections (Print Mode, Advanced). Adds 4 new fields to form state, validation, DTO, and persistence.

**New sections:**
- Section 4: Print Mode (In ảnh + Loại ảnh in)
- Section 5: Advanced (Bật Buffer + Kích thước Buffer)

**Modified files:**
```
src/components/printer/PrinterConfigForm.tsx              (EXTEND with 2 sections)
src-tauri/src/interface/tauri/dtos/printer_dto.rs       (ADD 4 fields to PrinterConfigDto)
src-tauri/src/interface/tauri/commands/printer.rs       (ADD validation + persist new fields)
```

**DO NOT touch:**
- Database schema (printer_configs table already has these columns from Story 2.1)
- Domain layer (no changes needed)
- Sections 1-3 (already complete, do not modify)
- PrinterSelector, PrinterStatus components (already complete)

### 🏗️ Architecture Context

From Story 2.5, we have:
- **PrinterConfigForm:** 3 sections (Printer Selection, Paper Settings, Layout)
- **Backend:** Tauri commands (list_printers, save_printer_config, get_printer_status)
- **DTOs:** PrinterDto, PrinterConfigDto, PrinterStatusDto
- **Validation:** react-hook-form + yup schema
- **UI:** @sapo/ui-components (TextField, Select, Checkbox, Button, Banner)

This story **extends** existing components, not creates new ones.

### 🗄️ Database Schema (ALREADY EXISTS - No Migration)

From Story 2.1, `printer_configs` table already has:
```sql
CREATE TABLE printer_configs (
    ...
    color_mode TEXT NOT NULL DEFAULT 'RGB',      -- RGB, ARGB, BGR, GRAY, BINARY
    print_as_image BOOLEAN NOT NULL DEFAULT 0,   -- 0 or 1
    enable_buffer BOOLEAN NOT NULL DEFAULT 0,    -- 0 or 1
    buffer_size_kb INTEGER DEFAULT NULL,         -- 1-1024 KB, NULL if disabled
    ...
);
```

**No migration needed** — columns exist, just need to persist values from form.

---

### 📦 Current Form State (Story 2.5)

```tsx
interface PrinterConfigFormData {
  printer_name: string;
  paper_size: string;
  paper_width: number | null | undefined;
  paper_height: number | null | undefined;
  orientation: string;
  margin_left: number;
  margin_right: number;
  margin_top: number;
  margin_bottom: number;
  // ADD THESE 4 FIELDS:
  // print_as_image: boolean;
  // color_mode: string;
  // enable_buffer: boolean;
  // buffer_size_kb: number | null | undefined;
}
```

---

### 🎨 UI Pattern: Conditional Rendering

**Pattern:** Checkbox toggles visibility of dependent field

```tsx
// 1. Watch the checkbox field
const checkboxField = watch('checkbox_field_name');

// 2. Checkbox with onChange
<Checkbox
  label="Label Text"
  checked={checkboxField}
  onChange={(checked) => setValue('checkbox_field_name', checked)}
/>

// 3. Conditional dependent field
{checkboxField && (
  <Controller
    name="dependent_field"
    control={control}
    render={({ field }) => (
      <InputComponent {...field} error={errors.dependent_field?.message} />
    )}
  />
)}
```

**Apply this pattern for:**
- `print_as_image` toggles `color_mode` dropdown (Section 4)
- `enable_buffer` toggles `buffer_size_kb` input (Section 5)

**Color mode options:**
```tsx
{ label: 'RGB (24-bit)', value: 'RGB' },
{ label: 'ARGB (32-bit với alpha)', value: 'ARGB' },
{ label: 'BGR (Windows default)', value: 'BGR' },
{ label: 'GRAY (8-bit grayscale)', value: 'GRAY' },
{ label: 'BINARY (1-bit monochrome)', value: 'BINARY' },
```

---

### 🧪 Yup Conditional Validation Pattern

```tsx
const schema = yup.object({
  // ... existing fields from Story 2.5 (see PrinterConfigForm.tsx lines 24-78)
  
  // NEW FIELDS for Story 2.6:
  print_as_image: yup.boolean().required(),
  
  color_mode: yup
    .string()
    .when('print_as_image', {
      is: true,
      then: (schema) =>
        schema
          .required('Loại ảnh in không được để trống khi bật "In ảnh"')
          .oneOf(
            ['RGB', 'ARGB', 'BGR', 'GRAY', 'BINARY'],
            'Loại ảnh in không hợp lệ'
          ),
    }),
  
  enable_buffer: yup.boolean().required(),
  
  buffer_size_kb: yup
    .number()
    .nullable()
    .transform((value, original) => (original === '' ? null : value))
    .typeError('Phải là số')
    .when('enable_buffer', {
      is: true,
      then: (schema) =>
        schema
          .required('Kích thước buffer bắt buộc khi bật buffer')
          .min(1, 'Kích thước buffer phải trong khoảng 1-1024 KB')
          .max(1024, 'Kích thước buffer phải trong khoảng 1-1024 KB'),
      otherwise: (schema) =>
        schema
          .nullable()
          .test(
            'buffer-disabled',
            'Phải bỏ trống kích thước buffer khi tắt buffer',
            (value) => value === null || value === undefined
          ),
    }),
});
```

---

### 🔧 Backend Validation Pattern

```rust
// In save_printer_config command

// Validate buffer_size_kb
if config.enable_buffer {
    match config.buffer_size_kb {
        Some(size) if (1..=1024).contains(&size) => {
            // Valid
        }
        Some(size) => {
            return Err(format!(
                "Kích thước buffer phải trong khoảng 1-1024 KB (nhận được: {} KB)", 
                size
            ));
        }
        None => {
            return Err("Kích thước buffer bắt buộc khi bật buffer".to_string());
        }
    }
} else if config.buffer_size_kb.is_some() {
    // Edge case: buffer disabled but size provided
    return Err("Không thể đặt kích thước buffer khi buffer đã tắt".to_string());
}

// Validate color_mode
const VALID_COLOR_MODES: &[&str] = &["RGB", "ARGB", "BGR", "GRAY", "BINARY"];
if !VALID_COLOR_MODES.contains(&config.color_mode.as_str()) {
    return Err(format!(
        "Loại ảnh in không hợp lệ: '{}'. Chỉ chấp nhận: RGB, ARGB, BGR, GRAY, BINARY",
        config.color_mode
    ));
}

// Update SQL INSERT/UPDATE
db.execute(
    "INSERT INTO printer_configs (..., print_as_image, color_mode, enable_buffer, buffer_size_kb, ...)
     VALUES (..., ?19, ?20, ?21, ?22, ...)
     ON CONFLICT(device_id) DO UPDATE SET
         ...
         print_as_image = excluded.print_as_image,
         color_mode = excluded.color_mode,
         enable_buffer = excluded.enable_buffer,
         buffer_size_kb = excluded.buffer_size_kb,
         ...",
    params![..., config.print_as_image, config.color_mode, config.enable_buffer, config.buffer_size_kb, ...]
)?;
```

---
    params![..., config.print_as_image, config.color_mode, config.enable_buffer, config.buffer_size_kb, ...]
)?;
```

### 📍 Current File Context

**PrinterConfigDto location:** `src-tauri/src/interface/tauri/dtos/printer_dto.rs`
- Current: ~26 lines (ends at line 26)
- Add 4 fields after line 25 (before closing brace)
- Expected final: ~34 lines

**PrinterConfigForm location:** `src/components/printer/PrinterConfigForm.tsx`
- Current: 315 lines with 3 sections
- Add ~160 lines for Sections 4-5
- Expected final: ~475 lines

---

### 🎯 Essential References

**Must Read First:**
1. **PrinterConfigForm.tsx** (315 lines) — your extension target
2. **Story 2.5** + **Story 2.5.1** — patterns and learnings
3. **printer_dto.rs** — DTO structure you'll extend

**For Validation Logic:**
- **commands/printer.rs** — backend validation patterns
- **architecture.md FR-2.3** — color mode technical details

---

### 🚨 Common Pitfalls from Story 2.5

**Current File Locations:**
- `src/components/printer/PrinterConfigForm.tsx` — 315 lines, 3 sections
- `src/components/printer/PrinterSelector.tsx` — 83 lines
- `src/components/printer/PrinterStatus.tsx` — 69 lines
- `src-tauri/src/interface/tauri/commands/printer.rs` — Tauri commands
- `src-tauri/src/interface/tauri/dtos/printer_dto.rs` — DTOs

**From Story 2.5 learnings:**
- Use `Controller` from react-hook-form for @sapo/ui components
- Use `watch()` to track field values for conditional rendering
- Use `setValue()` with `{ shouldValidate: true }` for programmatic updates
- Number inputs: `value={field.value?.toString() || ''}` to handle null/undefined
- Checkbox: `checked={watch('field')}` and `onChange={(checked) => setValue('field', checked)}`

**From Story 2.5.1 migration:**
- All form elements use @sapo/ui-components
- Banner for notifications (success/error)
- TextField for number inputs with error prop
- Select with options array format `{ label, value }`
- Checkbox with label and checked props

### 🎨 Visual Separation Strategy (AC-7)

**Approach:** Add styled section category headers

```tsx
// Category Header Component
const SectionCategory = ({ title }: { title: string }) => (
  <h2 style={{ 
    fontSize: '20px', 
    fontWeight: 600, 
    marginBottom: '16px',
    borderBottom: '2px solid #e0e0e0',
    paddingBottom: '8px'
  }}>
    {title}
  </h2>
);

// Usage:
<SectionCategory title="Cài đặt cơ bản" />
{/* Sections 1-3 */}

<SectionCategory title="Cài đặt nâng cao" />
{/* Sections 4-5 */}
```

--- 
    marginBottom: '16px',
    borderBottom: '2px solid #e0e0e0',
    paddingBottom: '8px'
  }}>
    Cài đặt nâng cao
  </h2>

  {/* Section 4: Print Mode */}
  <div style={{ marginBottom: '24px', ... }}>...</div>
  
  {/* Section 5: Advanced */}
  <div style={{ marginBottom: '24px', ... }}>...</div>
</div>
```

### 🚨 Common Pitfalls from Story 2.5

**Frontend:**
1. ❌ **DO NOT** forget `nullable()` and `.transform()` for optional number fields
2. ❌ **DO NOT** use `.typeError()` without message — always add Vietnamese text
3. ❌ **DO NOT** skip `when()` conditions — color_mode/buffer_size_kb are conditionally required
4. ❌ **DO NOT** forget to handle null in TextField value: `field.value?.toString() || ''`

**Backend:**
2. ❌ **DO NOT** skip validation — always validate before persisting
3. ❌ **DO NOT** return English error messages — all errors must be Vietnamese
4. ❌ **DO NOT** forget to update both INSERT and UPDATE in UPSERT query

**Testing:**
1. ❌ **DO NOT** skip testing checkbox toggles — ensure conditional fields show/hide
2. ❌ **DO NOT** test only valid inputs — test edge cases (0 KB, 1025 KB, invalid color modes)
3. ❌ **DO NOT** assume defaults work — verify checkboxes start unchecked, dropdowns have default values

### 📄 Color Mode Technical Context (from Architecture.md)

**FR-3.3: Color Mode Conversion**
- RGB: 24-bit (8 bits per channel) — standard color printing
- ARGB: 32-bit với alpha channel — supports transparency
- BGR: Windows default byte order — some printers require this format
- GRAY: 8-bit grayscale — monochrome printing
- BINARY: 1-bit monochrome — lowest quality, fastest printing

**User-facing labels** (Vietnamese with technical details):
- "RGB (24-bit)" — most common
- "ARGB (32-bit với alpha)" — with transparency
- "BGR (Windows default)" — for compatibility
- "GRAY (8-bit grayscale)" — black and white
- "BINARY (1-bit monochrome)" — fastest


### 🔗 Dependencies

**Blocked by:** Story 2.5 (DONE), Story 2.5.1 (DONE) — must complete before starting this story

**Blocks:** None — Story 2.6 is the final UI story in Epic 2

### 💡 Implementation Strategy

**Order of Implementation:**
1. **Backend first:** Update DTOs → Add validation → Update persistence
2. **Frontend next:** Update interface → Add fields → Extend validation → Add UI sections
3. **Test:** Cargo tests → TypeScript compilation → Manual browser testing

**Why backend first?**
- DTOs must match between Rust and TypeScript
- Validation rules defined once, enforced both sides
- Backend compilation errors caught early
- Frontend can use updated types immediately

**Verification checkpoints:**
- After Task 2: `cargo build && cargo test` must pass
- After Task 4: `pnpm exec tsc --noEmit` must pass with updated interface
- After Task 6: Full form renders with 5 sections
- After Task 8: Manual test all conditional rendering + validation + save

## File Modification Plan

**Backend (Rust):**
1. `src-tauri/src/interface/tauri/dtos/printer_dto.rs`
   - ADD: 4 fields to PrinterConfigDto struct
   - Lines affected: ~10 lines

2. `src-tauri/src/interface/tauri/commands/printer.rs`
   - ADD: Validation for color_mode and buffer_size_kb
   - UPDATE: SQL UPSERT query with 4 new fields
   - Lines affected: ~30 lines

**Frontend (TypeScript):**
3. `src/components/printer/PrinterConfigForm.tsx`
   - UPDATE: PrinterConfigFormData interface (+4 fields)
   - UPDATE: yup schema (+4 field validations with conditionals)
   - UPDATE: defaultValues (+4 default values)
   - ADD: Section 4 container with Checkbox + conditional Select (~40 lines)
   - ADD: Section 5 container with Checkbox + conditional TextField (~30 lines)
   - ADD: Visual headers for "Cài đặt cơ bản" and "Cài đặt nâng cao" (~20 lines)
   - Lines affected: ~140 new lines, ~20 modified lines

**Total estimated changes:** ~200 lines across 3 files

