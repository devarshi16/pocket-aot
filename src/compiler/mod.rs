mod charset;
mod emit;
mod parse;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone)]
pub struct Error {
    pub line: usize,
    pub message: String,
}
impl Error {
    pub fn new(line: usize, message: &str) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}
impl std::error::Error for Error {}

pub struct Output {
    pub c: String,
    pub assets: String,
    pub apis: Vec<String>,
    pub warnings: Vec<String>,
}
fn radix_number(body: &str, radix: u32, fractional_digits: usize) -> Result<i32, Error> {
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let invalid = || {
        Error::new(
            0,
            if radix == 2 {
                "invalid binary number"
            } else {
                "invalid hexadecimal number"
            },
        )
    };
    // At least one digit is required, including when the integer part is omitted.
    // Checking the complete body also rejects a second point instead of silently
    // accepting only the first fraction.
    if (whole.is_empty() && fraction.is_empty())
        || !whole.chars().all(|ch| ch.is_digit(radix))
        || !fraction.chars().all(|ch| ch.is_digit(radix))
    {
        return Err(invalid());
    }
    if fraction.len() > fractional_digits {
        return Err(Error::new(
            0,
            if radix == 2 {
                "binary fractions support at most 16 bits"
            } else {
                "hex fractions support at most four digits"
            },
        ));
    }
    let whole = if whole.is_empty() {
        0
    } else {
        u32::from_str_radix(whole, radix).map_err(|_| invalid())?
    };
    let fraction = if fraction.is_empty() {
        0
    } else {
        let bits_per_digit = if radix == 2 { 1 } else { 4 };
        u32::from_str_radix(fraction, radix).map_err(|_| invalid())?
            << ((fractional_digits - fraction.len()) * bits_per_digit)
    };
    Ok((whole.wrapping_shl(16) | fraction) as i32)
}
fn number(s: &str) -> Result<i32, Error> {
    if s.starts_with("0b") || s.starts_with("0B") {
        return radix_number(&s[2..], 2, 16);
    }
    if s.starts_with("0x") || s.starts_with("0X") {
        return radix_number(&s[2..], 16, 4);
    }
    let value = s
        .parse::<f64>()
        .map_err(|_| Error::new(0, "invalid decimal number"))?;
    // PICO-8 wraps large literals into 16.16 range: 63191 reads as -2345,
    // which still peeks address 63191.
    if !value.is_finite() || value.abs() >= 2f64.powi(47) {
        return Err(Error::new(0, "number outside PICO-8 16.16 range"));
    }
    Ok(((value * 65536.0).round() as i64) as i32)
}
fn array(name: &str, bytes: &[u8]) -> String {
    let mut out = format!("const uint8_t {name}[{}]={{\n", bytes.len());
    for c in bytes.chunks(32) {
        out.push_str(&c.iter().map(u8::to_string).collect::<Vec<_>>().join(","));
        out.push_str(",\n");
    }
    out.push_str("};\n");
    out
}

/// Music (0x3100) and SFX (0x3200) bytes in PICO-8 memory layout, from the
/// text sections. An SFX line is 4 header bytes (editor mode, speed, loop
/// start, loop end) then 32 notes of pitch(2) waveform(1) volume(1) effect(1)
/// hex digits. A music line is a flag byte and four channel bytes.
fn sound_rom(sfx: Option<&String>, music: Option<&String>) -> Result<Vec<u8>, Error> {
    let mut rom = vec![0u8; 0x1200];
    let hex = |text: &str| {
        u32::from_str_radix(text, 16).map_err(|_| Error::new(0, "invalid hex sound data"))
    };
    if let Some(section) = sfx {
        for (n, line) in section.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let line = line.trim();
            if n >= 64 || line.len() != 168 || !line.is_ascii() {
                return Err(Error::new(0, "invalid __sfx__ dimensions"));
            }
            let base = 0x100 + n * 68;
            for k in 0..4 {
                rom[base + 64 + k] = hex(&line[k * 2..k * 2 + 2])? as u8;
            }
            for note in 0..32 {
                let at = 8 + note * 5;
                let pitch = hex(&line[at..at + 2])? & 63;
                let wave = hex(&line[at + 2..at + 3])?;
                let volume = hex(&line[at + 3..at + 4])? & 7;
                let effect = hex(&line[at + 4..at + 5])? & 7;
                let bits = pitch | (wave & 7) << 6 | volume << 9 | effect << 12 | (wave >> 3) << 15;
                rom[base + note * 2] = bits as u8;
                rom[base + note * 2 + 1] = (bits >> 8) as u8;
            }
        }
    }
    if let Some(section) = music {
        for (n, line) in section.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if n >= 64 || fields.len() != 2 || fields[0].len() != 2 || fields[1].len() != 8 {
                return Err(Error::new(0, "invalid __music__ row"));
            }
            let flags = hex(fields[0])?;
            for ch in 0..4 {
                let mut byte = hex(&fields[1][ch * 2..ch * 2 + 2])? as u8 & 0x7f;
                if ch < 3 && flags & (1 << ch) != 0 {
                    byte |= 0x80;
                }
                rom[n * 4 + ch] = byte;
            }
        }
    }
    Ok(rom)
}

