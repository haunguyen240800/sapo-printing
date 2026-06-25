use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::shared::event_bus::{EventBus, EventBusError, EventHandler};

/// EventBus implementation that emits Tauri IPC events to the frontend.
/// Replaces InMemoryEventBus (no-op) to enable real-time UI updates.
/// Also supports handler subscription for backend event processing.
pub struct TauriEventBus {
    app_handle: AppHandle,
    handlers: std::sync::Mutex<std::collections::HashMap<String, Vec<Arc<dyn EventHandler>>>>,
}

impl TauriEventBus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            handlers: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl EventBus for TauriEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        tracing::info!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            payload = payload,
            bus = "tauri",
            "EventBus: publishing event"
        );

        // 1. Invoke all registered backend handlers in background thread (non-blocking)
        let handlers = self.handlers.lock().unwrap();
        let handler_count = handlers.get(event_type).map(|h| h.len()).unwrap_or(0);
        tracing::info!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            handler_count = handler_count,
            "EventBus: found handlers for event"
        );

        if let Some(handler_list) = handlers.get(event_type) {
            for handler in handler_list {
                let handler_clone = Arc::clone(handler);
                let event_type_owned = event_type.to_string();
                let payload_owned = payload.to_string();

                // Spawn handler execution in background thread to avoid blocking UI
                std::thread::spawn(move || {
                    handler_clone.handle(&event_type_owned, &payload_owned);
                });
            }
        }
        drop(handlers); // Release lock before emitting to frontend

        // 2. Emit to Tauri frontend (existing logic)
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

    fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(handler);

        tracing::debug!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            "TauriEventBus: handler subscribed"
        );
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
