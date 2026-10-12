#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Compiling OpenWire (release)..."
cd "$ROOT"
if [[ -f Cargo.lock ]]; then
  cargo build --release --locked
else
  cargo build --release
fi

echo "==> Assembling the Arch Linux package..."
cd "$ROOT/packaging"
rm -rf pkg src ./*.pkg.tar.zst
makepkg -f

echo "==> Done:"
ls -lh ./*.pkg.tar.zst
