use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, StretchDIBits, GetDeviceCaps,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, SRCCOPY,
    LOGPIXELSX, LOGPIXELSY, HORZRES, VERTRES, HORZSIZE, VERTSIZE,
    DEVMODEW, DEVMODE_FIELD_FLAGS,
};
use windows::Win32::Graphics::Printing::{ClosePrinter, DocumentPropertiesW, OpenPrinterW};
use windows::Win32::Storage::Xps::{
    StartDocW, StartPage, EndPage, EndDoc, DOCINFOW,
};
use windows::core::{PCWSTR, HSTRING};

use super::backend::{GraphicsBackend, NativeGraphicsContext};

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

    /// Returns the printer's full DEVMODE (from DocumentPropertiesW) with only
    /// the paper-size fields patched. Preserves orientation, print-direction, and
    /// all other driver-specific settings — this is why browser print works and a
    /// zeroed DEVMODEW does not.
    ///
    /// Falls back to a minimal DEVMODEW with explicit portrait orientation if the
    /// printer cannot be queried (e.g. printer offline during job setup).
    fn build_devmode(printer_name: &str, paper_width_mm: f32, paper_height_mm: f32) -> Vec<u8> {
        const DM_PAPERSIZE: u32 = 0x0002;
        const DM_PAPERLENGTH: u32 = 0x0004;
        const DM_PAPERWIDTH: u32 = 0x0008;
        const DM_OUT_BUFFER: u32 = 2;
        const DMPAPER_USER: i16 = 256;

        let printer_hstr = HSTRING::from(printer_name);
        let printer_pcwstr = PCWSTR(printer_hstr.as_ptr());

        unsafe {
            let mut hprinter = HANDLE::default();
            if OpenPrinterW(printer_pcwstr, &mut hprinter, None).is_ok() {
                // First call: pass fMode=0 to retrieve required buffer size.
                let size = DocumentPropertiesW(
                    HWND::default(),
                    hprinter,
                    printer_pcwstr,
                    None,
                    None,
                    0,
                );
                if size > 0 {
                    let mut buf = vec![0u8; size as usize];
                    let ok = DocumentPropertiesW(
                        HWND::default(),
                        hprinter,
                        printer_pcwstr,
                        Some(buf.as_mut_ptr() as *mut DEVMODEW),
                        None,
                        DM_OUT_BUFFER,
                    );
                    let _ = ClosePrinter(hprinter);
                    if ok >= 0 {
                        // Patch only paper-size fields; keep everything else the driver set.
                        let dm = &mut *(buf.as_mut_ptr() as *mut DEVMODEW);
                        dm.dmFields.0 |= DM_PAPERSIZE | DM_PAPERLENGTH | DM_PAPERWIDTH;
                        dm.Anonymous1.Anonymous1.dmPaperSize = DMPAPER_USER;
                        dm.Anonymous1.Anonymous1.dmPaperWidth = (paper_width_mm * 10.0) as i16;
                        dm.Anonymous1.Anonymous1.dmPaperLength = (paper_height_mm * 10.0) as i16;
                        tracing::debug!(
                            "DEVMODE from driver: orientation={}, paperSize={}, w={}dmm, h={}dmm",
                            dm.Anonymous1.Anonymous1.dmOrientation,
                            dm.Anonymous1.Anonymous1.dmPaperSize,
                            dm.Anonymous1.Anonymous1.dmPaperWidth,
                            dm.Anonymous1.Anonymous1.dmPaperLength,
                        );
                        return buf;
                    }
                } else {
                    let _ = ClosePrinter(hprinter);
                }
            }
        }

        // Fallback: minimal DEVMODEW with portrait orientation.
        tracing::warn!("DocumentPropertiesW failed for '{}', using fallback DEVMODEW", printer_name);
        const DM_ORIENTATION: u32 = 0x0001;
        const DMORIENT_PORTRAIT: i16 = 1;
        let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        dm.dmSpecVersion = 0x0401;
        dm.dmFields = DEVMODE_FIELD_FLAGS(DM_ORIENTATION | DM_PAPERSIZE | DM_PAPERLENGTH | DM_PAPERWIDTH);
        unsafe {
            dm.Anonymous1.Anonymous1.dmOrientation = DMORIENT_PORTRAIT;
            dm.Anonymous1.Anonymous1.dmPaperSize = DMPAPER_USER;
            dm.Anonymous1.Anonymous1.dmPaperWidth = (paper_width_mm * 10.0) as i16;
            dm.Anonymous1.Anonymous1.dmPaperLength = (paper_height_mm * 10.0) as i16;
        }
        let dm_size = std::mem::size_of::<DEVMODEW>();
        let mut buf = vec![0u8; dm_size];
        unsafe {
            std::ptr::copy_nonoverlapping(&dm as *const _ as *const u8, buf.as_mut_ptr(), dm_size);
        }
        buf
    }
}

