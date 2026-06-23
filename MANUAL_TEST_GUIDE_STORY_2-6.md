# Manual Test Guide - Story 2.6

## Prerequisites
- Tauri backend running: `cd src-tauri && cargo run`
- Frontend dev server: `pnpm run dev`
- At least one printer connected to system

## Test Scenarios

### Test 1: Visual Separation
**Goal:** Verify "Cài đặt cơ bản" and "Cài đặt nâng cao" category headers appear

**Steps:**
1. Load printer configuration form
2. Verify "Cài đặt cơ bản" header appears before Section 1
3. Verify "Cài đặt nâng cao" header appears before Section 4
4. Check headers have consistent styling (20px font, 600 weight, bottom border)

**Expected Result:**
- Two category headers visible with clear visual separation
- 5 sections total numbered 1-5

---

### Test 2: Section 4 - Print Mode (Default State)
**Goal:** Verify Section 4 renders correctly with default values

**Steps:**
1. Load form
2. Scroll to Section 4: "In ảnh"
3. Verify "In ảnh" checkbox is UNCHECKED by default
4. Verify "Loại ảnh in" dropdown is HIDDEN

**Expected Result:**
- Checkbox unchecked
- Dropdown not visible
- No validation errors

---

### Test 3: Section 4 - Enable Print as Image
**Goal:** Verify color mode dropdown appears when checkbox enabled

**Steps:**
1. Check "In ảnh" checkbox
2. Verify "Loại ảnh in" dropdown appears
3. Verify dropdown shows 5 options:
   - RGB (24-bit) — default selected
   - ARGB (32-bit với alpha)
   - BGR (Windows default)
   - GRAY (8-bit grayscale)
   - BINARY (1-bit monochrome)
4. Select different color mode (e.g., GRAY)
5. Uncheck "In ảnh" checkbox
6. Verify dropdown disappears
7. Re-check "In ảnh"
8. Verify dropdown reappears with RGB (reset value)

**Expected Result:**
- Dropdown appears/disappears based on checkbox
- 5 color mode options available
- Dropdown resets to RGB when checkbox unchecked and re-checked

---

### Test 4: Section 5 - Advanced (Default State)
**Goal:** Verify Section 5 renders correctly with default values

**Steps:**
1. Load form
2. Scroll to Section 5: "Cài đặt nâng cao"
3. Verify "Bật Printing Buffer" checkbox is UNCHECKED
4. Verify "Kích thước Buffer (KB)" input is HIDDEN

**Expected Result:**
- Checkbox unchecked
- Buffer size input not visible
- No validation errors

---

### Test 5: Section 5 - Enable Buffer
**Goal:** Verify buffer size input appears when checkbox enabled

**Steps:**
1. Check "Bật Printing Buffer" checkbox
2. Verify "Kích thước Buffer (KB)" input appears
3. Verify input has helpText: "Khoảng cho phép: 1-1024 KB"
4. Leave input empty and try to submit
5. Verify validation error: "Kích thước buffer bắt buộc khi bật buffer"

**Expected Result:**
- Input appears when checkbox checked
- Empty input triggers validation error
- Error message in Vietnamese

---

### Test 6: Buffer Size Validation - Range
**Goal:** Verify buffer size validates 1-1024 KB range

**Steps:**
1. Enable "Bật Printing Buffer"
2. Enter 0 in buffer size → verify error
3. Enter 1 → verify no error
4. Enter 512 → verify no error
5. Enter 1024 → verify no error
6. Enter 1025 → verify error
7. Enter -10 → verify error

**Expected Result:**
- Values < 1: error "Kích thước buffer phải trong khoảng 1-1024 KB"
- Values 1-1024: no error
- Values > 1024: error "Kích thước buffer phải trong khoảng 1-1024 KB"

---

### Test 7: Buffer Size Reset on Disable
**Goal:** Verify buffer size clears when checkbox unchecked

