/// Compile-time emoji dataset: (name, emoji_char)
pub const EMOJI_DATA: &[(&str, &str)] = &[
    // Smileys & Emotion
    ("grinning", "😀"), ("smiley", "😃"), ("smile", "😄"), ("grin", "😁"),
    ("laughing", "😆"), ("sweat_smile", "😅"), ("rofl", "🤣"), ("joy", "😂"),
    ("slightly_smiling_face", "🙂"), ("wink", "😉"), ("blush", "😊"),
    ("innocent", "😇"), ("heart_eyes", "😍"), ("star_struck", "🤩"),
    ("kissing_heart", "😘"), ("yum", "😋"), ("stuck_out_tongue", "😛"),
    ("thinking", "🤔"), ("shushing", "🤫"), ("zipper_mouth", "🤐"),
    ("raised_eyebrow", "🤨"), ("neutral_face", "😐"), ("expressionless", "😑"),
    ("no_mouth", "😶"), ("smirk", "😏"), ("unamused", "😒"),
    ("rolling_eyes", "🙄"), ("grimacing", "😬"), ("lying_face", "🤥"),
    ("relieved", "😌"), ("pensive", "😔"), ("sleepy", "😪"),
    ("drooling", "🤤"), ("sleeping", "😴"), ("mask", "😷"),
    ("nerd", "🤓"), ("sunglasses", "😎"), ("clown", "🤡"),
    ("cowboy", "🤠"), ("partying", "🥳"), ("confused", "😕"),
    ("worried", "😟"), ("frowning", "☹️"), ("persevere", "😣"),
    ("confounded", "😖"), ("tired", "😫"), ("weary", "😩"),
    ("pleading", "🥺"), ("cry", "😢"), ("sob", "😭"),
    ("scream", "😱"), ("rage", "😡"), ("angry", "😠"),
    ("skull", "💀"), ("poop", "💩"), ("ghost", "👻"),
    ("alien", "👽"), ("robot", "🤖"), ("clap", "👏"),
    // Gestures
    ("thumbsup", "👍"), ("thumbsdown", "👎"), ("punch", "👊"),
    ("wave", "👋"), ("ok_hand", "👌"), ("pinching_hand", "🤏"),
    ("v", "✌️"), ("crossed_fingers", "🤞"), ("love_you", "🤟"),
    ("metal", "🤘"), ("point_up", "☝️"), ("point_down", "👇"),
    ("point_left", "👈"), ("point_right", "👉"), ("middle_finger", "🖕"),
    ("raised_hand", "✋"), ("muscle", "💪"), ("pray", "🙏"),
    ("handshake", "🤝"), ("heart_hands", "🫶"), ("palms_up", "🤲"),
    // Hearts & symbols
    ("heart", "❤️"), ("orange_heart", "🧡"), ("yellow_heart", "💛"),
    ("green_heart", "💚"), ("blue_heart", "💙"), ("purple_heart", "💜"),
    ("black_heart", "🖤"), ("white_heart", "🤍"), ("broken_heart", "💔"),
    ("sparkling_heart", "💖"), ("100", "💯"), ("boom", "💥"),
    ("fire", "🔥"), ("star", "⭐"), ("sparkles", "✨"),
    ("zap", "⚡"), ("snowflake", "❄️"),
    // People & Nature
    ("eyes", "👀"), ("brain", "🧠"), ("baby", "👶"),
    ("dog", "🐶"), ("cat", "🐱"), ("fox", "🦊"),
    ("bear", "🐻"), ("unicorn", "🦄"), ("bee", "🐝"),
    ("butterfly", "🦋"), ("turtle", "🐢"), ("snake", "🐍"),
    ("whale", "🐳"), ("dolphin", "🐬"), ("crab", "🦀"),
    // Nature
    ("sunflower", "🌻"), ("rose", "🌹"), ("tulip", "🌷"),
    ("cherry_blossom", "🌸"), ("four_leaf_clover", "🍀"),
    ("christmas_tree", "🎄"), ("cactus", "🌵"), ("mushroom", "🍄"),
    // Food & Drink
    ("pizza", "🍕"), ("hamburger", "🍔"), ("fries", "🍟"),
    ("taco", "🌮"), ("sushi", "🍣"), ("ramen", "🍜"),
    ("ice_cream", "🍦"), ("cake", "🎂"), ("cookie", "🍪"),
    ("coffee", "☕"), ("beer", "🍺"), ("wine", "🍷"),
    ("tropical_drink", "🍹"), ("popcorn", "🍿"),
    // Activities
    ("soccer", "⚽"), ("basketball", "🏀"), ("football", "🏈"),
    ("baseball", "⚾"), ("tennis", "🎾"), ("trophy", "🏆"),
    ("medal", "🏅"), ("video_game", "🎮"), ("dart", "🎯"),
    ("guitar", "🎸"), ("music", "🎵"), ("microphone", "🎤"),
    ("headphones", "🎧"), ("art", "🎨"), ("film", "🎬"),
    // Travel & Places
    ("car", "🚗"), ("rocket", "🚀"), ("airplane", "✈️"),
    ("earth", "🌍"), ("moon", "🌙"), ("sun", "☀️"),
    ("rainbow", "🌈"), ("umbrella", "☂️"), ("snowman", "⛄"),
    ("house", "🏠"), ("tent", "⛺"),
    // Objects
    ("gift", "🎁"), ("balloon", "🎈"), ("tada", "🎉"),
    ("confetti_ball", "🎊"), ("bell", "🔔"), ("megaphone", "📣"),
    ("book", "📖"), ("pencil", "✏️"), ("bulb", "💡"),
    ("wrench", "🔧"), ("hammer", "🔨"), ("key", "🔑"),
    ("lock", "🔒"), ("link", "🔗"), ("gem", "💎"),
    ("money", "💰"), ("credit_card", "💳"), ("envelope", "✉️"),
    ("package", "📦"), ("phone", "📱"), ("computer", "💻"),
    ("keyboard", "⌨️"), ("clock", "🕐"),
    // Symbols
    ("check", "✅"), ("x", "❌"), ("warning", "⚠️"),
    ("question", "❓"), ("exclamation", "❗"), ("no_entry", "⛔"),
    ("recycle", "♻️"), ("white_check_mark", "✔️"),
    ("new", "🆕"), ("free", "🆓"), ("sos", "🆘"),
    ("arrow_up", "⬆️"), ("arrow_down", "⬇️"),
    ("arrow_left", "⬅️"), ("arrow_right", "➡️"),
    // Flags
    ("flag_us", "🇺🇸"), ("flag_gb", "🇬🇧"), ("flag_jp", "🇯🇵"),
    ("flag_fr", "🇫🇷"), ("flag_de", "🇩🇪"),
    ("rainbow_flag", "🏳️‍🌈"), ("pirate_flag", "🏴‍☠️"),
];

