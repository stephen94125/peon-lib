<div align="center">

# `Peon`

![Static Badge](https://img.shields.io/badge/mission-zero_trust_AI_agent_workspaces-purple)
<br />
![GitHub top language](https://img.shields.io/github/languages/top/stephen94125/peon-lib)
![GitHub last commit](https://img.shields.io/github/last-commit/stephen94125/peon-lib)
[![crates.io](https://img.shields.io/crates/v/peon-core.svg)](https://crates.io/crates/peon-core)
[![License](https://img.shields.io/badge/License-MIT%20&%20Apache--2.0-green.svg)](https://opensource.org/licenses/MIT)

<h3>The AI agent framework that says <em>no</em>.</h3>

<p align="center">
  <strong>English</strong> ·
  <a href="README_zh.md">中文</a>
</p>

</div>

---

**Other frameworks give LLMs a shell and pray. Peon gives LLMs a leash and proves it works.**

Most agent frameworks (LangChain, AutoGPT, CrewAI) focus on _what_ an AI can do.
Peon focuses on _what it cannot_ — and enforces that at the architecture level, not the prompt level.

<div align="center">

```
User "3856588331" sends: "Roll a 128-sided die"

✅ read_skill("roll-dice")        → Skill loaded, paths unlocked
✅ execute_script("roll.sh", 128) → Whitelist check passed, enforcer approved
✅ Agent response: "You rolled **30** on a d128!"
```

```
Same bot, different user (not in policy):

✅ read_skill("roll-dice")        → Skill loaded...
⛔ ALL PERMISSIONS DENIED          → Path never entered whitelist
⛔ execute_script("roll.sh", 128) → SECURITY VIOLATION: not in whitelist
🤖 Agent response: "I cannot execute this script — permission denied."
```

**Same bot. Same code. Same skill. Different user → different outcome.**
That's zero-trust.

</div>

---

## ⚡ See It In Action

Here's what actually happens when a Telegram user sends `"Roll a 128-sided die"` to a Peon-powered bot:

```log
INFO  peon_telegram       Received message from chat ID 3856588331
INFO  peon_core::agent    User input (uid=3856588331): 幫我骰一個128面的骰子
INFO  peon_runtime::agent Agent run: uid='3856588331'

# Turn 1: LLM discovers the skill
INFO  peon_runtime::agent Tool call: read_skill({"skill_name":"roll-dice"})
INFO  peon_core::scanner  Added to execute whitelist: .../roll-dice/scripts/roll.sh

# Turn 2: LLM executes with the unlocked path
INFO  peon_runtime::agent Tool call: execute_script({"path":"...roll.sh","arguments":["128"]})
INFO  peon_core::tools    Execute access granted for: .../roll-dice/scripts/roll.sh

# Turn 3: Done.
INFO  peon_runtime::agent Agent response (turn 3): 你丟出 128 面骰的結果是：30
```

Now change one line in `user_permissions.csv` — remove the user's access:

```log
INFO  peon_core::tools    Tool call: read_skill('roll-dice')
WARN  peon_core::scanner  All permissions denied for path 'roll.sh' — not added to any whitelist
WARN  peon_core::tools    SECURITY VIOLATION: './scripts/roll.sh' not in execute whitelist — blocked
INFO  peon_runtime::agent Agent response: "I cannot execute this script — permission denied."
```

**The LLM retried twice. It tried relative paths. It tried re-reading the skill. Nothing worked.** The path was never whitelisted because the enforcer rejected the user identity at the scan layer — two layers before execution.

---

## 🧠 How It Works

```
┌──────────────┐     ┌────────────────────┐     ┌──────────────────┐
│   LLM Brain  │────▶│  Peon Security     │────▶│  OS Execution    │
│  (Reasoning) │     │  Matrix (Casbin)   │     │  (Scripts/Files) │
│              │     │                    │     │                  │
│ "Execute X"  │     │ UID? ✓            │     │ bash roll.sh 128 │
│              │     │ Whitelist? ✓       │     │                  │
│              │     │ File ACL? ✓        │     │ → stdout: "30"   │
│              │     │ User ACL? ✓        │     │                  │
└──────────────┘     └────────────────────┘     └──────────────────┘
                        ▲ Fails ANY check?
                        │ → Blocked. Period.
```

**Defence in Depth, not Defence by Prompt:**

| Layer              | What                                                        | Bypass-proof?                              |
| :----------------- | :---------------------------------------------------------- | :----------------------------------------- |
| **Whitelist**      | Only paths discovered from `SKILL.md` are executable        | ✅ LLM cannot invent paths                 |
| **File ACL**       | `file_permissions.txt` — system-wide deny/allow rules       | ✅ Not visible to LLM                      |
| **User ACL**       | `user_permissions.csv` — per-user RBAC via Casbin           | ✅ UID injected physically, not via prompt |
| **RequestContext** | UID is passed as a Rust struct, not a task-local or env var | ✅ Unforgeable by the LLM                  |

---

## 📦 Quick Start

### Install

```bash
cargo add peon-core
```

Or from source:

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib
cargo build --release
```

### Configure

```dotenv
# .env
PROVIDER=openai          # openai | anthropic | gemini | openrouter
MODEL=gpt-4o-mini
API_KEY=sk-...

PEON_SKILLS_DIR=skills
PEON_FILE_PERMISSIONS=file_permissions.txt
PEON_USER_PERMISSIONS=user_permissions.csv
```

### Run

```rust
let agent = PeonAgentBuilder::new().await?.default_prompt().build();
let response = agent.prompt("Roll a 20-sided die", "user_123").await?;
// The UID "user_123" is physically passed to every tool call.
// The LLM cannot forge, override, or escalate it. Period.
```

---

## 🧩 Workspace Crates

| Crate                                 | crates.io                                                                                                 | Purpose                                                                    |
| :------------------------------------ | :-------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------- |
| **[`peon-runtime`](peon-runtime/)**   | [![crates.io](https://img.shields.io/crates/v/peon-runtime.svg)](https://crates.io/crates/peon-runtime)   | Custom LLM runtime — provider abstraction, agent loop, multimodal messages |
| **[`peon-core`](peon-core/)**         | [![crates.io](https://img.shields.io/crates/v/peon-core.svg)](https://crates.io/crates/peon-core)         | Zero-trust engine — skill scanner, Casbin enforcers, tool sandboxing       |
| **[`peon-cli`](peon-cli/)**           | [![crates.io](https://img.shields.io/crates/v/peon-cli.svg)](https://crates.io/crates/peon-cli)           | Unix-style CLI — pipe stdin, use in CI/CD                                  |
| **[`peon-telegram`](peon-telegram/)** | [![crates.io](https://img.shields.io/crates/v/peon-telegram.svg)](https://crates.io/crates/peon-telegram) | Multi-user Telegram bot with per-user identity isolation                   |

---

## 🛡️ Security Model

> [!WARNING]
> **Strict Zero-Trust**: Peon requires `file_permissions.txt` and `user_permissions.csv` to exist. If missing, the agent **panics on startup** — no silent fallback, no "allow-all" default.

**`file_permissions.txt`** — What the agent can touch:

```text
x, ./skills/*         # Allow execution within skills
!x, /bin/rm           # Block rm, always, no exceptions
r, ./data/*           # Allow reading data files
!r, ./secrets/*       # Block reading secrets
```

**`user_permissions.csv`** — Who can do what:

```csv
p, *, *, *, allow                  # Allow everyone (development mode)
p, 3856588331, *, execute, allow   # Only this Telegram user can execute
p, admin_role, *, *, allow         # Role-based access
g, alice, admin_role               # Alice inherits admin permissions
```

---

## 🔧 Skills System

Skills are **Markdown files that define what an LLM is allowed to do** — not just what it _can_ do.

```
skills/
└── roll-dice/
    ├── SKILL.md           # Instructions + path declarations
    └── scripts/
        └── roll.sh        # The actual executable
```

```markdown
---
name: roll-dice
description: Roll dice using a random number generator.
---

To roll a die, execute: `./scripts/roll.sh <sides>`
```

When `read_skill("roll-dice")` is called, Peon:

1. Reads the SKILL.md content
2. Extracts all referenced paths (`./scripts/roll.sh`)
3. Resolves them to absolute paths via `canonicalize()`
4. Checks the enforcer for each path
5. Only whitelisted + enforcer-approved paths get added

**The LLM never sees the filesystem. It only sees what Peon explicitly unlocks.**

---

## 🗺️ Roadmap

| Status | Feature                                                                     |
| :----- | :-------------------------------------------------------------------------- |
| ✅     | Zero-trust tool execution with dual-layer enforcement                       |
| ✅     | Custom LLM runtime (`peon-runtime`) — OpenAI, Anthropic, Gemini, OpenRouter |
| ✅     | Telegram bot with per-user identity isolation                               |
| ✅     | Skill discovery, dynamic whitelisting, session reset                        |
| 🔜     | **Telegram**: Rich responses — images, files, formatted messages            |
| 🔜     | **Telegram**: Multimodal input — photos, voice, documents                   |
| 🔜     | **Discord**: Bot integration                                                |
| 🔜     | **CLI**: End-to-end verification with real API providers                    |
| 🗓️     | WASM runtime support for browser-based agents                               |
| 🗓️     | Persistent conversation memory across sessions                              |

---

## 🤝 Supported Providers

Peon uses its own runtime (`peon-runtime`) with native support for:

**OpenAI** · **Anthropic** · **Gemini** · **OpenRouter** (access to 200+ models)

---

## 📄 Meta

<a href="https://github.com/stephen94125/peon-lib/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=stephen94125/peon-lib" alt="contrib.rocks" />
</a>

Made with [contrib.rocks](https://contrib.rocks).

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
