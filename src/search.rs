//! Ranked, multi-term search over apps.

use crate::config::AppEntry;

/// Scores how well `app` matches `query`.
///
/// Every whitespace-separated term must match the name, group, tags or
/// target (case-insensitive). Returns `None` if any term does not match.
/// Matches at the start of the name rank highest.
pub fn score(app: &AppEntry, query: &str) -> Option<u32> {
    let name = app.name.to_lowercase();
    let group = app.group.as_deref().unwrap_or_default().to_lowercase();
    let target = app.target.to_lowercase();
    let tags: Vec<String> = app.tags.iter().map(|t| t.to_lowercase()).collect();

    let mut total = 0;
    for term in query.split_whitespace().map(str::to_lowercase) {
        let term_score = if name.starts_with(&term) {
            100
        } else if name
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word.starts_with(&term))
        {
            70
        } else if name.contains(&term) {
            50
        } else if tags.iter().any(|t| t.starts_with(&term)) {
            30
        } else if group.starts_with(&term) || tags.iter().any(|t| t.contains(&term)) {
            20
        } else if target.contains(&term) {
            5
        } else {
            return None;
        };
        total += term_score;
    }
    Some(total)
}

/// Returns indices of matching apps, best match first (ties keep config order).
pub fn search(apps: &[AppEntry], query: &str) -> Vec<usize> {
    let mut hits: Vec<(usize, u32)> = apps
        .iter()
        .enumerate()
        .filter_map(|(i, app)| score(app, query).map(|s| (i, s)))
        .collect();
    hits.sort_by_key(|&(_, score)| std::cmp::Reverse(score)); // stable sort keeps config order for ties
    hits.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, target: &str, group: Option<&str>, tags: &[&str]) -> AppEntry {
        AppEntry {
            group: group.map(Into::into),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..AppEntry::new(name, target)
        }
    }

    fn apps() -> Vec<AppEntry> {
        vec![
            app("Visual Studio Code", "code.exe", Some("Dev"), &["editor"]),
            app("Notepad", "notepad.exe", Some("Tools"), &["editor", "text"]),
            app("Task Manager", "taskmgr.exe", Some("System"), &[]),
            app("Code Search", "https://cs.example", Some("Web"), &[]),
        ]
    }

    #[test]
    fn empty_query_matches_everything_in_order() {
        assert_eq!(search(&apps(), ""), [0, 1, 2, 3]);
        assert_eq!(search(&apps(), "   "), [0, 1, 2, 3]);
    }

    #[test]
    fn prefix_beats_word_start() {
        // "Code Search" starts with "code"; "Visual Studio Code" has it as a word.
        assert_eq!(search(&apps(), "code"), [3, 0]);
    }

    #[test]
    fn matches_tags_group_and_target() {
        assert_eq!(search(&apps(), "editor"), [0, 1]);
        assert_eq!(search(&apps(), "system"), [2]);
        assert_eq!(search(&apps(), "taskmgr"), [2]);
    }

    #[test]
    fn all_terms_must_match() {
        assert_eq!(search(&apps(), "editor note"), [1]);
        assert!(search(&apps(), "editor zzz").is_empty());
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(search(&apps(), "NOTEPAD"), [1]);
    }
}
