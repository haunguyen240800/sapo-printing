use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DIB_RGB_COLORS, DeleteDC,
    GetDeviceCaps, HDC, HORZRES, HORZSIZE, LOGPIXELSX, LOGPIXELSY, SRCCOPY, StretchDIBits, VERTRES,
    VERTSIZE,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, OpenPrinterW, PRINTER_HANDLE,
};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::core::{HSTRING, PCWSTR};

use super::backend::{GraphicsBackend, NativeGraphicsContext, PageOrientation};

const DM_ORIENTATION: u32 = 0x0001;
const DM_PAPERSIZE: u32 = 0x0002;
const DM_PAPERLENGTH: u32 = 0x0004;
const DM_PAPERWIDTH: u32 = 0x0008;
const DM_OUT_BUFFER: u32 = 2;
const DM_IN_BUFFER: u32 = 8;
const DOCUMENT_PROPERTIES_OK: i32 = 1;
const DMORIENT_PORTRAIT: i16 = 1;
const DMORIENT_LANDSCAPE: i16 = 2;
const DMPAPER_USER: i16 = 256;

pub struct WindowsGraphicsBackend {
    hdc: Option<HDC>,
}

impl WindowsGraphicsBackend {
    pub fn new() -> Self {
        Self { hdc: None }
    }

