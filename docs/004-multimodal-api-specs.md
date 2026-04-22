# 004-multimodal-api-specs.md

## [English Version]

### Overview
As of 2026, there is a fundamental logical distinction between "Multimodal (Image, Audio, Video)" inputs and outputs across major LLM providers. 
- **Input:** Media encoding can generally be embedded directly within chat requests.
- **Output:** Except for audio, images and videos typically require independent generative models or Tool Calls (Skills).

---

### 1. OpenAI API (GPT-4o / GPT-5.4 Series)
#### Input
- **Image:** ✅ Supported via Base64 encoding or public URLs in Chat Completions.
- **Audio:** ✅ Supported. GPT-4o and Realtime API natively understand audio, including tone and background noise.
- **Video:** ⚠️ Not natively supported as a single file. Developers must perform **Frame Extraction** locally (e.g., in Rust) and send frames as an array of Base64 images alongside the audio file.

#### Output
- **Image:** ❌ No direct output. Requires a separate call to the **Images API (DALL-E series)**.
- **Audio:** ✅ Supported. Chat Completions can return synthesized audio directly.
- **Video:** ❌ No direct output. Requires independent endpoints like **Sora**.

---

### 2. Gemini API (Gemini 1.5 Pro/Flash & Newer)
Gemini offers the most comprehensive native support for multimodal files, especially with its massive context window.
#### Input
- **Image:** ✅ Supported via Base64 inline or Google AI File API.
- **Audio:** ✅ Supported. Can process up to 10+ hours of audio natively.
- **Video:** ✅ **Fully Supported.** Unlike OpenAI, Gemini accepts direct video uploads (File API/Cloud Storage). It analyzes video tracks and audio natively without manual frame extraction.

#### Output
- **Image:** ❌ No direct output. Requires **Imagen** models.
- **Audio:** ✅ Supported. Access to high-fidelity audio/music generation via **Lyria 3**.
- **Video:** ❌ No direct output. Requires specialized endpoints like **Veo 3.1**.

---

### 3. OpenRouter API (Unified Routing Layer)
OpenRouter standardizes the multimodal interface across different underlying providers.
#### Input
- ✅ Full support for Base64 and URLs. Uses a standardized payload format: `{"type": "image_url" | "audio" | "video"}`.
- **Advantage:** OpenRouter handles the abstraction. Your Rust code maintains one format, while OpenRouter routes it correctly to Gemini or OpenAI.

#### Output
- Recently added support for **Audio Responses** for compatible models.
- Image and Video outputs still require calls to specific generative model endpoints.

---

### Architecture Recommendations for Peon Agent
Integrating with your `peon-core` (Zero-trust runtime) and Telegram Bot:

1. **User Input Handling (Telegram -> LLM):**
   - Use `teloxide` to download `msg.photo()` or `msg.voice()` into memory as `Bytes`.
   - Convert to Base64 and wrap into the OpenRouter message format.

2. **Model Output Handling (LLM -> Telegram) — Use Tool Calling:**
   - **Avoid** streaming raw Base64 media in text (unstable and token-expensive).
   - Define a Tool/Skill: `generate_image(prompt: String)`.
   - **Flow:**
     - User asks: "Draw a camping van."
     - LLM triggers: `generate_image({"prompt": "VW T5 Camper..."})`.
     - **Peon Runtime** intercepts the Tool Call, executes the request via DALL-E/Imagen API.
     - Telegram Bot sends the resulting photo: `bot.send_photo(chat_id, bytes)`.
     - Return "Success" status to the LLM to continue the conversation.

---

## [中文版]

### 概述
截至 2026 年，主流 LLM 供應商對於「多模態（圖片、音訊、影片）」的輸入與輸出，在邏輯上有很大的區別：
- **輸入端 (Input):** 影音編碼通常可以直接嵌入聊天請求中。
- **輸出端 (Output):** 除了音訊外，圖片與影片通常需要依賴獨立的生成模型或 Tool Call (Skill)。

---

### 1. OpenAI API (GPT-4o / GPT-5.4 系列)
#### 輸入 (Input)
- **圖片 (Image):** ✅ 支援。可在 Chat Completions 中直接傳遞 Base64 或 URL。
- **音訊 (Audio):** ✅ 支援。GPT-4o 與 Realtime API 原生支援音訊輸入，能理解語氣與背景音。
- **影片 (Video):** ⚠️ 不直接支援單一檔案。需在本地端（如 Rust）進行「**抽幀 (Frame extraction)**」，將影片轉為多張圖片 Base64 陣列與音訊檔一同送入。

#### 輸出 (Output)
- **圖片 (Image):** ❌ 不直接輸出。需呼叫獨立的 **Images API (DALL-E 系列)**。
- **音訊 (Audio):** ✅ 支援。Chat Completions 可直接回傳合成好的音訊。
- **影片 (Video):** ❌ 不支援。需透過 **Sora** 等獨立端點生成。

---

### 2. Gemini API (Gemini 1.5 Pro/Flash 及更新版)
Gemini 在原生多模態檔案支援度上最為完整，擁有超長上下文優勢。
#### 輸入 (Input)
- **圖片 (Image):** ✅ 支援。可透過 Base64 或 File API 傳入。
- **音訊 (Audio):** ✅ 支援。能分析長達十幾小時的音訊檔。
- **影片 (Video):** ✅ **完全支援。** 勝過 OpenAI；可直接透過 File API 上傳影片，模型會原生分析影音軌，不需手動抽幀。

#### 輸出 (Output)
- **圖片 (Image):** ❌ 不直接輸出。需呼叫 **Imagen** 模型。
- **音訊 (Audio):** ✅ 支援。可調用 **Lyria 3** 等高音質音樂/音軌生成模型。
- **影片 (Video):** ❌ 不直接輸出。需調用 **Veo 3.1** 等獨立端點。

---

### 3. OpenRouter API (統一路由層)
OpenRouter 負責抹平各家供應商的介面差異。
#### 輸入 (Input)
- ✅ 全部支援 Base64 或 URL。統一使用 `{"type": "image_url" | "audio" | "video"}` 格式。
- **優勢:** 你的 Rust 程式碼只需維護一種傳輸格式，由 OpenRouter 負責將影片轉發給 Gemini 或處理其他模型的相容性。

#### 輸出 (Output)
- 近期開始支援具備語音能力模型的「**音訊回覆 (Audio responses)**」。
- 圖片與影片輸出依然需要呼叫特定的生成模型端點。

---

### 給 Peon Agent 的架構實作建議
結合目前的 `peon-core` 權限隔離架構與 Telegram Bot：

1. **使用者輸入處理 (Telegram -> LLM):**
   - 當 Telegram 收到圖片或語音，使用 `teloxide` 將檔案下載為記憶體中的 `Bytes`。
   - 在 Rust 中轉為 Base64，並封裝進 OpenRouter 的 Message 格式發送。

2. **模型輸出處理 (LLM -> Telegram) — 強烈建議使用 Tool Calling:**
   - **避免** 讓 LLM 在文字流中吐出媒體編碼（這會消耗大量 Token 且不穩定）。
   - 定義一個 Skill，例如：`generate_image(prompt: String)`。
   - **實作流程:**
     - 使用者要求：「畫一張露營車的圖」。
     - LLM 決定調用 `generate_image({"prompt": "福斯 T5 露營車..."})`。
     - **Peon Runtime** 攔截此 Tool Call，由 Rust 呼叫 DALL-E 或 Imagen API。
     - Telegram Bot 傳送結果：`bot.send_photo(chat_id, bytes)`。
     - 將「發送成功」的結果回傳給 LLM 繼續對話。
