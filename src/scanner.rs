use crate::enforcer::FileEnforcer;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs;
use tokio::sync::RwLock;

/// Shared, live-updated read path whitelist.
/// Passed to `ReadFileTool` so `definition()` always reflects the latest scan.
pub type SharedReadPaths = Arc<RwLock<HashSet<String>>>;

/// Shared, live-updated execute path whitelist.
/// Passed to `ExecuteScriptTool` so `definition()` always reflects the latest scan.
pub type SharedExecutePaths = Arc<RwLock<HashSet<String>>>;

// ==========================================
// Data structures
// ==========================================

/// Required fields extracted from SKILL.md frontmatter.
#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: Option<String>,
}

/// Complete metadata of a skill, ready for prompt injection.
#[derive(Debug, Clone)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    /// Absolute path to the SKILL.md file, per the Agent Skills spec.
    pub location: String,
}

// ==========================================
// Discovery & parsing
// ==========================================

/// Directories that are never skills and should never be recursed into.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".cache",
    "__pycache__",
    "dist",
    "build",
];

/// Maximum number of directories to visit before stopping, per spec recommendation.
const MAX_DIRS: usize = 2000;

/// Scans `base_dir` **recursively** (up to `max_depth` levels) for skill
/// subdirectories, each containing a file named exactly `SKILL.md`.
///
/// Follows the Agent Skills specification (lenient validation mode):
/// - Name validation issues are warned but load continues.
/// - Missing/empty description causes the skill to be skipped (required field).
/// - Duplicate names: first-found wins (log a warning on collision).
/// - `location` is always an absolute path.
///
/// The spec recommends `max_depth` of 4–6; pass `None` to use the default of 4.
pub async fn scan_skills(
    base_dir: &str,
    max_depth: Option<usize>,
) -> anyhow::Result<Vec<SkillMeta>> {
    let depth = max_depth.unwrap_or(4);

    // Resolve to an absolute path so `location` fields are always absolute.
    let base_path = Path::new(base_dir)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(base_dir));

    if !base_path.exists() {
        warn!(
            "Skills directory '{}' not found — returning empty skills list",
            base_path.display()
        );
        return Ok(Vec::new());
    }

    info!(
        "Scanning for skills in '{}' (max depth: {})",
        base_path.display(),
        depth
    );

    // Use a HashMap keyed by name for O(1) collision detection.
    let mut found: HashMap<String, SkillMeta> = HashMap::new();
    // Atomic counter to enforce the max-directory safety bound.
    let dir_count = AtomicUsize::new(0);
    scan_dir_recursive(&base_path, depth, &mut found, &dir_count).await;

    let mut skills: Vec<SkillMeta> = found.into_values().collect();
    // Sort by name for deterministic output.
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    info!(
        "Scan complete: found {} skill(s) (visited {} directories)",
        skills.len(),
        dir_count.load(Ordering::Relaxed)
    );
    Ok(skills)
}

/// Recursive helper — walks `dir`, collecting skills into `found`.
///
/// This is a Box-pinned async fn because async recursion requires
/// an explicit heap allocation to avoid infinite-sized futures on the stack.
fn scan_dir_recursive<'a>(
    dir: &'a Path,
    depth_remaining: usize,
    found: &'a mut HashMap<String, SkillMeta>,
    dir_count: &'a AtomicUsize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        if depth_remaining == 0 {
            return;
        }

        // Enforce the max-directory safety bound.
        if dir_count.fetch_add(1, Ordering::Relaxed) >= MAX_DIRS {
            warn!(
                "Reached directory limit ({}). Stopping scan early.",
                MAX_DIRS
            );
            return;
        }

        let mut entries = match fs::read_dir(dir).await {
            Ok(e) => e,
            Err(e) => {
                warn!("Could not read directory '{}': {}", dir.display(), e);
                return;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_owned(),
                None => continue,
            };

            // Skip non-skill directories.
            if SKIP_DIRS.contains(&dir_name.as_str()) {
                continue;
            }

            // Per reference lib: prefer SKILL.md, fallback to skill.md.
            let skill_md_path = find_skill_md(&path);

            if let Some(ref skill_path) = skill_md_path {
                // This directory is a skill — try to load it.
                if let Some(meta) = try_load_skill(skill_path, &dir_name).await {
                    // Duplicate check: first-found wins (project-scope convention).
                    if let Some(existing) = found.get(&meta.name) {
                        warn!(
                            "Name collision: '{}' already loaded from '{}'. Ignoring duplicate at '{}'.",
                            meta.name, existing.location, meta.location
                        );
                    } else {
                        debug!("Loaded skill '{}' from '{}'", meta.name, meta.location);
                        found.insert(meta.name.clone(), meta);
                    }
                }
                // A skill dir can still contain nested skills — keep recursing.
            }

            // Always recurse into subdirectories (they may contain nested skills).
            scan_dir_recursive(&path, depth_remaining - 1, found, dir_count).await;
        }
    })
}

