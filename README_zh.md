<div align="center">

# `Peon`

![Static Badge](https://img.shields.io/badge/mission-zero_trust_AI_agent_workspaces-purple)
<br />
![GitHub top language](https://img.shields.io/github/languages/top/stephen94125/peon-lib)
![GitHub last commit](https://img.shields.io/github/last-commit/stephen94125/peon-lib)
[![License](https://img.shields.io/badge/License-MIT%20&%20Apache--2.0-green.svg)](https://opensource.org/licenses/MIT)

<div align="center">
<h4><code>Peon</code> 是一个企业级、基于零信任架构打造的自主 AI Agent 框架。</h4>
</div>

<p align="center">
  <a href="README.md">English</a> ·
  <strong>中文</strong>
</p>

</div>

[最新更新](#最新更新) •
[背景与缘起](#背景与缘起) •
[核心哲学](#核心哲学) •
[安装说明](#安装说明) •
[开发模块一览](#开发模块一览) •
[使用与环境设定](#使用与环境设定) •
[系统权限与安全模型](#系统权限与安全模型) •
[开发者与贡献](#开发者与贡献)

---

## 背景与缘起

自从 Autonomous Agents（自主代理）的浪潮崛起后，我们看到了**数量极为庞大**的 AI 框架（如 LangChain 或 AutoGPT）诞生，但它们往往是“盲目地”将操作系统的 Terminal 权限直接交给 LLM 去执行。

这个技术演进无疑是激动人心的，但是 _直接给予 AI 毫无节制的 `bash` 执行能力，将严重限制它被部署到高敏感度的企业服务器环境中。_

<div align="center">
<h4>换句话说，AI Agent 不只是面临“智力”层面的天花板，它更面临了<em>“信任与资安”</em>层面的巨大挑战。</h4>
</div>

**Peon 正是为了解决这个痛点而生。它首创将真实的 RBAC / ABAC (基于角色与属性) 的动态权限管理系统，最为深刻地整合进了 AI 的工具调用工作流 (Tool Execution Loop) 之中！**

Peon 通过完全解耦“大脑推理层 (Reasoning Layer)”与“系统作业层 (Execution Layer)”实现了真正的 **纵深防御 (Defence in Depth)**。在 Peon 底下，LLM 绝对无法捏造一个未授权的 `rm -rf /` 指令。每一个工具的调用、每一次的文件读写、或者是脚本的运行，都必须经过极度严苛、立基于 Casbin 机制的安全矩阵进行核实。

## 最新更新

若想要进行深度探究，请前往各模块内部专属的文件夹查看更深度的细节（例如 `peon-core/README.md`）。

## 导览列

- [`Peon`](#peon)
  - [背景与缘起](#背景与缘起)
  - [最新更新](#最新更新)
  - [核心哲学](#核心哲学)
    - [“先证明，再触碰”法则](#先证明再触碰法则-prove-it-before-you-touch-it)
  - [安装说明](#安装说明)
  - [使用与环境设定](#使用与环境设定)
    - [Peon CLI (终端机介面)](#peon-cli)
    - [Peon Telegram (TG 机器人)](#peon-telegram)
    - [Peon LINE (LINE 原生推送)](#peon-line)
  - [我们的技能设计 (Skills)](#我们的技能设计-skills)
  - [系统权限与安全模型](#系统权限与安全模型)
  - [深度支援的 AI 模型厂商](#深度支援的-ai-模型厂商)

## 核心哲学

> 若缺乏了有效的控制限制，智慧并不叫自动化；它叫做一场灾难。

我们深信 AI 的目的是拿来加速快速验证以及执行任务，但当我们跨入企业级的 AI 开发时，所有事情的出发点都必须是建立在代理人行为的 **安全性与可稽核性 (Auditability)** 之上。

### 先证明，再触碰法则 (Prove it before you touch it)

我们的框架将原本的指令层强硬分为两条绝不重疊的水管：

1. **大脑系统 (`rig` / LLM 供应商)**: 最终下定论：“请替我在目标 Y 上执行 X 脚本”。
2. **执法系统 (Casbin / PeonCore 核心)**: 拦截所有请求。用发话者的身份（Identity）去与文件系统中的“明确设定特权”做交互比对，计算并给出 `允许 (Allow)` 或 `拦截 (Deny)` 的结果。

唯有两个系统完美的对齐一致，底层的内容与脚本才会真正的生效触发。

## 安装说明

### 环境准备

欲由开源代码直接建置 Peon，请[确保你已经安装了 Rust 以及 Cargo 工具链](https://rustup.rs/)。

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 从源码开始编译

下载整个工作区 (Workspace) 并编译项目：

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib
cargo build --release
```

编译出来的执行文件将都会顺利打包在 `./target/release/` 文件夹内。

## 环境变数设定

Peon 生态系被设计为以 Cargo Workspace 的方式管理。也因为每个子系统都具备不同的属性，**所以每一个单独的客户端项目都必须要独立设定一份专属的 `.env`。**

你只需要进入想执行的模块文件夹内复制范例文件即可：

```bash
cd peon-cli
cp .env.example .env
```

基础的配置参数长这样：

```dotenv
# 模型与供应商
PROVIDER=openai
DEFAULT_MODEL=gpt-4o
# OPENAI_API_KEY=sk-...

# 核心安全策略引擎存放路径（必须为相对路径）
PEON_SKILLS_DIR=skills
PEON_FILE_PERMISSIONS_PATH=file_permissions.txt
PEON_USER_PERMISSIONS_PATH=user_permissions.csv
```

## 开发模块一览

Peon 被妥善的隔离出好几块不同的乐高积木模块，你可以根据自身的需求独立启动。

| 模块名称            | 重心说明                                                                   |
| :------------------ | :------------------------------------------------------------------------- |
| **`peon-core`**     | 核心安全引擎，模型供应商底层串接，API 设计                                 |
| **`peon-cli`**      | 主攻标准 Unix I/O 流，适合搭配 `cat` 以及 CI/CD 管线测试                   |
| **`peon-telegram`** | Long-polling 服务器，适合做多人群组助理                                    |
| **`peon-line`**     | 整合了 Axum 的服务器接口，专注于手机端的原生地图、视频与图片等丰富介面推送 |

## 使用与环境设定

当你欲使用的那个根目录设定好专属你的 `.env` 以后，启动就变成极其简单的事。

### Peon CLI

`peon-cli` 用起来就像一个好用的标准 GNU 指令工具，你可以通过旗标 (`-m`) 与标准输入 (`stdin`) 喂给它背景知识。

```bash
# 基础问答
cargo run -p peon-cli -- -m "如何在 linux 内做一个基础网络路由排错？"

# 将大段文档做 Pipeline 给到模型分析 (stdin)
cat /var/log/syslog | cargo run -p peon-cli -- -m "替我解析这串日志内部有没有因为内存造成的崩溃问题。"
```

当你指定 `RUST_LOG=debug` 时，你可以清楚看见这个零信任防护网，是如何一层一层解析路径、调度判断并批准指令的！

### Peon Telegram

想要在私信或是群组拥有一个完全原生的 TG Assistant？
打开 `peon-telegram/.env`，输入你的 BotFather Token：

```dotenv
TELOXIDE_TOKEN="123456789:ABCdefGHIjklmNoPQRsTuvwxyZ"
```

然后发动引擎服务器：

```bash
cargo run -p peon-telegram
```

### Peon LINE

专门设计给终端用户、最高细节质量的 LINE 官方机器人引擎。采用 `axum` 动态绑定 Webhook。突破了官方恼人的 `reply_message` 全盘批次送出限制，给予模型自己决定的“渐进式”异步回应。让 AI 能亲自对用户传送地址定位点、语音录音与庆祝贴图。

打开 `peon-line/.env`，放入金钥：

```dotenv
LINE_CHANNEL_SECRET=your_secret
LINE_CHANNEL_ACCESS_TOKEN=your_token
```

```bash
cargo run -p peon-line
```

机器人预设会监听 `0.0.0.0:3000/callback`，测试时可以搭配 `ngrok` 作使用。

## 我们的技能设计 (Skills)

Peon 的 _技能 (Skills)_ 系统截然不同于一般项目简单敷衍的 function call 打包。

我们采用在 `skills/SKILL.md` 内部撰写类 `Markdown/XML` 的方式（请注意：系統預設讀取的是 **`./skills`** 而非 `./.skills`），用纯粹的简体中文或英文来叙做工具描述（Prompt 指令），大幅增加 LLM 的理解能力。

架构举例如下：

```markdown
---
name: network_scanner
description: Executes Nmap on a target subnet.
---

当使用者发出要求需要检查网络区段的请求，请一定要调用这个工具与底下的核心 `scan.sh` 来分析。记得加上过滤层的参数 `--safe`。
```

这其中所有的复杂转换皆会被 Peon 给隐藏。框架会在服务刚建立的时候扫描这个环境，将安全网织好，动态赋予 LLM 新的特权，甚至连服务器都不需要重开！

## 系统权限与安全模型

若文件权限或身分角色没有被事先记录在 Casbin 设定表中，任何 LLM 所发起的行为都将遭无情封锁。

> [!WARNING]
> **强硬零信任机制**: Peon 预设会在当前执行目录 (`./`) 下寻找 `file_permissions.txt` 与 `user_permissions.csv`。若找不到这些文件，Agent 将会**直接 Panic 拒绝启动**，以确保系统不会在无权限控制的情况下裸奔。
>
> 您可以通过以下环境变数来手动指定这些文件的路径：
>
> - `PEON_FILE_PERMISSIONS_PATH`: 物理路径权限表位置。
> - `PEON_USER_PERMISSIONS_PATH`: 身分角色权限表位置。

**1. `file_permissions.txt`** (黑白名单防护机制)

```text
# 允许模型执行任何包裹在 skills 里面的可执行文件
# (系統預設會從 ./skills/* 讀取權限)
x, ./skills/*
# 以最高顺位强硬阻止触碰一切有关 rm 的危险行为
!x, /bin/rm
```

**2. `user_permissions.csv`** (身份管理与分配)

```csv
# 把原生的 'agent' 身份丢给名为 system_admin 的虚拟角色
g, agent, system_admin
# 作为系统管理员，拥有无节制的行为判定能力
p, system_admin, *, *, allow
```

Peon 的底层核心将会默默地将每一次的方法与系统接触拦截至此，作比對後放行。

## 深度支援的 AI 模型厂商

所有的多模型分发介面都已经由 Peon 替各位妥善处理，使用者在绝大多数时间内都只需要更改 `PROVIDER` 取代模型。包含下列各大最夯的厂商：

Anthropic · Azure · Cohere · Deepseek · Gemini · Groq · Huggingface · Hyperbolic · Llamafile · Mira · Mistral · Moonshot · Ollama · OpenAI · OpenRouter · Perplexity · Together · xAI

## 开发者与贡献

<a href="https://github.com/stephen94125/peon-lib/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=stephen94125/peon-lib" alt="contrib.rocks" />
</a>

本贡献看板由 [contrib.rocks](https://contrib.rocks) 所产生。
