use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct ExecPolicy {
    rules: Vec<CompiledRule>,
}

impl ExecPolicy {
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn load_from_str(raw: &str) -> Result<Self, String> {
        let file: ExecPolicyFile =
            toml::from_str(raw).map_err(|e| format!("parse execpolicy: {e}"))?;
        let mut compiled = Vec::new();
        for rule in file.rules {
            if let Some(rule) = CompiledRule::from_rule(rule)? {
                compiled.push(rule);
            }
        }
        Ok(Self { rules: compiled })
    }

    pub fn is_allowed(&self, argv: &[String]) -> bool {
        for rule in &self.rules {
            if rule.matches(argv) {
                return matches!(rule.effect, ExecPolicyEffect::Allow);
            }
        }
        false
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct ExecPolicyFile {
    version: u32,
    #[serde(rename = "rule")]
    rules: Vec<ExecPolicyRule>,
}

impl Default for ExecPolicyFile {
    fn default() -> Self {
        Self {
            version: 1,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct ExecPolicyRule {
    id: Option<String>,
    effect: ExecPolicyEffect,
    argv_exact: Option<Vec<String>>,
    argv_glob: Option<Vec<String>>,
    argv_shorthand: Option<String>,
}

impl Default for ExecPolicyRule {
    fn default() -> Self {
        Self {
            id: None,
            effect: ExecPolicyEffect::Allow,
            argv_exact: None,
            argv_glob: None,
            argv_shorthand: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ExecPolicyEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone)]
enum RuleMatcher {
    Exact(Vec<String>),
    Glob(Vec<String>),
}

#[derive(Debug, Clone)]
struct CompiledRule {
    #[allow(dead_code)]
    id: Option<String>,
    effect: ExecPolicyEffect,
    matcher: RuleMatcher,
}

impl CompiledRule {
    fn from_rule(rule: ExecPolicyRule) -> Result<Option<Self>, String> {
        let matcher = if let Some(tokens) = rule.argv_exact.clone() {
            RuleMatcher::Exact(tokens)
        } else if let Some(tokens) = rule.argv_glob.clone() {
            RuleMatcher::Glob(tokens)
        } else if let Some(shorthand) = rule.argv_shorthand.as_ref() {
            RuleMatcher::Glob(parse_shorthand(shorthand)?)
        } else {
            return Ok(None);
        };

        Ok(Some(Self {
            id: rule.id,
            effect: rule.effect,
            matcher,
        }))
    }

    fn matches(&self, argv: &[String]) -> bool {
        match &self.matcher {
            RuleMatcher::Exact(tokens) => match_exact(tokens, argv),
            RuleMatcher::Glob(tokens) => match_glob(tokens, argv),
        }
    }
}

fn parse_shorthand(raw: &str) -> Result<Vec<String>, String> {
    let mut tokens =
        shell_words::split(raw).map_err(|e| format!("parse argv_shorthand: {e}"))?;
    if tokens.is_empty() {
        return Err("argv_shorthand is empty".to_string());
    }

    if let Some(last) = tokens.last_mut() {
        if last.ends_with(":*") {
            let base = last.trim_end_matches(":*");
            if base.is_empty() {
                tokens.pop();
            } else {
                *last = base.to_string();
            }
            tokens.push("*".to_string());
        }
    }

    Ok(tokens)
}

fn match_exact(pattern: &[String], argv: &[String]) -> bool {
    if pattern.len() != argv.len() {
        return false;
    }
    for (p, a) in pattern.iter().zip(argv.iter()) {
        if !token_eq(p, a) {
            return false;
        }
    }
    true
}

fn match_glob(pattern: &[String], argv: &[String]) -> bool {
    let mut idx = 0usize;
    let mut pidx = 0usize;
    while pidx < pattern.len() {
        let token = &pattern[pidx];
        if token == "**" {
            return true;
        }
        let Some(arg) = argv.get(idx) else {
            return false;
        };
        if !match_token(token, arg) {
            return false;
        }
        idx += 1;
        pidx += 1;
    }
    idx == argv.len()
}

fn token_eq(pattern: &str, value: &str) -> bool {
    let (p, v) = normalize_pair(pattern, value);
    p == v
}

fn match_token(pattern: &str, value: &str) -> bool {
    let (p, v) = normalize_pair(pattern, value);
    wildcard_match(&p, &v)
}

fn normalize_pair<'a>(pattern: &'a str, value: &'a str) -> (String, String) {
    if cfg!(target_os = "windows") {
        return (pattern.to_lowercase(), value.to_lowercase());
    }
    (pattern.to_string(), value.to_string())
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star: Option<usize> = None;
    let mut match_idx = 0usize;

    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            match_idx = ti;
            pi += 1;
        } else if let Some(star_idx) = star {
            pi = star_idx + 1;
            match_idx += 1;
            ti = match_idx;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }

    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_match_requires_same_length() {
        let raw = r#"
version = 1

[[rule]]
effect = "allow"
argv_exact = ["git", "status"]
"#;
        let policy = ExecPolicy::load_from_str(raw).expect("parse");
        assert!(policy.is_allowed(&argv(&["git", "status"])));
        assert!(!policy.is_allowed(&argv(&["git", "status", "-s"])));
    }

    #[test]
    fn glob_match_accepts_trailing_tokens() {
        let raw = r#"
version = 1

[[rule]]
effect = "allow"
argv_glob = ["gh", "*"]
"#;
        let policy = ExecPolicy::load_from_str(raw).expect("parse");
        assert!(policy.is_allowed(&argv(&["gh", "status"])));
        assert!(!policy.is_allowed(&argv(&["git", "status"])));
    }

    #[test]
    fn glob_double_star_matches_remaining() {
        let raw = r#"
version = 1

[[rule]]
effect = "allow"
argv_glob = ["npm", "**"]
"#;
        let policy = ExecPolicy::load_from_str(raw).expect("parse");
        assert!(policy.is_allowed(&argv(&["npm"])));
        assert!(policy.is_allowed(&argv(&["npm", "test", "--", "x"])));
    }

    #[test]
    fn shorthand_parses_colon_star() {
        let raw = r#"
version = 1

[[rule]]
effect = "allow"
argv_shorthand = "git add:*"
"#;
        let policy = ExecPolicy::load_from_str(raw).expect("parse");
        assert!(policy.is_allowed(&argv(&["git", "add", "file.txt"])));
        assert!(!policy.is_allowed(&argv(&["git", "status"])));
    }
}
