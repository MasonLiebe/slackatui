use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<@(\w+(?:\|\w+)?)>").unwrap());

static EMOJI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(\w+):").unwrap());

/// Parse a Slack message body: expand mentions, emoji, and HTML entities.
pub fn parse_message(
    text: &str,
    emoji_enabled: bool,
    user_cache: &HashMap<String, String>,
) -> String {
    let mut result = text.to_string();

    if emoji_enabled {
        result = parse_emoji(&result);
    }

    result = parse_mentions(&result, user_cache);
    result = htmlescape::decode_html(&result).unwrap_or(result);

    result
}

/// Replace `<@U12345|name>` or `<@U12345>` with `@name`.
/// Uses the user cache to resolve user IDs to names.
fn parse_mentions(text: &str, user_cache: &HashMap<String, String>) -> String {
    MENTION_RE
        .replace_all(text, |caps: &regex::Captures| {
            let inner = &caps[1];
            let user_id = inner.split('|').next().unwrap_or(inner);

            let name = user_cache
                .get(user_id)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            format!("@{}", name)
        })
        .into_owned()
}

/// Replace `:emoji_name:` with the corresponding Unicode emoji.
/// Uses the `emojis` crate for lookup by GitHub/Slack shortcode.
fn parse_emoji(text: &str) -> String {
    EMOJI_RE
        .replace_all(text, |caps: &regex::Captures| {
            let shortcode = &caps[1];
            match emojis::get_by_shortcode(shortcode) {
                Some(emoji) => emoji.as_str().to_string(),
                None => caps[0].to_string(), // Leave unknown shortcodes as-is
            }
        })
        .into_owned()
}

/// Convert a Slack timestamp (e.g. "1234567890.123456") to a base62 short ID.
/// Used for thread references so users can type `/thread <id> <msg>`.
pub fn hash_id(timestamp: &str) -> String {
    const BASE62: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";

    let float_val: f64 = timestamp.parse().unwrap_or(0.0);
    let mut input = float_val as u64;

    if input == 0 {
        return String::new();
    }

    let mut hash = String::new();
    while input > 0 {
        hash.insert(0, BASE62[(input % 62) as usize] as char);
        input /= 62;
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Mention parsing ----

    #[test]
    fn test_parse_mention_with_name() {
        let mut cache = HashMap::new();
        cache.insert("U12345".to_string(), "alice".to_string());

        let result = parse_mentions("<@U12345|alice> hello", &cache);
        assert_eq!(result, "@alice hello");
    }

    #[test]
    fn test_parse_mention_without_pipe() {
        let mut cache = HashMap::new();
        cache.insert("U12345".to_string(), "bob".to_string());

        let result = parse_mentions("<@U12345> hello", &cache);
        assert_eq!(result, "@bob hello");
    }

    #[test]
    fn test_parse_mention_unknown_user() {
        let cache = HashMap::new();
        let result = parse_mentions("<@U99999> hello", &cache);
        assert_eq!(result, "@unknown hello");
    }

    #[test]
    fn test_parse_multiple_mentions() {
        let mut cache = HashMap::new();
        cache.insert("U1".to_string(), "alice".to_string());
        cache.insert("U2".to_string(), "bob".to_string());

        let result = parse_mentions("<@U1> and <@U2> are here", &cache);
        assert_eq!(result, "@alice and @bob are here");
    }

    #[test]
    fn test_parse_no_mentions() {
        let cache = HashMap::new();
        let result = parse_mentions("hello world", &cache);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_parse_mention_in_middle() {
        let mut cache = HashMap::new();
        cache.insert("U1".to_string(), "alice".to_string());

        let result = parse_mentions("hey <@U1>, how are you?", &cache);
        assert_eq!(result, "hey @alice, how are you?");
    }

    // ---- Emoji parsing ----

    #[test]
    fn test_parse_emoji_known() {
        let result = parse_emoji(":thumbsup:");
        assert_eq!(result, "👍");
    }

    #[test]
    fn test_parse_emoji_heart() {
        let result = parse_emoji(":heart:");
        assert_eq!(result, "❤\u{fe0f}");
    }

    #[test]
    fn test_parse_emoji_unknown() {
        let result = parse_emoji(":nonexistent_emoji_xyz:");
        assert_eq!(result, ":nonexistent_emoji_xyz:");
    }

    #[test]
    fn test_parse_emoji_multiple() {
        let result = parse_emoji(":thumbsup: great :heart:");
        assert!(result.contains("👍"));
        assert!(result.contains("great"));
    }

    #[test]
    fn test_parse_emoji_no_emoji() {
        let result = parse_emoji("no emoji here");
        assert_eq!(result, "no emoji here");
    }

    // ---- HTML unescaping ----

    #[test]
    fn test_parse_message_html_entities() {
        let cache = HashMap::new();
        let result = parse_message("hello &amp; world &lt;tag&gt;", false, &cache);
        assert_eq!(result, "hello & world <tag>");
    }

    #[test]
    fn test_parse_message_combined() {
        let mut cache = HashMap::new();
        cache.insert("U1".to_string(), "alice".to_string());

        let result = parse_message("<@U1> said &amp; :thumbsup:", true, &cache);
        assert!(result.contains("@alice"));
        assert!(result.contains("&"));
        assert!(result.contains("👍"));
    }

    #[test]
    fn test_parse_message_emoji_disabled() {
        let cache = HashMap::new();
        let result = parse_message(":thumbsup: hello", false, &cache);
        assert_eq!(result, ":thumbsup: hello");
    }

    // ---- hash_id ----

    #[test]
    fn test_hash_id_basic() {
        let id = hash_id("1234567890.123456");
        assert!(!id.is_empty());
        // Should be a short base62 string
        for c in id.chars() {
            assert!(c.is_ascii_alphanumeric());
        }
    }

    #[test]
    fn test_hash_id_deterministic() {
        let id1 = hash_id("1234567890.123456");
        let id2 = hash_id("1234567890.123456");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_hash_id_different_inputs() {
        let id1 = hash_id("1234567890.000000");
        let id2 = hash_id("9876543210.000000");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_hash_id_zero() {
        let id = hash_id("0.0");
        assert!(id.is_empty());
    }

    #[test]
    fn test_hash_id_invalid() {
        let id = hash_id("not_a_number");
        assert!(id.is_empty());
    }

    #[test]
    fn test_hash_id_matches_go_behavior() {
        // The Go code does: int(parseFloat(timestamp))
        // So "1234567890.999" -> 1234567890 -> base62
        let id = hash_id("1234567890.999");
        let id2 = hash_id("1234567890.000");
        // Both should produce the same hash since int truncation
        assert_eq!(id, id2);
    }
}