impl GraphicsBackend for WindowsGraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str, output_path: Option<&str>, paper_width_mm: f32, paper_height_mm: f32) -> Result<(), String> {
        let printer_hstr = HSTRING::from(printer_name);

        // Get the driver's full DEVMODE and patch only the paper size.
        // This preserves orientation and all other driver settings —
        // equivalent to what the browser does when printing.
        let devmode_buf = Self::build_devmode(printer_name, paper_width_mm, paper_height_mm);

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
            return Err(format!("Failed to create Device Context for printer '{}'", printer_name));
        }

        self.hdc = Some(hdc);

        // 2. Start Document
        let doc_name_w = Self::to_wstring(doc_name);
        let output_w = output_path.map(Self::to_wstring);
        
        let doc_info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name_w.as_ptr()),
            lpszOutput: output_w.as_ref().map_or(PCWSTR::null(), |w| PCWSTR(w.as_ptr())),
            lpszDatatype: PCWSTR::null(),
            fwType: 0,
        };

        let result = unsafe { StartDocW(hdc, &doc_info) };
        if result <= 0 {
            unsafe { DeleteDC(hdc) };
            self.hdc = None;
            return Err("Failed to start document (StartDocW returned error)".to_string());
        }

        Ok(())
    }

    fn get_dpi(&self) -> (u32, u32) {
        if let Some(hdc) = self.hdc {
            unsafe {
                let log_x = GetDeviceCaps(hdc, LOGPIXELSX);
                let log_y = GetDeviceCaps(hdc, LOGPIXELSY);

                let horz_res = GetDeviceCaps(hdc, HORZRES);
                let vert_res = GetDeviceCaps(hdc, VERTRES);
                let horz_size = GetDeviceCaps(hdc, HORZSIZE); // in mm
                let vert_size = GetDeviceCaps(hdc, VERTSIZE); // in mm

                let phys_dpi_x = if horz_size > 0 { (horz_res as f64 / (horz_size as f64 / 25.4)).round() as u32 } else { log_x as u32 };
                let phys_dpi_y = if vert_size > 0 { (vert_res as f64 / (vert_size as f64 / 25.4)).round() as u32 } else { log_y as u32 };

                tracing::info!(
                    "Printer DC metrics: LOGPIXELS={}x{}, HORZRES={}x{}, SIZE={}x{}mm, PHYS_DPI={}x{}",
                    log_x, log_y, horz_res, vert_res, horz_size, vert_size, phys_dpi_x, phys_dpi_y
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
                let w = GetDeviceCaps(hdc, HORZRES) as u32;
                let h = GetDeviceCaps(hdc, VERTRES) as u32;
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
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
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
                bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD { rgbBlue: 0, rgbGreen: 0, rgbRed: 0, rgbReserved: 0 }; 1],
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

    fn end_document(&mut self) {
        if let Some(hdc) = self.hdc {
            unsafe {
                EndDoc(hdc);
                DeleteDC(hdc);
            }
            self.hdc = None;
        }
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
                DeleteDC(hdc);
            }
        }
    }
}
