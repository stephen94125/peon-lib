# Peon Lib

> **Enterprise-grade local agent executor with zero-trust tool security.**
>
> While other agent frameworks hand the LLM a blank cheque over your filesystem,
> peon-lib operates on a strict **"prove it before you touch it"** principle.
> Every file read and every script execution is gated behind two independent enforcement layers —
> the LLM is physically incapable of reaching paths it was never explicitly granted.

---

## Why peon-lib?

Popular agent frameworks wire an LLM directly to a bash or file-read tool with a free-form `path: string` argument.
This is a textbook **Arbitrary Code Execution** vulnerability: ask any capable model to "think creatively" and it will happily run `/bin/sh -c "rm -rf ~"` or exfiltrate credentials.

peon-lib was built to close that gap at the architecture level, not the prompt level.

---

## The Three-Axes Tool Architecture

All agent capabilities are expressed through exactly four tools:

| Tool | Purpose |
|------|---------|
| `list_all_skills` | Discover what skills are available |
| `read_skill` | Load a skill's SKILL.md and **unlock** its declared paths |
| `read_file` | Read a file — only if its path is whitelisted |
| `execute_script` | Run a script — only if its path is whitelisted |

There is no general-purpose `bash`, `shell`, or `write_file` tool.
The LLM cannot invent capabilities that don't exist.

---

## Security Model: Defence in Depth

### Layer 0 — Dynamic Enum Constraint (LLM-side)

Every time the LLM requests a tool definition, `read_file` and `execute_script` return a JSON Schema with an `enum` constraint built from the live whitelist:

```json
{
  "path": {
    "type": "string",
    "enum": ["/absolute/path/to/skill/scripts/run.sh"],
    "description": "Exact script path — must be one of the whitelisted paths"
  }
}
```

If the whitelist is empty (no skill has been read yet), the enum collapses to `[""]` — a dead-end that signals to the LLM there are no available scripts.
**The button simply doesn't exist until the path is earned.**

### Layer 1 — Hard Whitelist Check (Runtime, enforced in Rust)

Even if the LLM somehow crafts a request with a non-enum path (e.g. via a malformed tool call), the first line of `call()` checks the `Arc<RwLock<HashSet<String>>>` directly:

```rust
let guard = self.allowed_paths.read().await;
if !guard.contains(&args.path) {
    warn!("SECURITY VIOLATION: '{}' not in execute whitelist — blocked", args.path);
    return Err(ToolCallError::new(
        "Permission Denied: not a whitelisted script path. Call read_skill first."
    ));
}
```

This is pure Rust. No amount of prompt engineering bypasses it.

### Layer 2 — Enforcer (Casbin-ready)

A `FileEnforcer` stub sits behind Layer 1, evaluating `(subject, action, resource)` triples.
Currently defaults to allow-all for development, but is structured as the exact integration point for a Casbin policy engine — enabling fine-grained RBAC/ABAC rules per user, role, or resource pattern.

---

## How Paths Get Whitelisted: The Scan Pipeline

Paths never enter the whitelist through operator configuration or LLM requests.
They enter **only** through the following pipeline:

```
read_skill("roll-dice")
       │
       ▼
  Read SKILL.md content
       │
       ▼
  Regex scan for path-like strings  ← ./scripts/roll.sh, references/api.md, etc.
       │
       ▼
  Resolve each path relative to the skill's base directory → absolute path
       │
       ▼
  Existence check (tokio::fs::metadata)
       │
       ▼
  Enforcer check (read? execute?)
       │
       ▼
  Insert into Arc<RwLock<HashSet>>  ← shared live reference held by the tools
       │
       ▼
  Next tool definition() call returns updated enum
```

This means:
- **Paths must physically exist** at the moment the skill is read.
- **Paths must pass the enforcer** before being whitelisted.
- **Paths are scoped to one session** — `reset_session()` clears all whitelists between conversations.

---

## Execution Flow (Full Example)

The following is an annotated real run with `RUST_LOG=debug`:

