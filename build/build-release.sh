#!/usr/bin/env bash
# 正式发布构建：fat LTO + 单代码生成单元，体积最小、运行最快，但出包慢 ——
# 末尾的跨 crate 优化是单线程的，只有一个核在跑；产出安装包（tauri 打包链）。
# 预发布 / 日常打包用 build/build-prerelease.sh。
#
# 用法：bash build/build-release.sh
# 对应的 PowerShell 版：build\build-release.ps1

set -euo pipefail

# 仓库根：本脚本在 build/ 下，往上一层
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ---- 构建期临时目录 ----
# 与 .cargo/config.toml 的 [env] 同一口径：把 TMP/TEMP 指到仓库的 target/，
# 免得系统临时目录让 MSVC 的 lib.exe / link.exe 报 LNK1104（Windows 上的构建）
build_tmp="$root/target"
mkdir -p "$build_tmp"
export MML_BUILD_TMP="$build_tmp"
export TMP="$build_tmp"
export TEMP="$build_tmp"

# vendor 插件（tauri-plugin-decoration）要先还原：cargo 解析依赖图时就要读它的
# Cargo.toml，早于任何构建脚本。脚本幂等，已还原过会直接跳过。
bash "$root/mml-gui/vendor/prepare.sh"

cd "$root/mml-gui"
echo '正在构建 release（fat LTO，含安装包）...'
npm run tauri build
