use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::Sender,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Cartridge {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub description: String,
    pub controls: Vec<String>,
    pub filename: String,
    pub sha256: String,
    pub accent: [u8; 3],
    pub cover: String,
    pub credits: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub cart_image: String,
    #[serde(default)]
    pub source_file: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub rank: Option<u32>,
}

impl Cartridge {
    pub fn ready(&self) -> bool {
        (self.status.is_empty() || self.status == "ready")
            && !self.filename.is_empty()
            && !self.sha256.is_empty()
    }
    pub fn state_label(&self) -> &'static str {
        if self.ready() {
            "READY"
        } else if self.status == "source_only" {
            "BUILD DEMO"
        } else if self.status == "download_failed" {
            "DOWNLOAD FAILED"
        } else {
            "NEEDS CONVERSION"
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Catalog {
    pub cartridges: Vec<Cartridge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Cartridge>,
}

#[derive(Clone, Debug)]
pub enum Device {
    Boot(PathBuf),
    Serial(PathBuf),
    Unmounted,
    Offline,
    Multiple,
}

impl Device {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Boot(_) => "PICO READY TO FLASH",
            Self::Serial(_) => "PICO CONNECTED",
            Self::Unmounted => "BOOTSEL / MOUNT THE DRIVE",
            Self::Offline => "CONNECT YOUR PICO",
            Self::Multiple => "CONNECT ONE PICO AT A TIME",
        }
    }
    pub fn available(&self) -> bool {
        matches!(self, Self::Boot(_) | Self::Serial(_))
    }
}

pub enum Update {
    Stage(f32, String),
    Imported(String),
    Done(Result<String, String>),
}

pub fn root_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("POCKET_ARCADE_HOME") {
        return p.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        for parent in exe.ancestors().skip(1).take(4) {
            if parent.join("catalog.json").exists() {
                return parent.into();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn load_catalog(root: &Path) -> Result<Catalog, String> {
    let data = fs::read(root.join("catalog.json")).map_err(|e| e.to_string())?;
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}

fn boot_mounts() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    let bases = [
        PathBuf::from("/media").join(&user),
        PathBuf::from("/run/media").join(&user),
    ];
    let mut found = Vec::new();
    for base in bases {
        if let Ok(entries) = fs::read_dir(base) {
            for item in entries.flatten() {
                let p = item.path();
                if valid_mount(&p).is_ok() && !found.contains(&p) {
                    found.push(p);
                }
            }
        }
    }
    found
}

pub fn serial_ports() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir("/dev/serial/by-id") {
        for item in entries.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with("usb-Raspberry_Pi_Pico_")
                || name.starts_with("usb-MicroPython_Board_in_FS_mode_")
            {
                found.push(item.path());
            }
        }
    }
    found.sort();
    found
}

pub fn detect() -> Device {
    let mounts = boot_mounts();
    let ports = serial_ports();
    if mounts.len() + ports.len() > 1 {
        return Device::Multiple;
    }
    if let Some(p) = mounts.first() {
        return Device::Boot(p.clone());
    }
    if let Some(p) = ports.first() {
        return Device::Serial(p.clone());
    }
    if let Ok(entries) = fs::read_dir("/sys/bus/usb/devices") {
        for entry in entries.flatten() {
            let p = entry.path();
            if fs::read_to_string(p.join("idVendor"))
                .unwrap_or_default()
                .trim()
                == "2e8a"
                && fs::read_to_string(p.join("idProduct"))
                    .unwrap_or_default()
                    .trim()
                    == "0003"
            {
                return Device::Unmounted;
            }
        }
    }
    Device::Offline
}

pub fn valid_mount(path: &Path) -> Result<(), String> {
    let info = fs::read_to_string(path.join("INFO_UF2.TXT"))
        .map_err(|_| "RPI-RP2 drive is not mounted".to_string())?;
    if info.lines().any(|line| line.trim() == "Board-ID: RPI-RP2") {
        Ok(())
    } else {
        Err("This is not an RP2040 RPI-RP2 bootloader drive".into())
    }
}

