//! Output helpers — JSON emission and plain-text formatting.

use bimo_core::models::ModelEntry;
use serde::Serialize;

/// Prints `value` as pretty JSON to stdout.
pub fn emit_json<T: Serialize>(value: &T) -> crate::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Prints `values` as a pretty JSON array to stdout.
pub fn emit_json_array<T: Serialize>(values: &[T]) -> crate::Result<()> {
    println!("{}", serde_json::to_string_pretty(values)?);
    Ok(())
}

/// Truncates `s` to at most `max` characters, appending `…` when cut.
pub fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(max).collect();
        out.push('…');
        out
    }
}

/// Prints a list of model entries as a table, or as a JSON array with `json`.
pub fn print_models(json: bool, models: &[ModelEntry]) -> crate::Result<()> {
    if json {
        return emit_json_array(models);
    }
    if models.is_empty() {
        println!("(no models)");
        return Ok(());
    }
    for m in models {
        let context = m
            .limit
            .as_ref()
            .and_then(|l| l.context)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "—".to_string());
        let input_cost = m
            .cost
            .as_ref()
            .and_then(|c| c.input)
            .map(|p| format!("${p}/M"))
            .unwrap_or_else(|| "—".to_string());
        println!(
            "{:<40} {:<36} ctx={:<10} in={}",
            truncate(&m.id, 40),
            truncate(&m.name, 36),
            context,
            input_cost,
        );
    }
    Ok(())
}
