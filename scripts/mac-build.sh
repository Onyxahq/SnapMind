#!/bin/sh
# macOS 打包并安装到 /Applications。
# - 钥匙串里有「SnapMind Local Signing」证书时用它签名：系统的屏幕录制授权认「证书 + 应用 ID」，重新编译不丢授权；
#   没有就退回 ad-hoc 签名（每次重编都要重新授权）。
# - 装好后删掉 target 里的 .app，免得启动台出现两个 SnapMind（dmg 保留）。
set -e
cd "$(dirname "$0")/.."

IDENTITY="SnapMind Local Signing"
if security find-identity -p codesigning | grep -q "\"$IDENTITY\""; then
  export APPLE_SIGNING_IDENTITY="$IDENTITY"
else
  echo "未找到「$IDENTITY」证书，使用 ad-hoc 签名"
fi

pnpm tauri build --bundles app,dmg

APP=target/release/bundle/macos/SnapMind.app
osascript -e 'quit app "SnapMind"' 2>/dev/null || true
rm -rf /Applications/SnapMind.app
ditto "$APP" /Applications/SnapMind.app
rm -rf "$APP"
codesign -d -r- /Applications/SnapMind.app 2>&1 | tail -1
open /Applications/SnapMind.app
