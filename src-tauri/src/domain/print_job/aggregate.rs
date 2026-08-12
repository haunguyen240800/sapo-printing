use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::errors::PrintJobError;
use super::events::*;
use super::value_objects::{PrintJobId, PrintJobSettings, PrintStatus, PrinterId};

pub const MAX_RETRY_COUNT: u32 = 3;

/// PrintJob aggregate root.
///
/// Encapsulates the lifecycle of a print request and enforces all business rules:
/// state transitions, retry limits, cancellation guards. Carries the document
/// reference (`pdf_url`) and the rendered-file location (`output_path`) directly.
///
/// The destination printer is referenced by **identity only** (`PrinterId`),
/// following the DDD "Reference by Identity" rule for cross-context references.
/// The OS printer state itself is not a domain aggregate.
#[derive(Debug, Serialize, Deserialize)]
pub struct PrintJob {
    id: PrintJobId,
    status: PrintStatus,
    retry_count: u32,
    pdf_url: String,
    output_path: Option<String>,
    printer_id: PrinterId,
    created_at: i64,
    completed_at: Option<i64>,
    error_message: Option<String>,
    pub settings: PrintJobSettings,
    #[serde(skip)]
    events: Vec<Box<dyn DomainEvent>>,
}

impl Clone for PrintJob {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            status: self.status.clone(),
            retry_count: self.retry_count,
            pdf_url: self.pdf_url.clone(),
            output_path: self.output_path.clone(),
            printer_id: self.printer_id.clone(),
            created_at: self.created_at,
            completed_at: self.completed_at,
            error_message: self.error_message.clone(),
            settings: self.settings.clone(),
            events: Vec::new(),
        }
    }
}

impl PrintJob {
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Creates a new PrintJob in PENDING status.
    /// Emits a PrintJobCreated event.
    pub fn new(pdf_url: String, printer_id: PrinterId, settings: PrintJobSettings) -> Self {
        Self::new_with_output_path(pdf_url, printer_id, settings, None)
    }

    /// Creates a new PrintJob with optional output path for Print-to-PDF printers.
    pub fn new_with_output_path(
        pdf_url: String,
        printer_id: PrinterId,
        settings: PrintJobSettings,
        output_path: Option<String>,
    ) -> Self {
        let id = PrintJobId::new();
        let created_at = Self::now();
        let mut job = Self {
            id: id.clone(),
            status: PrintStatus::Pending,
            retry_count: 0,
            pdf_url,
            output_path,
            printer_id: printer_id.clone(),
            created_at,
            completed_at: None,
            error_message: None,
            settings,
            events: Vec::new(),
        };
        job.push_event(Box::new(PrintJobCreated::new(
            id,
            job.pdf_url.clone(),
            printer_id,
        )));
        job
    }

    /// Reconstructs a PrintJob from persisted state (no events emitted).
    pub fn reconstruct(
        id: PrintJobId,
        status: PrintStatus,
        retry_count: u32,
        pdf_url: String,
        printer_id: PrinterId,
        created_at: i64,
        completed_at: Option<i64>,
        error_message: Option<String>,
        output_path: Option<String>,
        settings: PrintJobSettings,
    ) -> Self {
        Self {
            id,
            status,
            retry_count,
            pdf_url,
            output_path,
            printer_id,
            created_at,
            completed_at,
            error_message,
            settings,
            events: Vec::new(),
        }
    }

    fn push_event(&mut self, event: Box<dyn DomainEvent>) {
        self.events.push(event);
    }

