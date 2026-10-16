use pocket_arcade::compiler;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err("Usage: pocket-aot INPUT.p8 OUTPUT_DIR [TITLE]".into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let title = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| input.file_stem().unwrap().to_string_lossy().into_owned());
    let source = fs::read(&input)?;
    let text = std::str::from_utf8(&source)?;
    let code = compiler::compile(text, &title)?;
    fs::create_dir_all(&output)?;
    fs::write(output.join("game.c"), &code.c)?;
    fs::write(output.join("assets.c"), &code.assets)?;
    let report = json!({"compiler":"Pocket AOT 0.2 / experimental subset","title":title,"source":input,"source_sha256":format!("{:x}",Sha256::digest(&source)),"source_modified":false,"target":"RP2040 / Waveshare LCD 1.3","api_calls":code.apis,"warnings":code.warnings,"output":"native C; compile with converter/build.py to get UF2"});
    fs::write(
        output.join("conversion.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )?;
    println!("Converted {} -> {}", input.display(), output.display());
    for w in code.warnings {
        println!("Note: {w}");
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Conversion failed: {e}");
        std::process::exit(1);
    }
}
