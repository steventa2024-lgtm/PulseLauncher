//! Launcher search: ranks apps, commands, workflows, indexed files, the
//! calculator and an optional web-search action.
//!
//! Query prefixes (documented in the UI and README):
//! `@` apps only, `>` commands, `/` workflows, `#` files & folders,
//! `=` calculator, `?` web search.

use serde::Serialize;

use crate::models::{AppEntry, UserPreferences};

pub mod calc;
pub mod files;
pub mod fuzzy;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    App,
    Command,
    Workflow,
    File,
    Folder,
    Calculator,
    WebSearch,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Stable key: `<kind>:<id>` so selection survives re-ordering.
    pub key: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    pub score: i64,
    pub highlights: Vec<usize>,
    pub app_id: Option<String>,
    pub workflow_id: Option<String>,
    pub path: Option<String>,
    pub value: Option<String>,
    pub favorite: bool,
    pub missing: bool,
    pub icon_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkflowSearchItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_command: bool,
    pub needs_setup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    All,
    Apps,
    Commands,
    Workflows,
    Files,
    Calc,
    Web,
}

pub fn parse_mode(q: &str) -> (Mode, &str) {
    let t = q.trim_start();
    let mut chars = t.chars();
    let mode = match chars.next() {
        Some('@') => Mode::Apps,
        Some('>') => Mode::Commands,
        Some('/') => Mode::Workflows,
        Some('#') => Mode::Files,
        Some('=') => return (Mode::Calc, t),
        Some('?') => Mode::Web,
        _ => return (Mode::All, t.trim()),
    };
    (mode, chars.as_str().trim())
}

pub struct SearchContext<'a> {
    pub apps: &'a [AppEntry],
    pub workflows: &'a [WorkflowSearchItem],
    pub files: &'a [(String, String, bool)],
    pub prefs: &'a UserPreferences,
    pub limit: usize,
}

fn usage_bonus(app: &AppEntry) -> i64 {
    let mut b = (app.launch_count * 6).min(120);
    if app.favorite {
        b += 60;
    }
    if let Some(at) = &app.last_launched_at {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(at) {
            let days = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_days();
            b += (30 - days.clamp(0, 30)) * 2;
        }
    }
    b
}

fn app_result(app: &AppEntry, score: i64, highlights: Vec<usize>) -> SearchResult {
    let subtitle = match app.target.kind {
        crate::models::AppKind::Packaged => "Windows app".to_string(),
        crate::models::AppKind::Uri => app.target.launch_target.clone(),
        _ => app
            .target
            .resolved_target
            .clone()
            .unwrap_or_else(|| app.target.launch_target.clone()),
    };
    SearchResult {
        key: format!("app:{}", app.id),
        kind: ResultKind::App,
        title: app.name.clone(),
        subtitle,
        score,
        highlights,
        app_id: Some(app.id.clone()),
        workflow_id: None,
        path: None,
        value: None,
        favorite: app.favorite,
        missing: app.status == crate::models::TargetStatus::Missing,
        icon_key: Some(app.id.clone()),
    }
}