pub fn compile(text: &str, title: &str) -> Result<Output, Error> {
    if !text.starts_with("pico-8 cartridge") {
        return Err(Error::new(
            1,
            "initial converter accepts PICO-8 text .p8 cartridges only",
        ));
    }
    let mut sections: BTreeMap<&str, String> = BTreeMap::new();
    let mut section = "header";
    for line in text.lines() {
        if line.starts_with("__") && line.ends_with("__") {
            section = line;
            sections.entry(section).or_default();
        } else {
            sections
                .entry(section)
                .or_default()
                .push_str(&format!("{line}\n"));
        }
    }
    let lua = sections
        .get("__lua__")
        .ok_or_else(|| Error::new(1, "missing __lua__ section"))?;
    let mut p = parse::Parser::new(lua)?;
    let ast = p.parse()?;
    let (c, apis) = emit::emit(&ast, title).map_err(|mut e| {
        if e.line == 0 {
            e.message = format!("conversion stopped: {}", e.message);
        }
        e
    })?;
    let mut gfx = vec![0u8; 8192];
    let mut map = vec![0u8; 4096];
    let mut flags = vec![0u8; 256];
    if let Some(s) = sections.get("__gfx__") {
        for (y, line) in s.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            if y >= 128 || line.len() != 128 {
                return Err(Error::new(0, "invalid __gfx__ dimensions"));
            }
            for (x, ch) in line.chars().enumerate() {
                let n = ch
                    .to_digit(16)
                    .ok_or_else(|| Error::new(0, "non-hex sprite data"))?
                    as u8;
                gfx[y * 64 + x / 2] |= n << ((x % 2) * 4);
            }
        }
    }
    for (key, target, width) in [("__map__", &mut map, 256), ("__gff__", &mut flags, 256)] {
        if let Some(s) = sections.get(key) {
            let mut offset = 0;
            for line in s.lines().filter(|l| !l.trim().is_empty()) {
                if line.len() > width || line.len() % 2 != 0 {
                    return Err(Error::new(0, "invalid asset row"));
                }
                for pair in line.as_bytes().chunks(2) {
                    if offset >= target.len() {
                        return Err(Error::new(0, "too much map/flag data"));
                    }
                    target[offset] = u8::from_str_radix(
                        std::str::from_utf8(pair).map_err(|_| Error::new(0, "non-ASCII asset"))?,
                        16,
                    )
                    .map_err(|_| Error::new(0, "invalid hex asset"))?;
                    offset += 1;
                }
            }
        }
    }
    let sound = sound_rom(sections.get("__sfx__"), sections.get("__music__"))?;
    let assets = "#include <stdint.h>\n".to_string()
        + &array("p8_cart_gfx", &gfx)
        + &array("p8_cart_map", &map)
        + &array("p8_cart_flags", &flags)
        + &array("p8_cart_sound", &sound);
    let mut warnings = vec![
        "Experimental subset; not full PICO-8 compatibility.".into(),
        "Extended text glyphs use placeholders; RNG is not PICO-8-identical.".into(),
    ];
    if apis.iter().any(|a| a == "sfx" || a == "music") {
        warnings.push("Sound calls are silent: this LCD has no audio hardware.".into());
    }
    if apis.iter().any(|a| a == "stat") {
        warnings
            .push("stat(1) reports native instruction-budget usage, not PICO-8 CPU timing.".into());
    }
    Ok(Output {
        c,
        assets,
        apis,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_input_fixture_generates_native_functions() {
        let o = compile(
            "pico-8 cartridge\n__lua__\nx=0\nfunction _update60() if btnp(0) then x+=1 end end\nfunction _draw() cls(1) pset(x,1,8) end",
            "Input fixture",
        )
        .unwrap();
        assert!(o.c.contains("p8_dispatch"));
        assert!(o.apis.contains(&"btnp".into()));
        assert!(o.c.contains("p8_target_hz=60"));
    }
    #[test]
    fn closures_generate_native_dispatch() {
        let o=compile("pico-8 cartridge\n__lua__\nlocal x=0\nf=function() x+=1 return x end\nfunction _draw() f() end", "x").unwrap();
        assert!(o.c.contains("p8_closure("));
        assert!(o.c.contains("p8_cell_set("));
    }
    #[test]
    fn fixed_literals_wrap_hex_and_keep_fractions() {
        assert_eq!(number("0xffff.ffff").unwrap(), -1);
        assert_eq!(number("2.5").unwrap(), 163840);
    }

    #[test]
    fn radix_fractions_without_integer_digits_keep_exact_fixed_bits() {
        for (source, bits) in [
            ("0x.8", 0x0000_8000),
            ("0X.0001", 0x0000_0001),
            ("0x.ffff", 0x0000_ffff),
            ("0x8000.0001", 0x8000_0001),
            ("0x10000.8", 0x0000_8000),
            ("0x1.", 0x0001_0000),
            ("0b.1", 0x0000_8000),
            ("0B.0000000000000001", 0x0000_0001),
            ("0b.1111111111111111", 0x0000_ffff),
            ("0b1000000000000000.0000000000000001", 0x8000_0001),
            ("0b1.", 0x0001_0000),
        ] {
            assert_eq!(number(source).unwrap() as u32, bits, "{source}");
        }
        let output = compile(
            include_str!("../../converter/tests/fraction_literals.p8"),
            "Fraction literals",
        )
        .unwrap();
        assert!(output.c.contains("p8_num((int32_t)UINT32_C(32768))"));
        assert!(output.c.contains("p8_num((int32_t)UINT32_C(1))"));
    }

    #[test]
    fn large_decimal_literals_wrap_and_exponents_are_names() {
        assert_eq!(number("63191").unwrap(), (63191i64 << 16) as i32);
        assert_eq!(number("32768").unwrap(), i32::MIN);
        // `0e0` is the number 0 followed by the name `e0`.
        let output = compile(
            "pico-8 cartridge\n__lua__\nlocal a,b=0,0e0,c=1\nx=.5e1=2\nsrand()\nfunction _draw() mget(0,0,0) end",
            "test",
        )
        .unwrap();
        assert!(output.c.contains("UINT32_C(32768)"));
    }

    #[test]
    fn empty_malformed_or_excess_precision_radix_literals_are_rejected() {
        for literal in [
            "0x",
            "0X.",
            "0b",
            "0B.",
            "0x.8.1",
            "0x1.2.3",
            "0b.1.0",
            "0b1.0.1",
            "0xg",
            "0b2",
            "0b.2",
            "0x.00001",
            "0b.00000000000000001",
            "0x100000000",
            "0b100000000000000000000000000000000",
        ] {
            assert!(number(literal).is_err(), "number accepted {literal}");
            let source = format!("pico-8 cartridge\n__lua__\nx={literal}");
            assert!(compile(&source, "invalid").is_err(), "compiled {literal}");
        }
    }

    #[test]
    fn unknown_apis_and_unsupported_overloads_stop_conversion() {
        for (call, reason) in [
            ("cstore(0,0,0)", "unsupported function/API"),
            ("save(\"x\")", "unsupported function/API"),
            ("line(1,2,3,4,5,6)", "overload"),
            ("rawset({},1)", "overload"),
            ("select()", "overload"),
        ] {
            let source = format!("pico-8 cartridge\n__lua__\nfunction _draw() {call} end");
            assert!(compile(&source, "test")
                .err()
                .unwrap()
                .message
                .contains(reason));
        }
    }

    #[test]
    fn native_api_results_and_draw_overloads_compile() {
        let output = compile(
            "pico-8 cartridge\n__lua__\nfunction _init() local x,y=camera() local c=cursor local a,b=c() local r={select(1,4,5)} local q=assert(true,8,9) rawset({},1,rawlen({})) end\nfunction _draw() clip(0,0,8,8,true) palt(0) sset(0,0) line() line(7) line(1,2) line(3,4,9) end",
            "native-results",
        ).unwrap();
        assert!(output
            .c
            .contains("return p8_api_id(P8_API_camera,argc,argv)"));
        assert!(output
            .c
            .contains("return p8_api_id(P8_API_cursor,argc,argv)"));
        assert!(output
            .c
            .contains("return p8_api_id(P8_API_select,argc,argv)"));
        assert!(output
            .c
            .contains("return p8_api_id(P8_API_assert,argc,argv)"));
        assert!(output
            .c
            .contains("V result=p8_api_id(P8_API_rawlen,argc,argv);return p8_return(1,&result)"));
    }

    #[test]
    fn comments_containing_includes_are_ignored() {
        compile(
            "pico-8 cartridge\n__lua__\n--[[\n#include easing.lua\n]]\nfunction _draw() end",
            "test",
        )
        .unwrap();
    }
}