pub fn verify_image(data: &[u8], expected: &str) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(data)) != expected {
        return Err("Cartridge checksum failed. The file has changed; nothing was flashed.".into());
    }
    if data.is_empty() || data.len() % 512 != 0 {
        return Err("Invalid UF2 image length".into());
    }
    let count = data.len() / 512;
    let word = |b: &[u8], pos: usize| u32::from_le_bytes(b[pos..pos + 4].try_into().unwrap());
    for (i, block) in data.chunks_exact(512).enumerate() {
        if word(block, 0) != 0x0a324655
            || word(block, 4) != 0x9e5d5157
            || word(block, 508) != 0x0ab16f30
            || word(block, 8) & 0x2000 == 0
            || word(block, 28) != 0xe48bff56
            || word(block, 16) != 256
            || word(block, 20) != i as u32
            || word(block, 24) != count as u32
            || word(block, 12) != 0x10000000 + (i as u32) * 256
            || word(block, 12) > 0x101fff00
        {
            return Err(format!("Unsupported or corrupt RP2040 UF2 block {}", i));
        }
    }
    Ok(())
}

fn progress(tx: &Sender<Update>, n: f32, message: impl Into<String>) {
    let _ = tx.send(Update::Stage(n, message.into()));
}

pub fn copy_image(mount: &Path, data: &[u8], tx: &Sender<Update>) -> Result<(), String> {
    valid_mount(mount)?;
    let mut output = fs::File::create(mount.join("POCKET.UF2"))
        .map_err(|e| format!("Cannot write to the Pico: {e}"))?;
    let mut sent = 0;
    for chunk in data.chunks(16384) {
        output
            .write_all(chunk)
            .map_err(|e| format!("Transfer stopped before completion: {e}"))?;
        sent += chunk.len();
        progress(
            tx,
            0.25 + 0.6 * sent as f32 / data.len() as f32,
            "Writing cartridge to Pico...",
        );
    }
    progress(
        tx,
        0.85,
        "Finishing USB transfer. This can take up to a minute...",
    );
    output
        .sync_all()
        .map_err(|e| format!("Final USB sync failed: {e}. Check the Pico before retrying."))?;
    Ok(())
}

