use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SLACK_API_BASE: &str = "https://slack.com/api";

#[derive(Debug)]
pub enum SlackError {
    Http(String),
    Api(String),
    Json(String),
}

impl std::fmt::Display for SlackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlackError::Http(msg) => write!(f, "Slack HTTP error: {}", msg),
            SlackError::Api(msg) => write!(f, "Slack API error: {}", msg),
            SlackError::Json(msg) => write!(f, "Slack JSON error: {}", msg),
        }
    }
}

impl std::error::Error for SlackError {}

/// Low-level Slack API client wrapping reqwest with a token.
pub struct SlackClient {
    http: Client,
    token: String,
}

impl SlackClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: Client::new(),
            token: token.to_string(),
        }
    }

    /// Generic helper for GET requests to the Slack API.
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(params)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let body = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&body).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&body).map_err(|e| SlackError::Json(e.to_string()))
    }

    /// Generic helper for POST requests to the Slack API (JSON body).
    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: &impl Serialize,
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let text = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))
    }

    /// Generic helper for POST requests with form-encoded body.
    async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .form(params)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let text = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))
    }

    // ---- auth.test ----

    /// Validate the token and return the authenticated user/team info.
    pub async fn auth_test(&self) -> Result<AuthTestResponse, SlackError> {
        self.post_form("auth.test", &[]).await
    }

    // ---- users.list ----

    /// Fetch all users in the workspace. Returns a map of user_id -> username
    /// for non-deleted users.
    pub async fn get_users(&self) -> Result<HashMap<String, String>, SlackError> {
        let resp: UsersListResponse = self.get("users.list", &[]).await?;
        let mut cache = HashMap::new();
        for user in resp.members {
            if !user.deleted {
                cache.insert(user.id, user.name);
            }
        }
        Ok(cache)
    }

    // ---- users.info ----

    /// Get info for a single user by ID.
    pub async fn get_user_info(&self, user_id: &str) -> Result<UserInfo, SlackError> {
        let resp: UserInfoResponse = self.get("users.info", &[("user", user_id)]).await?;
        Ok(resp.user)
    }

    // ---- bots.info ----

    /// Get info for a bot by bot ID.
    pub async fn get_bot_info(&self, bot_id: &str) -> Result<BotInfo, SlackError> {
        let resp: BotInfoResponse = self.get("bots.info", &[("bot", bot_id)]).await?;
        Ok(resp.bot)
    }

    // ---- users.getPresence ----

    /// Get the presence status of a user ("active" or "away").
    pub async fn get_user_presence(&self, user_id: &str) -> Result<String, SlackError> {
        let resp: PresenceResponse = self.get("users.getPresence", &[("user", user_id)]).await?;
        Ok(resp.presence)
    }

    // ---- users.setPresence ----

    /// Set the current user's presence (typically "auto").
    pub async fn set_user_presence(&self, presence: &str) -> Result<(), SlackError> {
        let _: SlackEnvelope = self
            .post_json(
                "users.setPresence",
                &serde_json::json!({ "presence": presence }),
            )
            .await?;
        Ok(())
    }

    // ---- conversations.list ----

    /// Fetch all conversations (channels, groups, IMs, MPIMs) with pagination.
    pub async fn get_conversations(&self) -> Result<Vec<Conversation>, SlackError> {
        let mut all = Vec::new();
        let mut cursor = String::new();

        loop {
            let mut params = vec![
                ("exclude_archived", "true"),
                ("limit", "1000"),
                ("types", "public_channel,private_channel,im,mpim"),
            ];
            if !cursor.is_empty() {
                params.push(("cursor", &cursor));
            }

            let resp: ConversationsListResponse = self.get("conversations.list", &params).await?;
            all.extend(resp.channels);

            match resp.response_metadata {
                Some(meta) if !meta.next_cursor.is_empty() => {
                    cursor = meta.next_cursor;
                }
                _ => break,
            }
        }

        Ok(all)
    }

    // ---- conversations.history ----

    /// Fetch message history for a channel.
    pub async fn get_conversation_history(
        &self,
        channel_id: &str,
        limit: u32,
    ) -> Result<Vec<SlackMessage>, SlackError> {
        let limit_str = limit.to_string();
        let params = [
            ("channel", channel_id),
            ("limit", &limit_str),
            ("inclusive", "false"),
        ];
        let resp: ConversationHistoryResponse = self.get("conversations.history", &params).await?;
        Ok(resp.messages)
    }

    /// Fetch a single message by its timestamp ID.
    pub async fn get_message_by_id(
        &self,
        channel_id: &str,
        message_ts: &str,
    ) -> Result<Option<SlackMessage>, SlackError> {
        let params = [
            ("channel", channel_id),
            ("limit", "1"),
            ("inclusive", "true"),
            ("latest", message_ts),
        ];
        let resp: ConversationHistoryResponse = self.get("conversations.history", &params).await?;
        Ok(resp.messages.into_iter().next())
    }

    // ---- conversations.replies ----

    /// Fetch all threaded replies for a parent message, with pagination.
    pub async fn get_conversation_replies(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<SlackMessage>, SlackError> {
        let mut all = Vec::new();
        let mut cursor = String::new();

        loop {
            let mut params = vec![
                ("channel", channel_id),
                ("ts", thread_ts),
                ("limit", "200"),
            ];
            if !cursor.is_empty() {
                params.push(("cursor", &cursor));
            }

            let resp: ConversationRepliesResponse =
                self.get("conversations.replies", &params).await?;
            all.extend(resp.messages);

            match resp.response_metadata {
                Some(meta) if !meta.next_cursor.is_empty() => {
                    cursor = meta.next_cursor;
                }
                _ => break,
            }
        }

        Ok(all)
    }

    // ---- Mark as read ----

    /// Mark a conversation as read up to the current time.
    /// Uses conversations.mark which works for all conversation types.
    pub async fn mark_conversation(
        &self,
        channel_id: &str,
        timestamp: &str,
    ) -> Result<(), SlackError> {
        let _: SlackEnvelope = self
            .post_json(
                "conversations.mark",
                &serde_json::json!({
                    "channel": channel_id,
                    "ts": timestamp,
                }),
            )
            .await?;
        Ok(())
    }

    // ---- chat.postMessage ----

    /// Send a message to a channel.
    pub async fn send_message(
        &self,
        channel_id: &str,
        text: &str,
    ) -> Result<PostMessageResponse, SlackError> {
        self.post_json(
            "chat.postMessage",
            &serde_json::json!({
                "channel": channel_id,
                "text": text,
                "as_user": true,
                "link_names": true,
            }),
        )
        .await
    }

    /// Send a threaded reply to a message.
    pub async fn send_reply(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
    ) -> Result<PostMessageResponse, SlackError> {
        self.post_json(
            "chat.postMessage",
            &serde_json::json!({
                "channel": channel_id,
                "text": text,
                "thread_ts": thread_ts,
                "as_user": true,
                "link_names": true,
            }),
        )
        .await
    }

    // ---- chat.command ----

    /// Send a slash command (undocumented endpoint).
    pub async fn send_command(
        &self,
        channel_id: &str,
        command: &str,
        text: &str,
    ) -> Result<(), SlackError> {
        let _: SlackEnvelope = self
            .post_form(
                "chat.command",
                &[
                    ("channel", channel_id),
                    ("command", command),
                    ("text", text),
                ],
            )
            .await?;
        Ok(())
    }

    // ---- rtm.connect ----

    /// Get the WebSocket URL for an RTM connection.
    pub async fn rtm_connect(&self) -> Result<RtmConnectResponse, SlackError> {
        self.post_form("rtm.connect", &[]).await
    }
}