```
INFO  agent        🚀 Peon agent starting up...
INFO  scanner      Scanning for skills in '.skills/' (max depth: 4)
DEBUG scanner      Loaded skill 'roll-dice' from '.skills/roll-dice/SKILL.md'
INFO  scanner      Scan complete: found 2 skill(s)

INFO  agent        User input: Roll a 20-sided die

DEBUG tools        read_file definition: 0 path(s) in whitelist     ← locked
DEBUG tools        execute_script definition: 0 path(s) in whitelist ← locked

INFO  tools        Tool call: read_skill('roll-dice')
DEBUG tools        Reading SKILL.md from: .skills/roll-dice/SKILL.md
DEBUG scanner      Regex scan extracted 1 path(s) from content
DEBUG scanner      Path './scripts/roll.sh' resolved to '/abs/.../roll.sh' — file exists
DEBUG enforcer     action='execute', resource='.../roll.sh'
INFO  scanner      Added to execute whitelist: .../roll.sh

DEBUG tools        execute_script definition: 1 path(s) in whitelist ← unlocked

INFO  tools        Tool call: execute_script('.../roll.sh', args=["20"])
INFO  tools        Execute access granted for: .../roll.sh
DEBUG tools        Resolved interpreter: 'bash'
DEBUG tools        exit_code=0, stdout_len=1

INFO  agent        Agent response: rolled a 4.

INFO  scanner      Session reset — all whitelists cleared
```

Zero hallucination. Zero arbitrary execution. Every step is logged and auditable.

---

## Provider Support

peon-lib uses `AnyModel` — a unified enum-dispatch wrapper over every provider supported by [rig-core](https://github.com/0xPlaygrounds/rig):

```
Anthropic · Azure · Cohere · Deepseek · Gemini · Groq · Huggingface
Hyperbolic · Llamafile · Mira · Mistral · Moonshot · Ollama
OpenAI · OpenRouter · Perplexity · Together · xAI
```

Switch providers with two environment variables — no code changes:

```bash
DEFAULT_PROVIDER=gemini DEFAULT_MODEL=gemini-2.5-flash cargo run --example agent
DEFAULT_PROVIDER=anthropic DEFAULT_MODEL=claude-opus-4-5 cargo run --example agent
```

Each provider is gated behind a feature flag to keep binary size minimal (critical for future WASM targets).

---

## Structured Logging

Replace all `println!` with the `log` facade — plug in any backend without changing library code:

| Environment | Backend |
|---|---|
| Terminal / CI | `env_logger` (included in dev-deps) |
| Android | `android_logger` |
| Browser / WASM | `console_log` |
| Structured JSON | `tracing-subscriber` |

Log levels:

| Level | What you see |
|---|---|
| `ERROR` | Failed reads, YAML parse errors, process spawn failures |
| `WARN` | Permission denials (yellow), name validation issues, scan limits |
| `INFO` | Tool calls, skill discovery, permission grants, user I/O |
| `DEBUG` | Path resolution, whitelist state, enforcer evaluations, schema generation |

```bash
RUST_LOG=info cargo run --example agent    # production-like
RUST_LOG=debug cargo run --example agent   # full audit trail
```

---

## Agent Skills Format

peon-lib implements the [Agent Skills](https://agentskills.io) open specification.

Skills live in `.skills/` (or any configured directory):

```
.skills/
└── roll-dice/
    ├── SKILL.md          ← frontmatter: name, description; body: instructions + file paths
    └── scripts/
        └── roll.sh       ← whitelisted automatically when SKILL.md is read
```

`SKILL.md` frontmatter:

```yaml
---
name: roll-dice
description: Roll dice using a random number generator.
---
```

Any path-like string in the body (`./scripts/roll.sh`, `references/api.md`) is extracted by regex
and added to the whitelist if it exists on disk and passes the enforcer.

---

## Quick Start

```bash
# Clone and set up environment
cp .env.example .env
# Edit .env: set DEFAULT_PROVIDER, DEFAULT_MODEL, and your API key

# Run the example agent
RUST_LOG=info cargo run --example agent

# Run tests
cargo test
```

---

## Roadmap

- [ ] **CLI tool** — interactive shell with session management, multi-turn conversation, and skill hot-reload
- [ ] **File permission management via Casbin** — replace the allow-all enforcer stub with real RBAC/ABAC policies scoped to files, directories, and action types
- [ ] **Personnel permission management via Casbin** — user/role-level access control; different agents or human operators get different whitelists
- [ ] **WASM support** — compile peon-lib to WebAssembly for in-browser or edge deployment; all feature-flagged providers already designed with this in mind

---

## License

Apache 2.0
