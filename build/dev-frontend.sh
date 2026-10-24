#!/usr/bin/env bash
# 只启动前端：mml-vue 的 vite 开发服务器，端口 1420。
# 用于 Rust 后端以 debug 模式单独运行（cargo run）时配合前端热更新；
# npm run tauri dev 已自带 vite，不要同时运行这个脚本。
# 浏览器直连只能看到无 IPC 数据的空状态。
#
# 用法：bash build/dev-frontend.sh
# 对应的 PowerShell 版：build\dev-frontend.ps1

set -euo pipefail

# 仓库根：本脚本在 build/ 下，往上一层
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$root/mml-vue"
echo '正在启动前端开发服务器: http://localhost:1420 ...'
npm run dev
