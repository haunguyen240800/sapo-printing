use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::application::ports::event_bus::{EventBus, EventHandler};

const JOB_STATUS_EVENTS: &[&str] = &[
    "PrintJobCreated",
    "PrintJobQueued",
    "PrintJobDownloaded",
    "PrintJobSubmitted",
    "PrintJobPrinting",
    "PrintJobCompleted",
    "PrintJobFailed",
    "PrintJobCancelled",
];

const TAURI_EVENT_NAME: &str = "job_status_changed";

pub struct PrintJobEventEmitter {
    app_handle: AppHandle,
}

impl PrintJobEventEmitter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn register(self, bus: &Arc<dyn EventBus>) -> Arc<Self> {
        let me = Arc::new(self);
        for event_type in JOB_STATUS_EVENTS {
            bus.subscribe(event_type, me.clone() as Arc<dyn EventHandler>);
        }
        me
    }
}

impl EventHandler for PrintJobEventEmitter {
    fn handle(&self, event_type: &str, payload: &str) {
        let parsed: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    target = "sapo_printer::ui_presenter",
                    event_type,
                    error = %e,
                    "PrintJobEventEmitter: invalid JSON payload, skipping emit"
                );
                return;
            }
        };

        let job_id = parsed.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
        if job_id.is_empty() {
            tracing::warn!(
                target = "sapo_printer::ui_presenter",
                event_type,
                "PrintJobEventEmitter: missing job_id in payload, skipping emit"
            );
            return;
        }

        let status = event_type_to_status(event_type);
        let progress = status_to_progress(status);
        let error_message = parsed.get("reason").and_then(|v| v.as_str());
        let error_code = parsed.get("error_code").and_then(|v| v.as_str());

        let ui_payload = serde_json::json!({
            "job_id": job_id,
            "status": status,
            "progress": progress,
            "error_message": error_message,
            "error_code": error_code,
        });

        if let Err(e) = self.app_handle.emit(TAURI_EVENT_NAME, ui_payload) {
            tracing::warn!(
                target = "sapo_printer::ui_presenter",
                event_type,
                error = %e,
                "PrintJobEventEmitter: Tauri emit failed"
            );
        }
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
