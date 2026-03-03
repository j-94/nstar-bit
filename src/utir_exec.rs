use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::receipt::Effect;
use crate::utir::{Operation, Policy, UtirDocument};

#[derive(Debug, Clone)]
pub struct GuardConfig {
    pub allowed_domains: Vec<String>,
    pub allowed_commands: Vec<String>,
    pub blocked_patterns: Vec<String>,
    pub max_exec_ms: u64,
    pub max_file_bytes: u64,
    pub max_response_bytes: u64,
    pub sandbox_root: Option<PathBuf>,
    pub allow_all_commands: bool,
}

impl GuardConfig {
    pub fn from_env() -> Self {
        let allowed_domains = std::env::var("GRAPH_ALLOWED_DOMAINS")
            .unwrap_or_else(|_| "localhost,127.0.0.1".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        let allowed_commands = std::env::var("GRAPH_ALLOWED_COMMANDS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(default_allowed_commands);

        let blocked_patterns = std::env::var("GRAPH_BLOCKED_PATTERNS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(default_blocked_patterns);

        let max_exec_ms = std::env::var("GRAPH_MAX_EXEC_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300_000);

        let max_file_bytes = std::env::var("GRAPH_MAX_FILE_BYTES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(100 * 1024 * 1024);

        let max_response_bytes = std::env::var("GRAPH_MAX_RESPONSE_BYTES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10 * 1024 * 1024);

        let sandbox_root = std::env::var("GRAPH_SANDBOX_ROOT")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        let sandbox_root = sandbox_root.map(|p| normalize_path_lexical(&p));

        let allow_all_commands = std::env::var("GRAPH_ALLOW_ALL_COMMANDS")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            allowed_domains,
            allowed_commands,
            blocked_patterns,
            max_exec_ms,
            max_file_bytes,
            max_response_bytes,
            sandbox_root,
            allow_all_commands,
        }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            return normalize_path_lexical(&p);
        }
        if let Some(root) = &self.sandbox_root {
            return normalize_path_lexical(&root.join(p));
        }
        normalize_path_lexical(&p)
    }

    fn is_within_sandbox(&self, path: &Path) -> bool {
        if let Some(root) = &self.sandbox_root {
            let p = normalize_path_lexical(path);
            p.starts_with(root)
        } else {
            true
        }
    }

    fn is_command_safe(&self, command: &str) -> bool {
        for pattern in &self.blocked_patterns {
            if command.contains(pattern) {
                return false;
            }
        }

        if self.allow_all_commands {
            return true;
        }

        let first_word = command.split_whitespace().next().unwrap_or("");
        self.allowed_commands.iter().any(|cmd| cmd == first_word)
    }

    fn is_url_allowed(&self, url: &str) -> bool {
        match reqwest::Url::parse(url) {
            Ok(parsed) => parsed.host_str().map(|host| {
                self.allowed_domains.iter().any(|domain| host.ends_with(domain))
            }).unwrap_or(false),
            Err(_) => false,
        }
    }
}

#[derive(Clone)]
struct ExecContext {
    guard: GuardConfig,
    policy: Option<Policy>,
}

pub fn execute_utir(doc: &UtirDocument, guard: &GuardConfig) -> Vec<Effect> {
    let ctx = ExecContext {
        guard: guard.clone(),
        policy: doc.policy.clone(),
    };

    let outcome = execute_operation_list(&doc.operations, &ctx);
    outcome.effects
}

struct Outcome {
    effects: Vec<Effect>,
    success: bool,
}

fn execute_operation_list(ops: &[Operation], ctx: &ExecContext) -> Outcome {
    let mut effects = Vec::new();
    let mut success = true;

    for op in ops {
        let outcome = execute_operation(op, ctx);
        success = success && outcome.success;
        effects.extend(outcome.effects);
        if !outcome.success {
            break;
        }
    }

    Outcome { effects, success }
}

fn execute_operation(op: &Operation, ctx: &ExecContext) -> Outcome {
    if let Some(max_risk) = ctx.policy.as_ref().map(|p| p.max_risk) {
        let risk = risk_score(op);
        if risk > max_risk {
            return Outcome {
                effects: vec![Effect::Blocked {
                    op: op_label(op),
                    reason: format!("risk {:.2} exceeds max_risk {:.2}", risk, max_risk),
                }],
                success: false,
            };
        }
    }

    match op {
        Operation::Shell { command, timeout, working_dir, env, allow_network: _, capture_output } => {
            if !ctx.guard.is_command_safe(command) {
                return Outcome {
                    effects: vec![Effect::Blocked {
                        op: format!("shell:{}", command),
                        reason: "command blocked by guardrails".to_string(),
                    }],
                    success: false,
                };
            }
            let timeout = parse_duration(timeout, ctx.guard.max_exec_ms);
            let work_dir = working_dir.as_ref().map(|d| ctx.guard.resolve_path(d));
            if let Some(wd) = work_dir.as_ref() {
                if !ctx.guard.is_within_sandbox(wd) {
                    return Outcome {
                        effects: vec![Effect::Blocked {
                            op: format!("shell:{} (cwd={})", command, wd.display()),
                            reason: "working_dir outside sandbox".to_string(),
                        }],
                        success: false,
                    };
                }
            }
            let (ok, stdout, stderr, status) = run_shell(command, timeout, work_dir.as_ref().map(|p| p.as_path()), env, *capture_output);
            let effect = Effect::Exec {
                cmd: command.to_string(),
                ok,
                status,
                stdout_sha256: stdout.as_ref().map(|s| crate::receipt::sha256_hex_str(s)),
                stderr_sha256: stderr.as_ref().map(|s| crate::receipt::sha256_hex_str(s)),
                error: if ok { None } else { Some(stderr.unwrap_or_else(|| "shell failed".to_string())) },
            };
            Outcome { effects: vec![effect], success: ok }
        }
        Operation::FsRead { path, max_size, .. } => {
            let full_path = ctx.guard.resolve_path(path);
            if !ctx.guard.is_within_sandbox(&full_path) {
                return Outcome {
                    effects: vec![Effect::Blocked { op: format!("fs.read:{}", path), reason: "path outside sandbox".to_string() }],
                    success: false,
                };
            }
            let max_bytes = parse_size(max_size, ctx.guard.max_file_bytes);
            match std::fs::read(&full_path) {
                Ok(bytes) => {
                    if bytes.len() as u64 > max_bytes {
                        return Outcome { effects: vec![Effect::ReadFile { path: path.to_string(), bytes: bytes.len(), sha256: crate::receipt::sha256_hex_bytes(&bytes), ok: false, content: None, error: Some("file too large".to_string()) }], success: false };
                    }
                    let content = String::from_utf8(bytes.clone()).ok().map(|mut s| {
                        if s.len() > 8000 {
                            s.truncate(8000);
                            s.push_str("\n...[TRUNCATED FOR LENGTH]");
                        }
                        s
                    });
                    Outcome {
                        effects: vec![Effect::ReadFile { path: path.to_string(), bytes: bytes.len(), sha256: crate::receipt::sha256_hex_bytes(&bytes), ok: true, content, error: None }],
                        success: true,
                    }
                }
                Err(e) => Outcome {
                    effects: vec![Effect::ReadFile { path: path.to_string(), bytes: 0, sha256: crate::receipt::sha256_hex_str(""), ok: false, content: None, error: Some(e.to_string()) }],
                    success: false,
                },
            }
        }
        Operation::FsWrite { path, content, create_dirs, .. } => {
            let full_path = ctx.guard.resolve_path(path);
            if !ctx.guard.is_within_sandbox(&full_path) {
                return Outcome {
                    effects: vec![Effect::Blocked { op: format!("fs.write:{}", path), reason: "path outside sandbox".to_string() }],
                    success: false,
                };
            }
            if content.len() as u64 > ctx.guard.max_file_bytes {
                return Outcome { effects: vec![Effect::WriteFile { path: path.to_string(), bytes: content.len(), sha256: crate::receipt::sha256_hex_str(content), ok: false, error: Some("content too large".to_string()) }], success: false };
            }
            if *create_dirs {
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            match std::fs::write(&full_path, content) {
                Ok(_) => Outcome { effects: vec![Effect::WriteFile { path: path.to_string(), bytes: content.len(), sha256: crate::receipt::sha256_hex_str(content), ok: true, error: None }], success: true },
                Err(e) => Outcome { effects: vec![Effect::WriteFile { path: path.to_string(), bytes: content.len(), sha256: crate::receipt::sha256_hex_str(content), ok: false, error: Some(e.to_string()) }], success: false },
            }
        }
        Operation::HttpGet { url, headers, timeout, max_response_size } => {
            if !ctx.guard.is_url_allowed(url) {
                return Outcome { effects: vec![Effect::Blocked { op: format!("http.get:{}", url), reason: "domain not allowed".to_string() }], success: false };
            }
            let timeout = parse_duration(timeout, ctx.guard.max_exec_ms);
            let client = match reqwest::blocking::Client::builder().timeout(timeout).build() {
                Ok(c) => c,
                Err(e) => return Outcome { effects: vec![Effect::HttpGet { url: url.to_string(), status: None, bytes: 0, sha256: None, ok: false, error: Some(e.to_string()) }], success: false },
            };
            let mut req = client.get(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    match resp.bytes() {
                        Ok(bytes) => {
                            let max_bytes = parse_size(max_response_size, ctx.guard.max_response_bytes);
                            if bytes.len() as u64 > max_bytes {
                                return Outcome { effects: vec![Effect::HttpGet { url: url.to_string(), status: Some(status), bytes: bytes.len(), sha256: None, ok: false, error: Some("response too large".to_string()) }], success: false };
                            }
                            let ok = status >= 200 && status < 300;
                            Outcome { effects: vec![Effect::HttpGet { url: url.to_string(), status: Some(status), bytes: bytes.len(), sha256: Some(crate::receipt::sha256_hex_bytes(&bytes)), ok, error: if ok { None } else { Some(format!("http status {}", status)) } }], success: ok }
                        }
                        Err(e) => Outcome { effects: vec![Effect::HttpGet { url: url.to_string(), status: Some(status), bytes: 0, sha256: None, ok: false, error: Some(e.to_string()) }], success: false },
                    }
                }
                Err(e) => Outcome { effects: vec![Effect::HttpGet { url: url.to_string(), status: None, bytes: 0, sha256: None, ok: false, error: Some(e.to_string()) }], success: false },
            }
        }
        Operation::GitPatch { repo_path, patch_content, commit_message, author } => {
            let full_path = ctx.guard.resolve_path(repo_path);
            if !ctx.guard.is_within_sandbox(&full_path) {
                return Outcome { effects: vec![Effect::Blocked { op: format!("git.patch:{}", repo_path), reason: "path outside sandbox".to_string() }], success: false };
            }
            let patch_file = full_path.join(".graph_patch.tmp");
            if let Err(e) = std::fs::write(&patch_file, patch_content) {
                return Outcome { effects: vec![Effect::GitPatch { repo_path: repo_path.to_string(), ok: false, error: Some(e.to_string()) }], success: false };
            }
            let check = Command::new("git")
                .args(["apply", "--check", patch_file.to_string_lossy().as_ref()])
                .current_dir(&full_path)
                .output();
            if let Ok(out) = check {
                if !out.status.success() {
                    let _ = std::fs::remove_file(&patch_file);
                    return Outcome { effects: vec![Effect::GitPatch { repo_path: repo_path.to_string(), ok: false, error: Some(String::from_utf8_lossy(&out.stderr).to_string()) }], success: false };
                }
            }
            let apply = Command::new("git")
                .args(["apply", patch_file.to_string_lossy().as_ref()])
                .current_dir(&full_path)
                .output();
            if let Ok(out) = apply {
                if !out.status.success() {
                    let _ = std::fs::remove_file(&patch_file);
                    return Outcome { effects: vec![Effect::GitPatch { repo_path: repo_path.to_string(), ok: false, error: Some(String::from_utf8_lossy(&out.stderr).to_string()) }], success: false };
                }
            }
            let commit = Command::new("git")
                .args(["commit", "-a", "-m", commit_message, "--author", author])
                .current_dir(&full_path)
                .output();
            let _ = std::fs::remove_file(&patch_file);
            match commit {
                Ok(out) => {
                    let ok = out.status.success();
                    Outcome { effects: vec![Effect::GitPatch { repo_path: repo_path.to_string(), ok, error: if ok { None } else { Some(String::from_utf8_lossy(&out.stderr).to_string()) } }], success: ok }
                }
                Err(e) => Outcome { effects: vec![Effect::GitPatch { repo_path: repo_path.to_string(), ok: false, error: Some(e.to_string()) }], success: false },
            }
        }
        Operation::AssertFileExists { path } => {
            let full_path = ctx.guard.resolve_path(path);
            let exists = full_path.exists();
            Outcome { effects: vec![Effect::Assert { assert_kind: "file_exists".to_string(), ok: exists, message: if exists { format!("exists:{}", path) } else { format!("missing:{}", path) } }], success: exists }
        }
        Operation::AssertShellSuccess { command, timeout, expected_output } => {
            if !ctx.guard.is_command_safe(command) {
                return Outcome { effects: vec![Effect::Blocked { op: format!("assert.shell:{}", command), reason: "command blocked by guardrails".to_string() }], success: false };
            }
            let timeout = parse_duration(timeout, ctx.guard.max_exec_ms);
            let (ok, stdout, stderr, status) = run_shell(command, timeout, None, &HashMap::new(), true);
            if !ok {
                return Outcome { effects: vec![Effect::Assert { assert_kind: "shell_success".to_string(), ok: false, message: stderr.unwrap_or_else(|| "shell failed".to_string()) }], success: false };
            }
            if let Some(expected) = expected_output {
                if let Some(out) = stdout {
                    if out.contains(expected) {
                        return Outcome { effects: vec![Effect::Assert { assert_kind: "shell_success".to_string(), ok: true, message: "expected output matched".to_string() }], success: true };
                    }
                    return Outcome { effects: vec![Effect::Assert { assert_kind: "shell_success".to_string(), ok: false, message: format!("expected '{}' not found", expected) }], success: false };
                }
            }
            Outcome { effects: vec![Effect::Assert { assert_kind: "shell_success".to_string(), ok: ok, message: format!("status={:?}", status) }], success: ok }
        }
        Operation::Attempt { operation } => {
            let out = execute_operation(operation, ctx);
            Outcome {
                effects: out.effects,
                success: true,
            }
        }
        Operation::Sequence { steps } => execute_operation_list(steps, ctx),
        Operation::Parallel { steps, .. } => execute_parallel(steps, ctx),
        Operation::Conditional { condition, then_op, else_op } => {
            let cond = execute_operation(condition, ctx);
            if cond.success {
                execute_operation(then_op, ctx)
            } else if let Some(else_branch) = else_op.as_deref() {
                execute_operation(else_branch, ctx)
            } else {
                Outcome { effects: cond.effects, success: true }
            }
        }
        Operation::Retry { operation, max_attempts, backoff } => {
            let mut effects = Vec::new();
            let delay = parse_duration(backoff, ctx.guard.max_exec_ms);
            for attempt in 1..=*max_attempts {
                let out = execute_operation(operation, ctx);
                effects.extend(out.effects);
                if out.success {
                    return Outcome { effects, success: true };
                }
                if attempt < *max_attempts {
                    thread::sleep(delay);
                }
            }
            Outcome { effects, success: false }
        }
    }
}

fn execute_parallel(steps: &[Operation], ctx: &ExecContext) -> Outcome {
    let (tx, rx) = mpsc::channel::<Outcome>();
    for op in steps {
        let tx = tx.clone();
        let op = op.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let out = execute_operation(&op, &ctx);
            let _ = tx.send(out);
        });
    }
    drop(tx);
    let mut effects = Vec::new();
    let mut success = true;
    for out in rx {
        success = success && out.success;
        effects.extend(out.effects);
    }
    Outcome { effects, success }
}

fn run_shell(
    command: &str,
    timeout: Duration,
    work_dir: Option<&Path>,
    env: &HashMap<String, String>,
    capture_output: bool,
) -> (bool, Option<String>, Option<String>, Option<i32>) {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    if let Some(dir) = work_dir {
        cmd.current_dir(dir);
    }
    if capture_output {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    cmd.envs(env);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (false, None, Some(e.to_string()), None),
    };

    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return (false, None, Some("command timed out".to_string()), None);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = if let Some(mut out) = child.stdout.take() {
                    let mut buf = Vec::new();
                    let _ = out.read_to_end(&mut buf);
                    if buf.is_empty() { None } else { Some(String::from_utf8_lossy(&buf).to_string()) }
                } else {
                    None
                };
                let stderr = if let Some(mut err) = child.stderr.take() {
                    let mut buf = Vec::new();
                    let _ = err.read_to_end(&mut buf);
                    if buf.is_empty() { None } else { Some(String::from_utf8_lossy(&buf).to_string()) }
                } else {
                    None
                };
                return (status.success(), stdout, stderr, status.code());
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => return (false, None, Some(e.to_string()), None),
        }
    }
}

