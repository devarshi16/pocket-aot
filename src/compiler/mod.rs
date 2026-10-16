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
fn number(s: &str) -> Result<i32, Error> {
    if s.starts_with("0b") || s.starts_with("0B") {
        let mut p = s[2..].split('.');
        let whole = u32::from_str_radix(p.next().unwrap_or("0"), 2)
            .map_err(|_| Error::new(0, "invalid binary number"))?;
        let frac = p.next().unwrap_or("");
        if frac.len() > 16 {
            return Err(Error::new(0, "binary fractions support at most 16 bits"));
        }
        let fraction = if frac.is_empty() {
            0
        } else {
            u32::from_str_radix(&format!("{frac:0<16}"), 2)
                .map_err(|_| Error::new(0, "invalid binary fraction"))?
        };
        return Ok((whole.wrapping_shl(16) | fraction) as i32);
    }
    let value = if s.starts_with("0x") || s.starts_with("0X") {
        let mut p = s[2..].split('.');
        let whole = u32::from_str_radix(p.next().unwrap_or("0"), 16)
            .map_err(|_| Error::new(0, "invalid hexadecimal number"))?;
        let frac = p.next().unwrap_or("");
        if frac.len() > 4 {
            return Err(Error::new(0, "hex fractions support at most four digits"));
        }
        let fraction = if frac.is_empty() {
            0
        } else {
            u32::from_str_radix(&format!("{frac:0<4}"), 16)
                .map_err(|_| Error::new(0, "invalid hex fraction"))?
        };
        return Ok((whole.wrapping_shl(16) | fraction) as i32);
    } else {
        s.parse::<f64>()
            .map_err(|_| Error::new(0, "invalid decimal number"))?
    };
    if !value.is_finite() || !(-32768.0..=32768.0).contains(&value) {
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
    let assets = "#include <stdint.h>\n".to_string()
        + &array("p8_cart_gfx", &gfx)
        + &array("p8_cart_map", &map)
        + &array("p8_cart_flags", &flags);
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
    fn unknown_apis_and_unsupported_overloads_stop_conversion() {
        for (call, reason) in [
            ("peek(0)", "unsupported function/API"),
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
