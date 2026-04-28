use std::sync::OnceLock;

pub(super) fn detect_default_shell() -> String {
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED.get_or_init(detect_default_shell_uncached).clone()
}

fn detect_default_shell_uncached() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Some(pwsh) = detect_latest_pwsh() {
            return pwsh;
        }
        if let Some(powershell) = where_first("powershell.exe") {
            return powershell;
        }
        return std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

#[cfg(target_os = "windows")]
fn detect_latest_pwsh() -> Option<String> {
    let mut candidates = Vec::new();

    candidates.extend(common_pwsh_paths());
    candidates.extend(where_all("pwsh.exe"));

    let candidates = dedupe_preserve_order(candidates);
    if candidates.is_empty() {
        return None;
    }

    let mut best: Option<(String, SemVer)> = None;
    let mut fallback: Option<String> = None;

    for path in candidates {
        if fallback.is_none() {
            fallback = Some(path.clone());
        }
        let Some(ver) = probe_pwsh_version(&path) else {
            continue;
        };
        match &best {
            None => best = Some((path, ver)),
            Some((_, best_ver)) => {
                if ver > *best_ver {
                    best = Some((path, ver));
                }
            }
        }
    }

    best.map(|(path, _)| path).or(fallback)
}

#[cfg(target_os = "windows")]
fn common_pwsh_paths() -> Vec<String> {
    use std::path::PathBuf;

    let mut out = Vec::new();

    let program_files = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or_else(|_| r"C:\Program Files".to_string());

    let program_files_x86 = std::env::var("ProgramFiles(x86)")
        .unwrap_or_else(|_| r"C:\Program Files (x86)".to_string());

    let candidates = [
        PathBuf::from(&program_files).join(r"PowerShell\7\pwsh.exe"),
        PathBuf::from(&program_files).join(r"PowerShell\7-preview\pwsh.exe"),
        PathBuf::from(&program_files).join(r"PowerShell\6\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\7\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\7-preview\pwsh.exe"),
        PathBuf::from(&program_files_x86).join(r"PowerShell\6\pwsh.exe"),
    ];

    for path in candidates {
        if path.exists() {
            out.push(path.to_string_lossy().to_string());
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn where_first(exe: &str) -> Option<String> {
    where_all(exe).into_iter().next()
}

#[cfg(target_os = "windows")]
fn where_all(exe: &str) -> Vec<String> {
    use std::process::Command;

    let output = match Command::new("where").arg(exe).output() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(target_os = "windows")]
fn probe_pwsh_version(pwsh_path: &str) -> Option<SemVer> {
    use std::process::{Command, Stdio};

    let script = "$v=$PSVersionTable.PSSemVer;if($null -ne $v){$v.ToString()}else{$PSVersionTable.PSVersion.ToString()}";

    let mut child = Command::new(pwsh_path)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => return None,
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(ver) = SemVer::parse(line.trim()) {
            return Some(ver);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for v in values {
        let key = v.to_lowercase();
        if seen.insert(key) {
            out.push(v);
        }
    }
    out
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<SemVerIdent>,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum SemVerIdent {
    Numeric(u64),
    Alpha(String),
}

#[cfg(any(target_os = "windows", test))]
impl SemVer {
    fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if s.is_empty() {
            return None;
        }

        let s = s.split('+').next().unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };

        let mut nums = core.split('.').map(|p| p.trim()).filter(|p| !p.is_empty());
        let major = nums.next()?.parse::<u64>().ok()?;
        let minor = nums.next().unwrap_or("0").parse::<u64>().ok()?;
        let patch = nums.next().unwrap_or("0").parse::<u64>().ok()?;

        let mut pre_idents = Vec::new();
        if let Some(pre) = pre {
            for part in pre.split('.').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                if part.chars().all(|c| c.is_ascii_digit()) {
                    pre_idents.push(SemVerIdent::Numeric(part.parse::<u64>().ok()?));
                } else {
                    pre_idents.push(SemVerIdent::Alpha(part.to_string()));
                }
            }
        }

        Some(Self {
            major,
            minor,
            patch,
            pre: pre_idents,
        })
    }
}

#[cfg(any(target_os = "windows", test))]
impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(any(target_os = "windows", test))]
impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            (false, false) => {}
        }

        for (a, b) in self.pre.iter().zip(other.pre.iter()) {
            match a.cmp(b) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }

        self.pre.len().cmp(&other.pre.len())
    }
}

#[cfg(any(target_os = "windows", test))]
impl PartialOrd for SemVerIdent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(any(target_os = "windows", test))]
impl Ord for SemVerIdent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (self, other) {
            (SemVerIdent::Numeric(a), SemVerIdent::Numeric(b)) => a.cmp(b),
            (SemVerIdent::Numeric(_), SemVerIdent::Alpha(_)) => Ordering::Less,
            (SemVerIdent::Alpha(_), SemVerIdent::Numeric(_)) => Ordering::Greater,
            (SemVerIdent::Alpha(a), SemVerIdent::Alpha(b)) => a.cmp(b),
        }
    }
}

#[cfg(test)]
mod semver_tests {
    use super::SemVer;

    #[test]
    fn semver_parse_and_ordering() {
        let stable = SemVer::parse("7.4.2").unwrap();
        let older = SemVer::parse("7.4.1").unwrap();
        let preview = SemVer::parse("7.5.0-preview.1").unwrap();
        let rc = SemVer::parse("7.5.0-rc.1").unwrap();

        assert!(stable > older);
        assert!(preview > stable);
        assert!(rc > preview);
        assert!(SemVer::parse("7.5.0").unwrap() > rc);
    }
}