/// Finds the SKILL.md file in a directory.
/// Prefers `SKILL.md` (uppercase) but accepts `skill.md` (lowercase) as fallback,
/// matching the official reference library behaviour.
fn find_skill_md(dir: &Path) -> Option<PathBuf> {
    for name in &["SKILL.md", "skill.md"] {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Attempts to load and validate a single `SKILL.md` file.
/// Returns `None` if the skill should be skipped.
async fn try_load_skill(skill_md_path: &Path, dir_name: &str) -> Option<SkillMeta> {
    let content = match fs::read_to_string(skill_md_path).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read {:?}: {} — skipping skill", skill_md_path, e);
            return None;
        }
    };

    let yaml_str = match extract_frontmatter(&content) {
        Some(y) => y,
        None => {
            error!(
                "No valid frontmatter in {:?} — skipping skill",
                skill_md_path
            );
            return None;
        }
    };

    let frontmatter = match serde_yaml::from_str::<SkillFrontmatter>(&yaml_str) {
        Ok(fm) => fm,
        Err(e) => {
            error!(
                "YAML parse error in {:?}: {} — skipping skill",
                skill_md_path, e
            );
            return None;
        }
    };

    // description is required — missing/empty → skip.
    let description = match frontmatter.description {
        Some(ref d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => {
            error!(
                "Skill '{}' in {:?} has no description — skipping",
                frontmatter.name, skill_md_path
            );
            return None;
        }
    };

    // Lenient name validation — warn but still load.
    validate_skill_name(&frontmatter.name, dir_name, skill_md_path);

    let location = skill_md_path
        .canonicalize()
        .unwrap_or_else(|_| skill_md_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    Some(SkillMeta {
        name: frontmatter.name,
        description,
        location,
    })
}

/// Validates the `name` field against the Agent Skills spec and emits warnings.
/// Does NOT prevent loading — lenient mode per spec §Lenient validation.
fn validate_skill_name(name: &str, dir_name: &str, path: &Path) {
    if name != dir_name {
        warn!(
            "Skill name '{}' does not match directory '{}' at {:?}",
            name, dir_name, path
        );
    }
    if name.len() > 64 {
        warn!("Skill name '{}' exceeds 64 characters at {:?}", name, path);
    }
    // Per reference validator: allows Unicode alphanumeric (not just ASCII).
    if name != name.to_lowercase() {
        warn!("Skill name '{}' must be lowercase at {:?}", name, path);
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        warn!(
            "Skill name '{}' contains invalid characters at {:?} (only letters, digits, hyphens allowed)",
            name, path
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        warn!(
            "Skill name '{}' must not start or end with a hyphen at {:?}",
            name, path
        );
    }
    if name.contains("--") {
        warn!(
            "Skill name '{}' must not contain consecutive hyphens at {:?}",
            name, path
        );
    }
}

// ==========================================
// Frontmatter extraction
// ==========================================

/// Extracts the YAML block between the first pair of `---` line delimiters.
///
/// Works line-by-line so that `---` embedded in YAML string values
/// (e.g. `description: see --- above`) never closes the block early.
pub fn extract_frontmatter(content: &str) -> Option<String> {
    let mut lines = content.lines();

    // The very first line must be exactly `---`.
    if lines.next()? != "---" {
        return None;
    }

    let mut yaml_lines: Vec<&str> = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(yaml_lines.join("\n"));
        }
        yaml_lines.push(line);
    }

    // Never found a closing `---`.
    None
}

// ==========================================
// Prompt generation
// ==========================================

/// Generates the `<available_skills>` XML catalog for injection into the system prompt.
///
/// Returns an empty `<available_skills>` block when there are no skills,
/// matching the official reference library (`skills-ref/prompt.py`) behaviour.
pub fn generate_skills_xml(skills: &[SkillMeta]) -> String {
    if skills.is_empty() {
        return "<available_skills>\n</available_skills>".to_string();
    }

    let mut lines: Vec<String> = vec!["<available_skills>".to_string()];
    for skill in skills {
        lines.push("<skill>".to_string());
        lines.push("<name>".to_string());
        lines.push(escape_xml(&skill.name));
        lines.push("</name>".to_string());
        lines.push("<description>".to_string());
        lines.push(escape_xml(&skill.description));
        lines.push("</description>".to_string());
        lines.push("<location>".to_string());
        lines.push(escape_xml(&skill.location));
        lines.push("</location>".to_string());
        lines.push("</skill>".to_string());
    }
    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

/// Escapes special XML/HTML characters to prevent prompt injection.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ==========================================
// Regex path scanning (Phase 2)
// ==========================================

/// Extracts all path-like strings from text using a lenient regex.
///
/// Matches patterns like `./scripts/extract.py`, `references/api.md`, etc.
/// Returns a de-duplicated, order-preserving list.
pub fn scan_paths_in_content(content: &str) -> Vec<String> {
    let re = Regex::new(r"(?:\./)?(?:[\w.-]+/)+[\w.-]+(?:\.\w+)?").unwrap();
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for mat in re.find_iter(content) {
        let path = mat.as_str().to_string();
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }

    debug!("Regex scan extracted {} path(s) from content", paths.len());
    paths
}

// ==========================================
// PeonEngine — dynamic whitelist manager
// ==========================================

/// The core engine that manages dynamic path whitelists for tools.
///
/// After a skill's SKILL.md is read, the engine scans its content for
/// path-like strings, resolves them to absolute paths relative to the
/// skill's directory, checks existence and permissions, then appends
/// valid paths to the shared read/execute whitelists.
pub struct PeonEngine {
    enforcer: Arc<FileEnforcer>,
    /// Shared with `ReadFileTool` — both see the same live data.
    pub read_paths: SharedReadPaths,
    /// Shared with `ExecuteScriptTool` — both see the same live data.
    pub execute_paths: SharedExecutePaths,
}

impl PeonEngine {
    pub fn new(enforcer: Arc<FileEnforcer>) -> Self {
        Self {
            enforcer,
            read_paths: Arc::new(RwLock::new(HashSet::new())),
            execute_paths: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Scans skill content for paths, resolves them relative to `skill_base_dir`,
    /// checks existence + permissions, and appends to shared whitelists.
    pub async fn process_skill_content(
        &self,
        agent_id: &str,
        skill_base_dir: &Path,
        content: &str,
    ) {
        let raw_paths = scan_paths_in_content(content);

        for raw_path in &raw_paths {
            // Resolve relative paths against the skill's directory
            let absolute = skill_base_dir.join(raw_path);
            let resolved = absolute.canonicalize().unwrap_or(absolute.clone());
            let resolved_str = resolved.to_string_lossy().to_string();

            // 1. Existence check
            if fs::metadata(&resolved).await.is_err() {
                debug!(
                    "Path '{}' (resolved: '{}') does not exist — skipping",
                    raw_path,
                    resolved.display()
                );
                continue;
            }

            debug!(
                "Path '{}' resolved to '{}' — file exists",
                raw_path,
                resolved.display()
            );

            // 2. Permission checks & dynamic routing
            let can_read = self.enforcer.enforce(agent_id, "read", &resolved_str).await;
            let can_execute = self
                .enforcer
                .enforce(agent_id, "execute", &resolved_str)
                .await;

            if !can_read && !can_execute {
                warn!(
                    "All permissions denied for path '{}' — not added to any whitelist",
                    resolved_str
                );
                continue;
            }

            if can_read {
                let mut guard = self.read_paths.write().await;
                if guard.insert(resolved_str.clone()) {
                    info!("Added to read whitelist: {}", resolved_str);
                }
            }

            if can_execute {
                let mut guard = self.execute_paths.write().await;
                if guard.insert(resolved_str.clone()) {
                    info!("Added to execute whitelist: {}", resolved_str);
                }
            }
        }
    }

    /// Clears all whitelists (Phase 4: Context Sliding).
    pub async fn reset_session(&self) {
        self.read_paths.write().await.clear();
        self.execute_paths.write().await.clear();
        info!("Session reset — all whitelists cleared");
    }

    /// Generates the dynamic Tool JSON Schema array for the current turn.
    /// This is kept for reference/testing — in production the tools read directly
    /// from the shared `Arc<RwLock>` in their own `definition()` methods.
    pub async fn generate_tool_schemas(&self, skills: &[SkillMeta]) -> serde_json::Value {
        let skill_names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
        let read_paths: Vec<String> = {
            let guard = self.read_paths.read().await;
            let mut v: Vec<String> = guard.iter().cloned().collect();
            v.sort();
            v
        };
        let execute_paths: Vec<String> = {
            let guard = self.execute_paths.read().await;
            let mut v: Vec<String> = guard.iter().cloned().collect();
            v.sort();
            v
        };

        let mut tools = vec![serde_json::json!({
            "name": "read_skill",
            "description": "Read a skill's SKILL.md to get its instructions and available resources.",
            "parameters": {
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "enum": skill_names,
                        "description": "Name of the skill to read"
                    }
                },
                "required": ["skill_name"]
            }
        })];

        if !read_paths.is_empty() {
            tools.push(serde_json::json!({
                "name": "read_file",
                "description": "Read the contents of a pre-validated file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "enum": read_paths,
                            "description": "Path to the file to read"
                        }
                    },
                    "required": ["path"]
                }
            }));
        }

        if !execute_paths.is_empty() {
            tools.push(serde_json::json!({
                "name": "execute_script",
                "description": "Execute a pre-validated script with optional arguments.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "enum": execute_paths,
                            "description": "Path to the script to execute"
                        },
                        "arguments": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional CLI arguments to pass to the script"
                        }
                    },
                    "required": ["path"]
                }
            }));
        }

        tools.push(serde_json::json!({
            "name": "list_all_skills",
            "description": "List all available skills with their names, descriptions, and locations.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }));

        let schemas = serde_json::json!(tools);
        debug!("Generated tool schemas ({} tools)", tools.len());
        schemas
    }
}

// ==========================================
// Tests
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs as tfs;

    // ------------------------------------
    // Helper: build a temp skill directory
    // ------------------------------------
    async fn make_skill(base: &Path, name: &str, frontmatter: &str, body: &str) -> PathBuf {
        let dir = base.join(name);
        tfs::create_dir_all(&dir).await.unwrap();
        let content = format!("---\n{}\n---\n\n{}", frontmatter, body);
        tfs::write(dir.join("SKILL.md"), content).await.unwrap();
        dir
    }

    // ------------------------------------
    // extract_frontmatter unit tests
    // ------------------------------------

    #[test]
    fn test_extract_frontmatter_valid() {
        let content = "---\nname: my-skill\ndescription: Does stuff.\n---\n\n# Body";
        let result = extract_frontmatter(content);
        assert_eq!(
            result,
            Some("name: my-skill\ndescription: Does stuff.".to_string())
        );
    }

    #[test]
    fn test_extract_frontmatter_no_opening() {
        let content = "name: my-skill\ndescription: foo\n---\n";
        assert_eq!(extract_frontmatter(content), None);
    }

    #[test]
    fn test_extract_frontmatter_no_closing() {
        let content = "---\nname: my-skill\ndescription: foo\n";
        assert_eq!(extract_frontmatter(content), None);
    }

    #[test]
    fn test_extract_frontmatter_empty_body() {
        // A `---` value embedded in a YAML string must NOT close the block.
        let content = "---\nname: foo\ndescription: see --- above\n---\n";
        // The `---` inside the description line contains spaces around it,
        // so the line is NOT exactly `---` — it must NOT close the block.
        let result = extract_frontmatter(content);
        assert_eq!(
            result,
            Some("name: foo\ndescription: see --- above".to_string())
        );
    }

    // ------------------------------------
    // generate_skills_xml unit tests
    // ------------------------------------

    #[test]
    fn test_generate_skills_xml_empty() {
        let xml = generate_skills_xml(&[]);
        assert_eq!(xml, "<available_skills>\n</available_skills>");
    }

    #[test]
    fn test_generate_skills_xml_escapes() {
        let skills = vec![SkillMeta {
            name: "test-skill".to_string(),
            description: "Does <things> & stuff".to_string(),
            location: "/tmp/test-skill/SKILL.md".to_string(),
        }];
        let xml = generate_skills_xml(&skills);
        assert!(xml.contains("&lt;things&gt;"));
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("</available_skills>"));
    }

    #[test]
    fn test_generate_skills_xml_structure() {
        let skills = vec![SkillMeta {
            name: "roll-dice".to_string(),
            description: "Roll dice.".to_string(),
            location: "/home/user/.skills/roll-dice/SKILL.md".to_string(),
        }];
        let xml = generate_skills_xml(&skills);
        // Each tag on its own line, matching reference lib format.
        assert!(xml.contains("<name>\nroll-dice\n</name>"));
        assert!(xml.contains("<description>\nRoll dice.\n</description>"));
        assert!(xml.contains("<location>\n/home/user/.skills/roll-dice/SKILL.md\n</location>"));
    }

    #[test]
    fn test_find_skill_md_lowercase_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lower-skill");
        std::fs::create_dir_all(&dir).unwrap();
        // Only create lowercase skill.md
        std::fs::write(dir.join("skill.md"), "---\nname: lower-skill\n---\n").unwrap();
        let result = find_skill_md(&dir);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("skill.md"));
    }

    // ------------------------------------
    // scan_skills integration tests
    // ------------------------------------

    #[tokio::test]
    async fn test_scan_missing_dir() {
        let skills = scan_skills("/nonexistent/path/xyz123", None).await.unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_scan_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_scan_single_valid_skill() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill(
            tmp.path(),
            "roll-dice",
            "name: roll-dice\ndescription: Roll dice using a random number generator.",
            "To roll a die, run: echo $((RANDOM % 6 + 1))",
        )
        .await;

        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "roll-dice");
        assert!(skills[0].description.contains("Roll dice"));
        assert!(skills[0].location.ends_with("SKILL.md"));
    }

    #[tokio::test]
    async fn test_scan_skips_missing_description() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill(
            tmp.path(),
            "bad-skill",
            "name: bad-skill",
            "no description here",
        )
        .await;

        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert!(
            skills.is_empty(),
            "skill without description must be skipped"
        );
    }

    #[tokio::test]
    async fn test_scan_skips_bad_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("broken-skill");
        tfs::create_dir_all(&dir).await.unwrap();
        tfs::write(dir.join("SKILL.md"), "---\n: bad: yaml: [\n---\n")
            .await
            .unwrap();

        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_scan_recursive_nested() {
        let tmp = tempfile::tempdir().unwrap();

        // Root-level skill
        make_skill(
            tmp.path(),
            "top-skill",
            "name: top-skill\ndescription: A top-level skill.",
            "Do the top thing.",
        )
        .await;

        // Nested skill two levels deep
        let nested = tmp.path().join("top-skill").join("nested");
        tfs::create_dir_all(&nested).await.unwrap();
        make_skill(
            &nested,
            "nested-skill",
            "name: nested-skill\ndescription: A nested skill.",
            "Do the nested thing.",
        )
        .await;

        let skills = scan_skills(tmp.path().to_str().unwrap(), Some(4))
            .await
            .unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"top-skill"),
            "top-level skill must be found"
        );
        assert!(
            names.contains(&"nested-skill"),
            "nested skill must be found"
        );
    }

    #[tokio::test]
    async fn test_scan_depth_limit() {
        let tmp = tempfile::tempdir().unwrap();

        // Create a skill exactly at depth 1
        make_skill(
            tmp.path(),
            "shallow-skill",
            "name: shallow-skill\ndescription: Shallow.",
            "Shallow body.",
        )
        .await;

        // Create a skill too deep to reach with depth=1
        let deep = tmp.path().join("shallow-skill").join("deep");
        tfs::create_dir_all(&deep).await.unwrap();
        make_skill(
            &deep,
            "deep-skill",
            "name: deep-skill\ndescription: Too deep.",
            "Deep body.",
        )
        .await;

        // With max_depth=1: only the root children are visited, not grandchildren.
        let skills = scan_skills(tmp.path().to_str().unwrap(), Some(1))
            .await
            .unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"shallow-skill"));
        assert!(
            !names.contains(&"deep-skill"),
            "deep skill must not appear at depth=1"
        );
    }

    #[tokio::test]
    async fn test_scan_duplicate_name_first_wins() {
        let tmp = tempfile::tempdir().unwrap();

        // Two directories with the same skill name.
        make_skill(
            tmp.path(),
            "my-skill",
            "name: my-skill\ndescription: First copy.",
            "First.",
        )
        .await;

        // Second directory — different folder name, same skill name.
        let dir2 = tmp.path().join("my-skill-copy");
        tfs::create_dir_all(&dir2).await.unwrap();
        tfs::write(
            dir2.join("SKILL.md"),
            "---\nname: my-skill\ndescription: Second copy.\n---\n\nSecond.",
        )
        .await
        .unwrap();

        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert_eq!(skills.len(), 1, "duplicate name must be deduplicated");
    }

    #[tokio::test]
    async fn test_location_is_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill(
            tmp.path(),
            "abs-skill",
            "name: abs-skill\ndescription: Tests absolute path.",
            "Body.",
        )
        .await;

        let skills = scan_skills(tmp.path().to_str().unwrap(), None)
            .await
            .unwrap();
        assert_eq!(skills.len(), 1);
        assert!(
            Path::new(&skills[0].location).is_absolute(),
            "location must be an absolute path"
        );
    }

    // ------------------------------------
    // scan_paths_in_content unit tests
    // ------------------------------------

    #[test]
    fn test_scan_paths_extracts_relative_paths() {
        let content = r#"
To process PDFs, run ./scripts/extract.py with the --format flag.
See references/api.md for the full API documentation.
Also check out tools/lint.sh for linting.
"#;
        let paths = scan_paths_in_content(content);
        assert!(paths.contains(&"./scripts/extract.py".to_string()));
        assert!(paths.contains(&"references/api.md".to_string()));
        assert!(paths.contains(&"tools/lint.sh".to_string()));
    }

    #[test]
    fn test_scan_paths_empty_for_no_paths() {
        let content = "This is just plain text without any paths or file references.";
        let paths = scan_paths_in_content(content);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_scan_paths_deduplicates() {
        let content = r#"
Run ./scripts/deploy.sh first.
Then run ./scripts/deploy.sh again.
"#;
        let paths = scan_paths_in_content(content);
        assert_eq!(
            paths.iter().filter(|p| *p == "./scripts/deploy.sh").count(),
            1,
            "duplicate paths must be deduplicated"
        );
    }

    #[test]
    fn test_scan_paths_preserves_order() {
        let content = "Use b/second.py then a/first.py";
        let paths = scan_paths_in_content(content);
        assert_eq!(paths[0], "b/second.py");
        assert_eq!(paths[1], "a/first.py");
    }

    // ------------------------------------
    // generate_tool_schemas unit tests
    // ------------------------------------

    #[tokio::test]
    async fn test_generate_tool_schemas_minimal() {
        let enforcer = FileEnforcer::new();
        let engine = PeonEngine::new(enforcer);
        let skills = vec![SkillMeta {
            name: "pdf-processing".to_string(),
            description: "Process PDF files".to_string(),
            location: "/tmp/pdf-processing/SKILL.md".to_string(),
        }];
        let schemas = engine.generate_tool_schemas(&skills).await;
        let arr = schemas.as_array().unwrap();
        // Only read_skill + list_all_skills when whitelists are empty
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "read_skill");
        assert_eq!(arr[1]["name"], "list_all_skills");
        // Verify skill_name enum
        let skill_enum = &arr[0]["parameters"]["properties"]["skill_name"]["enum"];
        assert_eq!(skill_enum[0], "pdf-processing");
    }
}
