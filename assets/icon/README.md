# TermViAI application icon

TermViAI means Terminal Via AI: a terminal application made with AI assistance.
It currently has no built-in AI assistant or AI-powered terminal features.

`termviai-icon.png` is the transparent high-resolution source. The lavender
command prompt and cyan cursor express the terminal; the small four-point
sparkle is a nod to its AI-assisted creation, not an in-app capability.
The charcoal rounded tile matches the application palette. The icon has no
lettering so it remains legible at taskbar sizes.

Generated with the built-in imagegen tool. Visual design prompt:

> Production Windows icon for TermViAI (Terminal Via AI): a flat, straight-on
> charcoal rounded-square tile, bold lavender terminal prompt chevron and cyan
> cursor underscore, one cyan four-point sparkle, high contrast and thick
> clean strokes readable at 16–32 px. Transparent outside the tile. No text,
> letters, watermark, robot, brain, intricate circuitry or 3D perspective.

Run `python assets/icon/export.py` with Pillow installed (or `assets/icon/update.sh`)
to update the 128 px `terminal.png` used by the GUI and the multi-resolution
Windows `terminal.ico`. ICO sizes are 16, 20, 24, 32, 40, 48, 64, 96, 128 and 256 px.
The Windows executable and installer already consume `assets/windows/terminal.ico`.
The build script tracks that resource, so a new build embeds the new icon.

Pass `--macos` to also export the ICNS for future macOS builds. The existing
WezTerm SVG concepts are upstream historical assets, not the TermViAI master.
