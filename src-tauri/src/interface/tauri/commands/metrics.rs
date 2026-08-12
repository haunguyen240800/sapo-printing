use crate::AppContextState;
use crate::application::dto::MetricsDto;

pub fn execute_get_metrics(ctx: &AppContextState) -> Result<MetricsDto, String> {
    let use_case = ctx.get_metrics_uc.clone();
    let snapshot = use_case.execute().map_err(|e| format!("{}", e))?;

    Ok(MetricsDto::from(snapshot))
}
