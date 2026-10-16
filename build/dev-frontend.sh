#!/usr/bin/env bash
# 只启动前端：mml-vue 的 vite 开发服务器，端口 1420。
# 不启动 Tauri 桌面壳；浏览器访问 http://localhost:1420 即可预览界面（无 IPC 数据）。
#
# 用法：bash build/dev-frontend.sh
# 对应的 PowerShell 版：build\dev-frontend.ps1

set -euo pipefail

# 仓库根：本脚本在 build/ 下，往上一层
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$root/mml-vue"
echo '正在启动前端开发服务器: http://localhost:1420 ...'
npm run dev
