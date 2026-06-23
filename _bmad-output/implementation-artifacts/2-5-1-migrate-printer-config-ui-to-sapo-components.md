---
parent_story: 2-5-create-printer-configuration-ui-basic-settings-3-sections
baseline_commit: 9b49645
---

# Story 2.5.1: Migrate Printer Config UI to @sapo/ui-components

Status: ready-for-dev

## Story

As a **developer**,
I want **to migrate printer configuration UI from native HTML to @sapo/ui-components**,
So that **the implementation complies with AC-1 and AC-7 requirements**.

## Context

This is a follow-up to Story 2.5 code review. Two findings require UI component migration:
1. **AC-1 violation:** Form elements use native HTML instead of @sapo/ui-components
2. **AC-7 violation:** Notifications use plain divs instead of @sapo/ui Alert/Notification

Current implementation uses native HTML with inline styles. Need to replace with:
- `Input` for text/number inputs
- `Select` for dropdowns
- `Checkbox` for orientation toggle
- `Button` for submit
- `Alert` or `Notification` for success/error messages

## Acceptance Criteria

**AC-1: Replace all form elements with @sapo/ui-components**
- Replace native `<input type="number">` with `<Input type="number">`
- Replace native `<select>` with `<Select>` 
- Replace native `<input type="checkbox">` with `<Checkbox>`
- Replace native `<button>` with `<Button>`
- Maintain all existing validation and error display
- Preserve form functionality (react-hook-form integration)

**AC-2: Replace notification divs with @sapo/ui Alert or Notification**
- Replace success notification div with `<Alert>` or `<Notification>` component
- Replace error notification div with `<Alert>` or `<Notification>` component
- Maintain Vietnamese text and user experience
- Preserve auto-dismiss behavior if applicable

**AC-3: Verify no visual or functional regression**
- All 3 sections render correctly
- Form validation works as before
- Submit functionality unchanged
- Error messages display correctly
- Success/error notifications appear properly

## Tasks / Subtasks

- [ ] **Task 1: Research @sapo/ui-components API**
  - [ ] Read package documentation for Input, Select, Checkbox, Button components
  - [ ] Understand props and integration with react-hook-form
  - [ ] Check Alert vs Notification component differences

- [ ] **Task 2: Replace form input elements**
  - [ ] Replace paper width/height number inputs with Input component
  - [ ] Replace margin inputs (4 fields) with Input component
  - [ ] Update error message display to match @sapo/ui patterns
  - [ ] Test validation still works

- [ ] **Task 3: Replace select and checkbox**
  - [ ] Replace paper size select with Select component
  - [ ] Replace orientation checkbox with Checkbox component
  - [ ] Verify conditional rendering (custom paper inputs) still works

- [ ] **Task 4: Replace button and notifications**
  - [ ] Replace submit button with Button component
  - [ ] Replace success/error notification divs with Alert or Notification
  - [ ] Preserve button disabled state logic

- [ ] **Task 5: Manual testing**
  - [ ] Run `pnpm run dev` and test all form interactions
  - [ ] Verify validation errors display correctly
  - [ ] Test form submit success/error notifications
  - [ ] Check visual appearance matches design system

- [ ] **Task 6: TypeScript compilation verification**
  - [ ] Run `pnpm exec tsc --noEmit` — zero errors

## Dev Notes

### Affected Files
- `src/components/printer/PrinterConfigForm.tsx` — primary migration target (~300 lines affected)
- `src/components/printer/PrinterSelector.tsx` — may need Select component
- `src/components/printer/PrinterStatus.tsx` — may benefit from Badge/StatusIndicator

### Integration Notes

**react-hook-form Integration:**
```tsx
// Before (native)
<input {...register('field')} />

// After (@sapo/ui Input)
<Input {...register('field')} error={errors.field?.message} />
```

**Conditional Rendering:**
Ensure custom paper size inputs still show/hide based on paper_size selection.

**Validation Errors:**
@sapo/ui components typically accept `error` prop for validation messages. Ensure yup error messages pass through correctly.

### References
- Parent Story: `2-5-create-printer-configuration-ui-basic-settings-3-sections.md`
- Code Review Findings: See "Review Findings" section in parent story
- Package: `@sapo/ui-components` ^2.19.0
- Design System: Follow @sapo/ui patterns for consistency

## Estimated Effort
Small-Medium (2-3 hours) — Straightforward component replacement with testing
