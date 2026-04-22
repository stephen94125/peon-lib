### 📥 Incoming Message Types (Received from Telegram)

**Media, Files & Content**
- [x] `text` : Contains a pure text message.
- [ ] `animation` : Contains an animation (usually a GIF or an H.264/MPEG-4 video without sound).
- [x] `audio` : Contains an audio file (e.g., MP3) treated as a music track. → `ContentPart::Audio`
- [x] `document` : text/*, JSON, CSV → inlined as text; PDF → `ContentPart::File`; others → rejected with user-friendly error.
- [x] `photo` : Largest size downloaded → `ContentPart::ImageBase64 (image/jpeg)`
- [ ] `sticker` : Silently ignored.
- [ ] `story` : Silently ignored.
- [x] `video` : Downloaded → `ContentPart::VideoBase64`
- [ ] `video_note` : Silently ignored.
- [x] `voice` : OGG/Opus downloaded → `ContentPart::Audio (format: ogg)`
- [ ] `contact` : Silently ignored.
- [ ] `dice` : Silently ignored.
- [ ] `game` : Silently ignored.
- [ ] `poll` : Silently ignored.
- [ ] `venue` : Silently ignored.
- [x] `location` : Lat/Lon formatted as Google Maps URL → `ContentPart::Text`
- [ ] `invoice` : Silently ignored.
- [ ] `successful_payment` : Silently ignored.
- [ ] `web_app_data` : Silently ignored.
- [ ] `passport_data` : Silently ignored.

**Chat & Member Events**
- [ ] `new_chat_members` : Silently ignored.
- [ ] `left_chat_member` : Silently ignored.
- [ ] `new_chat_title` : Silently ignored.
- [ ] `new_chat_photo` : Silently ignored.
- [ ] `delete_chat_photo` : Silently ignored.
- [ ] `group_chat_created` : Silently ignored.
- [ ] `supergroup_chat_created` : Silently ignored.
- [ ] `channel_chat_created` : Silently ignored.
- [ ] `message_auto_delete_timer_changed` : Silently ignored.
- [ ] `migrate_to_chat_id` : Silently ignored.
- [ ] `migrate_from_chat_id` : Silently ignored.
- [ ] `pinned_message` : Silently ignored.
- [ ] `users_shared` : Silently ignored.
- [ ] `chat_shared` : Silently ignored.
- [ ] `connected_website` : Silently ignored.
- [ ] `write_access_allowed` : Silently ignored.
- [ ] `proximity_alert_triggered` : Silently ignored.
- [ ] `boost_added` : Silently ignored.
- [ ] `chat_background_set` : Silently ignored.

**Forum / Topic Events**
- [ ] `forum_topic_created` : Silently ignored.
- [ ] `forum_topic_edited` : Silently ignored.
- [ ] `forum_topic_closed` : Silently ignored.
- [ ] `forum_topic_reopened` : Silently ignored.
- [ ] `general_forum_topic_hidden` : Silently ignored.
- [ ] `general_forum_topic_unhidden` : Silently ignored.

**Giveaway Events**
- [ ] `giveaway_created` : Silently ignored.
- [ ] `giveaway` : Silently ignored.
- [ ] `giveaway_winners` : Silently ignored.
- [ ] `giveaway_completed` : Silently ignored.

**Voice/Video Chat Events**
- [ ] `video_chat_scheduled` : Silently ignored.
- [ ] `video_chat_started` : Silently ignored.
- [ ] `video_chat_ended` : Silently ignored.
- [ ] `video_chat_participants_invited` : Silently ignored.

---

### 📤 Outgoing Actions & Methods (Sent by Bot)

**Content Sending Methods**
- [x] `sendMessage` : Sends a standard text message. (default response path)
- [ ] `sendPhoto` : Not yet implemented as an LLM tool.
- [ ] `sendAudio` : Not yet implemented as an LLM tool.
- [x] `sendDocument` : Implemented via `send_csv` tool (LLM provides JSON rows → serialized to CSV → sent as attachment).
- [ ] `sendVideo` : Not yet implemented as an LLM tool.
- [ ] `sendAnimation` : Not yet implemented as an LLM tool.
- [x] `sendVoice` : Implemented via `send_voice` tool (LLM provides base64 audio).
- [ ] `sendVideoNote` : Not yet implemented.
- [ ] `sendMediaGroup` : Not yet implemented.
- [ ] `sendLocation` : Not yet implemented as an LLM tool.
- [ ] `sendVenue` : Not yet implemented.
- [ ] `sendContact` : Not yet implemented.
- [ ] `sendPoll` : Not yet implemented.
- [ ] `sendDice` : Not yet implemented.
- [x] `sendChatAction` : Implemented via `send_chat_action` tool (non-blocking; LLM specifies action string).
- [ ] `sendSticker` : Not yet implemented.
- [ ] `sendInvoice` : Not yet implemented.
- [ ] `sendGame` : Not yet implemented.

**Interactive UI Interfaces (Modifiers attached to sending methods)**
- [x] `InlineKeyboardMarkup` : Implemented via `send_inline_keyboard` tool. LLM specifies message + 2D button array with `callback_data` strings. Blocking (waits for Telegram API confirmation).
- [ ] `ReplyKeyboardMarkup` : Not yet implemented.
- [ ] `ReplyKeyboardRemove` : Not yet implemented.
- [ ] `ForceReply` : Not yet implemented.

**Message Manipulation (State Management)**
- [ ] `editMessageText` : Not yet implemented.
- [ ] `editMessageCaption` : Not yet implemented.
- [ ] `editMessageMedia` : Not yet implemented.
- [ ] `editMessageLiveLocation` : Not yet implemented.
- [ ] `stopMessageLiveLocation` : Not yet implemented.
- [ ] `editMessageReplyMarkup` : Not yet implemented.
- [ ] `stopPoll` : Not yet implemented.
- [ ] `deleteMessage` : Not yet implemented.
- [ ] `deleteMessages` : Not yet implemented.
- [ ] `forwardMessage` : Not yet implemented.
- [ ] `forwardMessages` : Not yet implemented.
- [ ] `copyMessage` : Not yet implemented.
- [ ] `copyMessages` : Not yet implemented.

**Chat Management (Message Level)**
- [ ] `pinChatMessage` : Not yet implemented.
- [ ] `unpinChatMessage` : Not yet implemented.
- [ ] `unpinAllChatMessages` : Not yet implemented.

---

### Implementation Notes

#### Input Pipeline (`main.rs` → `extract_content()`)
All media downloads use `bot.get_file()` + `bot.download_file()` (via `teloxide::net::Download` trait).
Base64 encoding uses `base64::engine::general_purpose::STANDARD`.

| Message type | ContentPart produced          | Notes |
|:-------------|:------------------------------|:------|
| `text`       | `Text`                        | Fast path: uses `agent.prompt()` |
| `photo`      | `[Text?] ImageBase64`         | Largest available size. Caption included if present. |
| `voice`      | `[Text?] Audio(ogg)`          | OGG/Opus directly from Telegram |
| `audio`      | `[Text?] Audio(mp3/ogg/wav)`  | MIME-sniffed format tag |
| `video`      | `[Text?] VideoBase64`         | MIME-sniffed media_type |
| `document`   | `[Text?] Text / File`         | text/* + JSON/CSV → inline; PDF → File block; others → error |
| `location`   | `Text`                        | Formatted with Google Maps link |

#### Output Tools (`tools.rs` → `PeonTool` implementations)

| Tool name            | Telegram API         | Blocking? | Notes |
|:---------------------|:---------------------|:----------|:------|
| `send_voice`         | `sendVoice`          | ✅ Yes    | base64 in, InputFile::memory |
| `send_csv`           | `sendDocument`       | ✅ Yes    | JSON rows → in-memory CSV |
| `send_inline_keyboard` | `sendMessage` + `InlineKeyboardMarkup` | ✅ Yes | 2D button array, MarkdownV2 |
| `send_chat_action`   | `sendChatAction`     | 🔥 No     | Fire and forget |

All output tools are injected **per-request** bound to the specific `ChatId`, so the LLM never sees credentials.
