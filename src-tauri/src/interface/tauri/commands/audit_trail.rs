use crate::application::use_cases::get_audit_trail::AuditTrailUseCase;
use crate::interface::tauri::dtos::audit_trail::{AuditEventDto, AuditTrailResponse};
use crate::AppContextState;

/// Execute the audit trail use case and map results to Tauri-compatible types.
/// Called from the `get_job_audit_trail` Tauri command in `main.rs`.
pub fn execute_get_job_audit_trail(
    job_id: String,
    ctx: &AppContextState,
) -> Result<AuditTrailResponse, String> {
    let use_case = AuditTrailUseCase::new(ctx.event_store.clone(), ctx.secret_manager.clone());

    let result = use_case.execute(&job_id).map_err(|e| format!("{}", e))?;

    let events: Vec<AuditEventDto> = result
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| AuditEventDto {
            sequence_number: event.sequence_number,
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            timestamp: event.timestamp,
            hmac_valid: result.event_hmac_valid.get(i).copied().unwrap_or(false),
        })
        .collect();

    Ok(AuditTrailResponse {
        job_id,
        events,
        chain_valid: result.report.chain_valid,
        tampered_count: result.report.tampered_events.len() as u64,
    })
}