    fn to_wstring(str: &str) -> Vec<u16> {
        OsStr::new(str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Maps common Win32 print error codes to a human-readable Vietnamese hint so
    /// the failure reason shown on the web is actionable instead of a raw number.
    fn describe_win32_error(code: u32) -> &'static str {
        match code {
            5 => "Không có quyền truy cập máy in (ERROR_ACCESS_DENIED)",
            6 => "Handle thiết bị không hợp lệ (ERROR_INVALID_HANDLE)",
            8 => "Không đủ bộ nhớ (ERROR_NOT_ENOUGH_MEMORY)",
            63 => "Tác vụ in đã bị hủy (ERROR_PRINT_CANCELLED)",
            87 => {
                "Tham số không hợp lệ — driver có thể không hỗ trợ khổ giấy tùy chỉnh (ERROR_INVALID_PARAMETER)"
            }
            112 => "Ổ đĩa spooler đầy (ERROR_DISK_FULL)",
            1722 => "Dịch vụ Print Spooler không chạy (RPC_S_SERVER_UNAVAILABLE)",
            1801 => "Tên máy in không hợp lệ (ERROR_INVALID_PRINTER_NAME)",
            1905 => "Máy in đã bị xóa/gỡ khỏi hệ thống (ERROR_PRINTER_DELETED)",
            1906 => {
                "Máy in đang ở trạng thái không hợp lệ, có thể offline hoặc bị tạm dừng (ERROR_INVALID_PRINTER_STATE)"
            }
            3004 => "Spooler báo chưa gọi StartDoc (ERROR_SPL_NO_STARTDOC)",
            _ => "Lỗi Win32 không xác định",
        }
    }

    /// Returns the printer's full DEVMODE (from DocumentPropertiesW) with the
    /// requested paper size and orientation merged by the driver. Preserves
    /// print-direction and all driver-specific settings — this is why browser
    /// print works and a zeroed DEVMODEW does not.
    ///
    /// Falls back to a minimal DEVMODEW with the requested orientation if the
    /// printer cannot be queried (e.g. printer offline during job setup).
    fn patch_devmode(
        dm: &mut DEVMODEW,
        paper_width_mm: f32,
        paper_height_mm: f32,
        orientation: PageOrientation,
    ) {
        dm.dmFields.0 |= DM_ORIENTATION | DM_PAPERSIZE | DM_PAPERLENGTH | DM_PAPERWIDTH;
        dm.Anonymous1.Anonymous1.dmOrientation = match orientation {
            PageOrientation::Portrait => DMORIENT_PORTRAIT,
            PageOrientation::Landscape => DMORIENT_LANDSCAPE,
        };
        dm.Anonymous1.Anonymous1.dmPaperSize = DMPAPER_USER;
        dm.Anonymous1.Anonymous1.dmPaperWidth = (paper_width_mm * 10.0) as i16;
        dm.Anonymous1.Anonymous1.dmPaperLength = (paper_height_mm * 10.0) as i16;
    }

    fn fallback_devmode(
        paper_width_mm: f32,
        paper_height_mm: f32,
        orientation: PageOrientation,
    ) -> Vec<u8> {
        let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
        dm.dmSize = size_of::<DEVMODEW>() as u16;
        dm.dmSpecVersion = 0x0401;
        Self::patch_devmode(&mut dm, paper_width_mm, paper_height_mm, orientation);

        let dm_size = size_of::<DEVMODEW>();
        let mut buf = vec![0u8; dm_size];
        unsafe {
            std::ptr::copy_nonoverlapping(&dm as *const _ as *const u8, buf.as_mut_ptr(), dm_size);
        }
        buf
    }

    fn build_devmode(
        printer_name: &str,
        paper_width_mm: f32,
        paper_height_mm: f32,
        orientation: PageOrientation,
    ) -> Result<Vec<u8>, String> {
        let printer_hstr = HSTRING::from(printer_name);
        let printer_pcwstr = PCWSTR(printer_hstr.as_ptr());

        unsafe {
            let mut hprinter = PRINTER_HANDLE::default();
            if OpenPrinterW(printer_pcwstr, &mut hprinter, None).is_ok() {
                let size = DocumentPropertiesW(None, hprinter, printer_pcwstr, None, None, 0);
                if size >= size_of::<DEVMODEW>() as i32 {
                    let mut buf = vec![0u8; size as usize];
                    let queried = DocumentPropertiesW(
                        None,
                        hprinter,
                        printer_pcwstr,
                        Some(buf.as_mut_ptr() as *mut DEVMODEW),
                        None,
                        DM_OUT_BUFFER,
                    );
                    if queried == DOCUMENT_PROPERTIES_OK {
                        let dm_ptr = buf.as_mut_ptr() as *mut DEVMODEW;
                        Self::patch_devmode(
                            &mut *dm_ptr,
                            paper_width_mm,
                            paper_height_mm,
                            orientation,
                        );

                        let merged = DocumentPropertiesW(
                            None,
                            hprinter,
                            printer_pcwstr,
                            Some(dm_ptr),
                            Some(dm_ptr.cast_const()),
                            DM_IN_BUFFER | DM_OUT_BUFFER,
                        );
                        let _ = ClosePrinter(hprinter);

                        if merged == DOCUMENT_PROPERTIES_OK {
                            let dm = &*dm_ptr;
                            let expected_orientation = match orientation {
                                PageOrientation::Portrait => DMORIENT_PORTRAIT,
                                PageOrientation::Landscape => DMORIENT_LANDSCAPE,
                            };
                            let actual_orientation = dm.Anonymous1.Anonymous1.dmOrientation;
                            if actual_orientation != expected_orientation {
                                return Err(format!(
                                    "Printer driver did not accept {:?} orientation (returned {})",
                                    orientation, actual_orientation
                                ));
                            }
                            tracing::debug!(
                                requested_orientation = ?orientation,
                                dm_orientation = dm.Anonymous1.Anonymous1.dmOrientation,
                                paper_size = dm.Anonymous1.Anonymous1.dmPaperSize,
                                paper_width_dmm = dm.Anonymous1.Anonymous1.dmPaperWidth,
                                paper_height_dmm = dm.Anonymous1.Anonymous1.dmPaperLength,
                                "DEVMODE merged and validated by printer driver"
                            );
                            return Ok(buf);
                        }

                        return Err(format!(
                            "Printer driver rejected the requested page settings (DocumentPropertiesW returned {})",
                            merged
                        ));
                    } else {
                        let _ = ClosePrinter(hprinter);
                    }
                } else {
                    let _ = ClosePrinter(hprinter);
                }
            }
        }

        tracing::warn!(
            printer_name,
            requested_orientation = ?orientation,
            "DocumentPropertiesW failed, using fallback DEVMODEW"
        );
        Ok(Self::fallback_devmode(
            paper_width_mm,
            paper_height_mm,
            orientation,
        ))
    }
}

impl GraphicsBackend for WindowsGraphicsBackend {
    fn begin_document(
        &mut self,
        printer_name: &str,
        doc_name: &str,
        output_path: Option<&str>,
        paper_width_mm: f32,
        paper_height_mm: f32,
        orientation: PageOrientation,
    ) -> Result<(), String> {
        let printer_hstr = HSTRING::from(printer_name);

        // Get the driver's full DEVMODE and merge the requested page settings.
        // This preserves private driver data while making orientation explicit.
        let devmode_buf =
            Self::build_devmode(printer_name, paper_width_mm, paper_height_mm, orientation)?;

        // 1. Create Device Context
        let hdc = unsafe {
            CreateDCW(
                PCWSTR::null(),
                &printer_hstr,
                PCWSTR::null(),
                Some(devmode_buf.as_ptr() as *const DEVMODEW),
            )
        };

        if hdc.is_invalid() {
            return Err(format!(
                "Failed to create Device Context for printer '{}'",
                printer_name
            ));
        }

        self.hdc = Some(hdc);

        // 2. Start Document
        let doc_name_w = Self::to_wstring(doc_name);
        let output_w = output_path.map(Self::to_wstring);

        let doc_info = DOCINFOW {
            cbSize: size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name_w.as_ptr()),
            lpszOutput: output_w
                .as_ref()
                .map_or(PCWSTR::null(), |w| PCWSTR(w.as_ptr())),
            lpszDatatype: PCWSTR::null(),
            fwType: 0,
        };

