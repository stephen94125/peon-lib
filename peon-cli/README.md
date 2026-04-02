# Peon CLI

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

`peon-cli` is the interactive command-line interface for the zero-trust Peon Agent layer.

It bridges your terminal automation and scripting pipelines directly to the intelligence layer of the agent, guaranteeing absolute safety through `peon-core` sandboxing.

## 🚀 Installation & Build

Currently, the CLI is built directly from the source repository.
From the workspace root directory, run:

```bash
# Build the binary
cargo build --release -p peon-cli

# Optionally, move it to your system binaries for global usage
sudo cp target/release/peon-cli /usr/local/bin/peon
```

## 🧠 Model Configuration

Peon uses `.env` configuration (matching `peon-core`) by default.
Ensure you export API keys to your environment, or create a `.env` in the directory you execute from.

```bash
export OPENAI_API_KEY="sk-..."
export DEFAULT_PROVIDER="openai"
export DEFAULT_MODEL="gpt-4o"
```

## 💻 Usage

`peon-cli` was designed with Unix composability in mind. Internal initialization loading logs are written safely to `stderr`, and the final AI response is printed directly to `stdout`. 

This guarantees pipeline tools like `grep`, `jq`, or redirects like `> file.txt` will **not** be corrupted by Agent startup logs.

### 1. Direct Argument Mode

You can ask the agent single-shot queries utilizing the `-m` (`--message`) parameter.

```bash
peon-cli -m "Roll a 20-sided die."
# Output:
# You rolled a 15!
```

### 2. Unix Pipe Mode (Standard Input)

The most powerful way to use `peon-cli` is by pipelining it through traditional tools. It will seamlessly detect if it is attached to a standard input pipe and process the incoming stream as its prompt.

```bash
# Pipe from a file extraction
cat my_code.py | peon-cli -m "Find the bug in this Python file and output only the fixed code." > fixed_code.py

# Pipe from generic shell commands
git diff HEAD~1 | peon-cli -m "Write a conventional commit message for these changes."
```

If you do not provide any parameters or piped input, `peon-cli` will gracefully exit, preventing your terminal from inadvertently hanging.