// ---- Response types ----

/// Minimal envelope for checking ok/error on any Slack API response.
#[derive(Debug, Deserialize)]
struct SlackEnvelope {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

/// Response from auth.test.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthTestResponse {
    #[allow(dead_code)]
    ok: bool,
    pub url: String,
    pub team: String,
    pub user: String,
    pub team_id: String,
    pub user_id: String,
}

/// Response from users.list.
#[derive(Debug, Deserialize)]
struct UsersListResponse {
    #[allow(dead_code)]
    ok: bool,
    members: Vec<UserMember>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserMember {
    id: String,
    name: String,
    #[serde(default)]
    deleted: bool,
}

/// Response from users.info.
#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[allow(dead_code)]
    ok: bool,
    user: UserInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub deleted: bool,
}

/// Response from bots.info.
#[derive(Debug, Deserialize)]
struct BotInfoResponse {
    #[allow(dead_code)]
    ok: bool,
    bot: BotInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotInfo {
    pub id: String,
    pub name: String,
}

/// Response from rtm.connect.
#[derive(Debug, Clone, Deserialize)]
pub struct RtmConnectResponse {
    #[allow(dead_code)]
    ok: bool,
    pub url: String,
}

/// Response from chat.postMessage.
#[derive(Debug, Clone, Deserialize)]
pub struct PostMessageResponse {
    #[allow(dead_code)]
    ok: bool,
    #[serde(default)]
    pub channel: String,
    #[serde(default, rename = "ts")]
    pub timestamp: String,
}

/// Response from users.getPresence.
#[derive(Debug, Deserialize)]
struct PresenceResponse {
    #[allow(dead_code)]
    ok: bool,
    presence: String,
}

/// Cursor-based pagination metadata from Slack.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResponseMetadata {
    #[serde(default)]
    pub next_cursor: String,
}