**Steps:**
1. Enable "Bật Printing Buffer"
2. Enter valid buffer size (e.g., 256)
3. Uncheck "Bật Printing Buffer"
4. Verify input disappears
5. Re-check "Bật Printing Buffer"
6. Verify input reappears EMPTY (reset)

**Expected Result:**
- Input value clears when checkbox unchecked
- Input reappears empty when re-checked

---

### Test 8: Form Submission - Valid Data
**Goal:** Verify form submits successfully with all fields

**Steps:**
1. Fill all required fields (Sections 1-3 from Story 2.5)
2. Enable "In ảnh" and select "ARGB"
3. Enable "Bật Printing Buffer" and enter 512 KB
4. Click "Lưu cấu hình"
5. Verify success Banner appears

**Expected Result:**
- Success message: "Đã lưu cấu hình máy in"
- No errors
- Form data persisted (check database or backend logs)

---

### Test 9: Backend Validation - Buffer Disabled with Size
**Goal:** Verify backend rejects inconsistent buffer state

**Steps:**
1. Open browser DevTools console
2. Manually call Tauri command:
   ```javascript
   await invoke('save_printer_config', {
     config: {
       printer_name: 'Test',
       paper_size: 'A4',
       orientation: 'Portrait',
       margin_left: 0,
       margin_right: 0,
       margin_top: 0,
       margin_bottom: 0,
       print_as_image: false,
       color_mode: 'RGB',
       enable_buffer: false,
       buffer_size_kb: 512  // Invalid: buffer disabled but size provided
     }
   });
   ```
3. Verify error: "Không thể đặt kích thước buffer khi buffer đã tắt"

**Expected Result:**
- Backend validation rejects inconsistent state
- Vietnamese error message returned

---

### Test 10: Backend Validation - Invalid Color Mode
**Goal:** Verify backend validates color mode enum

**Steps:**
1. Open browser DevTools console
2. Manually call Tauri command:
   ```javascript
   await invoke('save_printer_config', {
     config: {
       printer_name: 'Test',
       paper_size: 'A4',
       orientation: 'Portrait',
       margin_left: 0,
       margin_right: 0,
       margin_top: 0,
       margin_bottom: 0,
       print_as_image: true,
       color_mode: 'INVALID_MODE',  // Invalid
       enable_buffer: false,
       buffer_size_kb: null
     }
   });
   ```
3. Verify error contains: "Loại ảnh in không hợp lệ" and "RGB, ARGB, BGR, GRAY, BINARY"

**Expected Result:**
- Backend validation rejects invalid color mode
- Error message lists valid options

---

### Test 11: Existing Sections Unchanged
**Goal:** Verify Sections 1-3 still work correctly

**Steps:**
1. Test Section 1: Printer selection and status
2. Test Section 2: Paper size, custom dimensions, orientation
3. Test Section 3: All 4 margin inputs
4. Verify all existing validation still works
5. Verify form submission with only basic settings (Sections 1-3)

**Expected Result:**
- All existing functionality unchanged
- No regressions in Sections 1-3

---

## Checklist Summary

- [ ] Visual separation: "Cài đặt cơ bản" and "Cài đặt nâng cao" headers
- [ ] Section 4 checkbox toggles color mode dropdown
- [ ] Section 4 dropdown has 5 color mode options
- [ ] Section 4 resets to RGB when unchecked
- [ ] Section 5 checkbox toggles buffer size input
- [ ] Section 5 buffer size validates 1-1024 KB range
- [ ] Section 5 buffer size clears when unchecked
- [ ] Form submits successfully with all fields
- [ ] Backend validates buffer disabled with size provided
- [ ] Backend validates color mode enum
- [ ] Sections 1-3 unchanged and working

---

## Notes
- TypeScript compilation: 0 errors ✅
- Rust compilation: successful ✅
- Backend unit tests: 7 passed ✅
- Manual testing requires running Tauri app
