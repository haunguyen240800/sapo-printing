use std::path::Path;

use crate::application::errors::Error;
use crate::domain::print_job::PrintJobSettings;

pub trait PrintPort: Send + Sync {
    fn print(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
    ) -> Result<(), Error>;

    fn save_to_path(&self, pdf_path: &Path, output_path: &str) -> Result<(), Error>;
}
