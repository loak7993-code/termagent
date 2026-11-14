use crate::types::Effort;

/// Per-message heuristic behind `Effort::Auto`.
///
/// Cheap enough to run on every turn and deterministic, so the UI can show the
/// level it picked before the request goes out.
pub fn auto_level(text: &str, tool_rounds: u32) -> Effort {
    let chars = text.chars().count();
    let has_code = text.contains("```");
    let has_task_verb = [
        "refactor",
        "debug",
        "architect",
        "design",
        "migrate",
        "optimize",
        "security",
        "benchmark",
        "prove",
        "explain why",
        "review",
        "implement",
        "test",
        "fix",
    ]
    .iter()
    .any(|v| text.to_ascii_lowercase().contains(v));

    if has_code || has_task_verb || chars > 400 || tool_rounds >= 2 {
        Effort::High
    } else if chars < 120 && !text.contains('?') {
        Effort::Low
    } else {
        Effort::Medium
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_chat_is_low() {
        assert_eq!(auto_level("thanks", 0), Effort::Low);
        assert_eq!(auto_level("list files", 0), Effort::Low);
    }

    #[test]
    fn questions_are_medium() {
        assert_eq!(auto_level("what does this project do?", 0), Effort::Medium);
    }

    #[test]
    fn code_and_task_verbs_are_high() {
        assert_eq!(auto_level("```rust\nfn main() {}\n```", 0), Effort::High);
        assert_eq!(auto_level("refactor the auth module", 0), Effort::High);
        assert_eq!(auto_level("x".repeat(401).as_str(), 0), Effort::High);
    }

    #[test]
    fn tool_rounds_raise_effort() {
        assert_eq!(auto_level("continue", 2), Effort::High);
    }
}
