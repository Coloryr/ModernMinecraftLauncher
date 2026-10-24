#!/usr/bin/env bash
# 初始化开发环境：安装启动器两个前端工程的 npm 依赖。
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

for project in mml-vue mml-gui; do
    echo "正在安装 $project 的依赖..."
    (cd "$root/$project" && npm install)
done

echo '依赖安装完成。'
