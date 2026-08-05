use std::path::Path;

use crate::domain::print_job::PrintJobSettings;
use crate::shared::errors::InfrastructureError;

pub trait PrintService: Send + Sync {
    fn print(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
    ) -> Result<(), InfrastructureError>;

    fn save_to_path(
        &self,
        pdf_path: &Path,
        output_path: &str,
    ) -> Result<(), InfrastructureError>;
}