        let result = unsafe { StartDocW(hdc, &doc_info) };
        if result <= 0 {
            let os_err = std::io::Error::last_os_error();
            let _ = unsafe { DeleteDC(hdc) };
            self.hdc = None;
            let code = os_err.raw_os_error().unwrap_or(-1);
            return Err(format!(
                "Không thể bắt đầu in trên máy in '{}': {} [StartDocW={}, GetLastError={}: {}]",
                printer_name,
                Self::describe_win32_error(code as u32),
                result,
                code,
                os_err
            ));
        }

        Ok(())
    }

    fn get_dpi(&self) -> (u32, u32) {
        if let Some(hdc) = self.hdc {
            unsafe {
                let log_x = GetDeviceCaps(Some(hdc), LOGPIXELSX);
                let log_y = GetDeviceCaps(Some(hdc), LOGPIXELSY);

                let horz_res = GetDeviceCaps(Some(hdc), HORZRES);
                let vert_res = GetDeviceCaps(Some(hdc), VERTRES);
                let horz_size = GetDeviceCaps(Some(hdc), HORZSIZE); // in mm
                let vert_size = GetDeviceCaps(Some(hdc), VERTSIZE); // in mm

                let phys_dpi_x = if horz_size > 0 {
                    (horz_res as f64 / (horz_size as f64 / 25.4)).round() as u32
                } else {
                    log_x as u32
                };
                let phys_dpi_y = if vert_size > 0 {
                    (vert_res as f64 / (vert_size as f64 / 25.4)).round() as u32
                } else {
                    log_y as u32
                };

                tracing::info!(
                    "Printer DC metrics: LOGPIXELS={}x{}, HORZRES={}x{}, SIZE={}x{}mm, PHYS_DPI={}x{}",
                    log_x,
                    log_y,
                    horz_res,
                    vert_res,
                    horz_size,
                    vert_size,
                    phys_dpi_x,
                    phys_dpi_y
                );

                (phys_dpi_x, phys_dpi_y)
            }
        } else {
            (300, 300)
        }
    }

    fn get_page_pixels(&self) -> (u32, u32) {
        if let Some(hdc) = self.hdc {
            unsafe {
                let w = GetDeviceCaps(Some(hdc), HORZRES) as u32;
                let h = GetDeviceCaps(Some(hdc), VERTRES) as u32;
                tracing::info!("Printer DC printable area: {}x{} px", w, h);
                (w, h)
            }
        } else {
            (0, 0)
        }
    }

    fn begin_page(&mut self) {
        if let Some(hdc) = self.hdc {
            unsafe { StartPage(hdc) };
        }
    }

    fn native_context(&mut self) -> NativeGraphicsContext {
        if let Some(hdc) = self.hdc {
            // Note: Since HDC is just a transparent wrapper over an isize/pointer,
            // we extract its raw value for FFI boundaries.
            NativeGraphicsContext::Windows(hdc.0 as usize)
        } else {
            NativeGraphicsContext::Windows(0)
        }
    }

    fn draw_bitmap(&mut self, data: &[u8], x: i32, y: i32, width: u32, height: u32, bpp: u16) {
        if let Some(hdc) = self.hdc {
            // PDFium gives us top-down data (row 0 = top of page).
            // Many printer drivers — including most thermal label drivers — do not
            // honour a negative biHeight (top-down DIB flag) and always treat the
            // bitmap as bottom-up. Using a negative biHeight therefore causes row 0
            // (page top) to be rendered at the physical bottom → upside-down output.
            //
            // Fix: physically reverse the rows so the data is truly bottom-up, then
            // pass a positive biHeight. Bottom-up DIBs are universally supported by
            // every GDI printer driver.
            let bytes_per_row = width as usize * (bpp as usize / 8);
            let mut flipped = vec![0u8; data.len()];
            for row in 0..height as usize {
                let src = (height as usize - 1 - row) * bytes_per_row;
                let dst = row * bytes_per_row;
                flipped[dst..dst + bytes_per_row].copy_from_slice(&data[src..src + bytes_per_row]);
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: height as i32, // positive = bottom-up = universally supported
                    biPlanes: 1,
                    biBitCount: bpp,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }; 1],
            };

