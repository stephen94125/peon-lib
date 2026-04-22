### 📥 Incoming Message Types (Received from Telegram)

**Media, Files & Content**
- [o] `text` : Contains a pure text message.
- [ ] `animation` : Contains an animation (usually a GIF or an H.264/MPEG-4 video without sound).
- [ ] `audio` : Contains an audio file (e.g., MP3) treated as a music track.
- [ ] `document` : Contains a general file (e.g., PDF, ZIP, code script) or an uncompressed image.
- [ ] `photo` : Contains a compressed image (usually provides multiple sizes in an array).
- [ ] `sticker` : Contains a static, animated, or video sticker.
- [ ] `story` : Contains a forwarded Telegram story.
- [ ] `video` : Contains a video file (with sound).
- [ ] `video_note` : Contains a round video message (Telescope format).
- [ ] `voice` : Contains a voice note (audio message recorded directly via the microphone).
- [ ] `contact` : Contains a shared phone contact card.
- [ ] `dice` : Contains an animated emoji with a randomly generated integer value.
- [ ] `game` : Contains a Telegram HTML5 game.
- [ ] `poll` : Contains a regular poll or a quiz.
- [ ] `venue` : Contains venue information (location combined with a name and address).
- [ ] `location` : Contains a shared geographic location (latitude and longitude).
- [ ] `invoice` : Contains an invoice for a payment request.
- [ ] `successful_payment` : Contains a service message confirming a successful payment.
- [ ] `web_app_data` : Contains data payload sent from a Telegram Mini App (Web App).
- [ ] `passport_data` : Contains Telegram Passport data for identity verification.

**Chat & Member Events**
- [ ] `new_chat_members` : Triggered when new members are added to or join the group.
- [ ] `left_chat_member` : Triggered when a member leaves or is kicked from the group.
- [ ] `new_chat_title` : Triggered when the chat title is updated.
- [ ] `new_chat_photo` : Triggered when the chat profile photo is updated.
- [ ] `delete_chat_photo` : Triggered when the chat profile photo is deleted.
- [ ] `group_chat_created` : Triggered when a basic group is newly created.
- [ ] `supergroup_chat_created` : Triggered when a supergroup is newly created.
- [ ] `channel_chat_created` : Triggered when a channel is newly created.
- [ ] `message_auto_delete_timer_changed` : Triggered when the auto-delete settings are modified.
- [ ] `migrate_to_chat_id` : Triggered when a basic group is upgraded to a supergroup (contains the new chat ID).
- [ ] `migrate_from_chat_id` : Triggered when a supergroup is created from a basic group (contains the old chat ID).
- [ ] `pinned_message` : Triggered when a message is pinned in the chat.
- [ ] `users_shared` : Triggered when a user shares another user's contact info via a keyboard request button.
- [ ] `chat_shared` : Triggered when a user shares a group or channel via a keyboard request button.
- [ ] `connected_website` : Triggered when a user logs into a website via their Telegram account.
- [ ] `write_access_allowed` : Triggered when a user explicitly grants the bot permission to message them (often via Web Apps).
- [ ] `proximity_alert_triggered` : Triggered when a user approaches another user within a set distance in live location mode.
- [ ] `boost_added` : Triggered when a user boosts the chat (adds Premium boost to a channel/supergroup).
- [ ] `chat_background_set` : Triggered when a custom chat background is applied.

**Forum / Topic Events**
- [ ] `forum_topic_created` : Triggered when a new topic is created in a forum supergroup.
- [ ] `forum_topic_edited` : Triggered when a forum topic's name or icon is modified.
- [ ] `forum_topic_closed` : Triggered when a forum topic is closed to new messages.
- [ ] `forum_topic_reopened` : Triggered when a previously closed forum topic is reopened.
- [ ] `general_forum_topic_hidden` : Triggered when the default 'General' topic is hidden.
- [ ] `general_forum_topic_unhidden` : Triggered when the default 'General' topic is unhidden.

