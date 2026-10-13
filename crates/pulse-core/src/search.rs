//! Ranking and fuzzy matching for the launcher. Pure functions over in-memory data so a query
//! with ~1,000 entries stays far below the 50 ms target.

use crate::calc;
use crate::models::*;
use crate::workflow::model::{WorkflowDefinition, WorkflowKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    App,
    Exe,
    File,
    Folder,
    Command,
    Workflow,
    Calculator,
    Web,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ResultAction {
    LaunchApp { app_id: String },
    RunWorkflow { workflow_id: String },
    OpenPath { path: String },
    CopyText { text: String },
    WebSearch { url: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Stable id (`app:<id>`, `wf:<id>`, `file:<path>`, `calc`, `web`).
    pub id: String,
    pub category: Category,
    pub title: String,
    pub subtitle: String,
    pub score: i32,
    /// Matched character indices in `title` (for highlighting).
    pub match_indices: Vec<usize>,
    pub action: ResultAction,
    pub app_id: Option<String>,
    pub favorite: bool,
    pub section: Option<String>,
}

pub struct SearchInputs<'a> {
    pub apps: &'a [AppEntry],
    pub workflows: &'a [WorkflowDefinition],
    pub files: &'a [(String, String, bool)],
    pub files_enabled: bool,
    pub web_enabled: bool,
    pub web_url_template: &'a str,
    pub now: DateTime<Utc>,
}

#[derive(Debug, PartialEq)]
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
    match chars.next() {
        Some('>') => (Mode::Commands, chars.as_str().trim()),
        Some('@') => (Mode::Apps, chars.as_str().trim()),
        Some('/') => (Mode::Workflows, chars.as_str().trim()),
        Some('~') => (Mode::Files, chars.as_str().trim()),
        Some('=') => (Mode::Calc, chars.as_str().trim()),
        Some('?') => (Mode::Web, chars.as_str().trim()),
        _ => (Mode::All, t.trim_end()),
    }
}

fn is_boundary(prev: char) -> bool {
    !prev.is_alphanumeric()
}

/// Fuzzy subsequence match with prefix / word-boundary / consecutive bonuses.
/// Returns the score and the matched char indices of `text`.
pub fn fuzzy(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    let q: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some((0, vec![]));
    }
    let t: Vec<char> = text.chars().collect();
    let tl: Vec<char> = t.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    if q.len() > tl.len() {
        return None;
    }
    let n = tl.len();
    let m = q.len();
    const NEG: i32 = i32::MIN / 2;
    // dp[j][i]: best score matching q[..=j] with q[j] at text position i.
    let mut dp = vec![vec![NEG; n]; m];
    let mut from = vec![vec![usize::MAX; n]; m];
    for i in 0..n {
        if tl[i] == q[0] {
            let mut s = 10;
            if i == 0 {
                s += 40;
            } else if is_boundary(t[i - 1]) || (t[i].is_uppercase() && t[i - 1].is_lowercase()) {
                s += 25;
            }
            dp[0][i] = s - i as i32 / 2;
        }
    }
    for j in 1..m {
        for i in j..n {
            if tl[i] != q[j] {
                continue;
            }
            let mut best = NEG;
            let mut bk = usize::MAX;
            for k in (j - 1)..i {
                if dp[j - 1][k] == NEG {
                    continue;
                }
                let mut s = dp[j - 1][k] + 10;
                if k + 1 == i {
                    s += 18;
                } else {
                    s -= ((i - k - 1) as i32).min(12);
                }
                if is_boundary(t[i - 1]) || (t[i].is_uppercase() && t[i - 1].is_lowercase()) {
                    s += 20;
                }
                if s > best {
                    best = s;
                    bk = k;
                }
            }
            dp[j][i] = best;
            from[j][i] = bk;
        }
    }
    let (mut bi, mut bs) = (usize::MAX, NEG);
    for i in 0..n {
        if dp[m - 1][i] > bs {
            bs = dp[m - 1][i];
            bi = i;
        }
    }
    if bs == NEG {
        return None;
    }
    let mut idx = vec![0; m];
    let mut cur = bi;
    for j in (0..m).rev() {
        idx[j] = cur;
        if j > 0 {
            cur = from[j][cur];
        }
    }
    let mut score = bs;
    let ql: String = q.iter().collect();
    let tls: String = tl.iter().collect();
    if tls == ql {
        score += 1000;
    } else if tls.starts_with(&ql) {
        score += 400;
    } else if tls.contains(&ql) {
        score += 120;
    }
    score += 30 - (n as i32 - m as i32).min(30);
    Some((score, idx))
}

