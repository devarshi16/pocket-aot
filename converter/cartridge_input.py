"""Decode supported cartridge containers to text without executing their code."""
from pathlib import Path
import sys

VENDOR = Path(__file__).resolve().parent / "vendor/shrinko8"


def prepare_source(source: Path, destination: Path) -> Path:
    """Return .p8 input, or write decoded.p8 for a PNG/full-size raw ROM.

    Deliberately excludes code-only/tiny ROMs and appended multi-cart exports.
    Keep the original bytes untouched; the decoder verifies a present ROM hash.
    """
    suffix = source.suffix.lower()
    if suffix == ".p8":
        return source
    if suffix not in (".png", ".rom"):
        raise ValueError("Choose a PICO-8 .p8, .p8.png or .p8.rom cartridge.")
    # Import locally: plain text conversion needs no image/decoder dependency.
    sys.path.insert(0, str(VENDOR))
    from pico_cart import read_cart_from_image, read_cart_from_rom, write_cart_to_source
    from pico_defs import k_cart_size, k_trailer_size

    data = source.read_bytes()
    if suffix == ".rom" and len(data) not in (k_cart_size, k_cart_size + k_trailer_size):
        raise ValueError("Raw ROM must contain 32768 bytes, optionally followed by a 32-byte trailer; tiny ROMs are unsupported.")
    try:
        cart = read_cart_from_rom(data, allow_tiny=False) if suffix == ".rom" else read_cart_from_image(data)
        text = write_cart_to_source(cart)
    except Exception as error:
        raise ValueError(f"Invalid {suffix[1:].upper()} cartridge: {str(error) or type(error).__name__}") from error
    destination.mkdir(parents=True, exist_ok=True)
    decoded = destination / "decoded.p8"
    decoded.write_text(text, encoding="utf-8")
    return decoded
