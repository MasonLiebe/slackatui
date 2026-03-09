use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<@(\w+(?:\|\w+)?)>").unwrap());

static EMOJI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([\w+\-]+):").unwrap());

/// Matches Slack-formatted links: `<http://url|label>` or `<http://url>` or `<mailto:...>`
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<((?:https?://|mailto:)[^|>]+)(?:\|([^>]+))?>").unwrap());

/// Resolve a single emoji shortcode to its Unicode string.
/// Returns the Unicode emoji if found, otherwise `:name:`.
pub fn resolve_emoji(name: &str) -> String {
    if let Some(emoji) = emojis::get_by_shortcode(name) {
        emoji.as_str().to_string()
    } else if let Some(&emoji_str) = SLACK_EMOJI_ALIASES.get(name) {
        emoji_str.to_string()
    } else {
        format!(":{}:", name)
    }
}

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

    result = parse_links(&result);
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

/// Replace Slack-formatted links with OSC 8 terminal hyperlinks.
/// `<http://url|label>` → clickable `label`, `<http://url>` → clickable URL.
fn parse_links(text: &str) -> String {
    LINK_RE
        .replace_all(text, |caps: &regex::Captures| {
            let url = &caps[1];
            let label = caps.get(2).map(|m| m.as_str()).unwrap_or(url);
            // OSC 8 terminal hyperlink: \x1b]8;;URL\x1b\\LABEL\x1b]8;;\x1b\\
            format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, label)
        })
        .into_owned()
}

/// Slack emoji shortcodes that differ from GitHub/Unicode shortcodes.
/// Maps Slack name -> Unicode emoji string.
pub static SLACK_EMOJI_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Faces & expressions
    m.insert("smiley", "\u{1F603}");
    m.insert("smile", "\u{1F604}");
    m.insert("grinning", "\u{1F600}");
    m.insert("laughing", "\u{1F606}");
    m.insert("satisfied", "\u{1F606}");
    m.insert("sweat_smile", "\u{1F605}");
    m.insert("rofl", "\u{1F923}");
    m.insert("joy", "\u{1F602}");
    m.insert("slightly_smiling_face", "\u{1F642}");
    m.insert("upside_down_face", "\u{1F643}");
    m.insert("wink", "\u{1F609}");
    m.insert("blush", "\u{1F60A}");
    m.insert("innocent", "\u{1F607}");
    m.insert("smirk", "\u{1F60F}");
    m.insert("relieved", "\u{1F60C}");
    m.insert("pensive", "\u{1F614}");
    m.insert("confused", "\u{1F615}");
    m.insert("disappointed", "\u{1F61E}");
    m.insert("cry", "\u{1F622}");
    m.insert("sob", "\u{1F62D}");
    m.insert("angry", "\u{1F620}");
    m.insert("rage", "\u{1F621}");
    m.insert("exploding_head", "\u{1F92F}");
    m.insert("flushed", "\u{1F633}");
    m.insert("scream", "\u{1F631}");
    m.insert("fearful", "\u{1F628}");
    m.insert("cold_sweat", "\u{1F630}");
    m.insert("thinking_face", "\u{1F914}");
    m.insert("face_with_rolling_eyes", "\u{1F644}");
    m.insert("hushed", "\u{1F62F}");
    m.insert("grimacing", "\u{1F62C}");
    m.insert("lying_face", "\u{1F925}");
    m.insert("zipper_mouth_face", "\u{1F910}");
    m.insert("nerd_face", "\u{1F913}");
    m.insert("sunglasses", "\u{1F60E}");
    m.insert("star-struck", "\u{1F929}");
    m.insert("partying_face", "\u{1F973}");
    m.insert("smiling_face_with_tear", "\u{1F972}");
    m.insert("yawning_face", "\u{1F971}");
    m.insert("face_with_hand_over_mouth", "\u{1F92D}");
    m.insert("shushing_face", "\u{1F92B}");
    m.insert("stuck_out_tongue", "\u{1F61B}");
    m.insert("stuck_out_tongue_winking_eye", "\u{1F61C}");
    m.insert("stuck_out_tongue_closed_eyes", "\u{1F61D}");
    m.insert("drooling_face", "\u{1F924}");
    m.insert("unamused", "\u{1F612}");
    m.insert("weary", "\u{1F629}");
    m.insert("pleading_face", "\u{1F97A}");
    m.insert("sleeping", "\u{1F634}");
    m.insert("sleepy", "\u{1F62A}");
    m.insert("mask", "\u{1F637}");
    m.insert("face_with_thermometer", "\u{1F912}");
    m.insert("nauseated_face", "\u{1F922}");
    m.insert("face_vomiting", "\u{1F92E}");
    m.insert("hot_face", "\u{1F975}");
    m.insert("cold_face", "\u{1F976}");
    m.insert("woozy_face", "\u{1F974}");
    m.insert("dizzy_face", "\u{1F635}");
    m.insert("no_mouth", "\u{1F636}");
    m.insert("neutral_face", "\u{1F610}");
    m.insert("expressionless", "\u{1F611}");
    m.insert("open_mouth", "\u{1F62E}");
    m.insert("astonished", "\u{1F632}");
    m.insert("worried", "\u{1F61F}");
    m.insert("anguished", "\u{1F627}");
    m.insert("persevere", "\u{1F623}");
    m.insert("triumph", "\u{1F624}");
    m.insert("yum", "\u{1F60B}");
    m.insert("kissing_heart", "\u{1F618}");
    m.insert("kissing", "\u{1F617}");
    m.insert("kissing_closed_eyes", "\u{1F61A}");
    m.insert("kissing_smiling_eyes", "\u{1F619}");
    m.insert("heart_eyes", "\u{1F60D}");
    m.insert("hugging_face", "\u{1F917}");
    m.insert("money_mouth_face", "\u{1F911}");
    m.insert("face_with_monocle", "\u{1F9D0}");
    m.insert("clown_face", "\u{1F921}");
    m.insert("cowboy_hat_face", "\u{1F920}");
    // Monkeys
    m.insert("see_no_evil", "\u{1F648}");
    m.insert("hear_no_evil", "\u{1F649}");
    m.insert("speak_no_evil", "\u{1F64A}");
    // Characters
    m.insert("skull", "\u{1F480}");
    m.insert("skull_and_crossbones", "\u{2620}\u{FE0F}");
    m.insert("ghost", "\u{1F47B}");
    m.insert("alien", "\u{1F47E}");
    m.insert("robot_face", "\u{1F916}");
    m.insert("jack_o_lantern", "\u{1F383}");
    m.insert("poop", "\u{1F4A9}");
    m.insert("hankey", "\u{1F4A9}");
    // Hands & gestures
    m.insert("thumbsup", "\u{1F44D}");
    m.insert("+1", "\u{1F44D}");
    m.insert("thumbsdown", "\u{1F44E}");
    m.insert("-1", "\u{1F44E}");
    m.insert("wave", "\u{1F44B}");
    m.insert("clap", "\u{1F44F}");
    m.insert("raised_hands", "\u{1F64C}");
    m.insert("open_hands", "\u{1F450}");
    m.insert("palms_up_together", "\u{1F932}");
    m.insert("handshake", "\u{1F91D}");
    m.insert("pray", "\u{1F64F}");
    m.insert("ok_hand", "\u{1F44C}");
    m.insert("pinching_hand", "\u{1F90F}");
    m.insert("v", "\u{270C}\u{FE0F}");
    m.insert("crossed_fingers", "\u{1F91E}");
    m.insert("metal", "\u{1F918}");
    m.insert("call_me_hand", "\u{1F919}");
    m.insert("point_left", "\u{1F448}");
    m.insert("point_right", "\u{1F449}");
    m.insert("point_up_2", "\u{1F446}");
    m.insert("point_up", "\u{261D}\u{FE0F}");
    m.insert("point_down", "\u{1F447}");
    m.insert("middle_finger", "\u{1F595}");
    m.insert("raised_hand", "\u{270B}");
    m.insert("raised_back_of_hand", "\u{1F91A}");
    m.insert("muscle", "\u{1F4AA}");
    m.insert("fist", "\u{270A}");
    m.insert("punch", "\u{1F44A}");
    m.insert("facepunch", "\u{1F44A}");
    m.insert("writing_hand", "\u{270D}\u{FE0F}");
    m.insert("eyes", "\u{1F440}");
    m.insert("eye", "\u{1F441}\u{FE0F}");
    m.insert("brain", "\u{1F9E0}");
    // Hearts
    m.insert("heart", "\u{2764}\u{FE0F}");
    m.insert("orange_heart", "\u{1F9E1}");
    m.insert("yellow_heart", "\u{1F49B}");
    m.insert("green_heart", "\u{1F49A}");
    m.insert("blue_heart", "\u{1F499}");
    m.insert("purple_heart", "\u{1F49C}");
    m.insert("black_heart", "\u{1F5A4}");
    m.insert("white_heart", "\u{1F90D}");
    m.insert("brown_heart", "\u{1F90E}");
    m.insert("broken_heart", "\u{1F494}");
    m.insert("heartbeat", "\u{1F493}");
    m.insert("heartpulse", "\u{1F497}");
    m.insert("two_hearts", "\u{1F495}");
    m.insert("sparkling_heart", "\u{1F496}");
    m.insert("revolving_hearts", "\u{1F49E}");
    m.insert("heavy_heart_exclamation_mark_ornament", "\u{2763}\u{FE0F}");
    m.insert("heart_on_fire", "\u{2764}\u{FE0F}\u{200D}\u{1F525}");
    // Symbols & marks
    m.insert("100", "\u{1F4AF}");
    m.insert("white_check_mark", "\u{2705}");
    m.insert("heavy_check_mark", "\u{2714}\u{FE0F}");
    m.insert("ballot_box_with_check", "\u{2611}\u{FE0F}");
    m.insert("x", "\u{274C}");
    m.insert("heavy_multiplication_x", "\u{2716}\u{FE0F}");
    m.insert("warning", "\u{26A0}\u{FE0F}");
    m.insert("no_entry", "\u{26D4}");
    m.insert("no_entry_sign", "\u{1F6AB}");
    m.insert("bangbang", "\u{203C}\u{FE0F}");
    m.insert("interrobang", "\u{2049}\u{FE0F}");
    m.insert("question", "\u{2753}");
    m.insert("grey_question", "\u{2754}");
    m.insert("exclamation", "\u{2757}");
    m.insert("grey_exclamation", "\u{2755}");
    m.insert("recycle", "\u{267B}\u{FE0F}");
    m.insert("infinity", "\u{267E}\u{FE0F}");
    // Objects & things
    m.insert("fire", "\u{1F525}");
    m.insert("tada", "\u{1F389}");
    m.insert("sparkles", "\u{2728}");
    m.insert("star", "\u{2B50}");
    m.insert("star2", "\u{1F31F}");
    m.insert("dizzy", "\u{1F4AB}");
    m.insert("boom", "\u{1F4A5}");
    m.insert("collision", "\u{1F4A5}");
    m.insert("zap", "\u{26A1}");
    m.insert("sunny", "\u{2600}\u{FE0F}");
    m.insert("rainbow", "\u{1F308}");
    m.insert("cloud", "\u{2601}\u{FE0F}");
    m.insert("snowflake", "\u{2744}\u{FE0F}");
    m.insert("umbrella", "\u{2602}\u{FE0F}");
    m.insert("rocket", "\u{1F680}");
    m.insert("airplane", "\u{2708}\u{FE0F}");
    m.insert("gem", "\u{1F48E}");
    m.insert("bulb", "\u{1F4A1}");
    m.insert("bell", "\u{1F514}");
    m.insert("mega", "\u{1F4E3}");
    m.insert("loudspeaker", "\u{1F4E2}");
    m.insert("speech_balloon", "\u{1F4AC}");
    m.insert("thought_balloon", "\u{1F4AD}");
    m.insert("link", "\u{1F517}");
    m.insert("lock", "\u{1F512}");
    m.insert("unlock", "\u{1F513}");
    m.insert("key", "\u{1F511}");
    m.insert("pushpin", "\u{1F4CC}");
    m.insert("round_pushpin", "\u{1F4CD}");
    m.insert("paperclip", "\u{1F4CE}");
    m.insert("scissors", "\u{2702}\u{FE0F}");
    m.insert("pencil2", "\u{270F}\u{FE0F}");
    m.insert("pencil", "\u{1F4DD}");
    m.insert("memo", "\u{1F4DD}");
    m.insert("mag", "\u{1F50D}");
    m.insert("mag_right", "\u{1F50E}");
    m.insert("package", "\u{1F4E6}");
    m.insert("inbox_tray", "\u{1F4E5}");
    m.insert("outbox_tray", "\u{1F4E4}");
    m.insert("email", "\u{1F4E7}");
    m.insert("envelope", "\u{2709}\u{FE0F}");
    m.insert("envelope_with_arrow", "\u{1F4E9}");
    m.insert("bookmark", "\u{1F516}");
    m.insert("clipboard", "\u{1F4CB}");
    m.insert("calendar", "\u{1F4C5}");
    m.insert("date", "\u{1F4C5}");
    m.insert("chart_with_upwards_trend", "\u{1F4C8}");
    m.insert("chart_with_downwards_trend", "\u{1F4C9}");
    m.insert("bar_chart", "\u{1F4CA}");
    m.insert("file_folder", "\u{1F4C1}");
    m.insert("open_file_folder", "\u{1F4C2}");
    m.insert("wastebasket", "\u{1F5D1}\u{FE0F}");
    m.insert("gear", "\u{2699}\u{FE0F}");
    m.insert("wrench", "\u{1F527}");
    m.insert("hammer", "\u{1F528}");
    m.insert("hammer_and_wrench", "\u{1F6E0}\u{FE0F}");
    m.insert("shield", "\u{1F6E1}\u{FE0F}");
    // Food & drink
    m.insert("coffee", "\u{2615}");
    m.insert("tea", "\u{1F375}");
    m.insert("beer", "\u{1F37A}");
    m.insert("beers", "\u{1F37B}");
    m.insert("wine_glass", "\u{1F377}");
    m.insert("cocktail", "\u{1F378}");
    m.insert("tropical_drink", "\u{1F379}");
    m.insert("champagne", "\u{1F37E}");
    m.insert("pizza", "\u{1F355}");
    m.insert("hamburger", "\u{1F354}");
    m.insert("taco", "\u{1F32E}");
    m.insert("burrito", "\u{1F32F}");
    m.insert("cookie", "\u{1F36A}");
    m.insert("cake", "\u{1F370}");
    m.insert("birthday", "\u{1F382}");
    m.insert("icecream", "\u{1F366}");
    m.insert("doughnut", "\u{1F369}");
    m.insert("apple", "\u{1F34E}");
    m.insert("green_apple", "\u{1F34F}");
    // Animals
    m.insert("dog", "\u{1F436}");
    m.insert("cat", "\u{1F431}");
    m.insert("mouse", "\u{1F42D}");
    m.insert("hamster", "\u{1F439}");
    m.insert("rabbit", "\u{1F430}");
    m.insert("bear", "\u{1F43B}");
    m.insert("panda_face", "\u{1F43C}");
    m.insert("koala", "\u{1F428}");
    m.insert("tiger", "\u{1F42F}");
    m.insert("lion_face", "\u{1F981}");
    m.insert("cow", "\u{1F42E}");
    m.insert("pig", "\u{1F437}");
    m.insert("frog", "\u{1F438}");
    m.insert("monkey_face", "\u{1F435}");
    m.insert("chicken", "\u{1F414}");
    m.insert("penguin", "\u{1F427}");
    m.insert("bird", "\u{1F426}");
    m.insert("eagle", "\u{1F985}");
    m.insert("fox_face", "\u{1F98A}");
    m.insert("wolf", "\u{1F43A}");
    m.insert("unicorn_face", "\u{1F984}");
    m.insert("bee", "\u{1F41D}");
    m.insert("honeybee", "\u{1F41D}");
    m.insert("bug", "\u{1F41B}");
    m.insert("butterfly", "\u{1F98B}");
    m.insert("snail", "\u{1F40C}");
    m.insert("octopus", "\u{1F419}");
    m.insert("snake", "\u{1F40D}");
    m.insert("turtle", "\u{1F422}");
    m.insert("crab", "\u{1F980}");
    m.insert("whale", "\u{1F433}");
    m.insert("dolphin", "\u{1F42C}");
    m.insert("fish", "\u{1F41F}");
    m.insert("tropical_fish", "\u{1F420}");
    m.insert("shark", "\u{1F988}");
    // Nature & plants
    m.insert("deciduous_tree", "\u{1F333}");
    m.insert("evergreen_tree", "\u{1F332}");
    m.insert("palm_tree", "\u{1F334}");
    m.insert("cactus", "\u{1F335}");
    m.insert("seedling", "\u{1F331}");
    m.insert("herb", "\u{1F33F}");
    m.insert("shamrock", "\u{2618}\u{FE0F}");
    m.insert("four_leaf_clover", "\u{1F340}");
    m.insert("fallen_leaf", "\u{1F342}");
    m.insert("leaves", "\u{1F343}");
    m.insert("maple_leaf", "\u{1F341}");
    m.insert("mushroom", "\u{1F344}");
    m.insert("rose", "\u{1F339}");
    m.insert("sunflower", "\u{1F33B}");
    m.insert("tulip", "\u{1F337}");
    m.insert("cherry_blossom", "\u{1F338}");
    m.insert("bouquet", "\u{1F490}");
    // Misc popular
    m.insert("party_popper", "\u{1F389}");
    m.insert("confetti_ball", "\u{1F38A}");
    m.insert("balloon", "\u{1F388}");
    m.insert("gift", "\u{1F381}");
    m.insert("trophy", "\u{1F3C6}");
    m.insert("medal", "\u{1F3C5}");
    m.insert("crown", "\u{1F451}");
    m.insert("money_with_wings", "\u{1F4B8}");
    m.insert("moneybag", "\u{1F4B0}");
    m.insert("dollar", "\u{1F4B5}");
    m.insert("credit_card", "\u{1F4B3}");
    m.insert("computer", "\u{1F4BB}");
    m.insert("keyboard", "\u{2328}\u{FE0F}");
    m.insert("desktop_computer", "\u{1F5A5}\u{FE0F}");
    m.insert("iphone", "\u{1F4F1}");
    m.insert("telephone_receiver", "\u{1F4DE}");
    m.insert("tv", "\u{1F4FA}");
    m.insert("camera", "\u{1F4F7}");
    m.insert("video_camera", "\u{1F4F9}");
    m.insert("movie_camera", "\u{1F3A5}");
    m.insert("microphone", "\u{1F3A4}");
    m.insert("headphones", "\u{1F3A7}");
    m.insert("speaker", "\u{1F508}");
    m.insert("mute", "\u{1F507}");
    m.insert("sound", "\u{1F509}");
    m.insert("loud_sound", "\u{1F50A}");
    m.insert("hourglass", "\u{231B}");
    m.insert("hourglass_flowing_sand", "\u{23F3}");
    m.insert("stopwatch", "\u{23F1}\u{FE0F}");
    m.insert("alarm_clock", "\u{23F0}");
    m.insert("clock1", "\u{1F550}");
    m.insert("timer_clock", "\u{23F2}\u{FE0F}");
    // Arrows & directions
    m.insert("arrow_up", "\u{2B06}\u{FE0F}");
    m.insert("arrow_down", "\u{2B07}\u{FE0F}");
    m.insert("arrow_left", "\u{2B05}\u{FE0F}");
    m.insert("arrow_right", "\u{27A1}\u{FE0F}");
    m.insert("arrow_upper_right", "\u{2197}\u{FE0F}");
    m.insert("arrow_lower_right", "\u{2198}\u{FE0F}");
    m.insert("arrow_upper_left", "\u{2196}\u{FE0F}");
    m.insert("arrow_lower_left", "\u{2199}\u{FE0F}");
    m.insert("leftwards_arrow_with_hook", "\u{21A9}\u{FE0F}");
    m.insert("arrow_right_hook", "\u{21AA}\u{FE0F}");
    m.insert("arrows_counterclockwise", "\u{1F504}");
    m.insert("arrows_clockwise", "\u{1F503}");
    // Activities & sports
    m.insert("soccer", "\u{26BD}");
    m.insert("basketball", "\u{1F3C0}");
    m.insert("football", "\u{1F3C8}");
    m.insert("baseball", "\u{26BE}");
    m.insert("tennis", "\u{1F3BE}");
    m.insert("golf", "\u{26F3}");
    m.insert("video_game", "\u{1F3AE}");
    m.insert("dart", "\u{1F3AF}");
    m.insert("game_die", "\u{1F3B2}");
    m.insert("musical_note", "\u{1F3B5}");
    m.insert("notes", "\u{1F3B6}");
    m.insert("art", "\u{1F3A8}");
    // Flags & misc
    m.insert("checkered_flag", "\u{1F3C1}");
    m.insert("triangular_flag_on_post", "\u{1F6A9}");
    m.insert("crossed_flags", "\u{1F38C}");
    m.insert("white_flag", "\u{1F3F3}\u{FE0F}");
    m.insert("pirate_flag", "\u{1F3F4}\u{200D}\u{2620}\u{FE0F}");
    // Transport
    m.insert("car", "\u{1F697}");
    m.insert("red_car", "\u{1F697}");
    m.insert("taxi", "\u{1F695}");
    m.insert("bus", "\u{1F68C}");
    m.insert("ambulance", "\u{1F691}");
    m.insert("fire_engine", "\u{1F692}");
    m.insert("bike", "\u{1F6B2}");
    m.insert("ship", "\u{1F6A2}");
    m.insert("helicopter", "\u{1F681}");
    // People
    m.insert("man", "\u{1F468}");
    m.insert("woman", "\u{1F469}");
    m.insert("boy", "\u{1F466}");
    m.insert("girl", "\u{1F467}");
    m.insert("baby", "\u{1F476}");
    m.insert("older_man", "\u{1F474}");
    m.insert("older_woman", "\u{1F475}");
    m.insert("person_frowning", "\u{1F64D}");
    m.insert("person_with_pouting_face", "\u{1F64E}");
    m.insert("no_good", "\u{1F645}");
    m.insert("ok_woman", "\u{1F646}");
    m.insert("information_desk_person", "\u{1F481}");
    m.insert("raising_hand", "\u{1F64B}");
    m.insert("bow", "\u{1F647}");
    m.insert("man_technologist", "\u{1F468}\u{200D}\u{1F4BB}");
    m.insert("woman_technologist", "\u{1F469}\u{200D}\u{1F4BB}");
    m.insert("dancer", "\u{1F483}");
    m.insert("man_dancing", "\u{1F57A}");
    // Buildings & places
    m.insert("house", "\u{1F3E0}");
    m.insert("office", "\u{1F3E2}");
    m.insert("hospital", "\u{1F3E5}");
    m.insert("school", "\u{1F3EB}");
    m.insert("earth_americas", "\u{1F30E}");
    m.insert("earth_europe", "\u{1F30D}");
    m.insert("earth_asia", "\u{1F30F}");
    m.insert("globe_with_meridians", "\u{1F310}");
    m
});