fn usage_boost(count: i64, last: &Option<String>, now: DateTime<Utc>) -> i32 {
    let freq = ((count as f64 + 1.0).ln() * 28.0) as i32;
    let rec = last
        .as_deref()
        .and_then(|l| DateTime::parse_from_rfc3339(l).ok())
        .map(|d| {
            let days = (now - d.with_timezone(&Utc)).num_hours() as f64 / 24.0;
            (40.0 * (-days / 14.0).exp()) as i32
        })
        .unwrap_or(0);
    freq + rec
}

pub fn search(inp: &SearchInputs, raw_query: &str, limit: usize) -> Vec<SearchResult> {
    let (mode, q) = parse_mode(raw_query);
    let mut out: Vec<SearchResult> = Vec::new();

    if q.is_empty() && mode == Mode::All {
        return default_results(inp, limit);
    }

    let want_apps = matches!(mode, Mode::All | Mode::Apps);
    let want_cmds = matches!(mode, Mode::All | Mode::Commands);
    let want_wfs = matches!(mode, Mode::All | Mode::Workflows);
    let want_files = inp.files_enabled && matches!(mode, Mode::All | Mode::Files);

    if matches!(mode, Mode::All | Mode::Calc) {
        if let Some(v) = calc::evaluate(q) {
            if mode == Mode::Calc || calc::looks_like_expression(q) {
                let text = calc::format_result(v);
                out.push(SearchResult {
                    id: "calc".into(),
                    category: Category::Calculator,
                    title: format!("= {text}"),
                    subtitle: format!("{q}  -  Enter to copy result"),
                    score: 100_000,
                    match_indices: vec![],
                    action: ResultAction::CopyText { text },
                    app_id: None,
                    favorite: false,
                    section: None,
                });
            }
        }
    }

    if want_apps {
        for a in inp.apps.iter().filter(|a| !a.hidden) {
            let hay_tags = a.tags.join(" ");
            let primary = fuzzy(q, &a.name);
            let secondary = if primary.is_none() && !q.is_empty() {
                fuzzy(q, &hay_tags).map(|(s, _)| (s / 3, vec![]))
            } else {
                None
            };
            let Some((fs, idx)) = primary.or(secondary) else { continue };
            let mut score = fs + usage_boost(a.launch_count, &a.last_launched_at, inp.now);
            if a.favorite {
                score += 40;
            }
            if a.stale {
                score -= 60;
            }
            let (cat, sub) = if a.is_portable() {
                (Category::Exe, format!("Portable app  -  {}", a.target.path))
            } else {
                (Category::App, subtitle_for(a))
            };
            out.push(SearchResult {
                id: format!("app:{}", a.id),
                category: cat,
                title: a.name.clone(),
                subtitle: sub,
                score,
                match_indices: idx,
                action: ResultAction::LaunchApp { app_id: a.id.clone() },
                app_id: Some(a.id.clone()),
                favorite: a.favorite,
                section: None,
            });
        }
    }

    for w in inp.workflows.iter().filter(|w| w.enabled) {
        let wanted = match w.kind {
            WorkflowKind::Command => want_cmds,
            WorkflowKind::Workflow => want_wfs,
        };
        if !wanted {
            continue;
        }
        let Some((fs, idx)) = fuzzy(q, &w.name) else { continue };
        out.push(SearchResult {
            id: format!("wf:{}", w.id),
            category: if w.kind == WorkflowKind::Command { Category::Command } else { Category::Workflow },
            title: w.name.clone(),
            subtitle: w.description.clone(),
            score: fs - 5,
            match_indices: idx,
            action: ResultAction::RunWorkflow { workflow_id: w.id.clone() },
            app_id: None,
            favorite: false,
            section: None,
        });
    }

    if want_files && !q.is_empty() {
        let mut files: Vec<SearchResult> = inp
            .files
            .iter()
            .filter_map(|(p, n, is_dir)| {
                let (fs, idx) = fuzzy(q, n)?;
                Some(SearchResult {
                    id: format!("file:{p}"),
                    category: if *is_dir { Category::Folder } else { Category::File },
                    title: n.clone(),
                    subtitle: p.clone(),
                    score: fs - 30,
                    match_indices: idx,
                    action: ResultAction::OpenPath { path: p.clone() },
                    app_id: None,
                    favorite: false,
                    section: None,
                })
            })
            .collect();
        files.sort_by(|a, b| b.score.cmp(&a.score));
        files.truncate(8);
        out.extend(files);
    }

    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())).then_with(|| a.id.cmp(&b.id)));
    out.truncate(limit);

    if !q.is_empty() && inp.web_enabled && matches!(mode, Mode::All | Mode::Web) && !(mode == Mode::All && out.len() >= limit) {
        if let Some(url) = web_search_url(inp.web_url_template, q) {
            out.push(SearchResult {
                id: "web".into(),
                category: Category::Web,
                title: format!("Search the web for \u{201c}{q}\u{201d}"),
                subtitle: "Opens your default browser".into(),
                score: -100_000,
                match_indices: vec![],
                action: ResultAction::WebSearch { url },
                app_id: None,
                favorite: false,
                section: None,
            });
        }
    }
    out
}

