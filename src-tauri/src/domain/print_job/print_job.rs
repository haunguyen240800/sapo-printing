use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::errors::DomainError;
use super::print_job_events::*;
use super::job_id::JobId;
use super::print_status::PrintStatus;
use crate::domain::settings::PrintSettings;
use crate::domain::common::aggregate::{AggregateRoot, DomainEvent as CommonDomainEvent};

const MAX_RETRY_COUNT: u32 = 3;

/// PrintJob aggregate root.
///
/// Encapsulates all business rules for print job lifecycle:
/// - State transitions with validation
/// - Retry limit enforcement (max 3)
/// - Cancellation guards (cannot cancel COMPLETED/FAILED)
/// - Domain event collection (Outbox Pattern)
#[derive(Debug, Serialize, Deserialize)]
pub struct PrintJob {
    id: JobId,
    status: PrintStatus,
    retry_count: u32,
    pdf_url: String,
    printer_name: String,
    created_at: i64,
    completed_at: Option<i64>,
    error_message: Option<String>,
    output_path: Option<String>,
    pub settings: PrintSettings,
    #[serde(skip)]
    events: Vec<Box<dyn CommonDomainEvent>>,
}

impl Clone for PrintJob {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            status: self.status.clone(),
            retry_count: self.retry_count,
            pdf_url: self.pdf_url.clone(),
            printer_name: self.printer_name.clone(),
            created_at: self.created_at,
            completed_at: self.completed_at,
            error_message: self.error_message.clone(),
            output_path: self.output_path.clone(),
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
    pub fn new(pdf_url: String, printer_name: String, settings: PrintSettings) -> Self {
        Self::new_with_output_path(pdf_url, printer_name, settings, None)
    }

    /// Creates a new PrintJob with optional output path for Print-to-PDF printers.
    pub fn new_with_output_path(
        pdf_url: String,
        printer_name: String,
        settings: PrintSettings,
        output_path: Option<String>,
    ) -> Self {
        let id = JobId::new();
        let created_at = Self::now();
        let mut job = Self {
            id: id.clone(),
            status: PrintStatus::Pending,
            retry_count: 0,
            pdf_url,
            printer_name,
            created_at,
            completed_at: None,
            error_message: None,
            output_path,
            settings,
            events: Vec::new(),
        };
        job.push_event(Box::new(PrintJobCreated::new(
            id,
            job.pdf_url.clone(),
            job.printer_name.clone(),
        )));
        job
    }

    /// Reconstructs a PrintJob from persisted state (no events emitted).
    pub fn reconstruct(
        id: JobId,
        status: PrintStatus,
        retry_count: u32,
        pdf_url: String,
        printer_name: String,
        created_at: i64,
        completed_at: Option<i64>,
        error_message: Option<String>,
        output_path: Option<String>,
        settings: PrintSettings,
    ) -> Self {
        Self {
            id,
            status,
            retry_count,
            pdf_url,
            printer_name,
            created_at,
            completed_at,
            error_message,
            output_path,
            settings,
            events: Vec::new(),
        }
    }

    fn push_event(&mut self, event: Box<dyn CommonDomainEvent>) {
        self.events.push(event);
    }

    /// PENDING → QUEUED
    pub fn queue(&mut self) -> Result<(), DomainError> {
        if !self.status.can_transition_to(&PrintStatus::Queued) {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Queued".to_string(),
            });
        }
        self.status = PrintStatus::Queued;
        self.push_event(Box::new(PrintJobQueued::new(self.id.clone())));
        Ok(())
    }

    /// QUEUED → DOWNLOADED
    pub fn mark_downloaded(&mut self) -> Result<(), DomainError> {
        if !self.status.can_transition_to(&PrintStatus::Downloaded) {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Downloaded".to_string(),
            });
        }
        self.status = PrintStatus::Downloaded;
        self.push_event(Box::new(PrintJobDownloaded::new(self.id.clone())));
        Ok(())
    }

    /// DOWNLOADED → SUBMITTED_TO_QUEUE
    pub fn mark_submitted(&mut self) -> Result<(), DomainError> {
        if !self
            .status
            .can_transition_to(&PrintStatus::SubmittedToQueue)
        {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "SubmittedToQueue".to_string(),
            });
        }
        self.status = PrintStatus::SubmittedToQueue;
        self.push_event(Box::new(PrintJobSubmitted::new(self.id.clone())));
        Ok(())
    }

    /// SUBMITTED_TO_QUEUE → PRINTING
    pub fn mark_printing(&mut self) -> Result<(), DomainError> {
        if !self.status.can_transition_to(&PrintStatus::Printing) {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Printing".to_string(),
            });
        }
        self.status = PrintStatus::Printing;
        self.push_event(Box::new(PrintJobPrinting::new(self.id.clone())));
        Ok(())
    }

    /// PRINTING → COMPLETED
    pub fn complete(&mut self) -> Result<(), DomainError> {
        if !self.status.can_transition_to(&PrintStatus::Completed) {
            return Err(DomainError::InvalidStateTransition {
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
    pub fn fail(&mut self, reason: String) -> Result<(), DomainError> {
        if !self.status.can_transition_to(&PrintStatus::Failed) {
            return Err(DomainError::InvalidStateTransition {
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
    pub fn retry(&mut self) -> Result<(), DomainError> {
        if self.retry_count >= MAX_RETRY_COUNT {
            return Err(DomainError::MaxRetryExceeded);
        }
        if self.status != PrintStatus::Failed {
            return Err(DomainError::InvalidStateTransition {
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
    pub fn cancel(&mut self) -> Result<(), DomainError> {
        match self.status {
            PrintStatus::Completed => Err(DomainError::CannotCancelCompleted),
            PrintStatus::Failed => Err(DomainError::CannotCancelFailed),
            PrintStatus::Cancelled => Err(DomainError::CannotCancelCancelled),
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
    pub fn drain_events(&mut self) -> Vec<Box<dyn CommonDomainEvent>> {
        std::mem::take(&mut self.events)
    }

    // --- Accessors ---

    pub fn id(&self) -> &JobId {
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

    pub fn printer_name(&self) -> &str {
        &self.printer_name
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

    pub fn output_path(&self) -> Option<&String> {
        self.output_path.as_ref()
    }

    pub fn pending_events_count(&self) -> usize {
        self.events.len()
    }
}

impl AggregateRoot for PrintJob {
    fn domain_events(&self) -> &[Box<dyn CommonDomainEvent>] {
        &self.events
    }

    fn clear_domain_events(&mut self) {
        self.events.clear();
    }
}


