use tauri::{AppHandle, Emitter};

use crate::shared::event_bus::{EventBus, EventBusError};

/// EventBus implementation that emits Tauri IPC events to the frontend.
/// Replaces InMemoryEventBus (no-op) to enable real-time UI updates.
pub struct TauriEventBus {
    app_handle: AppHandle,
}

impl TauriEventBus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventBus for TauriEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        tracing::debug!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            bus = "tauri",
            "EventBus: event published"
        );
        let tauri_event = match event_type {
            "PrintJobCreated"
            | "PrintJobQueued"
            | "PrintJobDownloaded"
            | "PrintJobSubmitted"
            | "PrintJobPrinting"
            | "PrintJobCompleted"
            | "PrintJobFailed"
            | "PrintJobCancelled" => "job_status_changed",
            _ => return Ok(()),
        };

        let domain_payload: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| EventBusError::PublishFailed {
                reason: format!("Invalid JSON payload: {}", e),
            })?;

        let job_id = domain_payload["job_id"].as_str().unwrap_or("");
        if job_id.is_empty() {
            return Err(EventBusError::PublishFailed {
                reason: format!("Missing job_id in payload for event: {}", event_type),
            });
        }
        let status = event_type_to_status(event_type);
        let progress = status_to_progress(status);
        let error_message = domain_payload.get("reason").and_then(|v| v.as_str());

        let ui_payload = serde_json::json!({
            "job_id": job_id,
            "status": status,
            "progress": progress,
            "error_message": error_message,
        });

        self.app_handle
            .emit(tauri_event, ui_payload)
            .map_err(|e| EventBusError::PublishFailed {
                reason: format!("Tauri emit failed: {}", e),
            })?;

        Ok(())
    }
}

fn event_type_to_status(event_type: &str) -> &'static str {
    match event_type {
        "PrintJobCreated" => "PENDING",
        "PrintJobQueued" => "QUEUED",
        "PrintJobDownloaded" => "DOWNLOADED",
        "PrintJobSubmitted" => "SUBMITTED_TO_QUEUE",
        "PrintJobPrinting" => "PRINTING",
        "PrintJobCompleted" => "COMPLETED",
        "PrintJobFailed" => "FAILED",
        "PrintJobCancelled" => "CANCELLED",
        _ => "UNKNOWN",
    }
}

fn status_to_progress(status: &str) -> u8 {
    match status {
        "PENDING" => 0,
        "QUEUED" => 10,
        "DOWNLOADED" => 40,
        "SUBMITTED_TO_QUEUE" => 60,
        "PRINTING" => 80,
        "COMPLETED" => 100,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_to_status_mapping() {
        assert_eq!(event_type_to_status("PrintJobCreated"), "PENDING");
        assert_eq!(event_type_to_status("PrintJobQueued"), "QUEUED");
        assert_eq!(event_type_to_status("PrintJobDownloaded"), "DOWNLOADED");
        assert_eq!(
            event_type_to_status("PrintJobSubmitted"),
            "SUBMITTED_TO_QUEUE"
        );
        assert_eq!(event_type_to_status("PrintJobPrinting"), "PRINTING");
        assert_eq!(event_type_to_status("PrintJobCompleted"), "COMPLETED");
        assert_eq!(event_type_to_status("PrintJobFailed"), "FAILED");
        assert_eq!(event_type_to_status("PrintJobCancelled"), "CANCELLED");
        assert_eq!(event_type_to_status("UnknownEvent"), "UNKNOWN");
    }

    #[test]
    fn test_status_to_progress_mapping() {
        assert_eq!(status_to_progress("PENDING"), 0);
        assert_eq!(status_to_progress("QUEUED"), 10);
        assert_eq!(status_to_progress("DOWNLOADED"), 40);
        assert_eq!(status_to_progress("SUBMITTED_TO_QUEUE"), 60);
        assert_eq!(status_to_progress("PRINTING"), 80);
        assert_eq!(status_to_progress("COMPLETED"), 100);
        assert_eq!(status_to_progress("FAILED"), 0);
        assert_eq!(status_to_progress("CANCELLED"), 0);
        assert_eq!(status_to_progress("UNKNOWN"), 0);
    }
}
