//! Convert Unicode PICO-8 source text into the cartridge's byte character set.
//!
//! The mapping follows `k_charset`, `k_unicap_chars`, and `to_p8str` in the
//! vendored Shrinko8 `pico_defs.py`. Glyphs with U+FE0F are two Unicode scalars
//! representing one P8SCII byte. Unrecognized characters retain their UTF-8
//! bytes, matching PICO-8's source conversion rather than replacing text.

const P8SCII: [&str; 256] = [
    "\u{0}", // 0
    "¹",     // 1
    "²",     // 2
    "³",     // 3
    "⁴",     // 4
    "⁵",     // 5
    "⁶",     // 6
    "⁷",     // 7
    "⁸",     // 8
    "\u{9}", // 9
    "\u{a}", // 10
    "ᵇ",     // 11
    "ᶜ",     // 12
    "\u{d}", // 13
    "ᵉ",     // 14
    "ᶠ",     // 15
    "▮",     // 16
    "■",     // 17
    "□",     // 18
    "⁙",     // 19
    "⁘",     // 20
    "‖",     // 21
    "◀",     // 22
    "▶",     // 23
    "「",    // 24
    "」",    // 25
    "¥",     // 26
    "•",     // 27
    "、",    // 28
    "。",    // 29
    "゛",    // 30
    "゜",    // 31
    " ",     // 32
    "!",     // 33
    "\"",    // 34
    "#",     // 35
    "$",     // 36
    "%",     // 37
    "&",     // 38
    "'",     // 39
    "(",     // 40
    ")",     // 41
    "*",     // 42
    "+",     // 43
    ",",     // 44
    "-",     // 45
    ".",     // 46
    "/",     // 47
    "0",     // 48
    "1",     // 49
    "2",     // 50
    "3",     // 51
    "4",     // 52
    "5",     // 53
    "6",     // 54
    "7",     // 55
    "8",     // 56
    "9",     // 57
    ":",     // 58
    ";",     // 59
    "<",     // 60
    "=",     // 61
    ">",     // 62
    "?",     // 63
    "@",     // 64
    "A",     // 65
    "B",     // 66
    "C",     // 67
    "D",     // 68
    "E",     // 69
    "F",     // 70
    "G",     // 71
    "H",     // 72
    "I",     // 73
    "J",     // 74
    "K",     // 75
    "L",     // 76
    "M",     // 77
    "N",     // 78
    "O",     // 79
    "P",     // 80
    "Q",     // 81
    "R",     // 82
    "S",     // 83
    "T",     // 84
    "U",     // 85
    "V",     // 86
    "W",     // 87
    "X",     // 88
    "Y",     // 89
    "Z",     // 90
    "[",     // 91
    "\\",    // 92
    "]",     // 93
    "^",     // 94
    "_",     // 95
    "`",     // 96
    "a",     // 97
    "b",     // 98
    "c",     // 99
    "d",     // 100
    "e",     // 101
    "f",     // 102
    "g",     // 103
    "h",     // 104
    "i",     // 105
    "j",     // 106
    "k",     // 107
    "l",     // 108
    "m",     // 109
    "n",     // 110
    "o",     // 111
    "p",     // 112
    "q",     // 113
    "r",     // 114
    "s",     // 115
    "t",     // 116
    "u",     // 117
    "v",     // 118
    "w",     // 119
    "x",     // 120
    "y",     // 121
    "z",     // 122
    "{",     // 123
    "|",     // 124
    "}",     // 125
    "~",     // 126
    "○",     // 127
    "█",     // 128
    "▒",     // 129
    "🐱",    // 130
    "⬇️",    // 131
    "░",     // 132
    "✽",     // 133
    "●",     // 134
    "♥",     // 135
    "☉",     // 136
    "웃",    // 137
    "⌂",     // 138
    "⬅️",    // 139
    "😐",    // 140
    "♪",     // 141
    "🅾️",    // 142
    "◆",     // 143
    "…",     // 144
    "➡️",    // 145
    "★",     // 146
    "⧗",     // 147
    "⬆️",    // 148
    "ˇ",     // 149
    "∧",     // 150
    "❎",    // 151
    "▤",     // 152
    "▥",     // 153
    "あ",    // 154
    "い",    // 155
    "う",    // 156
    "え",    // 157
    "お",    // 158
    "か",    // 159
    "き",    // 160
    "く",    // 161
    "け",    // 162
    "こ",    // 163
    "さ",    // 164
    "し",    // 165
    "す",    // 166
    "せ",    // 167
    "そ",    // 168
    "た",    // 169
    "ち",    // 170
    "つ",    // 171
    "て",    // 172
    "と",    // 173
    "な",    // 174
    "に",    // 175
    "ぬ",    // 176
    "ね",    // 177
    "の",    // 178
    "は",    // 179
    "ひ",    // 180
    "ふ",    // 181
    "へ",    // 182
    "ほ",    // 183
    "ま",    // 184
    "み",    // 185
    "む",    // 186
    "め",    // 187
    "も",    // 188
    "や",    // 189
    "ゆ",    // 190
    "よ",    // 191
    "ら",    // 192
    "り",    // 193
    "る",    // 194
    "れ",    // 195
    "ろ",    // 196
    "わ",    // 197
    "を",    // 198
    "ん",    // 199
    "っ",    // 200
    "ゃ",    // 201
    "ゅ",    // 202
    "ょ",    // 203
    "ア",    // 204
    "イ",    // 205
    "ウ",    // 206
    "エ",    // 207
    "オ",    // 208
    "カ",    // 209
    "キ",    // 210
    "ク",    // 211
    "ケ",    // 212
    "コ",    // 213
    "サ",    // 214
    "シ",    // 215
    "ス",    // 216
    "セ",    // 217
    "ソ",    // 218
    "タ",    // 219
    "チ",    // 220
    "ツ",    // 221
    "テ",    // 222
    "ト",    // 223
    "ナ",    // 224
    "ニ",    // 225
    "ヌ",    // 226
    "ネ",    // 227
    "ノ",    // 228
    "ハ",    // 229
    "ヒ",    // 230
    "フ",    // 231
    "ヘ",    // 232
    "ホ",    // 233
    "マ",    // 234
    "ミ",    // 235
    "ム",    // 236
    "メ",    // 237
    "モ",    // 238
    "ヤ",    // 239
    "ユ",    // 240
    "ヨ",    // 241
    "ラ",    // 242
    "リ",    // 243
    "ル",    // 244
    "レ",    // 245
    "ロ",    // 246
    "ワ",    // 247
    "ヲ",    // 248
    "ン",    // 249
    "ッ",    // 250
    "ャ",    // 251
    "ュ",    // 252
    "ョ",    // 253
    "◜",     // 254
    "◝",     // 255
];

