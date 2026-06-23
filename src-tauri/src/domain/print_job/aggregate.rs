use serde::{Deserialize, Serialize};

use super::errors::DomainError;
use super::events::*;
use super::value_objects::*;

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
            printer_name: self.printer_name.clone(),
            events: Vec::new(),
        }
    }
}

impl PrintJob {
    /// Creates a new PrintJob in PENDING status.
    /// Emits a PrintJobCreated event.
    pub fn new(pdf_url: String, printer_name: String) -> Self {
        let id = JobId::new();
        let mut job = Self {
            id: id.clone(),
            status: PrintStatus::Pending,
            retry_count: 0,
            pdf_url,
            printer_name,
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
    /// Fields created_at/updated_at/completed_at are infrastructure-only — not stored in aggregate.
    pub fn reconstruct(
        id: JobId,
        status: PrintStatus,
        retry_count: u32,
        pdf_url: String,
        printer_name: String,
    ) -> Self {
        Self {
            id,
            status,
            retry_count,
            pdf_url,
            printer_name,
            events: Vec::new(),
        }
    }

    fn push_event(&mut self, event: Box<dyn DomainEvent>) {
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

    pub fn pending_events_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_job() -> PrintJob {
        PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        )
    }

    #[test]
    fn test_new_job_has_pending_status() {
        let job = make_job();
        assert_eq!(*job.status(), PrintStatus::Pending);
    }

    #[test]
    fn test_new_job_emits_created_event() {
        let job = make_job();
        assert_eq!(job.pending_events_count(), 1);
    }

    #[test]
    fn test_queue_transition() {
        let mut job = make_job();
        job.drain_events(); // clear creation events
        assert!(job.queue().is_ok());
        assert_eq!(*job.status(), PrintStatus::Queued);
        assert_eq!(job.pending_events_count(), 1);
    }

    #[test]
    fn test_full_happy_path() {
        let mut job = make_job();
        assert!(job.queue().is_ok());
        assert!(job.mark_downloaded().is_ok());
        assert!(job.mark_submitted().is_ok());
        assert!(job.mark_printing().is_ok());
        assert!(job.complete().is_ok());
        assert_eq!(*job.status(), PrintStatus::Completed);
    }

    #[test]
    fn test_cannot_queue_completed_job() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        job.complete().unwrap();
        // Try to queue again — should fail
        let result = job.queue();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            DomainError::InvalidStateTransition {
                from: "Completed".to_string(),
                to: "Queued".to_string(),
            }
        );
    }

    #[test]
    fn test_fail_from_queued() {
        let mut job = make_job();
        job.queue().unwrap();
        assert!(job.fail("Download timeout".to_string()).is_ok());
        assert_eq!(*job.status(), PrintStatus::Failed);
    }

    #[test]
    fn test_fail_from_printing() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        assert!(job.fail("Printer offline".to_string()).is_ok());
        assert_eq!(*job.status(), PrintStatus::Failed);
    }

    #[test]
    fn test_cannot_retry_after_max_retries() {
        let mut job = make_job();
        job.queue().unwrap();
        job.fail("err".to_string()).unwrap();
        // retry 1
        assert!(job.retry().is_ok());
        assert_eq!(job.retry_count(), 1);
        job.fail("err".to_string()).unwrap();
        // retry 2
        assert!(job.retry().is_ok());
        assert_eq!(job.retry_count(), 2);
        job.fail("err".to_string()).unwrap();
        // retry 3
        assert!(job.retry().is_ok());
        assert_eq!(job.retry_count(), 3);
        job.fail("err".to_string()).unwrap();
        // retry 4 — should fail
        let result = job.retry();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), DomainError::MaxRetryExceeded);
    }

    #[test]
    fn test_retry_increments_count() {
        let mut job = make_job();
        job.queue().unwrap();
        job.fail("err".to_string()).unwrap();
        assert_eq!(job.retry_count(), 0);
        job.retry().unwrap();
        assert_eq!(job.retry_count(), 1);
    }

    #[test]
    fn test_retry_transitions_to_queued() {
        let mut job = make_job();
        job.queue().unwrap();
        job.fail("err".to_string()).unwrap();
        job.retry().unwrap();
        assert_eq!(*job.status(), PrintStatus::Queued);
    }

    #[test]
    fn test_cannot_retry_from_non_failed_state() {
        let mut job = make_job();
        // Job is PENDING, not FAILED
        let result = job.retry();
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_cancel_completed_job() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        job.complete().unwrap();
        let result = job.cancel();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), DomainError::CannotCancelCompleted);
    }

    #[test]
    fn test_cannot_cancel_failed_job() {
        let mut job = make_job();
        job.queue().unwrap();
        job.fail("err".to_string()).unwrap();
        let result = job.cancel();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), DomainError::CannotCancelFailed);
    }

    #[test]
    fn test_cancel_from_pending() {
        let mut job = make_job();
        assert!(job.cancel().is_ok());
        assert_eq!(*job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cancel_from_queued() {
        let mut job = make_job();
        job.queue().unwrap();
        assert!(job.cancel().is_ok());
        assert_eq!(*job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cancel_from_downloaded() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        assert!(job.cancel().is_ok());
        assert_eq!(*job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cannot_cancel_cancelled_job() {
        let mut job = make_job();
        job.cancel().unwrap();
        let result = job.cancel();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), DomainError::CannotCancelCancelled);
    }

    #[test]
    fn test_cancel_emits_cancelled_event() {
        let mut job = make_job();
        job.drain_events();
        assert!(job.cancel().is_ok());
        assert_eq!(job.pending_events_count(), 1);
        let events = job.drain_events();
        assert_eq!(events[0].event_type(), "PrintJobCancelled");
    }

    #[test]
    fn test_fail_from_submitted_to_queue() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        assert!(job.fail("Printer offline".to_string()).is_ok());
        assert_eq!(*job.status(), PrintStatus::Failed);
    }

    #[test]
    fn test_mark_printing_emits_event() {
        let mut job = make_job();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.drain_events();
        assert!(job.mark_printing().is_ok());
        assert_eq!(job.pending_events_count(), 1);
        let events = job.drain_events();
        assert_eq!(events[0].event_type(), "PrintJobPrinting");
    }

    #[test]
    fn test_events_collected_on_transitions() {
        let mut job = make_job();
        assert_eq!(job.pending_events_count(), 1); // PrintJobCreated
        job.queue().unwrap();
        assert_eq!(job.pending_events_count(), 2); // + PrintJobQueued
        job.mark_downloaded().unwrap();
        assert_eq!(job.pending_events_count(), 3); // + PrintJobDownloaded
    }

    #[test]
    fn test_drain_events_clears_buffer() {
        let mut job = make_job();
        assert_eq!(job.pending_events_count(), 1);
        let events = job.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(job.pending_events_count(), 0);
    }

    #[test]
    fn test_drain_events_returns_correct_types() {
        let mut job = make_job();
        job.queue().unwrap();
        let events = job.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "PrintJobCreated");
        assert_eq!(events[1].event_type(), "PrintJobQueued");
    }

    #[test]
    fn test_accessors() {
        let job = make_job();
        assert_eq!(job.pdf_url(), "https://s3.example.com/doc.pdf");
        assert_eq!(job.printer_name(), "HP_LaserJet");
        assert_eq!(job.retry_count(), 0);
    }

    #[test]
    fn test_invalid_transition_pending_to_downloaded() {
        let mut job = make_job();
        let result = job.mark_downloaded();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_transition_pending_to_completed() {
        let mut job = make_job();
        let result = job.complete();
        assert!(result.is_err());
    }
}
