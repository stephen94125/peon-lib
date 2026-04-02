# Peon CLI

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

`peon-cli` 是专为零信任 Peon Agent 核心层设计的交互式命令行工具。

它将您的终端自动化与脚本串联工作流直接对接到 Agent 的智能层，并通过 `peon-core` 底层的物理沙箱确保操作的绝对安全。

## 🚀 安装与编译

目前，CLI 需要从仓库源码直接编译构建。
请在工作区根目录下运行：

```bash
# 构建二进制文件
cargo build --release -p peon-cli

# (可选) 将其移动到系统目录以便全局访问
sudo cp target/release/peon-cli /usr/local/bin/peon
```

## 🧠 模型配置

Peon 默认采用与 `peon-core` 一致的 `.env` 环境变量配置系统。
请确保您将所使用模型的 API 密钥导入到环境变量中，或直接在执行命令的目录下创建一个 `.env` 文件。

```bash
export OPENAI_API_KEY="sk-..."
export DEFAULT_PROVIDER="openai"
export DEFAULT_MODEL="gpt-4o"
```

## 💻 基础用法

`peon-cli` 在设计初衷上完全遵循了 Unix 哲学中的“高可组合性 (Composability)”。我们已经将所有底层的引擎初始化与安全扫描日志隔离到 `stderr` 错误流输出中；这意味着最终纯净的 AI 文本回答将**唯一**被投递到 `stdout` 标准流。

这种保护机制保证了您可以随意使用如 `grep`, `jq` 等工具过滤它的输出，或直接通过 `>` 重定向到文件，而不用担心被 Agent 启动时的日志污染到目标数据。

### 1. 命令行参数模式

您可以通过参数 `-m`（或 `--message`）一次性发出请求：

```bash
peon-cli -m "帮我掷一个 20 面的骰子"
# 输出结果:
# 你掷出了 15！
```

### 2. Unix 管道模式 (多程序串联)

`peon-cli` 最强大的用法就是像传统工具一样，通过管道 (Pipe) 在不同程序之间串联信息。它能非常智能地探测自己是否接入了管道，并会自动将流入的数据块转换成对话上下文。

```bash
# 将文本文档直接注入给 AI 进行改写
cat my_code.py | peon-cli -m "帮我找出这段 Python 代码的 Bug，并且在输出中只返回修改好的代码。" > fixed_code.py

# 将 Shell 命令的结果抛给 AI 总结
git diff HEAD~1 | peon-cli -m "阅读这些代码的改动，并写出一版符合规范的提交信息 (Commit Message)。"
```

为了防止终端不必要的长时间假死挂起，一旦程序侦测到您没有输入任何的 `-m` 指令且未连结有效数据管道输入时，`peon-cli` 将立刻抛出错误并安全退出。