/// Category definitions: (category_name, &[emoji_names])
pub const EMOJI_CATEGORIES: &[(&str, &[&str])] = &[
    ("Smileys", &["grinning", "smiley", "smile", "grin", "laughing", "sweat_smile", "rofl", "joy", "slightly_smiling_face", "wink", "blush", "innocent", "heart_eyes", "star_struck", "kissing_heart", "yum", "stuck_out_tongue", "thinking", "shushing", "zipper_mouth", "raised_eyebrow", "neutral_face", "expressionless", "no_mouth", "smirk", "unamused", "rolling_eyes", "grimacing", "lying_face", "relieved", "pensive", "sleepy", "drooling", "sleeping", "mask", "nerd", "sunglasses", "clown", "cowboy", "partying", "confused", "worried", "frowning", "persevere", "confounded", "tired", "weary", "pleading", "cry", "sob", "scream", "rage", "angry", "skull", "poop", "ghost", "alien", "robot", "clap"]),
    ("Gestures", &["thumbsup", "thumbsdown", "punch", "wave", "ok_hand", "pinching_hand", "v", "crossed_fingers", "love_you", "metal", "point_up", "point_down", "point_left", "point_right", "middle_finger", "raised_hand", "muscle", "pray", "handshake", "heart_hands", "palms_up"]),
    ("Hearts", &["heart", "orange_heart", "yellow_heart", "green_heart", "blue_heart", "purple_heart", "black_heart", "white_heart", "broken_heart", "sparkling_heart", "100", "boom", "fire", "star", "sparkles", "zap", "snowflake"]),
    ("People", &["eyes", "brain", "baby", "dog", "cat", "fox", "bear", "unicorn", "bee", "butterfly", "turtle", "snake", "whale", "dolphin", "crab"]),
    ("Nature", &["sunflower", "rose", "tulip", "cherry_blossom", "four_leaf_clover", "christmas_tree", "cactus", "mushroom"]),
    ("Food", &["pizza", "hamburger", "fries", "taco", "sushi", "ramen", "ice_cream", "cake", "cookie", "coffee", "beer", "wine", "tropical_drink", "popcorn"]),
    ("Activities", &["soccer", "basketball", "football", "baseball", "tennis", "trophy", "medal", "video_game", "dart", "guitar", "music", "microphone", "headphones", "art", "film"]),
    ("Travel", &["car", "rocket", "airplane", "earth", "moon", "sun", "rainbow", "umbrella", "snowman", "house", "tent"]),
    ("Objects", &["gift", "balloon", "tada", "confetti_ball", "bell", "megaphone", "book", "pencil", "bulb", "wrench", "hammer", "key", "lock", "link", "gem", "money", "credit_card", "envelope", "package", "phone", "computer", "keyboard", "clock"]),
    ("Symbols", &["check", "x", "warning", "question", "exclamation", "no_entry", "recycle", "white_check_mark", "new", "free", "sos", "arrow_up", "arrow_down", "arrow_left", "arrow_right"]),
    ("Flags", &["flag_us", "flag_gb", "flag_jp", "flag_fr", "flag_de", "rainbow_flag", "pirate_flag"]),
];

/// Look up emoji character by name
pub fn emoji_by_name(name: &str) -> Option<&'static str> {
    EMOJI_DATA.iter().find(|(n, _)| *n == name).map(|(_, e)| *e)
}
