# Peon LINE 整合模組 (`peon-line`)
[English Documentation](./README.md)

`peon-line` 模組是 OpenFang Peon 代理人框架專屬的 LINE Messaging API 整合服務。它基於 `axum` 網路框架與官方的 `line-bot-sdk-rust` 打造，擔任 Webhook 接收端，將強大的自主 AI 決策引擎與 LINE 結合。

## 🌟 核心特色

*   **非同步漸進式推播 (UX 升級)**：打破了 LINE API 傳統上一次 `reply_message` 只能回覆 5 個泡泡的限制，且解決了使用者在等待 AI 思考時的空白期。在 LLM 思考期間，它隨時能透過 `push_message` 提前將計算好的資料或圖片傳給使用者，最後再用 `reply_message` 優雅收尾。
*   **為 LLM 專屬打造的 LINE 原生工具**：我們將 LINE 的多媒體格式全部封裝成了 `rig` Agent 的標準工具 (Tools)，並替它們加上了強而有力的英文行為提示詞。Agent 能自主決定何時該發送這些多媒體內容給使用者：
    *   🖼️ **圖片發送工具**：拒絕傳送枯燥的 URL，直接顯示真實圖片。
    *   📍 **地圖定位工具**：生成原生的地理座標卡片，一鍵開啟導航。
    *   🎦 **影片發送工具**：在聊天室內直接載入並播放 MP4 檔案。
    *   🎵 **語音發送工具**：產生聲音檔給使用者直接聆聽。
    *   😺 **貼圖發送工具**：隨時依據情感發送合適的 LINE 貼圖，大幅增加好感度。

## 🚀 設定與執行

### 1. 環境變數設定

請在 `peon-line` 的根目錄下建立一份 `.env` 檔案，內容至少包含以下設定：

```dotenv
# 模型供應商設定
DEFAULT_PROVIDER=openai
DEFAULT_MODEL=gpt-5.4
#OPENAI_API_KEY=your_openai_key

# Peon 核心路徑權限
PEON_SKILLS_DIR=.skills
PEON_FILE_PERMISSIONS=file_permissions.txt
PEON_USER_PERMISSIONS=user_permissions.csv

# LINE 開發者金鑰 (必須填寫)
LINE_CHANNEL_SECRET=你的_channel_secret
LINE_CHANNEL_ACCESS_TOKEN=你的_channel_access_token
```

### 2. 啟動 Webhook 伺服器

```bash
cargo run -p peon-line
```

伺服器會於 `0.0.0.0:3000/callback` 監聽 Webhook 請求。正式使用時，你需要透過 `ngrok` 或 `Cloudflare Tunnels` 等工具將它暴露給外網，並將網址填回 LINE Developers Console 之中。

## 🔜 未來發展 (TODOs)

*   [ ] **Flex Message 彈性訊息**：實作動態 JSON 產生器，讓大型語言模型能產出複雜且精美的 UI 圖文介面。
*   [ ] **Imagemap 圖片地圖**：提供可點擊指定區域座標的大型橫幅照片互動。
*   [ ] **Coupon 優惠券**：串接原生 LINE Coupon API。
*   ~~**Template Message 樣板訊息**~~：*因已被 Flex Message 與純文字 Quick Reply 所取代，設為棄用 (Deprecated) 狀態，暫不實作。*
