# Hướng dẫn Manual Testing - Story 2.5.1

## Mục tiêu
Verify rằng UI migration sang @sapo/ui-components không gây regression về mặt chức năng và giao diện.

## Điều kiện tiên quyết
- Rust toolchain đã được cài đặt
- Tauri CLI đã được setup
- Có ít nhất 1 printer được kết nối với hệ thống (hoặc virtual printer)

## Các bước test

### 1. Khởi động Tauri Dev Mode

```bash
pnpm tauri dev
```

**Expected:** Ứng dụng desktop mở ra với form cấu hình máy in

### 2. Test Section 1 - Chọn máy in

**Test Case 2.1: Printer list loads**
- **Action:** Quan sát dropdown "Chọn máy in"
- **Expected:** 
  - Loading state hiển thị Spinner + "Đang tải..."
  - Sau khi load xong, dropdown hiển thị danh sách máy in
  - Máy in mặc định được auto-select
  - PrinterStatus hiển thị Badge với màu phù hợp (xanh = Online, xám = Offline, đỏ = Error)

**Test Case 2.2: Printer selection**
- **Action:** Chọn máy in khác từ dropdown
- **Expected:** 
  - Dropdown value thay đổi
  - PrinterStatus cập nhật theo máy in được chọn

### 3. Test Section 2 - Cài đặt giấy

**Test Case 3.1: Paper size selection**
- **Action:** Chọn các khổ giấy khác nhau (A4, A5, Letter, Custom)
- **Expected:** Select component hoạt động bình thường

**Test Case 3.2: Custom paper size validation**
- **Action:** 
  1. Chọn "Tùy chỉnh" từ paper size
  2. Để trống paper width/height
  3. Nhập giá trị < 50 hoặc > 500
- **Expected:**
  - Custom width/height TextField hiển thị khi chọn "Tùy chỉnh"
  - Error message hiển thị đúng dưới TextField khi validation fail
  - TextField có border đỏ khi có error
  - Submit button bị disable khi có validation error

**Test Case 3.3: Orientation toggle**
- **Action:** Click checkbox "In chiều ngang"
- **Expected:** Checkbox toggle đúng (checked/unchecked)

### 4. Test Section 3 - Lề trang

**Test Case 4.1: Margin validation**
- **Action:** Nhập giá trị không hợp lệ vào các trường lề (< 0 hoặc > 100)
- **Expected:**
  - Error message hiển thị đúng dưới TextField
  - TextField có border đỏ
  - Submit button bị disable

**Test Case 4.2: Valid margin input**
- **Action:** Nhập giá trị hợp lệ (0-100) vào tất cả 4 trường lề
- **Expected:**
  - Không có error message
  - Submit button enabled

### 5. Test Form Submission

**Test Case 5.1: Successful submission**
- **Action:** 
  1. Fill form với data hợp lệ
  2. Click "Lưu cấu hình"
- **Expected:**
  - Button hiển thị loading state với text "Đang lưu..." (nếu có loading prop)
  - Banner màu xanh hiển thị "Đã lưu cấu hình máy in"
  - Banner có thể dismiss bằng nút X

**Test Case 5.2: Failed submission**
- **Action:** 
  1. Ngắt kết nối backend (stop Tauri dev hoặc mock error)
  2. Submit form
- **Expected:**
  - Banner màu đỏ hiển thị error message
  - Banner có thể dismiss

### 6. Test Visual Appearance

**Checklist:**
- [ ] Font Inter được load đúng (không fallback sang system font)
- [ ] Tất cả components có styling nhất quán theo @sapo/ui design system
- [ ] Spacing và padding hợp lý
- [ ] Form responsive và không bị vỡ layout
- [ ] Colors theo @sapo/ui theme (không còn hardcoded colors như #1976d2)
- [ ] Focus states rõ ràng khi tab qua các form fields
- [ ] Error states có màu đỏ rõ ràng

### 7. Test Regression

**Test Case 7.1: All 3 sections render**
- **Expected:** Cả 3 sections (Chọn máy in, Cài đặt giấy, Lề trang) hiển thị đầy đủ

**Test Case 7.2: Conditional rendering**
- **Expected:** Custom paper inputs chỉ hiển thị khi chọn "Tùy chỉnh"

**Test Case 7.3: Form validation logic**
- **Expected:** Validation rules giống hệt như trước (same yup schema)

## Acceptance Criteria Verification

### AC-1: Form elements use @sapo/ui-components ✅
Code review confirms:
- TextField replaces `<input type="number">`
- Select replaces `<select>`
- Checkbox replaces `<input type="checkbox">`
- Button replaces `<button>`

### AC-2: Notifications use @sapo/ui components ✅
Code review confirms:
- Banner component replaces notification div
- Vietnamese text preserved
- onDismiss handler maintains dismiss functionality

### AC-3: No regression ⏳
Requires manual testing above

## Known Limitations

- Không có automated tests do project chưa setup test framework
- Backend Tauri commands cần hoạt động để test end-to-end flow
- Nếu không có printer thực, error message sẽ hiển thị ngay

## Reporting Issues

Nếu phát hiện bug hoặc regression, ghi chú:
1. Test case nào fail
2. Expected behavior
3. Actual behavior
4. Screenshots nếu có thể