            unsafe {
                StretchDIBits(
                    hdc,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    Some(flipped.as_ptr() as *const _),
                    &bmi,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
        }
    }

    fn end_page(&mut self) {
        if let Some(hdc) = self.hdc {
            unsafe { EndPage(hdc) };
        }
    }

    fn end_document(&mut self) -> Result<(), String> {
        let Some(hdc) = self.hdc.take() else {
            return Ok(());
        };

        let end_result = unsafe { EndDoc(hdc) };
        let _ = unsafe { DeleteDC(hdc) };

        // EndDoc returns > 0 on success; anything else means the document was
        // not committed to the spooler.
        if end_result <= 0 {
            return Err("Failed to finish document (EndDoc returned error)".to_string());
        }

        Ok(())
    }

    fn abort_document(&mut self) {
        if let Some(hdc) = self.hdc.take() {
            unsafe {
                AbortDoc(hdc);
                let _ = DeleteDC(hdc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patched_devmode(orientation: PageOrientation) -> DEVMODEW {
        let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
        WindowsGraphicsBackend::patch_devmode(&mut dm, 100.0, 150.0, orientation);
        dm
    }

    #[test]
    fn patches_landscape_and_custom_paper_fields() {
        let dm = patched_devmode(PageOrientation::Landscape);
        let printer_fields = unsafe { dm.Anonymous1.Anonymous1 };

        assert_eq!(
            dm.dmFields.0 & (DM_ORIENTATION | DM_PAPERSIZE | DM_PAPERLENGTH | DM_PAPERWIDTH),
            DM_ORIENTATION | DM_PAPERSIZE | DM_PAPERLENGTH | DM_PAPERWIDTH
        );
        assert_eq!(printer_fields.dmOrientation, DMORIENT_LANDSCAPE);
        assert_eq!(printer_fields.dmPaperSize, DMPAPER_USER);
        assert_eq!(printer_fields.dmPaperWidth, 1000);
        assert_eq!(printer_fields.dmPaperLength, 1500);
    }

    #[test]
    fn patches_portrait_orientation_explicitly() {
        let dm = patched_devmode(PageOrientation::Portrait);
        let printer_fields = unsafe { dm.Anonymous1.Anonymous1 };

        assert_eq!(
            dm.dmFields.0 & DM_ORIENTATION,
            DM_ORIENTATION,
            "fallback and driver DEVMODEs must explicitly carry orientation"
        );
        assert_eq!(printer_fields.dmOrientation, DMORIENT_PORTRAIT);
    }

    #[test]
    fn fallback_devmode_carries_requested_landscape_orientation() {
        let buffer =
            WindowsGraphicsBackend::fallback_devmode(100.0, 150.0, PageOrientation::Landscape);
        let dm = unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const DEVMODEW) };
        let printer_fields = unsafe { dm.Anonymous1.Anonymous1 };

        assert_eq!(printer_fields.dmOrientation, DMORIENT_LANDSCAPE);
        assert_eq!(printer_fields.dmPaperWidth, 1000);
        assert_eq!(printer_fields.dmPaperLength, 1500);
    }

    #[test]
    fn patching_public_devmode_fields_preserves_driver_private_data() {
        #[repr(C)]
        struct DriverDevmode {
            public: DEVMODEW,
            private: [u8; 8],
        }

        let mut driver_devmode = DriverDevmode {
            public: unsafe { std::mem::zeroed() },
            private: [0xA5; 8],
        };

        WindowsGraphicsBackend::patch_devmode(
            &mut driver_devmode.public,
            100.0,
            150.0,
            PageOrientation::Landscape,
        );

        assert_eq!(driver_devmode.private, [0xA5; 8]);
    }
}

/// RAII safety net: if the struct is dropped between `begin_document` and
/// `end_document` (e.g. via panic or early return), release the HDC so the
/// Windows GDI handle is not leaked. `EndDoc` is intentionally skipped here —
/// we cannot guarantee the document is in a printable state on the abnormal
/// path, so we only reclaim the device context.
impl Drop for WindowsGraphicsBackend {
    fn drop(&mut self) {
        if let Some(hdc) = self.hdc.take() {
            unsafe {
                let _ = DeleteDC(hdc);
            }
        }
    }
}
