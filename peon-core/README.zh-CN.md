# Peon Core

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

**Peon Core** 是驱动整个 Peon 框架的零信任引擎。它在架构层面确保了任何通过本框架初始化的 AI Agent 都没有物理能力擅自访问或执行未经明确授权的脚本与文件。

如果您想深入了解我们独创的“双层沙箱 (Dual-Layer Sandboxing)”和“动态白名单系统”背后的硬核原理解析，强烈推荐阅读我们的 [安全架构深度指南 (英文)](docs/security_architecture.md)。

---

## 🚀 快速起步 (Quick Start)

通过 Peon 创建一个极度安全的 AI Agent 只需要短短几行 Rust 代码：

```rust
use peon_core::agent::PeonAgent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 自动初始化权限拦截器与底层引擎，建立 Agent
    let agent = PeonAgent::new().await?;

    // 2. 与 Agent 展开对话
    let response = agent.prompt("请帮我掷一个 20 面的骰子！").await?;
    println!("Agent 回答: {}", response);

    Ok(())
}
```

### 立即跑跑看！

我们在 `examples/` 目录中已经为您准备好了开箱即用的范例程序。

1. 确保您已经在根目录配置好了 `.env` 的模型连线信息（例如：`DEFAULT_PROVIDER=openai` 以及对应的 `OPENAI_API_KEY=...`）。
2. 在您的工作区根目录执行以下指令：

```bash
# 基本的 Log 输出模式
cargo run -p peon-core --example simple_agent

# 深入模式：观察内部详细的白名单扫描、功能发掘与权限解锁过程
RUST_LOG=debug cargo run -p peon-core --example simple_agent
```

---

## 🔐 进阶权限管理 (Casbin)

Peon Core 的安全并不是写死在代码里的；我们集成了极其强大的工业级鉴权库：[Casbin](https://casbin.org/)。借助 Casbin，您可以随心所欲地配置复杂的 RBAC（基于角色的权限控制）或 ABAC（基于属性的权限控制）策略。

想更深入了解复杂策略的编写方法，强烈建议您参阅 [Casbin 官方语法说明](https://casbin.org/docs/how-it-works)。

> [!WARNING]
> **严格安控注意**: Peon 预设会从当前执行目录 (`./`) 读取权限设定。若找不到文件，系统将**直接 Panic 终止运行**。
>
> 您可以通过以下环境变数自定义位置：
> - `PEON_FILE_PERMISSIONS`: 物理文件权限路径。
> - `PEON_USER_PERMISSIONS`: 人员角色权限路径。

在使用 Agent 之前，您可以通过以下两个主要维度来编排您的安全网：

### 1. 物理文件与路径权限 (`file_permissions.txt`)

物理文件权限是一张针对 LLM 的**全局黑名单**，决定了在作业系统级别，Agent 是否有物理资格读取或执行某些路径的文件。

**环境设定:**
首先，将我们提供的范例档复制一份为正式的策略档：

```bash
cp file_permissions_example.txt file_permissions.txt
```

**语法规则:**
引擎在这里采用的是 **“拒绝优先 (Deny-Override)”** 逻辑。字段排序为： `操作类型 (action), 目标路径 (target_path)`。

- `r`: 允许读取 (Read)
- `x`: 允许执行 (Execute)
- `!r` / `!x`: **显式拒绝 (Deny)** 访问（拥有最高优先级！）

```text
# 显式允许 Agent 执行 skills 目录底下的所有脚本
# (请注意：系統預設讀取的是 ./skills 而非 ./.skills)
x, ./skills/*

# 无论如何，死锁核心机密文件，Agent 绝对无法读取
!r, ./secret_passwords.txt
```

### 2. 人员与角色权限 (`user_permissions.csv`)

人员权限控制着*是谁*正在和 Agent 交互，并根据用户的 ID 或角色赋予不同层级的 LLM 技能使用资格。

**环境设定:**
Peon Core 会在您的项目根目录原生读取 `user_permissions.csv` 文件来进行人员配置。

**语法规则:**
人员权限使用的是原汁原味的 Casbin 五元组核心语法： `p, 用户/角色, 模块/资源, 动作, 许可效果`。

```csv
# 1. 默认降级包容方案：允许所有人去读取和执行一切已解锁的技能
p, *, *, *, allow

# 2. 拉黑用户 "Bob"，不仅不能执行脚本，甚至一动也不能动
p, Bob, *, execute, deny

# 3. 配置用户组/角色设定 ('g' function)
# 禁止拥有 "standard_users" 角色的用户触碰根目录 /root 下的任何文件
p, standard_users, /root/*, *, deny

# 4. 将 Bob 加入到 "standard_users" 角色中 (从而继承该角色的拒绝策略)
g, Bob, standard_users
```

---

## 🛠 过程追溯与日志 (Logging)

由于 Peon 把底层安全性完全抽象掉了，通过日志（Log）来回溯 Agent 的一举一动与拦截情况是非常必要的。可通过更改环境变量 `RUST_LOG` 的值来控制：

- `RUST_LOG=info` - 日常信息，包含 Agent 回复文本以及工具发起的高层级请求。
- `RUST_LOG=debug` - 深入底层的详尽调试模式，可查看正则表达式的工作状况、Casbin 的判定依据以及每一条刚被加入白名单的物理路径。

---

## 📄 许可协议 (License)

本项目采用双重许可，您可以自行选择 [MIT 许可协议](../LICENSE-MIT) 或 [Apache 许可协议 2.0 版](../LICENSE-APACHE) 进行使用。
