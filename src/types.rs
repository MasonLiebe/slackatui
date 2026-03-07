use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of a Slack channel/conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType {
    Channel,
    Group,
    IM,
    MpIM,
}

/// Represents a channel entry in the sidebar channel list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelItem {
    pub id: String,
    pub name: String,
    pub channel_type: ChannelType,
    pub topic: String,
    pub user_id: String,
    pub presence: String,
    pub notification: bool,
    pub style_prefix: String,
    pub style_icon: String,
    pub style_text: String,
}

impl ChannelItem {
    pub fn new(id: String, name: String, channel_type: ChannelType) -> Self {
        Self {
            id,
            name,
            channel_type,
            topic: String::new(),
            user_id: String::new(),
            presence: String::new(),
            notification: false,
            style_prefix: String::new(),
            style_icon: String::new(),
            style_text: String::new(),
        }
    }

    /// Returns the display name for the channel, matching the Go GetChannelName behavior.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

/// A file attached to a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachedFile {
    pub file_id: String,
    pub title: String,
    pub url: String,
    pub is_image: bool,
}

/// A reaction on a message (emoji name + count + whether current user reacted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub name: String,
    pub emoji: String,
    pub count: u32,
    pub reacted: bool,
}

/// Represents a single chat message (or sub-message for attachments/files/replies).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub timestamp: String,
    pub time: DateTime<Local>,
    pub name: String,
    pub content: String,
    pub thread: String,
    pub reply_count: u32,
    pub reactions: Vec<Reaction>,
    pub files: Vec<AttachedFile>,
    pub messages: HashMap<String, Message>,
    pub style_time: String,
    pub style_thread: String,
    pub style_name: String,
    pub style_text: String,
    pub format_time: String,
}

impl Message {
    pub fn new(id: String, name: String, content: String, time: DateTime<Local>) -> Self {
        Self {
            id: id.clone(),
            timestamp: id,
            time,
            name,
            content,
            thread: String::new(),
            reply_count: 0,
            reactions: Vec::new(),
            files: Vec::new(),
            messages: HashMap::new(),
            style_time: String::new(),
            style_thread: String::new(),
            style_name: String::new(),
            style_text: String::new(),
            format_time: "15:04".to_string(),
        }
    }
}

/// The input mode of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Command,
    Insert,
    Search,
    React,
    Upload,
}

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Channels,
    Chat,
    Thread,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn test_channel_item_new() {
        let ch = ChannelItem::new(
            "C123".to_string(),
            "general".to_string(),
            ChannelType::Channel,
        );
        assert_eq!(ch.id, "C123");
        assert_eq!(ch.name, "general");
        assert_eq!(ch.channel_type, ChannelType::Channel);
        assert!(!ch.notification);
        assert!(ch.presence.is_empty());
    }

    #[test]
    fn test_channel_item_display_name() {
        let ch = ChannelItem::new(
            "C123".to_string(),
            "random".to_string(),
            ChannelType::Channel,
        );
        assert_eq!(ch.display_name(), "random");
    }

    #[test]
    fn test_message_new() {
        let now = Local::now();
        let msg = Message::new(
            "1234.5678".to_string(),
            "alice".to_string(),
            "hello world".to_string(),
            now,
        );
        assert_eq!(msg.id, "1234.5678");
        assert_eq!(msg.name, "alice");
        assert_eq!(msg.content, "hello world");
        assert!(msg.thread.is_empty());
        assert!(msg.messages.is_empty());
    }

    #[test]
    fn test_message_with_submessages() {
        let now = Local::now();
        let mut parent = Message::new(
            "1234.5678".to_string(),
            "alice".to_string(),
            "parent message".to_string(),
            now,
        );

        let reply = Message::new(
            "1234.9999".to_string(),
            "bob".to_string(),
            "reply message".to_string(),
            now,
        );

        parent.messages.insert(reply.id.clone(), reply);
        assert_eq!(parent.messages.len(), 1);
        assert!(parent.messages.contains_key("1234.9999"));
    }

    #[test]
    fn test_channel_types() {
        assert_ne!(ChannelType::Channel, ChannelType::Group);
        assert_ne!(ChannelType::IM, ChannelType::MpIM);
        assert_eq!(ChannelType::Channel, ChannelType::Channel);
    }

    #[test]
    fn test_mode_and_focus() {
        assert_ne!(Mode::Command, Mode::Insert);
        assert_ne!(Mode::Insert, Mode::Search);
        assert_ne!(Focus::Chat, Focus::Thread);
    }
}