    /// PENDING → QUEUED
    pub fn queue(&mut self) -> Result<(), PrintJobError> {
        if !self.status.can_transition_to(&PrintStatus::Queued) {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Queued".to_string(),
            });
        }
        self.status = PrintStatus::Queued;
        self.push_event(Box::new(PrintJobQueued::new(self.id.clone())));
        Ok(())
    }

    /// QUEUED → DOWNLOADED
    pub fn mark_downloaded(&mut self) -> Result<(), PrintJobError> {
        if !self.status.can_transition_to(&PrintStatus::Downloaded) {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Downloaded".to_string(),
            });
        }
        self.status = PrintStatus::Downloaded;
        self.push_event(Box::new(PrintJobDownloaded::new(self.id.clone())));
        Ok(())
    }

    /// DOWNLOADED → SUBMITTED_TO_QUEUE
    pub fn mark_submitted(&mut self) -> Result<(), PrintJobError> {
        if !self
            .status
            .can_transition_to(&PrintStatus::SubmittedToQueue)
        {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "SubmittedToQueue".to_string(),
            });
        }
        self.status = PrintStatus::SubmittedToQueue;
        self.push_event(Box::new(PrintJobSubmitted::new(self.id.clone())));
        Ok(())
    }

    /// SUBMITTED_TO_QUEUE → PRINTING
    pub fn mark_printing(&mut self) -> Result<(), PrintJobError> {
        if !self.status.can_transition_to(&PrintStatus::Printing) {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Printing".to_string(),
            });
        }
        self.status = PrintStatus::Printing;
        self.push_event(Box::new(PrintJobPrinting::new(self.id.clone())));
        Ok(())
    }

    /// PRINTING → COMPLETED
    pub fn complete(&mut self) -> Result<(), PrintJobError> {
        if !self.status.can_transition_to(&PrintStatus::Completed) {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Completed".to_string(),
            });
        }
        self.status = PrintStatus::Completed;
        self.completed_at = Some(Self::now());
        self.push_event(Box::new(PrintJobCompleted::new(self.id.clone())));
        Ok(())
    }

    /// Transition to FAILED (from QUEUED or PRINTING).
    pub fn fail(&mut self, reason: String) -> Result<(), PrintJobError> {
        if !self.status.can_transition_to(&PrintStatus::Failed) {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Failed".to_string(),
            });
        }
        self.status = PrintStatus::Failed;
        self.completed_at = Some(Self::now());
        self.error_message = Some(reason.clone());
        self.push_event(Box::new(PrintJobFailed::new(
            self.id.clone(),
            reason,
            self.retry_count,
        )));
        Ok(())
    }

    /// FAILED → QUEUED (only if retry_count < MAX_RETRY_COUNT).
    pub fn retry(&mut self) -> Result<(), PrintJobError> {
        if self.retry_count >= MAX_RETRY_COUNT {
            return Err(PrintJobError::MaxRetryExceeded);
        }
        if self.status != PrintStatus::Failed {
            return Err(PrintJobError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Queued".to_string(),
            });
        }
        self.retry_count += 1;
        self.status = PrintStatus::Queued;
        self.error_message = None;
        self.push_event(Box::new(PrintJobQueued::new(self.id.clone())));
        Ok(())
    }

    /// Cancel a non-terminal job. Cannot cancel COMPLETED, FAILED, or CANCELLED jobs.
    pub fn cancel(&mut self) -> Result<(), PrintJobError> {
        match self.status {
            PrintStatus::Completed => Err(PrintJobError::CannotCancelCompleted),
            PrintStatus::Failed => Err(PrintJobError::CannotCancelFailed),
            PrintStatus::Cancelled => Err(PrintJobError::CannotCancelCancelled),
            _ => {
                self.status = PrintStatus::Cancelled;
                self.completed_at = Some(Self::now());
                self.push_event(Box::new(PrintJobCancelled::new(self.id.clone())));
                Ok(())
            }
        }
    }

    /// Drains the internal event buffer, returning all collected events.
    /// Called after persistence (Outbox Pattern).
    pub fn drain_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }

    // --- Accessors ---

    pub fn id(&self) -> &PrintJobId {
        &self.id
    }

    pub fn status(&self) -> &PrintStatus {
        &self.status
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn pdf_url(&self) -> &str {
        &self.pdf_url
    }

    pub fn output_path(&self) -> Option<&str> {
        self.output_path.as_deref()
    }

    pub fn printer_id(&self) -> &PrinterId {
        &self.printer_id
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn completed_at(&self) -> Option<i64> {
        self.completed_at
    }

    pub fn error_message(&self) -> Option<&String> {
        self.error_message.as_ref()
    }

    pub fn pending_events_count(&self) -> usize {
        self.events.len()
    }
}
