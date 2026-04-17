<div align="center">

# `Peon`

![Static Badge](https://img.shields.io/badge/mission-zero_trust_AI_agent_workspaces-purple)
<br />
![GitHub top language](https://img.shields.io/github/languages/top/stephen94125/peon-lib)
![GitHub last commit](https://img.shields.io/github/last-commit/stephen94125/peon-lib)
[![crates.io](https://img.shields.io/crates/v/peon-core.svg)](https://crates.io/crates/peon-core)
[![License](https://img.shields.io/badge/License-MIT%20&%20Apache--2.0-green.svg)](https://opensource.org/licenses/MIT)

<h3>一个敢对 AI 说「不」的 Agent 框架。</h3>

<p align="center">
  <a href="README.md">English</a> ·
  <strong>中文</strong>
</p>

</div>

---

**其他框架给 LLM 一个 Shell 然后祈祷。Peon 给 LLM 一条锁链，并且证明这条锁链真的有效。**

大多数 Agent 框架（LangChain、AutoGPT、CrewAI）关注的是 AI _能做什么_。
Peon 关注的是 AI _不能做什么_ —— 并且在架构层面强制执行，而非仰赖 Prompt 层面的祈祷。

<div align="center">

```
用户 "3856588331" 发送: "帮我骰一个128面的骰子"

✅ read_skill("roll-dice")        → 技能加载，路径解锁
✅ execute_script("roll.sh", 128) → 白名单通过，执法器批准
✅ Agent 回应: "你丢出 128 面骰的结果是：30"
```

```
同一个机器人，不同用户（不在权限表内）:

✅ read_skill("roll-dice")        → 技能加载...
⛔ 全部权限拒绝                     → 路径根本没进入白名单
⛔ execute_script("roll.sh", 128) → 安全违规：不在白名单中
🤖 Agent 回应: "我无法执行该脚本 —— 权限被拒绝。"
```

**同一个机器人。同一份代码。同一个技能。不同用户 → 不同结果。**
这就是零信任。

</div>

---

## ⚡ 实际运行画面

以下是一个 Telegram 用户发送 `"帮我骰一个128面的骰子"` 时，Peon 驱动的机器人内部真实发生的事：

```log
INFO  peon_telegram       Received message from chat ID 3856588331
INFO  peon_core::agent    User input (uid=3856588331): 幫我骰一個128面的骰子
INFO  peon_runtime::agent Agent run: uid='3856588331'

# 第 1 轮: LLM 发现技能
INFO  peon_runtime::agent Tool call: read_skill({"skill_name":"roll-dice"})
INFO  peon_core::scanner  Added to execute whitelist: .../roll-dice/scripts/roll.sh

# 第 2 轮: LLM 使用已解锁的路径执行
INFO  peon_runtime::agent Tool call: execute_script({"path":"...roll.sh","arguments":["128"]})
INFO  peon_core::tools    Execute access granted for: .../roll-dice/scripts/roll.sh

# 第 3 轮: 完成。
INFO  peon_runtime::agent Agent response (turn 3): 你丟出 128 面骰的結果是：30
```

现在只要改一行 `user_permissions.csv` —— 移除该用户的权限：

```log
INFO  peon_core::tools    Tool call: read_skill('roll-dice')
WARN  peon_core::scanner  All permissions denied for path 'roll.sh' — not added to any whitelist
WARN  peon_core::tools    SECURITY VIOLATION: './scripts/roll.sh' not in execute whitelist — blocked
INFO  peon_runtime::agent Agent response: "我这边目前无法执行骰子脚本 — 权限被拒绝。"
```

**LLM 重试了两次。它尝试了相对路径。它试了重新读取技能。全部无效。** 因为该路径在扫描层就被执法器拒绝了 —— 在执行层的两层之前。

---

## 🧠 运作原理

```
┌──────────────┐     ┌──────────────────────┐     ┌──────────────────┐
│  LLM 大脑     │────▶│  Peon 安全矩阵        │────▶│  系统执行层       │
│  (推理层)     │     │  (Casbin 执法引擎)    │     │  (脚本/文件)      │
│              │     │                      │     │                  │
│ "执行 X"     │     │ UID 验证? ✓          │     │ bash roll.sh 128 │
│              │     │ 白名单? ✓            │     │                  │
│              │     │ 文件 ACL? ✓          │     │ → stdout: "30"   │
│              │     │ 用户 ACL? ✓          │     │                  │
└──────────────┘     └──────────────────────┘     └──────────────────┘
                        ▲ 任何一层不通过?
                        │ → 拦截。句号。
```

**纵深防御，而非 Prompt 防御：**

| 层级               | 功能                                                  | 防绕过?                          |
| :----------------- | :---------------------------------------------------- | :------------------------------- |
| **白名单**         | 仅 `SKILL.md` 中发现的路径可执行                      | ✅ LLM 无法凭空捏造路径          |
| **文件 ACL**       | `file_permissions.txt` — 系统级拒绝/允许规则          | ✅ LLM 完全看不到这层            |
| **用户 ACL**       | `user_permissions.csv` — 基于 Casbin 的 RBAC 身份管理 | ✅ UID 以物理方式注入，非 Prompt |
| **RequestContext** | UID 是 Rust 结构体，非 task-local 或环境变量          | ✅ LLM 无法伪造                  |

---

## 📦 快速开始

### 安装

```bash
cargo add peon-core
```

或从源码编译：

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib
cargo build --release
```

### 配置

```dotenv
# .env
PROVIDER=openai          # openai | anthropic | gemini | openrouter
MODEL=gpt-4o-mini
API_KEY=sk-...

PEON_SKILLS_DIR=skills
PEON_FILE_PERMISSIONS=file_permissions.txt
PEON_USER_PERMISSIONS=user_permissions.csv
```

### 运行

```rust
let agent = PeonAgentBuilder::new().await?.default_prompt().build();
let response = agent.prompt("骰一个20面骰", "user_123").await?;
// UID "user_123" 被物理传递到每一个工具调用中。
// LLM 无法伪造、覆盖或提权。句号。
```

---

## 🧩 工作区模块

| 模块                                  | crates.io                                                                                                 | 用途                                                 |
| :------------------------------------ | :-------------------------------------------------------------------------------------------------------- | :--------------------------------------------------- |
| **[`peon-runtime`](peon-runtime/)**   | [![crates.io](https://img.shields.io/crates/v/peon-runtime.svg)](https://crates.io/crates/peon-runtime)   | 自研 LLM 运行时 — 供应商抽象、Agent 循环、多模态消息 |
| **[`peon-core`](peon-core/)**         | [![crates.io](https://img.shields.io/crates/v/peon-core.svg)](https://crates.io/crates/peon-core)         | 零信任引擎 — 技能扫描器、Casbin 执法器、工具沙箱     |
| **[`peon-cli`](peon-cli/)**           | [![crates.io](https://img.shields.io/crates/v/peon-cli.svg)](https://crates.io/crates/peon-cli)           | Unix 风格 CLI — 支持 stdin 管道，可用于 CI/CD        |
| **[`peon-telegram`](peon-telegram/)** | [![crates.io](https://img.shields.io/crates/v/peon-telegram.svg)](https://crates.io/crates/peon-telegram) | 多用户 Telegram 机器人，每用户身份隔离               |

---

## 🛡️ 安全模型

> [!WARNING]
> **严格零信任**: Peon 要求 `file_permissions.txt` 和 `user_permissions.csv` 必须存在。如果缺失，Agent 将**直接 Panic 拒绝启动** —— 没有静默降级，没有默认全开。

**`file_permissions.txt`** — Agent 能碰什么：

```text
x, ./skills/*         # 允许执行 skills 内的脚本
!x, /bin/rm           # 永远封锁 rm，没有例外
r, ./data/*           # 允许读取数据文件
!r, ./secrets/*       # 封锁 secrets 目录
```

**`user_permissions.csv`** — 谁能做什么：

```csv
p, *, *, *, allow                  # 全员放行（开发模式）
p, 3856588331, *, execute, allow   # 只有此 Telegram 用户可执行
p, admin_role, *, *, allow         # 角色级权限
g, alice, admin_role               # Alice 继承 admin 权限
```

---

## 🔧 技能系统

技能是 **定义 LLM 被允许做什么的 Markdown 文件** —— 不仅仅是它能做什么。

```
skills/
└── roll-dice/
    ├── SKILL.md           # 指令 + 路径声明
    └── scripts/
        └── roll.sh        # 实际可执行文件
```

```markdown
---
name: roll-dice
description: 使用随机数生成器骰骰子。
---

要骰骰子，执行: `./scripts/roll.sh <面数>`
```

当 `read_skill("roll-dice")` 被调用时，Peon 会：

1. 读取 SKILL.md 内容
2. 提取所有引用的路径（`./scripts/roll.sh`）
3. 通过 `canonicalize()` 解析为绝对路径
4. 对每个路径执行执法器检查
5. 仅通过白名单 + 执法器双重认证的路径才会被加入

**LLM 永远看不到文件系统。它只能看到 Peon 明确解锁的内容。**

---

## 🗺️ 路线图

| 状态 | 功能                                                                          |
| :--- | :---------------------------------------------------------------------------- |
| ✅   | 双层执法的零信任工具执行                                                      |
| ✅   | 自研 LLM 运行时（`peon-runtime`）— 支持 OpenAI、Anthropic、Gemini、OpenRouter |
| ✅   | Telegram 机器人，每用户身份隔离                                               |
| ✅   | 技能发现、动态白名单、会话重置                                                |
| 🔜   | **Telegram**: 富文本回应 — 图片、文件、格式化消息                             |
| 🔜   | **Telegram**: 多模态输入 — 照片、语音、文档                                   |
| 🔜   | **Discord**: 机器人集成                                                       |
| 🔜   | **CLI**: 端到端验证                                                           |
| 🗓️   | WASM 运行时支持，用于浏览器端 Agent                                           |
| 🗓️   | 跨会话持久化对话记忆                                                          |

---

## 🤝 支持的供应商

Peon 使用自研运行时（`peon-runtime`），原生支持：

**OpenAI** · **Anthropic** · **Gemini** · **OpenRouter**（接入 200+ 模型）

---

## 📄 开发者与贡献

<a href="https://github.com/stephen94125/peon-lib/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=stephen94125/peon-lib" alt="contrib.rocks" />
</a>

本贡献看板由 [contrib.rocks](https://contrib.rocks) 所产生。

授权协议：[MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)。
