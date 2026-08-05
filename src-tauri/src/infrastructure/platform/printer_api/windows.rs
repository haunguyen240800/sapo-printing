use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, StretchDIBits, GetDeviceCaps,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, SRCCOPY,
    LOGPIXELSX, LOGPIXELSY, HORZRES, VERTRES, HORZSIZE, VERTSIZE,
    DEVMODEW, DEVMODE_FIELD_FLAGS,
};
use windows::Win32::Graphics::Printing::{ClosePrinter, DocumentPropertiesW, GetJobW, OpenPrinterW, JOB_INFO_2W};
use windows::Win32::Storage::Xps::{
    StartDocW, StartPage, EndPage, EndDoc, AbortDoc, DOCINFOW,
};
use windows::core::{PCWSTR, HSTRING};

use super::backend::{GraphicsBackend, NativeGraphicsContext};

/// Spooler `JOB_STATUS_*` bit flags (from winspool.h). Declared locally as `u32`
/// so status checks are independent of the `windows` crate's newtype wrappers.
const JS_ERROR: u32 = 0x0000_0002;
const JS_OFFLINE: u32 = 0x0000_0020;
const JS_PAPEROUT: u32 = 0x0000_0040;
const JS_PRINTED: u32 = 0x0000_0080;
const JS_DELETED: u32 = 0x0000_0100;
const JS_BLOCKED_DEVQ: u32 = 0x0000_0200;
const JS_USER_INTERVENTION: u32 = 0x0000_0400;

/// Maximum time to wait for a single spooled document to reach the printer.
const SPOOL_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// Interval between spooler status polls.
const SPOOL_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// Result of a single spooler status query.
enum JobQuery {
    /// Job is still tracked by the spooler; carries the raw `JOB_STATUS_*` bits.
    Status(u32),
    /// Job is no longer in the queue (printed and removed by the driver).
    Gone,
}

pub struct WindowsGraphicsBackend {
    hdc: Option<HDC>,
    /// Spooler job id returned by `StartDocW`, used to poll print completion.
    spool_job_id: Option<u32>,
    /// Printer name for the current document, needed to open the spooler queue.
    printer_name: Option<String>,
}

impl WindowsGraphicsBackend {
    pub fn new() -> Self {
        Self { hdc: None, spool_job_id: None, printer_name: None }
    }

    /// Poll the print spooler until the given job reaches `JOB_STATUS_PRINTED`,
    /// disappears from the queue (driver removed it after printing), or a
    /// failure/timeout occurs.
    ///
    /// Returns `Ok(())` on confirmed print (or benign disappearance) and `Err`
    /// with a human-readable reason otherwise.
    fn wait_for_printed(printer_name: &str, job_id: u32) -> Result<(), String> {
        // job_id 0 is not a valid spooler job (StartDocW failed to allocate one);
        // nothing to wait on.
        if job_id == 0 {
            return Ok(());
        }

        let printer_hstr = HSTRING::from(printer_name);
        let printer_pcwstr = PCWSTR(printer_hstr.as_ptr());

        let mut hprinter = HANDLE::default();
        if unsafe { OpenPrinterW(printer_pcwstr, &mut hprinter, None) }.is_err() {
            // Cannot query the spooler — treat as printed rather than failing a
            // job that most likely succeeded (matches prior "spool == success").
            tracing::warn!(
                "wait_for_printed: OpenPrinterW failed for '{}', assuming job {} printed",
                printer_name, job_id
            );
            return Ok(());
        }

        let deadline = std::time::Instant::now() + SPOOL_WAIT_TIMEOUT;
        let result = loop {
            match Self::query_job_status(hprinter, job_id) {
                // Job no longer in queue → spooler finished and removed it.
                JobQuery::Gone => break Ok(()),
                JobQuery::Status(status) => {
                    if status & JS_PRINTED != 0 {
                        break Ok(());
                    }
                    if status & (JS_ERROR | JS_DELETED | JS_PAPEROUT | JS_OFFLINE | JS_BLOCKED_DEVQ) != 0 {
                        break Err(format!(
                            "spooler reported failure for job {} (status=0x{:08X})",
                            job_id, status
                        ));
                    }
                    if status & JS_USER_INTERVENTION != 0 {
                        tracing::warn!(
                            "wait_for_printed: job {} needs user intervention (status=0x{:08X})",
                            job_id, status
                        );
                    }
                }
            }

            if std::time::Instant::now() >= deadline {
                break Err(format!(
                    "timed out after {}s waiting for job {} to print",
                    SPOOL_WAIT_TIMEOUT.as_secs(), job_id
                ));
            }
            std::thread::sleep(SPOOL_POLL_INTERVAL);
        };

        unsafe { let _ = ClosePrinter(hprinter); }
        result
    }

    /// Query a single spooler job's status via `GetJobW` (level 2).
    fn query_job_status(hprinter: HANDLE, job_id: u32) -> JobQuery {
        unsafe {
            // First call: discover required buffer size.
            let mut needed: u32 = 0;
            let _ = GetJobW(hprinter, job_id, 2, None, &mut needed);
            if needed == 0 {
                // No buffer needed → job is not in the queue anymore.
                return JobQuery::Gone;
            }

            let mut buf = vec![0u8; needed as usize];
            if !GetJobW(hprinter, job_id, 2, Some(buf.as_mut_slice()), &mut needed).as_bool() {
                // Job vanished between the two calls, or query failed → treat as gone.
                return JobQuery::Gone;
            }

            let info = &*(buf.as_ptr() as *const JOB_INFO_2W);
            JobQuery::Status(info.Status)
        }
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

        // StartDocW's positive return value is the spooler job identifier. Keep
        // it (with the printer name) so end_document can poll the job to
        // completion instead of assuming spool == printed.
        self.spool_job_id = Some(result as u32);
        self.printer_name = Some(printer_name.to_string());

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

    fn end_document(&mut self) -> Result<(), String> {
        let Some(hdc) = self.hdc.take() else {
            return Ok(());
        };

        let end_result = unsafe { EndDoc(hdc) };
        unsafe { DeleteDC(hdc) };

        let job_id = self.spool_job_id.take();
        let printer_name = self.printer_name.take();

        // EndDoc returns > 0 on success; anything else means the document was
        // not committed to the spooler.
        if end_result <= 0 {
            return Err("Failed to finish document (EndDoc returned error)".to_string());
        }

        // Block until the spooler confirms the job actually printed.
        match (printer_name, job_id) {
            (Some(name), Some(id)) => Self::wait_for_printed(&name, id),
            _ => Ok(()),
        }
    }

    fn abort_document(&mut self) {
        if let Some(hdc) = self.hdc.take() {
            unsafe {
                AbortDoc(hdc);
                DeleteDC(hdc);
            }
        }
        self.spool_job_id = None;
        self.printer_name = None;
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