fn subtitle_for(a: &AppEntry) -> String {
    match a.target.kind {
        TargetKind::Packaged => "Packaged app".into(),
        _ => a.resolved_path.clone().unwrap_or_else(|| a.target.path.clone()),
    }
}

fn default_results(inp: &SearchInputs, limit: usize) -> Vec<SearchResult> {
    let mut apps: Vec<&AppEntry> = inp.apps.iter().filter(|a| !a.hidden).collect();
    let mut out = Vec::new();
    let mk = |a: &AppEntry, section: &str, score: i32| SearchResult {
        id: format!("app:{}", a.id),
        category: if a.is_portable() { Category::Exe } else { Category::App },
        title: a.name.clone(),
        subtitle: subtitle_for(a),
        score,
        match_indices: vec![],
        action: ResultAction::LaunchApp { app_id: a.id.clone() },
        app_id: Some(a.id.clone()),
        favorite: a.favorite,
        section: Some(section.into()),
    };
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let mut used = std::collections::HashSet::new();
    let favs: Vec<_> = apps.iter().filter(|a| a.favorite).collect();
    for (i, a) in favs.iter().enumerate() {
        used.insert(a.id.clone());
        out.push(mk(a, "Pinned", 10_000 - i as i32));
    }
    let mut recent: Vec<&&AppEntry> = apps.iter().filter(|a| a.last_launched_at.is_some() && !used.contains(&a.id)).collect();
    recent.sort_by(|a, b| b.last_launched_at.cmp(&a.last_launched_at));
    for (i, a) in recent.iter().take(8).enumerate() {
        used.insert(a.id.clone());
        out.push(mk(a, "Recent", 5_000 - i as i32));
    }
    out.truncate(limit);
    out
}