**Giveaway Events**
- [ ] `giveaway_created` : Triggered when a Telegram Premium giveaway starts.
- [ ] `giveaway` : Triggered when a giveaway message is posted in the chat.
- [ ] `giveaway_winners` : Triggered when the winners of a giveaway are announced.
- [ ] `giveaway_completed` : Triggered when a giveaway ends (even if there are no winners).

**Voice/Video Chat Events**
- [ ] `video_chat_scheduled` : Triggered when a voice/video chat is scheduled for a future time.
- [ ] `video_chat_started` : Triggered when a voice/video chat officially starts.
- [ ] `video_chat_ended` : Triggered when a voice/video chat ends.
- [ ] `video_chat_participants_invited` : Triggered when users are invited to an active voice/video chat.

---

### 📤 Outgoing Actions & Methods (Sent by Bot)

**Content Sending Methods**
- [o] `sendMessage` : Sends a standard text message.
- [ ] `sendPhoto` : Sends a compressed image file.
- [ ] `sendAudio` : Sends an audio file intended to be treated as a music track.
- [ ] `sendDocument` : Sends a general file or an uncompressed image.
- [ ] `sendVideo` : Sends an MP4 video file.
- [ ] `sendAnimation` : Sends a silent animation (like a GIF).
- [ ] `sendVoice` : Sends an audio file as a playable voice note.
- [ ] `sendVideoNote` : Sends a rounded video message.
- [ ] `sendMediaGroup` : Sends an album containing an array of multiple photos or videos.
- [ ] `sendLocation` : Sends a static geographical point (latitude and longitude).
- [ ] `sendVenue` : Sends a location combined with venue details (name, address).
- [ ] `sendContact` : Sends a phone contact card.
- [ ] `sendPoll` : Sends a regular poll or a quiz.
- [ ] `sendDice` : Sends an animated emoji with a random outcome (e.g., rolling dice, slot machine).
- [ ] `sendChatAction` : Displays a temporary status indicator (e.g., "typing...", "uploading document...") to the user.
- [ ] `sendSticker` : Sends a static, animated, or video sticker.
- [ ] `sendInvoice` : Sends a payment invoice to a user.
- [ ] `sendGame` : Sends an HTML5 game.

**Interactive UI Interfaces (Modifiers attached to sending methods)**
- [ ] `InlineKeyboardMarkup` : Attaches interactive buttons directly underneath the message block (triggers callback queries).
- [ ] `ReplyKeyboardMarkup` : Replaces the user's default mobile keyboard with custom predefined response buttons.
- [ ] `ReplyKeyboardRemove` : Removes a previously set custom reply keyboard and restores the default one.
- [ ] `ForceReply` : Forces the user's client to automatically select the bot's message and open the reply input field.

**Message Manipulation (State Management)**
- [ ] `editMessageText` : Modifies the text content of a previously sent message.
- [ ] `editMessageCaption` : Modifies the caption text of a sent media message (photo, video, document).
- [ ] `editMessageMedia` : Replaces the actual media file in a previously sent message.
- [ ] `editMessageLiveLocation` : Updates the coordinates of a currently active live location message.
- [ ] `stopMessageLiveLocation` : Stops broadcasting updates for a live location message.
- [ ] `editMessageReplyMarkup` : Dynamically changes or removes the inline keyboard attached to a message.
- [ ] `stopPoll` : Closes an active poll so no additional votes can be cast.
- [ ] `deleteMessage` : Deletes a single specified message from the chat.
- [ ] `deleteMessages` : Deletes multiple specified messages at once from the chat.
- [ ] `forwardMessage` : Forwards a message from one chat to another (retains the original sender tag).
- [ ] `forwardMessages` : Forwards multiple messages at once.
- [ ] `copyMessage` : Copies a message's content and sends it as a new message from the bot (does not include the "forwarded from" tag).
- [ ] `copyMessages` : Copies multiple messages at once.

**Chat Management (Message Level)**
- [ ] `pinChatMessage` : Pins a specific message to the top of the chat interface.
- [ ] `unpinChatMessage` : Unpins a specifically pinned message.
- [ ] `unpinAllChatMessages` : Unpins all currently pinned messages in the chat at once.