/// Response from conversations.list.
#[derive(Debug, Deserialize)]
struct ConversationsListResponse {
    #[allow(dead_code)]
    ok: bool,
    channels: Vec<Conversation>,
    #[serde(default)]
    response_metadata: Option<ResponseMetadata>,
}

/// A Slack conversation (channel, group, IM, or MPIM).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Conversation {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub topic: ConversationTopic,
    #[serde(default)]
    pub is_channel: bool,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub is_im: bool,
    #[serde(default)]
    pub is_mpim: bool,
    #[serde(default)]
    pub is_member: bool,
    #[serde(default)]
    pub is_open: bool,
    #[serde(default)]
    pub unread_count: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConversationTopic {
    #[serde(default)]
    pub value: String,
}

/// Response from conversations.history.
#[derive(Debug, Deserialize)]
struct ConversationHistoryResponse {
    #[allow(dead_code)]
    ok: bool,
    messages: Vec<SlackMessage>,
}

/// Response from conversations.replies.
#[derive(Debug, Deserialize)]
struct ConversationRepliesResponse {
    #[allow(dead_code)]
    ok: bool,
    messages: Vec<SlackMessage>,
    #[serde(default)]
    response_metadata: Option<ResponseMetadata>,
}

/// A Slack message as returned by conversations.history / conversations.replies.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackMessage {
    #[serde(default, rename = "ts")]
    pub timestamp: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub bot_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub thread_ts: String,
    #[serde(default, rename = "subtype")]
    pub sub_type: String,
    #[serde(default)]
    pub attachments: Vec<SlackAttachment>,
    #[serde(default)]
    pub files: Vec<SlackFile>,
}

/// A Slack message attachment.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackAttachment {
    #[serde(default)]
    pub pretext: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub fields: Vec<SlackAttachmentField>,
}

/// A field within a Slack attachment.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackAttachmentField {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub value: String,
}

