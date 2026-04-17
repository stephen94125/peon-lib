<div align="center">

# `Peon`

![Static Badge](https://img.shields.io/badge/mission-zero_trust_AI_agent_workspaces-purple)
<br />
![GitHub top language](https://img.shields.io/github/languages/top/stephen94125/peon-lib)
![GitHub last commit](https://img.shields.io/github/last-commit/stephen94125/peon-lib)
[![License](https://img.shields.io/badge/License-MIT%20&%20Apache--2.0-green.svg)](https://opensource.org/licenses/MIT)

<div align="center">
<h4><code>Peon</code> is an enterprise-grade, zero-trust framework for autonomous AI Agents.</h4>
</div>

<p align="center">
  <strong>English</strong> ·
  <a href="README_zh.md">中文</a>
</p>

</div>

[Updates](#updates) •
[What and Why](#what-and-why) •
[Philosophy](#philosophy) •
[Installation](#installation) •
[Workspace Crates](#workspace-crates) •
[Usage & Environments](#usage--environments) •
[Skill Development](#skill-development) •
[Security Models](#security-models) •
[Meta](#meta)

---

## What and why

Since the explosive rise of autonomous agents, we've seen an **_extraordinary_** number of frameworks (like LangChain or AutoGPT) that blindly give LLMs access to terminal sandboxes.

It's all really exciting and powerful, but _giving an AI unrestricted `bash` execution limits its applicability in secure production environments._

<div align="center">
<h4>In other words, AI agents don't just have an intelligence problem—they have a <em>trust and security</em> problem.</h4>
</div>

**Peon was created to address this by introducing the concept of true RBAC/ABAC (Role and Attribute Based Access Control) deeply integrated into the AI tool execution loop.**

Peon achieves true **Defence in Depth** by decoupling the reasoning/intelligence layer from the operating system execution layer. In Peon, an LLM cannot hallucinate an arbitrary `rm -rf /` command. Every tool execution, file read, and network request must pass through a strict, Casbin-enforced security matrix.

## Updates

For a deep dive into Peon and its internals, read the documentation located within our specific module folders (`peon-core/README.md`).

## Navigation

- [`Peon`](#peon)
  - [What and why](#what-and-why)
  - [Updates](#updates)
  - [Philosophy](#philosophy)
    - [Prove it before you touch it](#prove-it-before-you-touch-it)
  - [Installation](#installation)
  - [Usage & Environments](#usage--environments)
    - [Peon CLI](#peon-cli)
    - [Peon Telegram](#peon-telegram)
    - [Peon LINE](#peon-line)
  - [Our approach to Skills](#our-approach-to-skills)
  - [Security Models](#security-models)
  - [Supported AI Providers](#supported-ai-providers)

## Philosophy

> Intelligence without control isn't automation; it's a liability.

We believe the purpose of AI is to rapidly prototype and execute tasks, but when we deal with Enterprise AI, we must start with the **security and auditability** of the agent's actions.

### Prove it before you touch it

Our approach breaks down agent execution into two disjoint pipelines:

1. **The Brain (`rig` / LLM)**: Decides to "execute script X on target Y".
2. **The Enforcer (Casbin / PeonCore)**: Intercepts the request. Evaluates the executing user's identity against the target's explicit file permissions to decide if it `Allows` or `Denies` the call.

Only when both systems align can the underlying operating system be invoked.

## Installation

### Prerequisites

To install Peon from source, [make sure Rust and Cargo are installed](https://rustup.rs/).

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### From Source

Clone the workspace and build the framework locally.

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib
cargo build --release
```

All compiled binaries will be located in `./target/release/`.

### Environment Variables & Setup

Peon supports multiple client applications natively within its Cargo Workspace. However, **each client MUST have its own `.env` file** to run.

You can initialize a project by copying the `.env.example` in each folder:

```bash
cd peon-cli
cp .env.example .env
```

A base configuration looks like this:

```dotenv
# Provider and Model
PROVIDER=openai
DEFAULT_MODEL=gpt-4o
# OPENAI_API_KEY=sk-...

# Policy Contexts (Requires relative paths)
PEON_SKILLS_DIR=skills
PEON_FILE_PERMISSIONS_PATH=file_permissions.txt
PEON_USER_PERMISSIONS_PATH=user_permissions.csv
```

## Workspace Crates

Peon is fundamentally divided into separated modules depending on the interface you wish to expose.

| Component           | Focus                                          |
| :------------------ | :--------------------------------------------- |
| **`peon-core`**     | Libraries, Security Engine, Integrations       |
| **`peon-cli`**      | Unix Standard I/O, Scripts, DevOps             |
| **`peon-telegram`** | Long-polling chat, collaborative bots          |
| **`peon-line`**     | Axum Webhooks, Rich UI media (locations/audio) |

## Usage & Environments

Once your `.env` is configured across the crates you desire, here is how you leverage them.

### Peon CLI

`peon-cli` acts like standard GNU utilities. You can pass instructions via flags or standard input.

```bash
# Basic chat request
cargo run -p peon-cli -- -m "How do I securely route a network?"

# Utilizing standard pipes (stdin) to provide context
cat /var/log/syslog | cargo run -p peon-cli -- -m "Find the out of memory panics in this log."
```

If you specify `RUST_LOG=debug`, you will see exactly how Peon is scanning its skills list and enforcing Casbin access dynamically.

### Peon Telegram

A native multi-user solution built with `teloxide`.
Add the following to your `peon-telegram/.env`:

```dotenv
TELOXIDE_TOKEN="123456789:ABCdefGHIjklmNoPQRsTuvwxyZ"
```

Then launch the daemon:

```bash
cargo run -p peon-telegram
```

The agent maintains distinct persistent sessions for each User ID that interacts with it.

### Peon LINE

A high-fidelity media integration designed for consumer-facing systems, utilizing `axum`. Peon hooks into the native messaging systems to send push-based UI elements, decoupling the traditional limitations of single-batch reply tokens.

Set the following in `peon-line/.env`:

```dotenv
LINE_CHANNEL_SECRET=your_secret
LINE_CHANNEL_ACCESS_TOKEN=your_token
```

```bash
cargo run -p peon-line
```

This runs an HTTP server on `0.0.0.0:3000/callback` out-of-the-box. Ensure you utilize `ngrok` or similar to expose this locally to LINE.

## Our approach to Skills

Peon _Skills_ are slightly different than typical generic tool definitions.

We use structured `Markdown/XML` logic inside a `skills/SKILL.md` file (Note: the default directory is **`./skills`**, not `./.skills`) to explicitly define both the _metadata_ of a script and the _LLM instructions_.

Here's an example of the philosophy:

```markdown
---
name: network_scanner
description: Executes Nmap on a target subnet.
---

When the user asks about network topology, parse the subnet and execute the sibling script `scan.sh`. Ensure you append `--safe` to the args.
```

Peon completely abstracts this structure, parsing it during bootstrapping, routing the security checks, and dynamically giving the LLM an `execute_skill` tool mapped physically to `scan.sh`.

## Security Models

No skill execution is allowed unless the user and file paths are cross-referenced with your root Casbin setup.

> [!WARNING]
> **Strict Zero-Trust**: Peon requires both `file_permissions.txt` and `user_permissions.csv` to exist in your current working directory (`./`) by default. If these files are missing, the agent will **fail to start (Panic)** to prevent insecure execution.
>
> You can override these paths using environment variables:
>
> - `PEON_FILE_PERMISSIONS_PATH`: Custom path for file ACLs.
> - `PEON_USER_PERMISSIONS_PATH`: Custom path for user/role ACLs.

**1. `file_permissions.txt`** (Denylist / Root ACL)

```text
# Give the agent access to execute scripts inside our skills folder
# (System default reads from ./skills/*)
x, ./skills/*
# Deny it from ever executing rm
!x, /bin/rm
```

**2. `user_permissions.csv`** (Identity & Role ACL)

```csv
# Assign the generic 'agent' user the sysadmin role
g, agent, system_admin
# System admins are permitted everything
p, system_admin, *, *, allow
```

Peon automatically intercepts every LLM function call to validate against these tables natively.

## Supported AI Providers

Peon supports a unified interface over almost any modern AI provider without changing your code, directly through `PROVIDER`:

Anthropic · Azure · Cohere · Deepseek · Gemini · Groq · Huggingface · Hyperbolic · Llamafile · Mira · Mistral · Moonshot · Ollama · OpenAI · OpenRouter · Perplexity · Together · xAI

## Meta

<a href="https://github.com/stephen94125/peon-lib/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=stephen94125/peon-lib" alt="contrib.rocks" />
</a>

Made with [contrib.rocks](https://contrib.rocks).
