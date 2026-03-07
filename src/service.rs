use std::collections::HashMap;

use chrono::{DateTime, Local, TimeZone};

use crate::parse;
use crate::slack::{Conversation, SlackClient, SlackMessage};
use crate::types::{ChannelItem, ChannelType, Message};

/// High-level service that wraps the Slack API client and manages
/// user/bot caches, channel lists, and message creation.
pub struct SlackService {
    pub client: SlackClient,
    pub user_cache: HashMap<String, String>,
    pub current_user_id: String,
    pub current_team: String,
    pub emoji_enabled: bool,
}

impl SlackService {
    pub fn new(client: SlackClient, emoji_enabled: bool) -> Self {
        Self {
            client,
            user_cache: HashMap::new(),
            current_user_id: String::new(),
            current_team: String::new(),
            emoji_enabled,
        }
    }

    /// Initialize the service: validate token, load user cache.
    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let auth = self.client.auth_test().await?;
        self.current_user_id = auth.user_id;
        self.current_team = auth.team;

        self.user_cache = self.client.get_users().await?;
        Ok(())
    }

    /// Fetch conversations and convert them to ChannelItems for the sidebar.
    pub async fn get_channels(&self) -> Result<Vec<ChannelItem>, Box<dyn std::error::Error>> {
        let conversations = self.client.get_conversations().await?;
        let mut channels = Vec::new();

        for conv in conversations {
            if let Some(item) = self.conversation_to_channel(&conv) {
                channels.push(item);
            }
        }

        // Sort: channels first, then groups, then IMs
        channels.sort_by(|a, b| {
            let type_order = |ct: &ChannelType| match ct {
                ChannelType::Channel => 0,
                ChannelType::Group => 1,
                ChannelType::MpIM => 2,
                ChannelType::IM => 3,
            };
            type_order(&a.channel_type)
                .cmp(&type_order(&b.channel_type))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Ok(channels)
    }

    /// Convert a Slack Conversation to a ChannelItem.
    fn conversation_to_channel(&self, conv: &Conversation) -> Option<ChannelItem> {
        let (channel_type, name) = if conv.is_im {
            let name = self
                .user_cache
                .get(&conv.user)
                .cloned()
                .unwrap_or_else(|| conv.user.clone());
            (ChannelType::IM, name)
        } else if conv.is_mpim {
            (ChannelType::MpIM, conv.name.clone())
        } else if conv.is_group {
            if !conv.is_member {
                return None;
            }
            (ChannelType::Group, conv.name.clone())
        } else if conv.is_channel {
            if !conv.is_member {
                return None;
            }
            (ChannelType::Channel, conv.name.clone())
        } else {
            return None;
        };

        let mut item = ChannelItem::new(conv.id.clone(), name, channel_type);
        item.topic = conv.topic.value.clone();
        item.user_id = conv.user.clone();
        item.notification = conv.unread_count > 0;

        Some(item)
    }

    /// Fetch channel history and convert to Messages.
    pub async fn get_messages(
        &self,
        channel_id: &str,
        count: u32,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let slack_msgs = self
            .client
            .get_conversation_history(channel_id, count)
            .await?;

        let mut messages = Vec::new();
        for sm in &slack_msgs {
            messages.push(self.slack_message_to_message(sm));
        }

        // API returns newest first; reverse so oldest is first
        messages.reverse();
        Ok(messages)
    }

    /// Fetch only messages newer than `oldest_ts` for a channel.
    pub async fn get_new_messages(
        &self,
        channel_id: &str,
        oldest_ts: &str,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let slack_msgs = self.client.get_new_messages(channel_id, oldest_ts).await?;

        let mut messages: Vec<Message> = slack_msgs
            .iter()
            .map(|sm| self.slack_message_to_message(sm))
            .collect();

        // API returns newest first; reverse so oldest is first
        messages.reverse();
        Ok(messages)
    }

    /// Fetch thread replies and convert to Messages.
    pub async fn get_thread_messages(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let slack_msgs = self
            .client
            .get_conversation_replies(channel_id, thread_ts)
            .await?;

        let mut messages = Vec::new();
        for sm in &slack_msgs {
            messages.push(self.slack_message_to_message(sm));
        }

        Ok(messages)
    }

    /// Convert a SlackMessage to our display Message type.
    pub fn slack_message_to_message(&self, sm: &SlackMessage) -> Message {
        let name = self.resolve_message_author(sm);
        let content = self.format_message_content(sm);
        let time = parse_slack_timestamp(&sm.timestamp);
        let hash = parse::hash_id(&sm.timestamp);

        let mut msg = Message::new(sm.timestamp.clone(), name, content, time);
        msg.id = hash;
        msg.reply_count = sm.reply_count;

        if !sm.thread_ts.is_empty() {
            msg.thread = sm.thread_ts.clone();
        }

        msg
    }

    /// Resolve a display name from user/bot/username fields (used by RTM handler).
    pub fn resolve_user_or_bot(&self, user_id: &str, bot_id: &str, username: &str) -> String {
        if !user_id.is_empty() {
            return self
                .user_cache
                .get(user_id)
                .cloned()
                .unwrap_or_else(|| user_id.to_string());
        }
        if !username.is_empty() {
            return username.to_string();
        }
        if !bot_id.is_empty() {
            return self
                .user_cache
                .get(bot_id)
                .cloned()
                .unwrap_or_else(|| bot_id.to_string());
        }
        "unknown".to_string()
    }

    /// Resolve the author name for a message, checking user cache and bot info.
    fn resolve_message_author(&self, sm: &SlackMessage) -> String {
        if !sm.user.is_empty() {
            return self
                .user_cache
                .get(&sm.user)
                .cloned()
                .unwrap_or_else(|| sm.user.clone());
        }

        if !sm.username.is_empty() {
            return sm.username.clone();
        }

        if !sm.bot_id.is_empty() {
            return self
                .user_cache
                .get(&sm.bot_id)
                .cloned()
                .unwrap_or_else(|| sm.bot_id.clone());
        }

        "unknown".to_string()
    }

    /// Format message content: parse mentions/emoji, append attachments/files.
    fn format_message_content(&self, sm: &SlackMessage) -> String {
        let mut content = parse::parse_message(&sm.text, self.emoji_enabled, &self.user_cache);

        // Append attachment text
        for att in &sm.attachments {
            if !att.pretext.is_empty() {
                content.push_str(&format!("\n{}", att.pretext));
            }
            if !att.title.is_empty() {
                content.push_str(&format!("\n{}", att.title));
            }
            if !att.text.is_empty() {
                content.push_str(&format!("\n{}", att.text));
            }
            for field in &att.fields {
                content.push_str(&format!("\n{}: {}", field.title, field.value));
            }
        }

        // Append file references
        for file in &sm.files {
            if !file.title.is_empty() {
                content.push_str(&format!("\n[file: {}]", file.title));
            }
        }

        content
    }

    /// Look up a username by user ID, fetching from API if not cached.
    pub async fn resolve_user(&mut self, user_id: &str) -> String {
        if let Some(name) = self.user_cache.get(user_id) {
            return name.clone();
        }

        match self.client.get_user_info(user_id).await {
            Ok(info) => {
                self.user_cache
                    .insert(user_id.to_string(), info.name.clone());
                info.name
            }
            Err(_) => user_id.to_string(),
        }
    }

    /// Look up a bot name by bot ID, fetching from API if not cached.
    pub async fn resolve_bot(&mut self, bot_id: &str) -> String {
        if let Some(name) = self.user_cache.get(bot_id) {
            return name.clone();
        }

        match self.client.get_bot_info(bot_id).await {
            Ok(info) => {
                self.user_cache
                    .insert(bot_id.to_string(), info.name.clone());
                info.name
            }
            Err(_) => bot_id.to_string(),
        }
    }

    /// Send a message or command to a channel.
    pub async fn send(
        &self,
        channel_id: &str,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check for slash commands
        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            let command = parts[0];
            let args = if parts.len() > 1 { parts[1] } else { "" };
            self.client.send_command(channel_id, command, args).await?;
            return Ok(());
        }

        match thread_ts {
            Some(ts) => {
                self.client.send_reply(channel_id, ts, text).await?;
            }
            None => {
                self.client.send_message(channel_id, text).await?;
            }
        }

        Ok(())
    }
}

