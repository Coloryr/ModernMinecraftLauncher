#!/usr/bin/env bash
# 初始化开发环境：装 mml-vue 的 npm 依赖，并确认 cargo tauri 可用。
# 只装依赖，不构建、不启动程序；克隆仓库后跑一次即可。
#
# 用法：bash build/init.sh
# 对应的 PowerShell 版：build\init.ps1

set -euo pipefail

# 仓库根：本脚本在 build/ 下，往上一层
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v npm >/dev/null 2>&1; then
    echo '未找到 npm，请先安装 Node.js 20.19+ 或 22.12+。' >&2
    exit 1
fi
if ! cargo tauri --version >/dev/null 2>&1; then
    echo '未找到 cargo tauri，请先执行 cargo install tauri-cli --version "^2" --locked。' >&2
    exit 1
fi

echo '正在安装 mml-vue 的依赖...'
(cd "$root/mml-vue" && npm install)

# 还原 vendor 插件；否则第一次 cargo check 会因 path 依赖缺 Cargo.toml 直接失败
bash "$root/build/vendor/tauri-plugin-decoration/prepare.sh"
# 还原打过补丁的 sdl3-src（Cargo.toml 的 [patch.crates-io] 指向它）
node "$root/build/vendor/sdl3-src/prepare.mjs"

echo '依赖安装完成。'
