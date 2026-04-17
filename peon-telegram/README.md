# Peon Telegram 

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

`peon-telegram` is a sub-crate that exposes the powerful Peon zero-trust engine directly to Telegram.

It listens for incoming chat messages and routes them through the native security sandbox before dispatching the AI's response securely back to the chat.

## 🚀 Quick Setup

To turn your isolated local agent into a live Telegram bot, follow these steps:

**1. Install Peon Telegram via Cargo**
```bash
cargo install peon-telegram
```

**2. Initialize your workspace**
Navigate to the directory where you want your bot to live, and run the initialization command. This safely creates a `.env` file, the `skills/` directory, and default 'Allow All' permission files.
```bash
peon-telegram --init
```

**3. Configure your Tokens**
1. Request a new Bot Token from [@BotFather](https://t.me/botfather) on Telegram.
2. Edit the newly generated `.env` file to include your tokens:
   ```dotenv
   DEFAULT_PROVIDER="openai" # Or gemini, anthropic...
   OPENAI_API_KEY="sk-..."
   
   # Add your Telegram Token here:
   TELOXIDE_TOKEN="123456789:ABCdefGHIjklmNoPQRsTuvwxyZ"
   ```

> [!TIP]
> Peon searches for skills in the **`./skills`** directory by default (not `./.skills`). Ensure your skills are placed there or set `PEON_SKILLS_DIR`.

> [!WARNING]
> Permission files (`file_permissions.txt` & `user_permissions.csv`) are actively enforced. Be sure to modify the auto-generated ones to restrict access for production!

**4. Run the bot!**
```bash
RUST_LOG=info peon-telegram
```

## 🔐 Isolation Strategy

For this first version, **all Telegram chats are 100% ephemeral and isolated**.

Every single message received spins up a completely fresh `PeonAgent`, resolving whitelist policies anew. This guarantees that if User A extracts a skill or authorizes a path, User B cannot indirectly trigger it.

*Long-term memory is currently disabled for security reasons until individual user context management is built.*

---

## 🗺️ Roadmap (Upcoming Features)

We are actively developing `peon-telegram` into a fully-fledged chat platform. Upcoming features include:

- [x] **Text Replies**: Basic MVP text responses and action dispatching.
- [ ] **Voice / Audio Processing**: Allow users to send voice notes, passing them to Whisper (or similar STT engines) before querying the Agent.
- [ ] **Vision & Document Analysis**: Submit pictures, PDFs, and data files via Telegram. Peon will automatically pipe these into the agent's context for summarization or debugging.
- [ ] **Interactive Keyboards (Inline Buttons)**: When the Agent detects a highly destructive command (e.g. dropping a database), it will output an Inline Button prompt `[Confirm Execution] [Cancel]` directly in the Telegram UI using Teloxide callbacks.
- [ ] **Persistent User Sessions**: Migrate from the ephemeral per-message initialization to a Casbin-backed session cache, allowing the agent to remember conversations on a per-User-ID basis securely.

---

## 💻 Contributing / Developers

If you want to build from source or contribute to the project:

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib

# Initialize workspace for testing
cargo run -p peon-telegram -- --init

# Run directly from source
RUST_LOG=info cargo run --release -p peon-telegram
```
