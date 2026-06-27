use std::process::Command;
use std::fs;
use std::path::PathBuf;

use super::backend::{GraphicsBackend, NativeGraphicsContext};

pub struct MacOsGraphicsBackend {
    printer_name: String,
    doc_name: String,
    current_page: u32,
    temp_dir: Option<PathBuf>,
    page_files: Vec<PathBuf>,
}

impl MacOsGraphicsBackend {
    pub fn new() -> Self {
        Self {
            printer_name: String::new(),
            doc_name: String::new(),
            current_page: 0,
            temp_dir: None,
            page_files: Vec::new(),
        }
    }
}

impl GraphicsBackend for MacOsGraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str) -> Result<(), String> {
        self.printer_name = printer_name.to_string();
        self.doc_name = doc_name.to_string();
        self.current_page = 0;
        self.page_files.clear();

        // Create a temporary directory for this print job
        let temp_dir = std::env::temp_dir().join(format!("sapo_print_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp dir for printing: {}", e))?;
            
        self.temp_dir = Some(temp_dir);
        Ok(())
    }

    fn begin_page(&mut self) {
        self.current_page += 1;
    }

    fn native_context(&mut self) -> NativeGraphicsContext {
        // macOS typically uses CGContextRef, but for this fallback implementation
        // we just return a null pointer to force the bitmap strategy
        NativeGraphicsContext::Mac(0)
    }

    fn draw_bitmap(&mut self, data: &[u8], _x: i32, _y: i32, width: u32, height: u32, bpp: u16) {
        if let Some(temp_dir) = &self.temp_dir {
            // Save the bitmap data to a temporary PNG file using the `image` crate.
            // Assuming data is in BGR or RGB format.
            let path = temp_dir.join(format!("page_{}.png", self.current_page));
            
            // We use image crate to save the raw bitmap data
            if bpp == 24 || bpp == 32 {
                // Determine color type. 32-bit is likely BGRA/RGBA, 24-bit is BGR/RGB
                // We map it to an RgbImage or RgbaImage.
                // Note: the image crate requires RGB or RGBA, if our data is BGR we might need to swap,
                // but CUPS handles standard formats nicely. For this implementation, we assume RGB.
                if let Some(img) = image::RgbImage::from_raw(width, height, data.to_vec()) {
                    let _ = img.save(&path);
                    self.page_files.push(path);
                }
            }
        }
    }

    fn end_page(&mut self) {
        // Handled in draw_bitmap
    }

    fn end_document(&mut self) {
        // Send all collected pages to CUPS using `lp` command
        for page_file in &self.page_files {
            if let Some(path_str) = page_file.to_str() {
                let status = Command::new("lp")
                    .arg("-d")
                    .arg(&self.printer_name)
                    .arg("-t")
                    .arg(&self.doc_name)
                    .arg(path_str)
                    .status();

                if let Err(e) = status {
                    eprintln!("Failed to execute lp command: {}", e);
                }
            }
        }

        // Cleanup temporary directory
        if let Some(temp_dir) = &self.temp_dir {
            let _ = fs::remove_dir_all(temp_dir);
        }
        self.temp_dir = None;
        self.page_files.clear();
    }
}
