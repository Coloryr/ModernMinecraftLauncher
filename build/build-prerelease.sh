#!/usr/bin/env bash
# 预发布构建：thin LTO + 16 个代码生成单元，能并行，出包快很多。
# 带 prerelease feature：主页面会提示「预发布版」，见 mml-gui/src-tauri/src/build_info.rs。
# 只产出可执行文件，不打安装包，产物在 <仓库>/target/prerelease/mml-gui.exe。
# 用的是根 Cargo.toml 里的 [profile.prerelease]，与 release 各有独立 target 子目录，
# 两个模式来回切不会互相刷缓存。正式发布用 build/build-release.sh。
#
# 用法：bash build/build-prerelease.sh
# 对应的 PowerShell 版：build\build-prerelease.ps1

set -euo pipefail

# 仓库根：本脚本在 build/ 下，往上一层
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ---- 构建期临时目录 ----
# 与 .cargo/config.toml 的 [env] 同一口径，口径见 build/build-release.sh
build_tmp="$root/target"
mkdir -p "$build_tmp"
export MML_BUILD_TMP="$build_tmp"
export TMP="$build_tmp"
export TEMP="$build_tmp"

# vendor 插件要先还原（cargo 解析依赖图时就要读它的 Cargo.toml，早于构建脚本）；
# 脚本幂等，已还原过会直接跳过
bash "$root/mml-gui/vendor/prepare.sh"

cd "$root/mml-gui/src-tauri"
echo '正在构建 prerelease（thin LTO）...'
# 必须带 custom-protocol：不带的话 tauri 把这次构建当成 dev，
# 前端去连 localhost:1420 而不是内嵌 dist，窗口永远不出来（见 Cargo.toml 的 [features]）
cargo build --profile prerelease --features custom-protocol,prerelease

echo
echo "产物: $root/target/prerelease/mml-gui.exe"
