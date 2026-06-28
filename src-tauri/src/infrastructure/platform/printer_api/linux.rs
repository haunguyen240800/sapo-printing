use std::process::Command;
use std::fs;
use std::path::PathBuf;

use super::backend::{GraphicsBackend, NativeGraphicsContext};

pub struct LinuxGraphicsBackend {
    printer_name: String,
    doc_name: String,
    current_page: u32,
    temp_dir: Option<PathBuf>,
    page_files: Vec<PathBuf>,
}

impl LinuxGraphicsBackend {
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

impl GraphicsBackend for LinuxGraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str, _output_path: Option<&str>) -> Result<(), String> {
        self.printer_name = printer_name.to_string();
        self.doc_name = doc_name.to_string();
        self.current_page = 0;
        self.page_files.clear();

        // Create a temporary directory for this print job
        let temp_dir = std::env::temp_dir().join(format!("sapo_print_linux_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp dir for printing: {}", e))?;
            
        self.temp_dir = Some(temp_dir);
        Ok(())
    }

    fn begin_page(&mut self) {
        self.current_page += 1;
    }

    fn get_dpi(&self) -> (u32, u32) {
        if self.printer_name.is_empty() {
            return (300, 300);
        }

        if let Ok(output) = Command::new("lpoptions")
            .arg("-p")
            .arg(&self.printer_name)
            .arg("-l")
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                for line in text.lines() {
                    let line_lower = line.to_lowercase();
                    if line_lower.contains("resolution") {
                        if let Some(start) = line.find('*') {
                            let rest = &line[start + 1..];
                            let end = rest.find(' ').or_else(|| rest.find("dpi")).unwrap_or(rest.len());
                            let value = &rest[..end];
                            let parts: Vec<&str> = value.split('x').collect();
                            if parts.len() == 2 {
                                if let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                                    return (x, y);
                                }
                            } else if parts.len() == 1 {
                                if let Ok(dpi) = parts[0].parse::<u32>() {
                                    return (dpi, dpi);
                                }
                            }
                        }
                    }
                }
            }
        }
        (300, 300)
    }

    fn native_context(&mut self) -> NativeGraphicsContext {
        // Return 0 for Cairo context to trigger fallback to bitmap rendering
        NativeGraphicsContext::Linux(0)
    }

    fn draw_bitmap(&mut self, data: &[u8], _x: i32, _y: i32, width: u32, height: u32, bpp: u16) {
        if let Some(temp_dir) = &self.temp_dir {
            // Save the bitmap data to a temporary PNG file using the `image` crate.
            let path = temp_dir.join(format!("page_{}.png", self.current_page));
            
            if bpp == 24 || bpp == 32 {
                if let Some(img) = image::RgbImage::from_raw(width, height, data.to_vec()) {
                    let _ = img.save(&path);
                    self.page_files.push(path);
                }
            }
        }
    }

    fn end_page(&mut self) {
        // Nothing needed here for Linux `lp` pipeline
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
                    eprintln!("Failed to execute lp command on Linux: {}", e);
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
