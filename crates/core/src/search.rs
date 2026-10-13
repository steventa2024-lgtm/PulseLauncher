use crate::calculator::{evaluate, format_number, looks_like_math};
use crate::models::{AppEntry, IndexedFile, SearchHit, WorkflowRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    All,
    Apps,
    Commands,
    Workflows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub mode: SearchMode,
    pub text: String,
}

pub fn parse_query(input: &str) -> ParsedQuery {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix('>') {
        return ParsedQuery {
            mode: SearchMode::Commands,
            text: rest.trim().to_string(),
        };
    }
    if let Some(rest) = trimmed.strip_prefix('@') {
        return ParsedQuery {
            mode: SearchMode::Apps,
            text: rest.trim().to_string(),
        };
    }
    if let Some(rest) = trimmed.strip_prefix('/') {
        return ParsedQuery {
            mode: SearchMode::Workflows,
            text: rest.trim().to_string(),
        };
    }
    ParsedQuery {
        mode: SearchMode::All,
        text: trimmed.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyHit {
    pub score: i32,
    pub indices: Vec<usize>,
}

fn norm_char(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

pub fn fuzzy_match(query: &str, target: &str) -> Option<FuzzyHit> {
    let query_chars: Vec<char> = query
        .chars()
        .map(norm_char)
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if query_chars.is_empty() {
        return Some(FuzzyHit {
            score: 1,
            indices: Vec::new(),
        });
    }
    let target_chars: Vec<char> = target.chars().collect();
    let norm: Vec<char> = target_chars.iter().copied().map(norm_char).collect();
    let mut indices = Vec::new();
    let mut query_index = 0;
    let mut score = 0;
    let mut previous: Option<usize> = None;
    for (index, ch) in norm.iter().enumerate() {
        if query_index < query_chars.len() && *ch == query_chars[query_index] {
            indices.push(index);
            score += 6;
            if previous == Some(index.saturating_sub(1)) && index > 0 {
                score += 10;
            }
            let boundary = index == 0 || !norm[index - 1].is_alphanumeric();
            if boundary {
                score += 18;
            }
            previous = Some(index);
            query_index += 1;
        }
    }
    if query_index != query_chars.len() {
        return None;
    }
    let needle: String = query.chars().map(norm_char).collect();
    let haystack: String = norm.into_iter().collect();
    if haystack == needle {
        score += 500;
    } else if haystack.starts_with(needle.trim()) {
        score += 220;
    } else if haystack.contains(needle.trim()) {
        score += 60;
    }
    Some(FuzzyHit { score, indices })
}

pub struct SearchInput<'a> {
    pub query: &'a str,
    pub apps: &'a [AppEntry],
    pub workflows: &'a [WorkflowRecord],
    pub files: &'a [IndexedFile],
    pub web_search: bool,
    pub limit: usize,
}

pub fn search(input: SearchInput<'_>) -> Vec<SearchHit> {
    let parsed = parse_query(input.query);
    let limit = input.limit.clamp(1, 100);
    if parsed.text.is_empty() {
        return empty_query_hits(input.apps, limit);
    }
    let mut hits = Vec::new();
    if matches!(parsed.mode, SearchMode::All | SearchMode::Apps) {
        for app in input.apps.iter().filter(|app| !app.hidden) {
            if let Some(mut hit) = score_app(app, &parsed.text) {
                hit.score += usage_boost(app);
                hits.push(hit);
            }
        }
    }
    if matches!(
        parsed.mode,
        SearchMode::All | SearchMode::Commands | SearchMode::Workflows
    ) {
        for workflow in input.workflows {
            if let Some(hit) = score_workflow(workflow, &parsed.text) {
                hits.push(hit);
            }
        }
    }
    if parsed.mode == SearchMode::All {
        for file in input.files {
            if let Some(hit) = score_file(file, &parsed.text) {
                hits.push(hit);
            }
        }
        if looks_like_math(&parsed.text) {
            match evaluate(&parsed.text) {
                Ok(value) => {
                    let shown = format_number(value);
                    hits.push(SearchHit {
                        id: format!("calc:{}", parsed.text),
                        kind: "calculator".into(),
                        title: shown.clone(),
                        subtitle: format!("Calculator · {}", parsed.text),
                        score: 10_000,
                        indices: Vec::new(),
                        app_id: None,
                        workflow_id: None,
                        path: None,
                        calculator_value: Some(shown),
                        url: None,
                    });
                }
                Err(err) => {
                    hits.push(SearchHit {
                        id: format!("calc:{}", parsed.text),
                        kind: "calculator".into(),
                        title: "Calculator".into(),
                        subtitle: err.to_string(),
                        score: 9_000,
                        indices: Vec::new(),
                        app_id: None,
                        workflow_id: None,
                        path: None,
                        calculator_value: None,
                        url: None,
                    });
                }
            }
        }
        if input.web_search && parsed.text.chars().count() >= 2 {
            hits.push(SearchHit {
                id: format!("web:{}", parsed.text),
                kind: "web".into(),
                title: format!("Search the web for \"{}\"", parsed.text),
                subtitle: "Opens DuckDuckGo in your browser".into(),
                score: 1,
                indices: Vec::new(),
                app_id: None,
                workflow_id: None,
                path: None,
                calculator_value: None,
                url: Some(web_url(&parsed.text)),
            });
        }
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    hits.truncate(limit);
    hits
}

fn usage_boost(app: &AppEntry) -> i32 {
    let mut score = 0;
    if app.favorite {
        score += 40;
    }
    if app.pinned {
        score += 30;
    }
    score += (app.launch_count.min(50) as i32) * 2;
    if app.missing {
        score -= 80;
    }
    score
}

fn score_app(app: &AppEntry, query: &str) -> Option<SearchHit> {
    let name = fuzzy_match(query, &app.display_name);
    let target = fuzzy_match(query, &app.launch_target);
    let best = match (name, target) {
        (Some(name), Some(target)) => {
            if name.score >= target.score {
                name
            } else {
                target
            }
        }
        (Some(hit), None) | (None, Some(hit)) => hit,
        (None, None) => return None,
    };
    Some(SearchHit {
        id: format!("app:{}", app.id),
        kind: "app".into(),
        title: app.display_name.clone(),
        subtitle: app
            .resolved_target
            .clone()
            .unwrap_or_else(|| app.launch_target.clone()),
        score: best.score,
        indices: best.indices,
        app_id: Some(app.id.clone()),
        workflow_id: None,
        path: Some(app.launch_target.clone()),
        calculator_value: None,
        url: None,
    })
}

fn score_workflow(workflow: &WorkflowRecord, query: &str) -> Option<SearchHit> {
    let hit = fuzzy_match(query, &workflow.name)?;
    Some(SearchHit {
        id: format!("workflow:{}", workflow.id),
        kind: "workflow".into(),
        title: workflow.name.clone(),
        subtitle: workflow.description.clone(),
        score: hit.score,
        indices: hit.indices,
        app_id: None,
        workflow_id: Some(workflow.id.clone()),
        path: None,
        calculator_value: None,
        url: None,
    })
}

fn score_file(file: &IndexedFile, query: &str) -> Option<SearchHit> {
    let hit = fuzzy_match(query, &file.name)?;
    Some(SearchHit {
        id: format!("file:{}", file.path),
        kind: "file".into(),
        title: file.name.clone(),
        subtitle: file.parent.clone(),
        score: hit.score.saturating_sub(15),
        indices: hit.indices,
        app_id: None,
        workflow_id: None,
        path: Some(file.path.clone()),
        calculator_value: None,
        url: None,
    })
}

fn empty_query_hits(apps: &[AppEntry], limit: usize) -> Vec<SearchHit> {
    let mut apps: Vec<&AppEntry> = apps
        .iter()
        .filter(|app| !app.hidden && !app.missing)
        .collect();
    apps.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then(right.favorite.cmp(&left.favorite))
            .then(right.last_launched_at.cmp(&left.last_launched_at))
            .then(
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase()),
            )
            .then(left.id.cmp(&right.id))
    });
    apps.into_iter()
        .take(limit)
        .map(|app| SearchHit {
            id: format!("app:{}", app.id),
            kind: "app".into(),
            title: app.display_name.clone(),
            subtitle: if app.pinned {
                "Pinned".into()
            } else if app.last_launched_at.is_some() {
                "Recent".into()
            } else {
                "Favorite".into()
            },
            score: 0,
            indices: Vec::new(),
            app_id: Some(app.id.clone()),
            workflow_id: None,
            path: Some(app.launch_target.clone()),
            calculator_value: None,
            url: None,
        })
        .collect()
}

