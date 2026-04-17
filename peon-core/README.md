# Peon Core

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

**Peon Core** is the zero-trust engine driving the Peon workspace. It ensures that any AI agent instantiated through our framework is physically powerless to access or execute scripts without explicit authorization.

For deep-dive architecture details on _how_ our dual-layer sandboxing and dynamic whitelisting system works, please read our [Security Architecture Handbook](docs/security_architecture.md).

---

## 🚀 Quick Start

Creating an AI agent with bullet-proof security requires only a few lines of Rust.

```rust
use peon_core::agent::PeonAgent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize the Enforcer and Engine
    let agent = PeonAgent::new().await?;

    // 2. Chat with the agent
    let response = agent.prompt("Roll a 20-sided die for me!").await?;
    println!("Agent answered: {}", response);

    Ok(())
}
```

### Try it yourself!

We've prepared an out-of-the-box example in the `examples/` directory.

1. Ensure your `.env` is configured with your chosen provider (e.g., `DEFAULT_PROVIDER=openai` and `OPENAI_API_KEY=...`).
2. Run the example from your workspace root:

```bash
# Basic log output
cargo run -p peon-core --example simple_agent

# See full internal scanning, discovery, and capability unlocking logs
RUST_LOG=debug cargo run -p peon-core --example simple_agent
```

---

## 🔐 Advanced Permission Management (Casbin)

Security in Peon Core isn't hardcoded; it relies on the industry-standard [Casbin Authorization Library](https://casbin.org/). Casbin provides an incredibly powerful way to enforce Role-Based Access Control (RBAC) and Attribute-Based Access Control (ABAC).

For a detailed understanding of how to write complex policies, we highly recommend reading the [Casbin Official Syntax Documentation](https://casbin.org/docs/how-it-works).

> [!WARNING]
> **Strict Enforcement**: Peon looks for permission files in your current working directory (`./`) by default. If they are missing, the engine will **Panic on startup**. 
>
> You can override these locations via environment variables:
> - `PEON_FILE_PERMISSIONS`: Custom path for file ACLs.
> - `PEON_USER_PERMISSIONS`: Custom path for user/role ACLs.

There are two primary permission axes you must configure for your Agent:

### 1. File & Path Permissions (`file_permissions.txt`)

File permissions authorize or reject access to physical paths on your operating system, acting as a global denylist against LLM actions.

**Setup:**
Copy the provided example to get started mapping your paths:

```bash
cp file_permissions_example.txt file_permissions.txt
```

**Formatting Rules:**
The enforcer utilizes a **Deny-Override** logic. The fields are defined as `action, target_path`.

- `r`: Read access
- `x`: Execute script access
- `!r` / `!x`: Explicitly **Deny** access (Highest priority)

```text
# Allow the agent to execute any script within the skills directory
# (Note: the default directory is ./skills, not ./.skills)
x, ./skills/*

# Block reading of a specific file universally, no matter what happens
!r, ./secret_passwords.txt
```

### 2. Personnel Permissions (`user_permissions.csv`)

Personnel permissions dictate _who_ is communicating with the agent, assigning them roles or restricting capabilities based on User IDs.

**Setup:**
Peon Core natively looks for `user_permissions.csv` in your root environment.

**Formatting Rules:**
These policies define who can interact with what using Casbin's core syntax: `p, user, resource, action, effect`.

```csv
# 1. Default fallback: allow everyone to read and execute universally
p, *, *, *, allow

# 2. Block user "Bob" from executing any scripts
p, Bob, *, execute, deny

# 3. Setting up a Group/Role ('g' function)
# Block the "standard_users" role from touching the /root directory
p, standard_users, /root/*, *, deny

# 4. Assigning Bob to the standard_users role
g, Bob, standard_users
```

---

## 🛠 Logging

Because Peon abstracts security entirely, debugging paths is essential.
Change the `RUST_LOG` environment variable to filter output:

- `RUST_LOG=info` - Standard agent responses and high-level tool calls.
- `RUST_LOG=debug` - Detailed tracing of Regex matching, Casbin evaluations, and exact file paths Whitelisted.

---

## 📄 License

This project is dual-licensed under either the [MIT license](../LICENSE-MIT) or the [Apache License, Version 2.0](../LICENSE-APACHE), at your option.