fn parse_duration(input: &str, max_ms: u64) -> Duration {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Duration::from_millis(max_ms);
    }
    if trimmed.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        let value = trimmed.parse::<u64>().unwrap_or(max_ms);
        return Duration::from_millis(value.min(max_ms));
    }
    let (num, unit) = trimmed.split_at(trimmed.len().saturating_sub(1));
    let value = num.parse::<u64>().unwrap_or(0);
    let millis = match unit {
        "s" | "S" => value * 1000,
        "m" | "M" => value * 60 * 1000,
        "h" | "H" => value * 60 * 60 * 1000,
        _ => max_ms,
    };
    Duration::from_millis(millis.min(max_ms))
}

fn parse_size(input: &str, max_bytes: u64) -> u64 {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return max_bytes;
    }
    let mut digits = String::new();
    let mut unit = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            unit.push(c);
        }
    }
    let value = digits.parse::<u64>().unwrap_or(0);
    let unit_upper = unit.trim().to_uppercase();
    let bytes = match unit_upper.as_str() {
        "KB" => value * 1024,
        "MB" => value * 1024 * 1024,
        "GB" => value * 1024 * 1024 * 1024,
        "B" | "" => value,
        _ => value,
    };
    bytes.min(max_bytes)
}

fn normalize_path_lexical(path: &Path) -> PathBuf {
    // Lexical normalization: resolves `.` and `..` without filesystem access.
    // This is required for sandbox checks on paths that may not yet exist.
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                // Don’t pop past a root/prefix component.
                if out.components().next().is_some() {
                    out.pop();
                }
            }
            Component::Normal(part) => out.push(part),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
        }
    }
    out
}

