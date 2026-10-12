#!/usr/bin/env python3
"""Export the TermViAI PNG master to desktop and Windows icon resources."""

import argparse
from pathlib import Path

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--macos", action="store_true", help="also update the macOS ICNS")
    args = parser.parse_args()
    icon_dir = Path(__file__).resolve().parent
    assets = icon_dir.parent
    master = Image.open(icon_dir / "termviai-icon.png").convert("RGBA")
    if master.width != master.height or master.width < 256:
        raise ValueError("The master must be square and at least 256 pixels wide")
    if master.getchannel("A").getextrema()[0] == 255:
        raise ValueError("The master must preserve transparent space outside the tile")

    desktop = master.resize((128, 128), Image.Resampling.LANCZOS)
    desktop.save(icon_dir / "terminal.png", optimize=True)
    sizes = [(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)]
    ico = assets / "windows" / "terminal.ico"
    master.save(ico, format="ICO", sizes=sizes)
    with Image.open(ico) as exported:
        if exported.ico.sizes() != set(sizes):
            raise ValueError("The ICO is missing a requested resolution")
    if args.macos:
        master.save(assets / "macos/WezTerm.app/Contents/Resources/terminal.icns", format="ICNS")
    print("Exported terminal.png and terminal.ico (16–256 px)")


if __name__ == "__main__":
    main()