pub fn web_search_url(template: &str, query: &str) -> Option<String> {
    let enc: String = query
        .bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![b as char],
            b' ' => vec!['+'],
            _ => format!("%{:02X}", b).chars().collect(),
        })
        .collect();
    let url = template.replace("{query}", &enc);
    crate::security::validate_web_url(&url).ok().map(|u| u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, count: i64, fav: bool) -> AppEntry {
        AppEntry {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            user_renamed: false,
            source: AppSource::StartMenuUser,
            target: ExecutableTarget::executable(format!("C:\\Apps\\{name}.exe")),
            resolved_path: None,
            identity_key: name.into(),
            provenance: String::new(),
            publisher: None,
            version: None,
            category: AppCategory::None,
            tags: vec![],
            favorite: fav,
            hidden: false,
            stale: false,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
            collection_ids: vec![],
            launch_count: count,
            last_launched_at: None,
        }
    }

    fn inputs<'a>(apps: &'a [AppEntry], wfs: &'a [WorkflowDefinition]) -> SearchInputs<'a> {
        SearchInputs { apps, workflows: wfs, files: &[], files_enabled: false, web_enabled: true, web_url_template: "https://duckduckgo.com/?q={query}", now: Utc::now() }
    }

    #[test]
    fn prefix_beats_scattered() {
        let a = fuzzy("code", "Code").unwrap().0;
        let b = fuzzy("code", "Xcode Helper").unwrap().0;
        let c = fuzzy("code", "Calculator Of Dev Everything").unwrap().0;
        assert!(a > b && b > c, "{a} {b} {c}");
    }

    #[test]
    fn word_boundary_acronym() {
        let (s, idx) = fuzzy("vsc", "Visual Studio Code").unwrap();
        assert_eq!(idx, vec![0, 7, 14]);
        assert!(s > 0);
        assert!(fuzzy("zzz", "Visual Studio Code").is_none());
    }

    #[test]
    fn match_indices_are_increasing() {
        let (_, idx) = fuzzy("pcut", "PulseCut").unwrap();
        assert!(idx.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn usage_weighting_and_exact_boost() {
        let apps = vec![app("Notes", 0, false), app("Notepad", 50, false), app("Note", 0, false)];
        let r = search(&inputs(&apps, &[]), "note", 10);
        assert_eq!(r[0].title, "Note", "exact match first");
        assert_eq!(r[1].title, "Notepad", "frequent before rare");
    }

    #[test]
    fn modes() {
        let apps = vec![app("Calc Tool", 0, false)];
        let r = search(&inputs(&apps, &[]), "2+2", 10);
        assert_eq!(r[0].category, Category::Calculator);
        assert_eq!(r[0].title, "= 4");
        let r = search(&inputs(&apps, &[]), "@calc", 10);
        assert_eq!(r[0].category, Category::App);
        assert!(r.iter().all(|x| x.category != Category::Web));
        let r = search(&inputs(&apps, &[]), "? hello world", 10);
        assert_eq!(r[0].category, Category::Web);
        assert!(matches!(&r[0].action, ResultAction::WebSearch { url } if url.ends_with("hello+world")));
    }

    #[test]
    fn web_disabled_never_returns_web() {
        let apps = vec![app("A", 0, false)];
        let mut i = inputs(&apps, &[]);
        i.web_enabled = false;
        assert!(search(&i, "zzzz", 10).is_empty());
    }

    #[test]
    fn web_url_encoding_blocks_injection() {
        let u = web_search_url("https://x.test/?q={query}", "a&b=c d/é").unwrap();
        assert_eq!(u, "https://x.test/?q=a%26b%3Dc+d%2F%C3%A9");
        assert!(web_search_url("javascript:{query}", "x").is_none());
    }

    #[test]
    fn empty_query_shows_pinned_then_recent() {
        let mut a = app("Pinned One", 0, true);
        a.last_launched_at = Some("2026-01-01T00:00:00Z".into());
        let mut b = app("Recent One", 3, false);
        b.last_launched_at = Some("2026-02-01T00:00:00Z".into());
        let c = app("Never Used", 0, false);
        let apps = vec![c, b, a];
        let r = search(&inputs(&apps, &[]), "", 10);
        assert_eq!(r.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), vec!["Pinned One", "Recent One"]);
        assert_eq!(r[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn hidden_apps_excluded() {
        let mut a = app("Secret", 0, false);
        a.hidden = true;
        assert!(search(&inputs(&[a], &[]), "secret", 10).iter().all(|r| r.category == Category::Web));
    }

    #[test]
    fn thousand_entries_under_budget() {
        let apps: Vec<AppEntry> = (0..1000).map(|i| app(&format!("Application Number {i} Studio"), i % 7, i % 97 == 0)).collect();
        let inp = inputs(&apps, &[]);
        let start = std::time::Instant::now();
        for q in ["app", "studio 9", "num", "xyz"] {
            let _ = search(&inp, q, 30);
        }
        let per = start.elapsed().as_millis() / 4;
        eprintln!("1000-entry search: ~{per} ms/query (debug build)");
        assert!(per < 50, "search took {per} ms per query");
    }

    #[test]
    fn stable_ordering_for_equal_scores() {
        let apps = vec![app("Alpha One", 0, false), app("Alpha Two", 0, false)];
        let r1 = search(&inputs(&apps, &[]), "alpha", 10);
        let r2 = search(&inputs(&apps, &[]), "alpha", 10);
        assert_eq!(r1, r2);
    }
}