fn default_allowed_commands() -> Vec<String> {
    vec![
        "echo", "ls", "cat", "mkdir", "rm", "cp", "mv", "git", "curl", "python3", "node", "npm", "cargo", "test", "grep", "find", "ast-grep", "sg",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_blocked_patterns() -> Vec<String> {
    vec![
        "rm -rf /",
        "sudo",
        "chmod 777",
        "> /dev/",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

fn risk_score(op: &Operation) -> f64 {
    match op {
        Operation::Shell { .. } => 0.7,
        Operation::FsRead { .. } => 0.2,
        Operation::FsWrite { .. } => 0.6,
        Operation::HttpGet { .. } => 0.5,
        Operation::GitPatch { .. } => 0.8,
        Operation::AssertFileExists { .. } => 0.1,
        Operation::AssertShellSuccess { .. } => 0.3,
        Operation::Attempt { operation } => risk_score(operation),
        Operation::Sequence { steps } => steps.iter().map(risk_score).fold(0.0, f64::max),
        Operation::Parallel { steps, .. } => steps.iter().map(risk_score).fold(0.0, f64::max),
        Operation::Conditional { condition, then_op, else_op } => {
            let mut max = risk_score(condition);
            max = max.max(risk_score(then_op));
            if let Some(else_branch) = else_op {
                max = max.max(risk_score(else_branch));
            }
            max
        }
        Operation::Retry { operation, .. } => risk_score(operation),
    }
}

fn op_label(op: &Operation) -> String {
    match op {
        Operation::Shell { command, .. } => format!("shell:{}", command),
        Operation::FsRead { path, .. } => format!("fs.read:{}", path),
        Operation::FsWrite { path, .. } => format!("fs.write:{}", path),
        Operation::HttpGet { url, .. } => format!("http.get:{}", url),
        Operation::GitPatch { repo_path, .. } => format!("git.patch:{}", repo_path),
        Operation::AssertFileExists { path } => format!("assert.file_exists:{}", path),
        Operation::AssertShellSuccess { command, .. } => format!("assert.shell_success:{}", command),
        Operation::Attempt { .. } => "attempt".to_string(),
        Operation::Sequence { .. } => "sequence".to_string(),
        Operation::Parallel { .. } => "parallel".to_string(),
        Operation::Conditional { .. } => "conditional".to_string(),
        Operation::Retry { .. } => "retry".to_string(),
    }
}
