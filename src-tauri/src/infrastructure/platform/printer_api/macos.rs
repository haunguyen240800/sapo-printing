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
    paper_width_mm: f32,
    paper_height_mm: f32,
}

impl MacOsGraphicsBackend {
    pub fn new() -> Self {
        Self {
            printer_name: String::new(),
            doc_name: String::new(),
            current_page: 0,
            temp_dir: None,
            page_files: Vec::new(),
            paper_width_mm: 0.0,
            paper_height_mm: 0.0,
        }
    }
}

impl GraphicsBackend for MacOsGraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str, _output_path: Option<&str>, paper_width_mm: f32, paper_height_mm: f32) -> Result<(), String> {
        self.printer_name = printer_name.to_string();
        self.doc_name = doc_name.to_string();
        self.current_page = 0;
        self.page_files.clear();
        self.paper_width_mm = paper_width_mm;
        self.paper_height_mm = paper_height_mm;

        let temp_dir = std::env::temp_dir().join(format!("sapo_print_{}", uuid::Uuid::new_v4()));
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
                    if line.to_lowercase().contains("resolution") {
                        if let Some(start) = line.find('*') {
                            let rest = &line[start + 1..];
                            let end = rest.find(' ').or_else(|| rest.find("dpi")).unwrap_or(rest.len());
                            let parts: Vec<&str> = rest[..end].split('x').collect();
                            if parts.len() == 2 {
                                if let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                                    return (x, y);
                                }
                            } else if let Ok(dpi) = rest[..end].parse::<u32>() {
                                return (dpi, dpi);
                            }
                        }
                    }
                }
            }
        }
        (300, 300)
    }

    fn get_page_pixels(&self) -> (u32, u32) {
        let (dpi_x, dpi_y) = self.get_dpi();
        let w = (self.paper_width_mm * dpi_x as f32 / 25.4) as u32;
        let h = (self.paper_height_mm * dpi_y as f32 / 25.4) as u32;
        (w.max(1), h.max(1))
    }

    fn native_context(&mut self) -> NativeGraphicsContext {
        NativeGraphicsContext::Mac(0)
    }

    fn draw_bitmap(&mut self, data: &[u8], _x: i32, _y: i32, width: u32, height: u32, bpp: u16) {
        let Some(temp_dir) = &self.temp_dir else { return };
        let path = temp_dir.join(format!("page_{}.png", self.current_page));

        // PDFium returns BGRx (32bpp, 4 bytes/pixel) or BGR (24bpp, 3 bytes/pixel).
        // image::RgbImage expects RGB — convert by swapping B and R and dropping the
        // padding byte for 32bpp.
        let rgb: Vec<u8> = match bpp {
            32 => data.chunks(4).flat_map(|px| [px[2], px[1], px[0]]).collect(),
            24 => data.chunks(3).flat_map(|px| [px[2], px[1], px[0]]).collect(),
            _ => return,
        };

        if let Some(img) = image::RgbImage::from_raw(width, height, rgb) {
            if img.save(&path).is_ok() {
                self.page_files.push(path);
            }
        }
    }

    fn end_page(&mut self) {}

    fn end_document(&mut self) {
        if self.page_files.is_empty() {
            return;
        }

        // Send all pages as a single print job. CUPS accepts multiple files on one
        // lp invocation and prints them in order as one job.
        let mut cmd = Command::new("lp");
        cmd.arg("-d").arg(&self.printer_name);
        cmd.arg("-t").arg(&self.doc_name);

        // Request the correct paper size if dimensions are known.
        if self.paper_width_mm > 0.0 && self.paper_height_mm > 0.0 {
            cmd.arg("-o").arg(format!(
                "media=Custom.{}x{}mm",
                self.paper_width_mm as u32,
                self.paper_height_mm as u32,
            ));
        }

        for page_file in &self.page_files {
            cmd.arg(page_file);
        }

        if let Err(e) = cmd.status() {
            eprintln!("Failed to execute lp command on macOS: {}", e);
        }

        if let Some(temp_dir) = &self.temp_dir {
            let _ = fs::remove_dir_all(temp_dir);
        }
        self.temp_dir = None;
        self.page_files.clear();
    }
}
