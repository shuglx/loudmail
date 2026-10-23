#!/usr/bin/env bash
# 手工打包 deb(在 arm64 构建机上执行)
# 依赖: dpkg-deb(宿主机为 arm64 时无需交叉处理)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"
ARCH="arm64"
BIN="src-tauri/target/release/loudmail"
STAGE="dist-deb/loudmail"
OUT="dist-deb/loudmail_${VERSION}_${ARCH}.deb"

if [ ! -f "$BIN" ]; then
  echo "错误: 未找到 $BIN,请先在 focal arm64 容器内执行 cargo build --release" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p \
  "$STAGE/DEBIAN" \
  "$STAGE/usr/bin" \
  "$STAGE/usr/share/applications" \
  "$STAGE/usr/share/icons/hicolor/32x32/apps" \
  "$STAGE/usr/share/icons/hicolor/128x128/apps" \
  "$STAGE/usr/share/icons/hicolor/256x256/apps" \
  "$STAGE/usr/share/icons/hicolor/512x512/apps"

cp "$BIN" "$STAGE/usr/bin/loudmail"
cp src-tauri/icons/32x32.png   "$STAGE/usr/share/icons/hicolor/32x32/apps/loudmail.png"
cp src-tauri/icons/128x128.png "$STAGE/usr/share/icons/hicolor/128x128/apps/loudmail.png"
cp src-tauri/icons/256x256.png "$STAGE/usr/share/icons/hicolor/256x256/apps/loudmail.png"
cp src-tauri/icons/512x512.png "$STAGE/usr/share/icons/hicolor/512x512/apps/loudmail.png"

cat > "$STAGE/usr/share/applications/loudmail.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=LoudMail
Name[zh_CN]=邮件报警
Comment=Email arrival alarm
Comment[zh_CN]=邮件到达报警提醒
Exec=/usr/bin/loudmail
Icon=loudmail
Terminal=false
Categories=Utility;
EOF

# 运行时依赖均为麒麟 V10 SP1(Ubuntu 20.04 基座)自带，
# 内网环境无法联网装包，故必须与目标机现状一致
cat > "$STAGE/DEBIAN/control" <<EOF
Package: loudmail
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
License: MIT
Depends: libwebkit2gtk-4.0-37, libgtk-3-0, libxkbcommon0, libasound2, libappindicator3-1
Maintainer: ryan <ryan@localhost>
Description: LoudMail email arrival alarm for Kylin V10 (arm64)
 邮件到达报警：按匹配策略轮询邮箱，命中后播放报警语音。
EOF

dpkg-deb --build --root-owner-group "$STAGE" "$OUT"
echo "打包完成: $OUT"