/// A file attached to a Slack message.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlackFile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url_private: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_client_new() {
        let client = SlackClient::new("xoxp-test-token");
        assert_eq!(client.token, "xoxp-test-token");
    }

    #[test]
    fn test_auth_test_response_deserialization() {
        let json = r#"{
            "ok": true,
            "url": "https://myteam.slack.com/",
            "team": "My Team",
            "user": "alice",
            "team_id": "T123",
            "user_id": "U456"
        }"#;

        let resp: AuthTestResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.user_id, "U456");
        assert_eq!(resp.team_id, "T123");
        assert_eq!(resp.user, "alice");
        assert_eq!(resp.team, "My Team");
    }

    #[test]
    fn test_users_list_response_deserialization() {
        let json = r#"{
            "ok": true,
            "members": [
                {"id": "U1", "name": "alice", "deleted": false},
                {"id": "U2", "name": "bob", "deleted": true},
                {"id": "U3", "name": "charlie", "deleted": false}
            ]
        }"#;

        let resp: UsersListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.members.len(), 3);
        assert!(!resp.members[0].deleted);
        assert!(resp.members[1].deleted);
    }

    #[test]
    fn test_users_list_filtering() {
        // Simulate the cache-building logic
        let members = vec![
            UserMember {
                id: "U1".into(),
                name: "alice".into(),
                deleted: false,
            },
            UserMember {
                id: "U2".into(),
                name: "bob".into(),
                deleted: true,
            },
            UserMember {
                id: "U3".into(),
                name: "charlie".into(),
                deleted: false,
            },
        ];

        let mut cache = HashMap::new();
        for user in members {
            if !user.deleted {
                cache.insert(user.id, user.name);
            }
        }

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("U1").unwrap(), "alice");
        assert_eq!(cache.get("U3").unwrap(), "charlie");
        assert!(!cache.contains_key("U2"));
    }

    #[test]
    fn test_user_info_response_deserialization() {
        let json = r#"{
            "ok": true,
            "user": {
                "id": "U123",
                "name": "alice",
                "deleted": false
            }
        }"#;

        let resp: UserInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.user.id, "U123");
        assert_eq!(resp.user.name, "alice");
    }

    #[test]
    fn test_bot_info_response_deserialization() {
        let json = r#"{
            "ok": true,
            "bot": {
                "id": "B123",
                "name": "mybot"
            }
        }"#;

        let resp: BotInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.bot.id, "B123");
        assert_eq!(resp.bot.name, "mybot");
    }

    #[test]
    fn test_presence_response_deserialization() {
        let json = r#"{"ok": true, "presence": "active"}"#;
        let resp: PresenceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.presence, "active");
    }

    #[test]
    fn test_slack_envelope_ok() {
        let json = r#"{"ok": true}"#;
        let env: SlackEnvelope = serde_json::from_str(json).unwrap();
        assert!(env.ok);
        assert!(env.error.is_none());
    }

    #[test]
    fn test_slack_envelope_error() {
        let json = r#"{"ok": false, "error": "invalid_auth"}"#;
        let env: SlackEnvelope = serde_json::from_str(json).unwrap();
        assert!(!env.ok);
        assert_eq!(env.error.unwrap(), "invalid_auth");
    }

    #[test]
    fn test_slack_error_display() {
        let err = SlackError::Api("not_authed".to_string());
        assert_eq!(format!("{}", err), "Slack API error: not_authed");

        let err = SlackError::Http("connection refused".to_string());
        assert_eq!(
            format!("{}", err),
            "Slack HTTP error: connection refused"
        );
    }

    #[test]
    fn test_conversation_deserialization_channel() {
        let json = r#"{
            "id": "C123",
            "name": "general",
            "is_channel": true,
            "is_group": false,
            "is_im": false,
            "is_mpim": false,
            "is_member": true,
            "topic": {"value": "General discussion"},
            "unread_count": 5
        }"#;

        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert_eq!(conv.id, "C123");
        assert_eq!(conv.name, "general");
        assert!(conv.is_channel);
        assert!(conv.is_member);
        assert!(!conv.is_im);
        assert_eq!(conv.topic.value, "General discussion");
        assert_eq!(conv.unread_count, 5);
    }

    #[test]
    fn test_conversation_deserialization_im() {
        let json = r#"{
            "id": "D456",
            "user": "U789",
            "is_channel": false,
            "is_im": true,
            "is_open": true,
            "unread_count": 0
        }"#;

        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert_eq!(conv.id, "D456");
        assert_eq!(conv.user, "U789");
        assert!(conv.is_im);
        assert!(conv.is_open);
        assert!(conv.name.is_empty());
    }

    #[test]
    fn test_conversation_deserialization_mpim() {
        let json = r#"{
            "id": "G789",
            "name": "mpdm-alice--bob-1",
            "is_group": true,
            "is_mpim": true,
            "is_member": true,
            "is_open": true
        }"#;

        let conv: Conversation = serde_json::from_str(json).unwrap();
        assert!(conv.is_group);
        assert!(conv.is_mpim);
        assert!(conv.is_open);
    }

    #[test]
    fn test_conversations_list_response() {
        let json = r#"{
            "ok": true,
            "channels": [
                {"id": "C1", "name": "general", "is_channel": true, "is_member": true},
                {"id": "C2", "name": "random", "is_channel": true, "is_member": true}
            ],
            "response_metadata": {"next_cursor": ""}
        }"#;

        let resp: ConversationsListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.channels.len(), 2);
        assert_eq!(resp.channels[0].name, "general");
    }

    #[test]
    fn test_conversations_list_with_cursor() {
        let json = r#"{
            "ok": true,
            "channels": [
                {"id": "C1", "name": "general", "is_channel": true}
            ],
            "response_metadata": {"next_cursor": "dGVhbTpDMDYxRkE1UEI="}
        }"#;

        let resp: ConversationsListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.channels.len(), 1);
        let cursor = resp.response_metadata.unwrap().next_cursor;
        assert_eq!(cursor, "dGVhbTpDMDYxRkE1UEI=");
    }

    #[test]
    fn test_slack_message_deserialization() {
        let json = r#"{
            "ts": "1234567890.123456",
            "user": "U123",
            "text": "Hello world!",
            "thread_ts": ""
        }"#;

        let msg: SlackMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.timestamp, "1234567890.123456");
        assert_eq!(msg.user, "U123");
        assert_eq!(msg.text, "Hello world!");
        assert!(msg.thread_ts.is_empty());
        assert!(msg.attachments.is_empty());
        assert!(msg.files.is_empty());
    }

    #[test]
    fn test_slack_message_with_thread() {
        let json = r#"{
            "ts": "1234567890.123456",
            "user": "U123",
            "text": "Thread parent",
            "thread_ts": "1234567890.123456"
        }"#;

        let msg: SlackMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.thread_ts, msg.timestamp);
    }

    #[test]
    fn test_slack_message_with_attachments() {
        let json = r#"{
            "ts": "123.456",
            "user": "U1",
            "text": "",
            "attachments": [
                {
                    "pretext": "Pre",
                    "text": "Attachment text",
                    "title": "Title",
                    "fields": [
                        {"title": "Field1", "value": "Val1"},
                        {"title": "Field2", "value": "Val2"}
                    ]
                }
            ]
        }"#;

        let msg: SlackMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.attachments.len(), 1);
        assert_eq!(msg.attachments[0].pretext, "Pre");
        assert_eq!(msg.attachments[0].text, "Attachment text");
        assert_eq!(msg.attachments[0].title, "Title");
        assert_eq!(msg.attachments[0].fields.len(), 2);
        assert_eq!(msg.attachments[0].fields[0].title, "Field1");
        assert_eq!(msg.attachments[0].fields[1].value, "Val2");
    }

    #[test]
    fn test_slack_message_with_files() {
        let json = r#"{
            "ts": "123.456",
            "user": "U1",
            "text": "",
            "files": [
                {
                    "id": "F123",
                    "title": "screenshot.png",
                    "url_private": "https://files.slack.com/files-pri/T1-F123/screenshot.png"
                }
            ]
        }"#;

        let msg: SlackMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.files.len(), 1);
        assert_eq!(msg.files[0].id, "F123");
        assert_eq!(msg.files[0].title, "screenshot.png");
        assert!(msg.files[0].url_private.starts_with("https://"));
    }

    #[test]
    fn test_slack_message_bot_message() {
        let json = r#"{
            "ts": "123.456",
            "bot_id": "B123",
            "username": "mybot",
            "text": "Bot message",
            "subtype": "bot_message"
        }"#;

        let msg: SlackMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.bot_id, "B123");
        assert_eq!(msg.username, "mybot");
        assert_eq!(msg.sub_type, "bot_message");
        assert!(msg.user.is_empty());
    }

    #[test]
    fn test_conversation_history_response() {
        let json = r#"{
            "ok": true,
            "messages": [
                {"ts": "1.0", "user": "U1", "text": "newest"},
                {"ts": "0.5", "user": "U2", "text": "older"}
            ]
        }"#;

        let resp: ConversationHistoryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.messages.len(), 2);
        assert_eq!(resp.messages[0].text, "newest");
        assert_eq!(resp.messages[1].text, "older");
    }

    #[test]
    fn test_conversation_replies_response() {
        let json = r#"{
            "ok": true,
            "messages": [
                {"ts": "1.0", "user": "U1", "text": "parent", "thread_ts": "1.0"},
                {"ts": "1.1", "user": "U2", "text": "reply", "thread_ts": "1.0"}
            ],
            "response_metadata": {"next_cursor": ""}
        }"#;

        let resp: ConversationRepliesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.messages.len(), 2);
        assert_eq!(resp.messages[0].thread_ts, "1.0");
        assert_eq!(resp.messages[1].text, "reply");
    }

    #[test]
    fn test_response_metadata_empty_cursor() {
        let meta = ResponseMetadata::default();
        assert!(meta.next_cursor.is_empty());
    }

    #[test]
    fn test_post_message_response_deserialization() {
        let json = r#"{
            "ok": true,
            "channel": "C123",
            "ts": "1234567890.123456"
        }"#;

        let resp: PostMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.channel, "C123");
        assert_eq!(resp.timestamp, "1234567890.123456");
    }

    #[test]
    fn test_post_message_response_minimal() {
        let json = r#"{"ok": true}"#;
        let resp: PostMessageResponse = serde_json::from_str(json).unwrap();
        assert!(resp.channel.is_empty());
        assert!(resp.timestamp.is_empty());
    }
}
