#!/usr/bin/env bash
# Export the TermViAI master; pass --macos to update ICNS as well.
set -euo pipefail
icon_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
python3 "$icon_dir/export.py" "$@"

