#!/bin/sh
# 构建 AppImage：需要 appimagetool（以及可选 linuxdeploy）。
set -eu
cd "$(dirname "$0")/.."

cargo build --release --locked
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APPDIR=$(mktemp -d)
trap 'rm -rf "$APPDIR"' EXIT

mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps"
cp target/release/gamehaven "$APPDIR/usr/bin/gamehaven"
cp packaging/gamehaven.desktop "$APPDIR/usr/share/applications/"
cp packaging/gamehaven.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/gamehaven.svg"

if ! command -v appimagetool >/dev/null 2>&1; then
    echo "未找到 appimagetool，AppDir 已生成：$APPDIR" >&2
    trap - EXIT
    exit 1
fi

appimagetool "$APPDIR" "GameHaven-${VERSION}-x86_64.AppImage"