const UNICAPS: [&str; 26] = [
    "𝘢", "𝘣", "𝘤", "𝘥", "𝘦", "𝘧", "𝘨", "𝘩", "𝘪", "𝘫", "𝘬", "𝘭", "𝘮", "𝘯", "𝘰", "𝘱", "𝘲", "𝘳", "𝘴",
    "𝘵", "𝘶", "𝘷", "𝘸", "𝘹", "𝘺", "𝘻",
];

/// Encode source characters as P8SCII, preserving escaped raw byte markers.
///
/// The lexer uses U+E080..U+E0FF for numeric string escapes so those byte values
/// remain distinct from source Unicode characters. ASCII is already encoded.
pub fn bytes(source: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len());
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        if character.is_ascii() {
            output.push(character as u8);
            continue;
        }
        if (0xe080..=0xe0ff).contains(&(character as u32)) {
            output.push((character as u32 - 0xe000) as u8);
            continue;
        }
        let mut utf8 = [0u8; 4];
        let scalar: &str = character.encode_utf8(&mut utf8);
        // Prefer a complete glyph sequence to its constituent Unicode scalars.
        if let Some(&next) = characters.peek() {
            let mut pair = String::from(scalar);
            pair.push(next);
            if let Some(index) = P8SCII.iter().position(|glyph| *glyph == pair) {
                output.push(index as u8);
                characters.next();
                continue;
            }
        }
        if let Some(index) = P8SCII.iter().position(|glyph| *glyph == scalar) {
            output.push(index as u8);
        } else if let Some(index) = UNICAPS.iter().position(|glyph| *glyph == scalar) {
            output.push(b'A' + index as u8);
        } else {
            output.extend_from_slice(scalar.as_bytes());
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_ascii_and_escaped_raw_bytes() {
        assert_eq!(bytes("Hello, WORLD!\n\t\0"), b"Hello, WORLD!\n\t\0");
        assert_eq!(bytes("\u{e080}\u{e094}\u{e0ff}"), [128, 148, 255]);
    }

    #[test]
    fn encodes_variation_selector_glyphs_as_single_bytes() {
        assert_eq!(bytes("⬆️⬇️⬅️➡️🅾️❎"), [148, 131, 139, 145, 142, 151]);
        assert_eq!(bytes("⬆"), "⬆".as_bytes());
    }

    #[test]
    fn encodes_japanese_and_unicode_capitals() {
        assert_eq!(bytes("あア"), [154, 204]);
        assert_eq!(
            bytes("𝘢𝘣𝘤𝘥𝘦𝘧𝘨𝘩𝘪𝘫𝘬𝘭𝘮𝘯𝘰𝘱𝘲𝘳𝘴𝘵𝘶𝘷𝘸𝘹𝘺𝘻"),
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        );
    }

    #[test]
    fn preserves_unknown_unicode_utf8() {
        assert_eq!(bytes("é🙂"), "é🙂".as_bytes());
    }

    #[test]
    fn every_cartridge_character_maps_back_to_its_byte() {
        for (index, glyph) in P8SCII.iter().enumerate() {
            assert_eq!(bytes(glyph), [index as u8], "glyph {index}");
        }
    }
}
