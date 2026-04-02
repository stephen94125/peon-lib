# Peon LINE Integration (`peon-line`)
[繁體中文介紹請見 README.zh-TW.md](./README.zh-TW.md)

The `peon-line` crate provides a fully-featured, asynchronous LINE Messaging API integration for the OpenFang Peon agent framework. Built on top of the `line-bot-sdk-rust` and `axum` web framework, it acts as a webhook receiver that bridges the LINE platform with the autonomous Peon Agent Builder.

## 🌟 Key Features

*   **Progressive UX Delivery**: Bypasses the standard 5-bubble batch limit by utilizing non-blocking `push_message` delivery during the LLM reasoning phase, and resolves the interaction gracefully with a concluding `reply_message`.
*   **LLM-Native Messaging Tools**: Exposes LINE's rich media formats as native Rust tools bound directly to the agent. The LLM can autonomously choose to send rich media via strictly-defined English prompting:
    *   🖼️ **Image Tool**: Direct image pushes instead of text URLs.
    *   📍 **Location Tool**: Interactive map cards and coordinates.
    *   🎦 **Video Tool**: Native MP4 playback within the LINE chat.
    *   🎵 **Audio Tool**: Direct M4A voice notes for the user to listen.
    *   😺 **Sticker Tool**: Send LINE stickers dynamically to enhance user affinity.
*   **Zero-Trust Compatible**: Integrates cleanly with Peon's core enforcers.

## 🚀 Setup & Execution

### 1. Environment Configuration

Create an `.env` file in the root of the `peon-line` crate with the following variables:

```dotenv
# LLM Provider Configuration
DEFAULT_PROVIDER=openai
DEFAULT_MODEL=gpt-5.4
#OPENAI_API_KEY=your_openai_key

# Peon Core Paths
PEON_SKILLS_DIR=.skills
PEON_FILE_PERMISSIONS=file_permissions.txt
PEON_USER_PERMISSIONS=user_permissions.csv

# LINE Configuration (REQUIRED)
LINE_CHANNEL_SECRET=your_channel_secret
LINE_CHANNEL_ACCESS_TOKEN=your_channel_access_token
```

### 2. Running the Webhook Server

```bash
cargo run -p peon-line
```

The server will start an Axum listener on `0.0.0.0:3000/callback`. You will likely need to expose this port using tools like `ngrok` or `Cloudflare Tunnels` to set up your LINE Developers Console Webhook URL.

## 🔜 Roadmap (TODOs)

*   [ ] **Flex Message Builder**: Implementing a dynamic JSON constructor to let the LLM generate highly complex UI layouts.
*   [ ] **Imagemap Messages**: Allowing interactive mapping and coordinates on large banner images.
*   [ ] **Coupon Messages**: Integrating LINE's native Coupon APIs.
*   ~~**Template Messages**~~: *Deprecated in favor of Flex Messages and standard Text + Quick Reply.*
