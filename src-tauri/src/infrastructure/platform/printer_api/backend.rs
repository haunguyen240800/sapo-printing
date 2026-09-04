pub enum NativeGraphicsContext {
    Windows(usize),
    Mac(usize),
    Linux(usize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PageOrientation {
    #[default]
    Portrait,
    Landscape,
}

impl PageOrientation {
    pub fn from_config(value: &str) -> Self {
        if value.trim().eq_ignore_ascii_case("landscape") {
            Self::Landscape
        } else {
            Self::Portrait
        }
    }

    pub fn effective_dimensions(self, width_mm: f32, height_mm: f32) -> (f32, f32) {
        match self {
            Self::Portrait => (width_mm, height_mm),
            Self::Landscape => (height_mm, width_mm),
        }
    }
}

pub trait GraphicsBackend {
    fn begin_document(
        &mut self,
        printer_name: &str,
        doc_name: &str,
        output_path: Option<&str>,
        paper_width_mm: f32,
        paper_height_mm: f32,
        orientation: PageOrientation,
    ) -> Result<(), String>;
    fn get_dpi(&self) -> (u32, u32);
    fn get_page_pixels(&self) -> (u32, u32);
    fn begin_page(&mut self);
    fn native_context(&mut self) -> NativeGraphicsContext;
    fn draw_bitmap(&mut self, data: &[u8], x: i32, y: i32, width: u32, height: u32, bpp: u16);
    fn end_page(&mut self);
    fn end_document(&mut self) -> Result<(), String>;
    fn abort_document(&mut self);
}

#[cfg(test)]
mod tests {
    use super::PageOrientation;

    #[test]
    fn parses_landscape_case_insensitively() {
        assert_eq!(
            PageOrientation::from_config("Landscape"),
            PageOrientation::Landscape
        );
        assert_eq!(
            PageOrientation::from_config("LANDSCAPE"),
            PageOrientation::Landscape
        );
        assert_eq!(
            PageOrientation::from_config("landscape"),
            PageOrientation::Landscape
        );
        assert_eq!(
            PageOrientation::from_config("  Landscape\t"),
            PageOrientation::Landscape
        );
    }

    #[test]
    fn unknown_orientation_falls_back_to_portrait() {
        assert_eq!(
            PageOrientation::from_config("diagonal"),
            PageOrientation::Portrait
        );
        assert_eq!(PageOrientation::from_config(""), PageOrientation::Portrait);
    }

    #[test]
    fn effective_dimensions_swap_only_for_landscape() {
        assert_eq!(
            PageOrientation::Portrait.effective_dimensions(100.0, 150.0),
            (100.0, 150.0)
        );
        assert_eq!(
            PageOrientation::Landscape.effective_dimensions(100.0, 150.0),
            (150.0, 100.0)
        );
    }
}

pub struct GraphicsBackendFactory;

impl GraphicsBackendFactory {
    pub fn create() -> Box<dyn GraphicsBackend> {
        #[cfg(target_os = "windows")]
        {
            Box::new(super::windows::WindowsGraphicsBackend::new())
        }
        #[cfg(target_os = "macos")]
        {
            Box::new(super::macos::MacGraphicsBackend::new())
        }
        #[cfg(target_os = "linux")]
        {
            Box::new(super::linux::LinuxGraphicsBackend::new())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            unimplemented!("Unsupported platform");
        }
    }
}
