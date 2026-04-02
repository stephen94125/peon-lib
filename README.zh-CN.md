# Peon Framework

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

> **基于 Rust 构建的企业级、零信任 AI Agent 框架。**
> 
> Peon 提供了一个高度安全的沙箱，通过真正的操作系统与人员级别的权限校验，严格限制并把关 LLM 的能力边界。

## 🚀 我们的愿景

市面上大多数 AI 框架盲目赋予 LLM 直接执行 `bash` 命令的权力，而 Peon 则秉持严格的 **“在触摸它之前先证明你的权限 (prove it before you touch it)”** 原则。在 Peon 中，LLM 绝对无法通过“幻觉 (hallucinate)”访问您的系统：每当它尝试读取文件或执行脚本，底层都会预先交叉比对您设定的 RBAC/ABAC 权限模型。

通过将 AI 交互层与系统执行层彻底解耦，Peon 实现了真正的**纵深防御 (Defence in Depth)**。

## 📦 项目结构

这是一个 Cargo Workspace（工作区）项目，包含以下库：

- **[`peon-core`](peon-core/README.zh-CN.md)**: 零信任引擎的核心。包含了各类工具封装、扫描架构以及基于 Casbin 的访问权限执行器。**(如果您想直接使用本框架，请从这里开始！)**
- **[`peon-cli`](peon-cli/README.zh-CN.md)**: 通用的 CLI 入口，用于将终端命令行与管道数据流 (`echo "数据" | peon-cli`) 直接注入到 AI 智能核心中。
- *`peon-discord / peon-telegram` (计划中)*: 聊天社交平台接入支持。

## 🧠 模型提供商无关 (Provider Agnostic)

得益于底层设计，Peon 完美支持了当下几乎**所有主流 AI 模型**的无缝切换，且无需修改一行代码：

Anthropic · Azure · Cohere · Deepseek · Gemini · Groq · Huggingface · Hyperbolic · Llamafile · Mira · Mistral · Moonshot · Ollama · OpenAI · OpenRouter · Perplexity · Together · xAI

您只需要在 `.env` 环境变量中简单配置即可实现瞬间切换：
```bash
DEFAULT_PROVIDER=gemini DEFAULT_MODEL=gemini-3.0-flash
```

## 📄 许可协议 (License)

本项目采用双重许可，您可以自行选择 [MIT 许可协议](LICENSE-MIT) 或 [Apache 许可协议 2.0 版](LICENSE-APACHE) 进行使用。
