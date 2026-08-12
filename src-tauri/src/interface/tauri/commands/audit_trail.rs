use crate::AppContextState;
use crate::application::dto::AuditTrailDto;

pub fn execute_get_job_audit_trail(
    job_id: String,
    ctx: &AppContextState,
) -> Result<AuditTrailDto, String> {
    let use_case = ctx.get_audit_trail_uc.clone();
    let result = use_case.execute(&job_id).map_err(|e| format!("{}", e))?;

    Ok(AuditTrailDto::from_result(job_id, result))
}
