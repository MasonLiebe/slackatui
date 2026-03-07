use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tokio_tungstenite::tungstenite;
use tracing::warn;

use super::SlackClient;

/// Events emitted by the RTM connection for the application to handle.
#[derive(Debug, Clone)]
pub enum RtmEvent {
    Message(MessageEvent),
    PresenceChange(PresenceChangeEvent),
    Error(String),
    Connected,
    Disconnected,
}

/// A message event from the RTM stream.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageEvent {
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub thread_ts: String,
    #[serde(default, rename = "subtype")]
    pub sub_type: String,
    #[serde(default)]
    pub bot_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub previous_message: Option<SubMessage>,
    #[serde(default)]
    pub message: Option<SubMessage>,
}

/// Sub-message within a message event (used for edits and replies).
#[derive(Debug, Clone, Deserialize)]
pub struct SubMessage {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub thread_ts: String,
    #[serde(default)]
    pub bot_id: String,
    #[serde(default)]
    pub username: String,
}

/// A presence change event from the RTM stream.
#[derive(Debug, Clone, Deserialize)]
pub struct PresenceChangeEvent {
    pub user: String,
    pub presence: String,
}

/// Raw RTM envelope — we just need the type field to dispatch.
#[derive(Debug, Deserialize)]
struct RtmEnvelope {
    #[serde(default, rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub reply_to: Option<u64>,
}

const PING_INTERVAL: Duration = Duration::from_secs(30);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Manages the RTM WebSocket connection with auto-reconnect.
/// Returns a receiver that emits RtmEvents.
pub fn start_rtm(
    client: SlackClient,
) -> mpsc::UnboundedReceiver<RtmEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        loop {
            match run_rtm_connection(&client, &tx).await {
                Ok(()) => {
                    // Clean disconnect
                    let _ = tx.send(RtmEvent::Disconnected);
                    break;
                }
                Err(e) => {
                    warn!("RTM connection error: {}, reconnecting in {:?}", e, RECONNECT_DELAY);
                    let _ = tx.send(RtmEvent::Error(e.to_string()));
                    let _ = tx.send(RtmEvent::Disconnected);
                    time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    });

    rx
}

/// Runs a single RTM WebSocket session. Returns on disconnect or error.
async fn run_rtm_connection(
    client: &SlackClient,
    tx: &mpsc::UnboundedSender<RtmEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get WebSocket URL
    let rtm_resp = client.rtm_connect().await?;
    let url = rtm_resp.url;

    // Connect
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let _ = tx.send(RtmEvent::Connected);

    let mut ping_interval = time::interval(PING_INTERVAL);
    let mut msg_id: u64 = 1;

    loop {
        tokio::select! {
            // Incoming WebSocket messages
            msg = ws_read.next() => {
                match msg {
                    Some(Ok(tungstenite::Message::Text(text))) => {
                        if let Some(event) = parse_rtm_event(&text) {
                            if tx.send(event).is_err() {
                                // Receiver dropped, shut down
                                return Ok(());
                            }
                        }
                    }
                    Some(Ok(tungstenite::Message::Ping(data))) => {
                        ws_write.send(tungstenite::Message::Pong(data)).await?;
                    }
                    Some(Ok(tungstenite::Message::Close(_))) | None => {
                        return Err("WebSocket closed".into());
                    }
                    Some(Err(e)) => {
                        return Err(Box::new(e));
                    }
                    _ => {} // Binary, Pong, Frame — ignore
                }
            }

            // Periodic ping to keep the connection alive
            _ = ping_interval.tick() => {
                let ping = serde_json::json!({
                    "id": msg_id,
                    "type": "ping",
                });
                msg_id += 1;
                ws_write.send(tungstenite::Message::Text(ping.to_string().into())).await?;
            }
        }
    }
}

