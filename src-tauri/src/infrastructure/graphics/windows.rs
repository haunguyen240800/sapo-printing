use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
// Removed unused imports
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, StretchDIBits,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, SRCCOPY,
};
use windows::Win32::Graphics::Printing::{
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
}

impl GraphicsBackend for WindowsGraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str) -> Result<(), String> {
        let printer_hstr = HSTRING::from(printer_name);
        
        // 1. Create Device Context
        let hdc = unsafe {
            CreateDCW(
                PCWSTR::null(),
                &printer_hstr,
                PCWSTR::null(),
                None,
            )
        };

        if hdc.is_invalid() {
            return Err(format!("Failed to create Device Context for printer '{}'", printer_name));
        }

        self.hdc = Some(hdc);

        // 2. Start Document
        let doc_name_w = Self::to_wstring(doc_name);
        
        let doc_info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name_w.as_ptr()),
            lpszOutput: PCWSTR::null(),
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

    fn draw_bitmap(&mut self, data: &[u8], width: u32, height: u32, bpp: u16) {
        if let Some(hdc) = self.hdc {
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down DIB
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
                    0,
                    0,
                    width as i32,
                    height as i32,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    Some(data.as_ptr() as *const _),
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