pub fn flash(root: &Path, cart: &Cartridge, tx: &Sender<Update>) -> Result<String, String> {
    if !cart.ready() {
        return Err(format!(
            "{} has no verified firmware to flash. {}",
            cart.title, cart.reason
        ));
    }
    progress(tx, 0.03, "Checking cartridge checksum and RP2040 format...");
    let bytes =
        fs::read(root.join(&cart.filename)).map_err(|e| format!("Cannot load cartridge: {e}"))?;
    verify_image(&bytes, &cart.sha256)?;
    let device = detect();
    if let Device::Serial(port) = device {
        progress(tx, 0.12, "Asking Pico to enter USB recovery...");
        let result = Command::new("python3")
            .arg(root.join("helpers/bootloader.py"))
            .arg(port)
            .output()
            .map_err(|e| format!("Could not start the USB helper: {e}"))?;
        if !result.status.success() {
            return Err(String::from_utf8_lossy(&result.stderr).trim().to_string());
        }
    } else if matches!(device, Device::Offline | Device::Multiple) {
        return Err(device.label().into());
    }
    progress(tx, 0.18, "Waiting for the RPI-RP2 drive...");
    let started = Instant::now();
    let mount = loop {
        if let Device::Boot(p) = detect() {
            break p;
        }
        if started.elapsed() > Duration::from_secs(25) {
            return Err("Pico recovery drive did not mount. Hold BOOTSEL while reconnecting, open RPI-RP2 in Files, then try again.".into());
        }
        thread::sleep(Duration::from_millis(250));
    };
    progress(tx, 0.23, "RP2040 confirmed. Starting transfer...");
    copy_image(&mount, &bytes, tx)?;
    progress(tx, 0.9, "Cartridge sent. Waiting for Pico to restart...");
    let started = Instant::now();
    loop {
        if !mount.join("INFO_UF2.TXT").exists() && !serial_ports().is_empty() {
            break;
        }
        if started.elapsed() > Duration::from_secs(15) {
            return Err("Image sent, but Pico restart was not detected. Check its display and USB connection.".into());
        }
        thread::sleep(Duration::from_millis(250));
    }
    progress(tx, 0.96, "Checking that the application starts...");
    let port = serial_ports()
        .into_iter()
        .next()
        .ok_or("Pico serial port disappeared before the startup check")?;
    let result = Command::new("python3")
        .arg(root.join("helpers/check_startup.py"))
        .arg(port)
        .arg(&cart.id)
        .output()
        .map_err(|e| format!("Could not check startup: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "Image sent, but startup check failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    let log_dir = root.join("logs");
    let _ = fs::create_dir_all(&log_dir);
    let _ = fs::write(
        log_dir.join("last-flash.txt"),
        format!(
            "{}\n{}\n{}",
            cart.id,
            cart.sha256,
            String::from_utf8_lossy(&result.stdout)
        ),
    );
    Ok(format!("{} is ready. Pick up your Pico!", cart.title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    fn fixture_uf2() -> Vec<u8> {
        let mut bytes = vec![0; 512];
        for (offset, word) in [
            (0, 0x0a324655u32),
            (4, 0x9e5d5157),
            (8, 0x2000),
            (12, 0x10000000),
            (16, 256),
            (20, 0),
            (24, 1),
            (28, 0xe48bff56),
            (508, 0x0ab16f30),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn standalone_uf2_fixture_is_verified_and_corruption_is_rejected() {
        let mut bytes = fixture_uf2();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        verify_image(&bytes, &hash).unwrap();
        bytes[32] ^= 1;
        assert!(verify_image(&bytes, &hash).is_err());
    }
    #[test]
    fn catalog_images_are_real_and_corruption_is_rejected() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let catalog = load_catalog(&root).unwrap();
        for cart in catalog.cartridges.iter().chain(catalog.recovery.iter()) {
            if !cart.ready() {
                continue;
            }
            let mut bytes = fs::read(root.join(&cart.filename)).unwrap();
            verify_image(&bytes, &cart.sha256).unwrap();
            bytes[32] ^= 1;
            assert!(verify_image(&bytes, &cart.sha256).is_err());
        }
    }
    #[test]
    fn rejects_wrong_chip_even_with_a_matching_hash() {
        let mut bytes = fixture_uf2();
        bytes[28..32].copy_from_slice(&0xe48bff59u32.to_le_bytes());
        let hash = format!("{:x}", Sha256::digest(&bytes));
        assert!(verify_image(&bytes, &hash).is_err());
    }
    #[test]
    fn transfer_only_accepts_verified_pico_mounts() {
        let base = std::env::temp_dir().join(format!("pocket arcade test {}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let (tx, _rx) = mpsc::channel();
        assert!(copy_image(&base, b"never write", &tx).is_err());
        assert!(!base.join("POCKET.UF2").exists());
        fs::write(base.join("INFO_UF2.TXT"), "Board-ID: RPI-RP2\n").unwrap();
        let bytes = fixture_uf2();
        copy_image(&base, &bytes, &tx).unwrap();
        assert_eq!(fs::read(base.join("POCKET.UF2")).unwrap(), bytes);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn unsupported_entries_never_reach_usb_or_file_transfer() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut cart = load_catalog(&root).unwrap().cartridges[0].clone();
        cart.status = "unsupported".into();
        cart.reason = "closures not supported".into();
        let (tx, rx) = mpsc::channel();
        assert!(flash(&root, &cart, &tx)
            .unwrap_err()
            .contains("no verified firmware"));
        assert!(rx.try_recv().is_err());
    }
}