/// Replace `:emoji_name:` with the corresponding Unicode emoji.
/// Tries the `emojis` crate first (GitHub shortcodes), then falls back
/// to the Slack-specific alias table.
fn parse_emoji(text: &str) -> String {
    EMOJI_RE
        .replace_all(text, |caps: &regex::Captures| {
            let shortcode = &caps[1];
            if let Some(emoji) = emojis::get_by_shortcode(shortcode) {
                emoji.as_str().to_string()
            } else if let Some(&emoji_str) = SLACK_EMOJI_ALIASES.get(shortcode) {
                emoji_str.to_string()
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

/// A styled segment of parsed mrkdwn text.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub code_block: bool,
    pub code_language: String,
    pub mention: bool,
}

impl StyledSegment {
    fn plain(text: String) -> Self {
        Self {
            text,
            bold: false,
            italic: false,
            strikethrough: false,
            code: false,
            code_block: false,
            code_language: String::new(),
            mention: false,
        }
    }
}

static MRKDWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Match code blocks (with optional language), inline code, bold, italic, strikethrough, @mentions
    // Order matters: longer/greedy patterns first
    Regex::new(r"(?s)```(\w*)\n?(.*?)```|`([^`]+)`|\*([^*]+)\*|_([^_]+)_|~([^~]+)~|(@[\w.\-]+)").unwrap()
});

/// Parse Slack mrkdwn formatting into styled segments.
/// Handles: `*bold*`, `_italic_`, `~strikethrough~`, `` `code` ``, `\`\`\`code blocks\`\`\``, `@mentions`
pub fn parse_mrkdwn(text: &str) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let mut last_end = 0;

    for caps in MRKDWN_RE.captures_iter(text) {
        let m = caps.get(0).unwrap();
        if m.start() > last_end {
            segments.push(StyledSegment::plain(text[last_end..m.start()].to_string()));
        }

        if let Some(code_body) = caps.get(2) {
            // Fenced code block: ```lang\ncode```
            let lang = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            segments.push(StyledSegment {
                text: code_body.as_str().to_string(),
                code: true,
                code_block: true,
                code_language: lang.to_string(),
                ..StyledSegment::plain(String::new())
            });
        } else if let Some(inline_code) = caps.get(3) {
            segments.push(StyledSegment {
                text: inline_code.as_str().to_string(),
                code: true,
                ..StyledSegment::plain(String::new())
            });
        } else if let Some(bold) = caps.get(4) {
            segments.push(StyledSegment {
                text: bold.as_str().to_string(),
                bold: true,
                ..StyledSegment::plain(String::new())
            });
        } else if let Some(italic) = caps.get(5) {
            segments.push(StyledSegment {
                text: italic.as_str().to_string(),
                italic: true,
                ..StyledSegment::plain(String::new())
            });
        } else if let Some(strike) = caps.get(6) {
            segments.push(StyledSegment {
                text: strike.as_str().to_string(),
                strikethrough: true,
                ..StyledSegment::plain(String::new())
            });
        } else if let Some(mention) = caps.get(7) {
            segments.push(StyledSegment {
                text: mention.as_str().to_string(),
                mention: true,
                ..StyledSegment::plain(String::new())
            });
        }

        last_end = m.end();
    }

    if last_end < text.len() {
        segments.push(StyledSegment::plain(text[last_end..].to_string()));
    }

    if segments.is_empty() {
        segments.push(StyledSegment::plain(text.to_string()));
    }

    segments
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

    #[test]
    fn test_parse_emoji_slack_aliases() {
        assert_eq!(parse_emoji(":smiley:"), "\u{1F603}");
        assert_eq!(parse_emoji(":smile:"), "\u{1F604}");
        assert_eq!(parse_emoji(":joy:"), "\u{1F602}");
        assert_eq!(parse_emoji(":fire:"), "\u{1F525}");
        assert_eq!(parse_emoji(":tada:"), "\u{1F389}");
        assert_eq!(parse_emoji(":100:"), "\u{1F4AF}");
        assert_eq!(parse_emoji(":rocket:"), "\u{1F680}");
        assert_eq!(parse_emoji(":eyes:"), "\u{1F440}");
        assert_eq!(parse_emoji(":pray:"), "\u{1F64F}");
        assert_eq!(parse_emoji(":thinking_face:"), "\u{1F914}");
        assert_eq!(parse_emoji(":wave:"), "\u{1F44B}");
        assert_eq!(parse_emoji(":+1:"), "\u{1F44D}");
        assert_eq!(parse_emoji(":white_check_mark:"), "\u{2705}");
        assert_eq!(parse_emoji(":warning:"), "\u{26A0}\u{FE0F}");
        assert_eq!(parse_emoji(":slightly_smiling_face:"), "\u{1F642}");
        assert_eq!(parse_emoji(":skull:"), "\u{1F480}");
        assert_eq!(parse_emoji(":coffee:"), "\u{2615}");
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

    // ---- parse_links ----

    #[test]
    fn test_parse_link_with_label() {
        let result = parse_links("<https://example.com|example.com>");
        assert!(result.contains("example.com"));
        assert!(result.contains("https://example.com"));
        // Should contain OSC 8 sequences
        assert!(result.contains("\x1b]8;;"));
        assert!(!result.contains('<'));
    }

    #[test]
    fn test_parse_link_without_label() {
        let result = parse_links("<https://example.com/path>");
        assert!(result.contains("https://example.com/path"));
        assert!(result.contains("\x1b]8;;"));
        assert!(!result.contains('<'));
    }

    #[test]
    fn test_parse_link_mailto() {
        let result = parse_links("<mailto:user@example.com|user@example.com>");
        assert!(result.contains("user@example.com"));
        assert!(result.contains("\x1b]8;;mailto:user@example.com"));
    }

    #[test]
    fn test_parse_link_mixed_with_text() {
        let result = parse_links("Check out <https://foo.bar|foo.bar> for more");
        assert!(result.starts_with("Check out "));
        assert!(result.ends_with(" for more"));
        assert!(result.contains("\x1b]8;;https://foo.bar"));
    }

    #[test]
    fn test_parse_link_multiple() {
        let result = parse_links("<https://a.com|A> and <https://b.com|B>");
        assert!(result.contains("\x1b]8;;https://a.com"));
        assert!(result.contains("\x1b]8;;https://b.com"));
    }

    #[test]
    fn test_parse_link_does_not_touch_mentions() {
        // <@U123> should not be matched by the link regex
        let result = parse_links("<@U123>");
        assert_eq!(result, "<@U123>");
    }

    #[test]
    fn test_parse_message_with_link() {
        let cache = HashMap::new();
        let result = parse_message("visit <https://mgldev.xyz|mgldev.xyz> now", false, &cache);
        assert!(result.contains("mgldev.xyz"));
        assert!(!result.contains("<https://"));
        assert!(result.contains("\x1b]8;;"));
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

    // ---- mrkdwn parsing ----

    #[test]
    fn test_mrkdwn_plain() {
        let segs = parse_mrkdwn("hello world");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "hello world");
        assert!(!segs[0].bold);
    }

    #[test]
    fn test_mrkdwn_bold() {
        let segs = parse_mrkdwn("this is *bold* text");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].text, "this is ");
        assert_eq!(segs[1].text, "bold");
        assert!(segs[1].bold);
        assert_eq!(segs[2].text, " text");
    }

    #[test]
    fn test_mrkdwn_italic() {
        let segs = parse_mrkdwn("this is _italic_ text");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1].text, "italic");
        assert!(segs[1].italic);
    }

    #[test]
    fn test_mrkdwn_strikethrough() {
        let segs = parse_mrkdwn("this is ~struck~ text");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1].text, "struck");
        assert!(segs[1].strikethrough);
    }

    #[test]
    fn test_mrkdwn_inline_code() {
        let segs = parse_mrkdwn("run `cargo build` now");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1].text, "cargo build");
        assert!(segs[1].code);
    }

    #[test]
    fn test_mrkdwn_code_block() {
        // ```fn main() {}``` — "fn" is captured as language, " main() {}" as body
        let segs = parse_mrkdwn("```fn main() {}```");
        assert_eq!(segs.len(), 1);
        assert!(segs[0].code);
        assert!(segs[0].code_block);
        assert_eq!(segs[0].code_language, "fn");
        assert_eq!(segs[0].text, " main() {}");
    }

    #[test]
    fn test_mrkdwn_code_block_no_language() {
        let segs = parse_mrkdwn("```\nhello world\n```");
        assert_eq!(segs.len(), 1);
        assert!(segs[0].code_block);
        assert_eq!(segs[0].code_language, "");
        assert_eq!(segs[0].text, "hello world\n");
    }

    #[test]
    fn test_mrkdwn_code_block_with_language() {
        let segs = parse_mrkdwn("```rust\nfn main() {}\n```");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "fn main() {}\n");
        assert!(segs[0].code_block);
        assert_eq!(segs[0].code_language, "rust");
    }

    #[test]
    fn test_mrkdwn_mention() {
        let segs = parse_mrkdwn("hello @alice and @bob");
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[1].text, "@alice");
        assert!(segs[1].mention);
        assert_eq!(segs[3].text, "@bob");
        assert!(segs[3].mention);
    }

    #[test]
    fn test_mrkdwn_mixed() {
        let segs = parse_mrkdwn("*bold* and _italic_ and `code`");
        assert_eq!(segs.len(), 5);
        assert!(segs[0].bold);
        assert_eq!(segs[1].text, " and ");
        assert!(segs[2].italic);
        assert_eq!(segs[3].text, " and ");
        assert!(segs[4].code);
    }

    #[test]
    fn test_mrkdwn_bullet_passthrough() {
        // Bullets are just text, not special formatting
        let segs = parse_mrkdwn("• item one\n• item two");
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("• item one"));
    }
}