pub fn web_url(query: &str) -> String {
    let mut encoded = String::new();
    for byte in query.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("https://duckduckgo.com/?q={encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppKind, SourceKind, WorkflowDocument};

    fn app(id: &str, name: &str, launches: i64, favorite: bool) -> AppEntry {
        AppEntry {
            id: id.into(),
            display_name: name.into(),
            kind: AppKind::Win32,
            source: SourceKind::Manual,
            launch_target: format!("/tmp/{id}"),
            resolved_target: Some(format!("/tmp/{id}")),
            arguments: Vec::new(),
            working_directory: None,
            icon_path: None,
            publisher: None,
            version: None,
            category: "Other".into(),
            hidden: false,
            favorite,
            pinned: false,
            user_modified: false,
            discovered: false,
            tags: Vec::new(),
            launch_count: launches,
            last_launched_at: None,
            missing: false,
            identity_key: format!("path:/tmp/{id}"),
            notes: None,
            created_at: "t".into(),
            updated_at: "t".into(),
            size_bytes: None,
        }
    }

    #[test]
    fn exact_and_prefix_rank_above_fuzzy_and_stay_stable() {
        let apps = vec![
            app("b", "Notepad Plus", 0, false),
            app("a", "Notepad", 0, false),
            app("c", "Notes", 20, true),
        ];
        let hits = search(SearchInput {
            query: "notepad",
            apps: &apps,
            workflows: &[],
            files: &[],
            web_search: false,
            limit: 10,
        });
        assert_eq!(hits[0].app_id.as_deref(), Some("a"));
        assert!(hits.iter().any(|hit| hit.app_id.as_deref() == Some("b")));
        let again = search(SearchInput {
            query: "notepad",
            apps: &apps,
            workflows: &[],
            files: &[],
            web_search: false,
            limit: 10,
        });
        assert_eq!(
            hits.iter().map(|hit| hit.id.clone()).collect::<Vec<_>>(),
            again.iter().map(|hit| hit.id.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn modes_and_calculator() {
        assert_eq!(parse_query("> copy").mode, SearchMode::Commands);
        assert_eq!(parse_query("@ code").mode, SearchMode::Apps);
        assert_eq!(parse_query("/ deploy").text, "deploy");
        let hits = search(SearchInput {
            query: "(1+2)*3",
            apps: &[],
            workflows: &[],
            files: &[],
            web_search: true,
            limit: 5,
        });
        assert_eq!(hits[0].kind, "calculator");
        assert_eq!(hits[0].calculator_value.as_deref(), Some("9"));
        assert!(hits.iter().any(|hit| hit.kind == "web"));
        assert!(web_url("a b").contains("a+b"));
    }

    #[test]
    fn workflow_prefix_search() {
        let workflow = WorkflowRecord {
            id: "wf".into(),
            name: "Open Downloads".into(),
            description: "Opens the Downloads folder".into(),
            template_key: Some("open-downloads".into()),
            template_version: Some(1),
            customized: false,
            document: WorkflowDocument {
                what_it_does: "x".into(),
                prerequisites_help: "y".into(),
                parameters: Vec::new(),
                values: Default::default(),
                steps: Vec::new(),
            },
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        let hits = search(SearchInput {
            query: "/ downloads",
            apps: &[],
            workflows: &[workflow],
            files: &[],
            web_search: false,
            limit: 5,
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "workflow");
    }

    #[test]
    fn thousand_item_rank_stays_responsive() {
        let apps: Vec<AppEntry> = (0..1000)
            .map(|index| {
                app(
                    &format!("id{index}"),
                    &format!("Tool {index}"),
                    index % 7,
                    index % 11 == 0,
                )
            })
            .collect();
        let started = std::time::Instant::now();
        let hits = search(SearchInput {
            query: "tool 42",
            apps: &apps,
            workflows: &[],
            files: &[],
            web_search: false,
            limit: 20,
        });
        let elapsed = started.elapsed();
        assert!(!hits.is_empty());
        assert!(
            elapsed.as_millis() < 500,
            "ranking 1000 apps took {} ms",
            elapsed.as_millis()
        );
    }
}
