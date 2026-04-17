# Peon Telegram

[🇨🇳 简体中文](README.zh-CN.md) | [🇬🇧 English](README.md)

`peon-telegram` 是用于将强大的 Peon 零信任安全引擎直接接入 Telegram 的独立子程序。

它会在后台监听聊天信息，并在投递给 AI 前确保经过底层沙箱的安全过滤，最后将 AI 生成的安全指令或文字安全地传回聊天室中。

## 🚀 快速启动

想把你本机的纯净 Agent 转变成一个随时待命的 Telegram 机器人，只需以下几步：

**1. 透过 Cargo 安装 Peon Telegram**

```bash
cargo install peon-telegram
```

**2. 初始化工作区环境**
在你想运行机器人的目录下输入初始化指令，系统会自动帮你建立干净的 `.env` 设定档、`skills/` 技能目录，以及全开预设的安全权限表。

```bash
peon-telegram --init
```

**3. 配置密钥与 Token**

1. 在 Telegram 上找到 [@BotFather](https://t.me/botfather) 并申请一个新的 Bot Token。
2. 打开刚刚自动生成的 `.env` 档案，填上你的 Token：

   ```dotenv
   PROVIDER="openai" # 或者 gemini, anthropic...
   API_KEY="sk-..."
   MODEL="gpt-4o"

   # 把你在 Telegram 申请到的 Token 贴在这里：
   TELOXIDE_TOKEN="123456789:ABCdefGHIjklmNoPQRsTuvwxyZ"
   ```

> [!TIP]
> 系統預設會從 **`./skills`** 目錄（而非 `./.skills`）讀取技能資訊。請確保您的技能資料夾路徑正確，或透過環境變數指定。

> [!WARNING]
> 机器人高度仰赖目录下的 `file_permissions.txt` 与 `user_permissions.csv` 才能执行 `--init` 预设提供的是无防护的允许所有设定，**请务必在正式上线前修饰与收紧！**

**4. 运行！**

```bash
RUST_LOG=info peon-telegram
```

## 🔐 严格的会话隔离战略

在目前的基础框架中，**所有的 Telegram 对话请求都是 100% 用完即弃且相互物理隔离的**。

我们对于每一条收到的新消息，都会在后台生成一个崭新的 `PeonAgent` 并重新载入 Casbin 白名单。这意味着即使“用户 A”解锁了能够开启某个高危技能的路径权限，“用户 B”也绝对无法通过接力触发该行径。

### 给 Telegram 用户分配具体权限

预设的 `--init` 指令会生成宽松的政策 (`p, *, *, *, allow`) 允许所有人生效。实际上正式上线后，你会希望限制只有你自己或团队成员能使用。

正如上方的执行记录，你可以从后台观察当使用者发讯息过来时所跳出的 Log，来获取该使用者专属的 Telegram UID (Chat ID)：

```log
[INFO  peon_telegram] Received message from chat ID 6649983588: 幫我丟一個128面的
```

拿到这个 `6649983588` 的唯一标识 UID 后，你就能轻易地进入 `user_permissions.csv` 为他量身打造专属规则啦！示范如下：

```csv
# user_permissions.csv

# 1. 强制 Deny 掉未知的散客（预设的 default fall-back）
p, *, *, *, deny

# 2. 把刚刚从控制台获得的 Telegram UID (6649983588) 赋予 'admin_role' 管理员角色
g, 6649983588, admin_role

# 3. 让所有属于 admin_role 的使用者拥有最高开火权限
p, admin_role, *, *, allow
```

_出于最严苛的安全考量，直到我们把「基于 User-ID 切分的缓存状态树」开发完毕前，Agent 会被禁止拥有上下文记忆。_

---

## 🗺️ 路线规划图 (Roadmap)

我们正积极地把 `peon-telegram` 从文字聊天机器人，推演成一个真正的多模态命令交互终端。即将到来的功能包含：

- [x] **基础文字回复**: MVP 版文字响应与纯文本下的技能触发判定。
- [ ] **语音/音频处理**: 允许用户直接在手机端录制语音，我们将会在接入 LLM 前利用 Whisper (或其他类似模型) 对其进行高效的转录解析。
- [ ] **图像视觉与文件分析**: 支持直接传输照片、PDF 和工程代码文件给机器人。Peon 将直接在沙箱内获取这些文件，作为 Agent 的环境上下文进行答疑与侦错。
- [ ] **互动键盘回调 (Inline Buttons)**: 当 Agent 判断下一步将涉及危险度极高的动作时（例如删库），除了 Casbin 鉴权外，将直接利用 Teloxide 的 Inline Keyboard 向用户的手机端扔出含有 `[安全放行] [立即拦截]` 按钮的操作卡片。
- [ ] **跨对话状态长驻**: 在确立更深度的安全隔离基准后，利用系统自带的 Casbin 体系为不同用户的对话窗口保留上下文进度与历史对答。

---

## 💻 开发者与源码贡献 (Contributing)

若您是开发者，或是想直接由源码进行编译与贡献：

```bash
git clone https://github.com/stephen94125/peon-lib.git
cd peon-lib

# 初始化测试工作区
cargo run -p peon-telegram -- --init

# 直接从源码运行
RUST_LOG=info cargo run --release -p peon-telegram
```