/// Parse a raw RTM JSON message into an RtmEvent.
fn parse_rtm_event(text: &str) -> Option<RtmEvent> {
    let envelope: RtmEnvelope = serde_json::from_str(text).ok()?;

    // Ignore replies to our pings
    if envelope.reply_to.is_some() {
        return None;
    }

    match envelope.event_type.as_str() {
        "message" => {
            let msg: MessageEvent = serde_json::from_str(text).ok()?;
            Some(RtmEvent::Message(msg))
        }
        "presence_change" => {
            let ev: PresenceChangeEvent = serde_json::from_str(text).ok()?;
            Some(RtmEvent::PresenceChange(ev))
        }
        "error" => {
            // Extract error description
            #[derive(Deserialize)]
            struct RtmError {
                #[serde(default)]
                error: RtmErrorDetail,
            }
            #[derive(Default, Deserialize)]
            struct RtmErrorDetail {
                #[serde(default)]
                msg: String,
            }
            let err: RtmError = serde_json::from_str(text).ok()?;
            Some(RtmEvent::Error(err.error.msg))
        }
        // hello, user_typing, etc. — ignored
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_event() {
        let json = r#"{
            "type": "message",
            "channel": "C123",
            "user": "U456",
            "text": "Hello world!",
            "ts": "1234567890.123456"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::Message(msg) => {
                assert_eq!(msg.channel, "C123");
                assert_eq!(msg.user, "U456");
                assert_eq!(msg.text, "Hello world!");
                assert_eq!(msg.ts, "1234567890.123456");
            }
            _ => panic!("expected Message event"),
        }
    }

    #[test]
    fn test_parse_message_changed() {
        let json = r#"{
            "type": "message",
            "subtype": "message_changed",
            "channel": "C123",
            "message": {
                "user": "U456",
                "text": "edited text",
                "ts": "1234.5678",
                "thread_ts": ""
            },
            "previous_message": {
                "user": "U456",
                "text": "original text",
                "ts": "1234.5678",
                "thread_ts": "1234.0000"
            },
            "ts": "9999.0000"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::Message(msg) => {
                assert_eq!(msg.sub_type, "message_changed");
                assert_eq!(msg.channel, "C123");
                let sub = msg.message.unwrap();
                assert_eq!(sub.text, "edited text");
                let prev = msg.previous_message.unwrap();
                assert_eq!(prev.text, "original text");
                assert_eq!(prev.thread_ts, "1234.0000");
            }
            _ => panic!("expected Message event"),
        }
    }

    #[test]
    fn test_parse_message_with_thread() {
        let json = r#"{
            "type": "message",
            "channel": "C123",
            "user": "U456",
            "text": "thread reply",
            "ts": "1234.9999",
            "thread_ts": "1234.5678"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::Message(msg) => {
                assert_eq!(msg.thread_ts, "1234.5678");
                assert_ne!(msg.ts, msg.thread_ts);
            }
            _ => panic!("expected Message event"),
        }
    }

    #[test]
    fn test_parse_bot_message() {
        let json = r#"{
            "type": "message",
            "subtype": "bot_message",
            "channel": "C123",
            "bot_id": "B789",
            "username": "testbot",
            "text": "bot says hi",
            "ts": "1234.5678"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::Message(msg) => {
                assert_eq!(msg.sub_type, "bot_message");
                assert_eq!(msg.bot_id, "B789");
                assert_eq!(msg.username, "testbot");
                assert!(msg.user.is_empty());
            }
            _ => panic!("expected Message event"),
        }
    }

    #[test]
    fn test_parse_presence_change() {
        let json = r#"{
            "type": "presence_change",
            "user": "U123",
            "presence": "active"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::PresenceChange(ev) => {
                assert_eq!(ev.user, "U123");
                assert_eq!(ev.presence, "active");
            }
            _ => panic!("expected PresenceChange event"),
        }
    }

    #[test]
    fn test_parse_presence_away() {
        let json = r#"{
            "type": "presence_change",
            "user": "U456",
            "presence": "away"
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::PresenceChange(ev) => {
                assert_eq!(ev.presence, "away");
            }
            _ => panic!("expected PresenceChange event"),
        }
    }

    #[test]
    fn test_parse_rtm_error() {
        let json = r#"{
            "type": "error",
            "error": {
                "code": 1,
                "msg": "Socket URL has expired"
            }
        }"#;

        let event = parse_rtm_event(json).unwrap();
        match event {
            RtmEvent::Error(msg) => {
                assert_eq!(msg, "Socket URL has expired");
            }
            _ => panic!("expected Error event"),
        }
    }

    #[test]
    fn test_parse_ping_reply_ignored() {
        let json = r#"{"reply_to": 1, "ok": true}"#;
        let event = parse_rtm_event(json);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_hello_ignored() {
        let json = r#"{"type": "hello"}"#;
        let event = parse_rtm_event(json);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_user_typing_ignored() {
        let json = r#"{"type": "user_typing", "channel": "C123", "user": "U456"}"#;
        let event = parse_rtm_event(json);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let event = parse_rtm_event("not json {{{");
        assert!(event.is_none());
    }

    #[test]
    fn test_message_event_deserialization() {
        let json = r#"{
            "channel": "C123",
            "user": "U456",
            "text": "hello",
            "ts": "1.0",
            "thread_ts": "",
            "subtype": "",
            "bot_id": "",
            "username": ""
        }"#;

        let msg: MessageEvent = serde_json::from_str(json).unwrap();
        assert_eq!(msg.channel, "C123");
        assert!(msg.previous_message.is_none());
        assert!(msg.message.is_none());
    }

    #[test]
    fn test_sub_message_deserialization() {
        let json = r#"{
            "user": "U789",
            "text": "sub text",
            "ts": "1.5",
            "thread_ts": "1.0"
        }"#;

        let sub: SubMessage = serde_json::from_str(json).unwrap();
        assert_eq!(sub.user, "U789");
        assert_eq!(sub.text, "sub text");
        assert_eq!(sub.thread_ts, "1.0");
    }

    #[test]
    fn test_rtm_event_enum_variants() {
        // Ensure all variants can be constructed
        let _ = RtmEvent::Connected;
        let _ = RtmEvent::Disconnected;
        let _ = RtmEvent::Error("test".into());
        let _ = RtmEvent::Message(MessageEvent {
            channel: String::new(),
            user: String::new(),
            text: String::new(),
            ts: String::new(),
            thread_ts: String::new(),
            sub_type: String::new(),
            bot_id: String::new(),
            username: String::new(),
            previous_message: None,
            message: None,
        });
        let _ = RtmEvent::PresenceChange(PresenceChangeEvent {
            user: "U1".into(),
            presence: "active".into(),
        });
    }
}