/// Parse a Slack timestamp string (e.g. "1234567890.123456") into a local DateTime.
pub fn parse_slack_timestamp(ts: &str) -> DateTime<Local> {
    let secs: i64 = ts
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    Local
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(Local::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slack::{SlackAttachment, SlackAttachmentField, SlackFile};

    fn make_service() -> SlackService {
        let client = SlackClient::new("xoxp-test");
        let mut svc = SlackService::new(client, true);
        svc.user_cache.insert("U1".into(), "alice".into());
        svc.user_cache.insert("U2".into(), "bob".into());
        svc.user_cache.insert("B1".into(), "testbot".into());
        svc
    }

    // ---- conversation_to_channel ----

    #[test]
    fn test_channel_conversion() {
        let svc = make_service();
        let conv = Conversation {
            id: "C123".into(),
            name: "general".into(),
            is_channel: true,
            is_member: true,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert_eq!(item.id, "C123");
        assert_eq!(item.name, "general");
        assert_eq!(item.channel_type, ChannelType::Channel);
    }

    #[test]
    fn test_channel_conversion_non_member_excluded() {
        let svc = make_service();
        let conv = Conversation {
            id: "C123".into(),
            name: "general".into(),
            is_channel: true,
            is_member: false,
            ..Default::default()
        };

        assert!(svc.conversation_to_channel(&conv).is_none());
    }

    #[test]
    fn test_im_conversion() {
        let svc = make_service();
        let conv = Conversation {
            id: "D123".into(),
            user: "U1".into(),
            is_im: true,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert_eq!(item.channel_type, ChannelType::IM);
        assert_eq!(item.name, "alice"); // resolved from user cache
        assert_eq!(item.user_id, "U1");
    }

    #[test]
    fn test_im_conversion_unknown_user() {
        let svc = make_service();
        let conv = Conversation {
            id: "D123".into(),
            user: "U999".into(),
            is_im: true,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert_eq!(item.name, "U999"); // falls back to user ID
    }

    #[test]
    fn test_group_conversion() {
        let svc = make_service();
        let conv = Conversation {
            id: "G123".into(),
            name: "secret-group".into(),
            is_group: true,
            is_member: true,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert_eq!(item.channel_type, ChannelType::Group);
        assert_eq!(item.name, "secret-group");
    }

    #[test]
    fn test_mpim_conversion() {
        let svc = make_service();
        let conv = Conversation {
            id: "G456".into(),
            name: "mpdm-alice--bob".into(),
            is_group: true,
            is_mpim: true,
            is_member: true,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert_eq!(item.channel_type, ChannelType::MpIM);
    }

    #[test]
    fn test_channel_with_unread() {
        let svc = make_service();
        let conv = Conversation {
            id: "C123".into(),
            name: "general".into(),
            is_channel: true,
            is_member: true,
            unread_count: 5,
            ..Default::default()
        };

        let item = svc.conversation_to_channel(&conv).unwrap();
        assert!(item.notification);
    }

    // ---- resolve_message_author ----

    #[test]
    fn test_resolve_author_user() {
        let svc = make_service();
        let sm = SlackMessage {
            user: "U1".into(),
            ..Default::default()
        };
        assert_eq!(svc.resolve_message_author(&sm), "alice");
    }

    #[test]
    fn test_resolve_author_unknown_user() {
        let svc = make_service();
        let sm = SlackMessage {
            user: "U999".into(),
            ..Default::default()
        };
        assert_eq!(svc.resolve_message_author(&sm), "U999");
    }

    #[test]
    fn test_resolve_author_bot_username() {
        let svc = make_service();
        let sm = SlackMessage {
            username: "webhookbot".into(),
            bot_id: "B999".into(),
            ..Default::default()
        };
        assert_eq!(svc.resolve_message_author(&sm), "webhookbot");
    }

    #[test]
    fn test_resolve_author_bot_id_cached() {
        let svc = make_service();
        let sm = SlackMessage {
            bot_id: "B1".into(),
            ..Default::default()
        };
        assert_eq!(svc.resolve_message_author(&sm), "testbot");
    }

    #[test]
    fn test_resolve_author_unknown() {
        let svc = make_service();
        let sm = SlackMessage::default();
        assert_eq!(svc.resolve_message_author(&sm), "unknown");
    }

    // ---- format_message_content ----

    #[test]
    fn test_format_plain_text() {
        let svc = make_service();
        let sm = SlackMessage {
            text: "hello world".into(),
            ..Default::default()
        };
        assert_eq!(svc.format_message_content(&sm), "hello world");
    }

    #[test]
    fn test_format_with_mention() {
        let svc = make_service();
        let sm = SlackMessage {
            text: "<@U1> hi".into(),
            ..Default::default()
        };
        assert_eq!(svc.format_message_content(&sm), "@alice hi");
    }

    #[test]
    fn test_format_with_html_entities() {
        let svc = make_service();
        let sm = SlackMessage {
            text: "a &amp; b &lt;c&gt;".into(),
            ..Default::default()
        };
        assert_eq!(svc.format_message_content(&sm), "a & b <c>");
    }

    #[test]
    fn test_format_with_attachment() {
        let svc = make_service();
        let sm = SlackMessage {
            text: "msg".into(),
            attachments: vec![SlackAttachment {
                pretext: "Pre".into(),
                title: "Title".into(),
                text: "Body".into(),
                fields: vec![SlackAttachmentField {
                    title: "F1".into(),
                    value: "V1".into(),
                }],
            }],
            ..Default::default()
        };
        let content = svc.format_message_content(&sm);
        assert!(content.starts_with("msg"));
        assert!(content.contains("Pre"));
        assert!(content.contains("Title"));
        assert!(content.contains("Body"));
        assert!(content.contains("F1: V1"));
    }

    #[test]
    fn test_format_with_file() {
        let svc = make_service();
        let sm = SlackMessage {
            text: "check this".into(),
            files: vec![SlackFile {
                id: "F1".into(),
                title: "image.png".into(),
                url_private: "https://example.com/image.png".into(),
            }],
            ..Default::default()
        };
        let content = svc.format_message_content(&sm);
        assert!(content.contains("[file: image.png]"));
    }

    // ---- slack_message_to_message ----

    #[test]
    fn test_slack_message_to_message_basic() {
        let svc = make_service();
        let sm = SlackMessage {
            timestamp: "1234567890.123456".into(),
            user: "U1".into(),
            text: "hello".into(),
            ..Default::default()
        };

        let msg = svc.slack_message_to_message(&sm);
        assert_eq!(msg.name, "alice");
        assert_eq!(msg.content, "hello");
        assert!(!msg.id.is_empty()); // hash_id
    }

    #[test]
    fn test_slack_message_to_message_thread_reply() {
        let svc = make_service();
        let sm = SlackMessage {
            timestamp: "1234567890.999999".into(),
            user: "U2".into(),
            text: "reply".into(),
            thread_ts: "1234567890.123456".into(),
            ..Default::default()
        };

        let msg = svc.slack_message_to_message(&sm);
        assert!(!msg.thread.is_empty());
    }

    // ---- parse_slack_timestamp ----

    #[test]
    fn test_parse_slack_timestamp_valid() {
        let dt = parse_slack_timestamp("1234567890.123456");
        assert_eq!(dt.timestamp(), 1234567890);
    }

    #[test]
    fn test_parse_slack_timestamp_zero() {
        let dt = parse_slack_timestamp("0.0");
        assert_eq!(dt.timestamp(), 0);
    }

    #[test]
    fn test_parse_slack_timestamp_invalid() {
        let dt = parse_slack_timestamp("not_a_timestamp");
        // Should fall back to 0 seconds epoch
        assert_eq!(dt.timestamp(), 0);
    }

    // ---- channel sorting ----

    #[test]
    fn test_channel_sorting_order() {
        let svc = make_service();
        let conversations = vec![
            Conversation {
                id: "D1".into(),
                user: "U1".into(),
                is_im: true,
                ..Default::default()
            },
            Conversation {
                id: "C1".into(),
                name: "zebra".into(),
                is_channel: true,
                is_member: true,
                ..Default::default()
            },
            Conversation {
                id: "G1".into(),
                name: "alpha-group".into(),
                is_group: true,
                is_member: true,
                ..Default::default()
            },
            Conversation {
                id: "C2".into(),
                name: "alpha".into(),
                is_channel: true,
                is_member: true,
                ..Default::default()
            },
        ];

        let mut channels: Vec<ChannelItem> = conversations
            .iter()
            .filter_map(|c| svc.conversation_to_channel(c))
            .collect();

        channels.sort_by(|a, b| {
            let type_order = |ct: &ChannelType| match ct {
                ChannelType::Channel => 0,
                ChannelType::Group => 1,
                ChannelType::MpIM => 2,
                ChannelType::IM => 3,
            };
            type_order(&a.channel_type)
                .cmp(&type_order(&b.channel_type))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        // Channels first (alpha, zebra), then groups, then IMs
        assert_eq!(channels[0].name, "alpha");
        assert_eq!(channels[1].name, "zebra");
        assert_eq!(channels[2].name, "alpha-group");
        assert_eq!(channels[3].name, "alice");
    }

    // ---- send (command parsing) ----

    #[test]
    fn test_slash_command_detection() {
        let text = "/status available";
        assert!(text.starts_with('/'));
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        assert_eq!(parts[0], "/status");
        assert_eq!(parts[1], "available");
    }

    #[test]
    fn test_slash_command_no_args() {
        let text = "/away";
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        assert_eq!(parts[0], "/away");
        assert_eq!(parts.len(), 1);
    }
}