pub fn search(query: &str, ctx: &SearchContext) -> Vec<SearchResult> {
    let (mode, q) = parse_mode(query);
    let limit = ctx.limit.clamp(1, 200);
    let mut out: Vec<SearchResult> = Vec::new();

    // Empty query: favourites and recents.
    if q.is_empty() && matches!(mode, Mode::All | Mode::Apps) {
        let mut apps: Vec<&AppEntry> = ctx
            .apps
            .iter()
            .filter(|a| !a.hidden && (a.favorite || a.last_launched_at.is_some()))
            .collect();
        apps.sort_by(|a, b| {
            b.favorite
                .cmp(&a.favorite)
                .then(b.last_launched_at.cmp(&a.last_launched_at))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out.extend(
            apps.into_iter()
                .take(limit)
                .map(|a| app_result(a, usage_bonus(a), vec![])),
        );
        return out;
    }

    if matches!(mode, Mode::All | Mode::Calc) {
        if let Some(v) = calc::evaluate(query.trim()) {
            let formatted = calc::format(v);
            out.push(SearchResult {
                key: "calc:result".into(),
                kind: ResultKind::Calculator,
                title: format!("= {formatted}"),
                subtitle: format!("{} - press Enter to copy", q.trim_start_matches('=').trim()),
                score: 2000,
                highlights: vec![],
                app_id: None,
                workflow_id: None,
                path: None,
                value: Some(formatted),
                favorite: false,
                missing: false,
                icon_key: None,
            });
        }
        if mode == Mode::Calc {
            return out;
        }
    }

    if matches!(mode, Mode::All | Mode::Apps) {
        for app in ctx.apps.iter().filter(|a| !a.hidden) {
            let mut best: Option<fuzzy::Match> = fuzzy::score(q, &app.name);
            let mut alt_score: Option<i64> = None;
            let aliases = std::iter::once(app.original_name.as_str())
                .chain(app.tags.iter().map(String::as_str))
                .chain(app.metadata.product_name.as_deref());
            for alias in aliases {
                if let Some(m) = fuzzy::score(q, alias) {
                    let s = m.score - 50;
                    if alt_score.is_none_or(|x| s > x) {
                        alt_score = Some(s);
                    }
                }
            }
            if let Some(alt) = alt_score {
                if best.as_ref().is_none_or(|b| alt > b.score) {
                    best = Some(fuzzy::Match {
                        score: alt,
                        positions: vec![],
                    });
                }
            }
            if let Some(m) = best {
                if m.score < 200 && q.chars().count() < 3 {
                    continue;
                }
                out.push(app_result(app, m.score + usage_bonus(app), m.positions));
            }
        }
    }

    if matches!(mode, Mode::All | Mode::Commands | Mode::Workflows) {
        for wf in ctx.workflows {
            if mode == Mode::Commands && !wf.is_command {
                continue;
            }
            if let Some(m) = fuzzy::score(q, &wf.name).or_else(|| {
                fuzzy::score(q, &wf.description).map(|m| fuzzy::Match {
                    score: m.score / 3,
                    positions: vec![],
                })
            }) {
                if m.score < 150 && mode == Mode::All {
                    continue;
                }
                out.push(SearchResult {
                    key: format!("workflow:{}", wf.id),
                    kind: if wf.is_command {
                        ResultKind::Command
                    } else {
                        ResultKind::Workflow
                    },
                    title: wf.name.clone(),
                    subtitle: if wf.needs_setup {
                        format!("Needs setup - {}", wf.description)
                    } else {
                        wf.description.clone()
                    },
                    score: m.score - if mode == Mode::All { 40 } else { 0 },
                    highlights: m.positions,
                    app_id: None,
                    workflow_id: Some(wf.id.clone()),
                    path: None,
                    value: None,
                    favorite: false,
                    missing: false,
                    icon_key: None,
                });
            }
        }
    }

    let files_enabled = ctx.prefs.search_files && !ctx.files.is_empty();
    if files_enabled && (mode == Mode::Files || (mode == Mode::All && q.chars().count() >= 2)) {
        let mut file_hits: Vec<SearchResult> = ctx
            .files
            .iter()
            .filter_map(|(path, name, is_dir)| {
                fuzzy::score(q, name)
                    .filter(|m| m.score >= 300 || mode == Mode::Files)
                    .map(|m| SearchResult {
                        key: format!("file:{path}"),
                        kind: if *is_dir {
                            ResultKind::Folder
                        } else {
                            ResultKind::File
                        },
                        title: name.clone(),
                        subtitle: path.clone(),
                        score: m.score - if mode == Mode::All { 150 } else { 0 },
                        highlights: m.positions,
                        app_id: None,
                        workflow_id: None,
                        path: Some(path.clone()),
                        value: None,
                        favorite: false,
                        missing: false,
                        icon_key: None,
                    })
            })
            .collect();
        file_hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.title.len().cmp(&b.title.len()))
        });
        file_hits.truncate(if mode == Mode::Files { limit } else { 8 });
        out.extend(file_hits);
    }

    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.key.cmp(&b.key))
    });
    // Deduplicate by key.
    let mut seen = std::collections::HashSet::new();
    out.retain(|r| seen.insert(r.key.clone()));
    out.truncate(limit);

    if ctx.prefs.web_search_enabled && !q.is_empty() && matches!(mode, Mode::All | Mode::Web) {
        let url = ctx.prefs.web_search_template.replace(
            "{query}",
            &url::form_urlencoded::byte_serialize(q.as_bytes()).collect::<String>(),
        );
        out.push(SearchResult {
            key: "web:search".into(),
            kind: ResultKind::WebSearch,
            title: format!("Search the web for \"{q}\""),
            subtitle: "Opens your default browser".into(),
            score: 0,
            highlights: vec![],
            app_id: None,
            workflow_id: None,
            path: None,
            value: Some(url),
            favorite: false,
            missing: false,
            icon_key: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn app(id: &str, name: &str, launches: i64, fav: bool) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            original_name: name.into(),
            source: DiscoveredSource::StartMenuUser,
            target: ExecutableTarget {
                kind: AppKind::Shortcut,
                launch_target: format!("C:\\{name}.lnk"),
                resolved_target: Some(format!("C:\\{name}.exe")),
                arguments: vec![],
                link_arguments: None,
                working_dir: None,
            },
            icon_path: None,
            icon_index: 0,
            favorite: fav,
            hidden: false,
            portable: false,
            category: None,
            tags: vec![],
            collection_ids: vec![],
            status: TargetStatus::Ok,
            metadata: Default::default(),
            launch_count: launches,
            last_launched_at: None,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
            last_seen_at: None,
            preview_asset_id: None,
        }
    }

    fn ctx<'a>(
        apps: &'a [AppEntry],
        wfs: &'a [WorkflowSearchItem],
        prefs: &'a UserPreferences,
    ) -> SearchContext<'a> {
        SearchContext {
            apps,
            workflows: wfs,
            files: &[],
            prefs,
            limit: 20,
        }
    }

    #[test]
    fn exact_beats_usage_and_order_is_stable() {
        let prefs = UserPreferences::default();
        let apps = vec![
            app("1", "Code", 0, false),
            app("2", "Codex Helper", 50, false),
            app("3", "Visual Studio Code", 0, false),
        ];
        let r = search("code", &ctx(&apps, &[], &prefs));
        assert_eq!(r[0].title, "Code");
        let r2 = search("code", &ctx(&apps, &[], &prefs));
        assert_eq!(
            r.iter().map(|x| &x.key).collect::<Vec<_>>(),
            r2.iter().map(|x| &x.key).collect::<Vec<_>>()
        );
    }

    #[test]
    fn usage_breaks_ties() {
        let prefs = UserPreferences::default();
        let apps = vec![
            app("1", "Pulse Alpha", 0, false),
            app("2", "Pulse Beta", 10, false),
        ];
        let r = search("pulse", &ctx(&apps, &[], &prefs));
        assert_eq!(r[0].title, "Pulse Beta");
    }

    #[test]
    fn modes_and_calculator() {
        let prefs = UserPreferences::default();
        let apps = vec![app("1", "Downloads Manager", 0, false)];
        let wfs = vec![WorkflowSearchItem {
            id: "w".into(),
            name: "Open Downloads".into(),
            description: "Opens the Downloads folder".into(),
            is_command: true,
            needs_setup: false,
        }];
        let c = ctx(&apps, &wfs, &prefs);
        let r = search("> downloads", &c);
        assert!(r.iter().all(|x| x.kind == ResultKind::Command));
        let r = search("@downloads", &c);
        assert!(r.iter().all(|x| x.kind == ResultKind::App));
        let r = search("12*12", &c);
        assert_eq!(r[0].value.as_deref(), Some("144"));
        assert!(
            search("? anything", &c).is_empty(),
            "web search disabled by default"
        );
    }

    #[test]
    fn empty_query_shows_favorites() {
        let prefs = UserPreferences::default();
        let apps = vec![app("1", "A", 0, false), app("2", "B", 0, true)];
        let r = search("", &ctx(&apps, &[], &prefs));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "B");
    }

    #[test]
    fn web_search_url_is_encoded() {
        let prefs = UserPreferences {
            web_search_enabled: true,
            ..UserPreferences::default()
        };
        let r = search("?a&b c", &ctx(&[], &[], &prefs));
        assert_eq!(
            r[0].value.as_deref(),
            Some("https://duckduckgo.com/?q=a%26b+c")
        );
    }

    #[test]
    fn thousand_entries_search_is_fast() {
        let prefs = UserPreferences::default();
        let apps: Vec<AppEntry> = (0..1000)
            .map(|i| {
                app(
                    &i.to_string(),
                    &format!("Application Number {i} Tool"),
                    i % 7,
                    i % 13 == 0,
                )
            })
            .collect();
        let start = std::time::Instant::now();
        for q in ["app", "num 5", "tool", "an9", "zzz"] {
            let _ = search(q, &ctx(&apps, &[], &prefs));
        }
        let per_query = start.elapsed() / 5;
        eprintln!("search over 1000 entries: {:?} per query", per_query);
        assert!(per_query.as_millis() < 250, "debug-build sanity bound");
    }
}
