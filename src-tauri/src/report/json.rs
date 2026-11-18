use crate::score::ScanSnapshot;
pub fn render(
    snapshot: &ScanSnapshot,
    score: &crate::domain::types::ScoreSnapshot,
) -> Result<String, crate::error::AppError> {
    serde_json::to_string_pretty(&serde_json::json!({"scan":snapshot,"score":score}))
        .map_err(|_| crate::error::AppError::Validation("JSON report could not be rendered".into()))
}
