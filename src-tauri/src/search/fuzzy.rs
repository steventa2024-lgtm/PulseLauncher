//! Small fuzzy matcher with exact / prefix / word-boundary / subsequence
//! tiers. Returns a score and the matched character indices for highlighting.

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub score: i64,
    pub positions: Vec<usize>,
}

fn is_boundary(chars: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = chars[i - 1];
    let cur = chars[i];
    !prev.is_alphanumeric()
        || (prev.is_lowercase() && cur.is_uppercase())
        || (prev.is_alphabetic() && cur.is_numeric())
}

pub fn score(query: &str, text: &str) -> Option<Match> {
    let q: Vec<char> = query.trim().to_lowercase().chars().collect();
    if q.is_empty() {
        return Some(Match {
            score: 0,
            positions: vec![],
        });
    }
    let orig: Vec<char> = text.chars().collect();
    let t: Vec<char> = orig.iter().flat_map(|c| c.to_lowercase().next()).collect();
    if q.len() > t.len() {
        return None;
    }
    let qs: String = q.iter().collect();
    let ts: String = t.iter().collect();

    if ts == qs {
        // Exact matches stay ahead of a heavily used prefix of a longer name.
        return Some(Match {
            score: 2000,
            positions: (0..t.len()).collect(),
        });
    }
    if ts.starts_with(&qs) {
        return Some(Match {
            score: 900 - (t.len() as i64 - q.len() as i64).min(100),
            positions: (0..q.len()).collect(),
        });
    }
    // Substring starting at a word boundary.
    let mut best_sub: Option<(usize, bool)> = None;
    for start in 0..=t.len() - q.len() {
        if t[start..start + q.len()] == q[..] {
            let b = is_boundary(&orig, start);
            if b {
                best_sub = Some((start, true));
                break;
            }
            if best_sub.is_none() {
                best_sub = Some((start, false));
            }
        }
    }
    if let Some((start, boundary)) = best_sub {
        let base = if boundary { 750 } else { 550 };
        return Some(Match {
            score: base - (start as i64).min(100),
            positions: (start..start + q.len()).collect(),
        });
    }
    // Initials / acronym: every query char matches a word start in order.
    let starts: Vec<usize> = (0..t.len())
        .filter(|&i| is_boundary(&orig, i) && t[i].is_alphanumeric())
        .collect();
    {
        let mut qi = 0;
        let mut pos = Vec::new();
        for &s in &starts {
            if qi < q.len() && t[s] == q[qi] {
                pos.push(s);
                qi += 1;
            }
        }
        if qi == q.len() {
            return Some(Match {
                score: 650 - (starts.len() as i64 - q.len() as i64).min(50),
                positions: pos,
            });
        }
    }
    // Generic subsequence with bonuses for boundaries and consecutive runs.
    let mut positions = Vec::with_capacity(q.len());
    let mut score: i64 = 300;
    let mut ti = 0;
    let mut last: Option<usize> = None;
    for &qc in &q {
        let mut found = None;
        while ti < t.len() {
            if t[ti] == qc {
                found = Some(ti);
                ti += 1;
                break;
            }
            ti += 1;
        }
        let i = found?;
        if is_boundary(&orig, i) {
            score += 10;
        }
        if let Some(l) = last {
            if i == l + 1 {
                score += 8;
            } else {
                score -= ((i - l) as i64).min(15);
            }
        } else {
            score -= (i as i64).min(30);
        }
        positions.push(i);
        last = Some(i);
    }
    Some(Match {
        score: score.clamp(1, 499),
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_are_ordered() {
        let exact = score("code", "Code").unwrap().score;
        let prefix = score("cod", "Code").unwrap().score;
        let word = score("code", "Visual Studio Code").unwrap().score;
        let acronym = score("vsc", "Visual Studio Code").unwrap().score;
        let inner = score("ode", "Code").unwrap().score;
        let subseq = score("vdo", "Visual Studio Code").unwrap().score;
        assert!(
            exact > prefix && prefix > word && word > acronym && acronym > inner && inner > subseq
        );
    }

    #[test]
    fn positions_for_highlighting() {
        assert_eq!(
            score("vsc", "Visual Studio Code").unwrap().positions,
            vec![0, 7, 14]
        );
        assert_eq!(
            score("stu", "Visual Studio").unwrap().positions,
            vec![7, 8, 9]
        );
        assert!(score("xyz", "Visual Studio").is_none());
        assert!(score("toolong", "abc").is_none());
    }

    #[test]
    fn unicode_and_case() {
        assert!(score("über", "Über Tool").is_some());
        assert!(score("PULSE", "pulsecut").is_some());
        assert!(score("pc", "PulseCut").unwrap().score >= 600);
    }
}
