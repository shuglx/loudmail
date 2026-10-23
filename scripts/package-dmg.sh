#!/usr/bin/env bash
# 手工打包 dmg(在 Apple Silicon 的 macOS 构建机上执行)
# 依赖: hdiutil / codesign(系统自带)，无需开发者账号
#
# Tauri v1 的 bundler 不签 bundle，只留下链接器打的临时 ad-hoc 签名(linker-signed)，
# `codesign --verify` 会失败，Gatekeeper 判成"已损坏"。这里在 .app 内容全部生成之后
# 补一次 bundle 级 ad-hoc 签名：仍过不了 spctl(没有 Developer ID)，但会从"结构损坏"
# 降级为正常的"未验证开发者"，用户可以用右键打开 / 系统设置的"仍要打开"放行。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "错误: dmg 只能在 macOS 上打包" >&2
  exit 1
fi

VERSION="$(node -p "require('./package.json').version")"
APP="src-tauri/target/release/bundle/macos/LoudMail.app"
STAGE="dist-dmg/stage"
STAGED_APP="$STAGE/LoudMail.app"
OUT="dist-dmg/LoudMail_${VERSION}_aarch64.dmg"

if [ ! -d "$APP" ]; then
  echo "错误: 未找到 $APP,请先执行 npx tauri build --bundles app" >&2
  exit 1
fi

# 1) bundle 级 ad-hoc 签名，不传 --identifier，取 Info.plist 的 CFBundleIdentifier
codesign --force --deep --sign - "$APP"

# 2) 组装 dmg 内容(应用 + /Applications 软链，拖拽安装用)
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$APP" "$STAGED_APP"
ln -s /Applications "$STAGE/Applications"

# 3) 校验 dmg 里那份副本，签名与包内容不一致就直接失败
codesign --verify --deep --strict --verbose=2 "$STAGED_APP"

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
hdiutil create \
  -volname "LoudMail" \
  -srcfolder "$STAGE" \
  -ov -format UDZO \
  "$OUT"

echo "打包完成: $OUT"
