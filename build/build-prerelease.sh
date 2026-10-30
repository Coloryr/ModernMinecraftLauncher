#!/usr/bin/env bash
# 预发布构建：thin LTO + 16 个代码生成单元，能并行，出包快很多。
# 带 prerelease feature：主页面会提示「预发布版」，见 mml-gui/src-tauri/src/build_info.rs。
# 只产出可执行文件，不打安装包；产物在 <仓库>/target/prerelease/，Windows 下带 .exe 后缀。
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

# 版本来自 Git 标签，先同步到应用清单
node "$root/build/set-version.mjs"

# vendor 插件要先还原（cargo 解析依赖图时就要读它的 Cargo.toml，早于构建脚本）；
# 脚本幂等，已还原过会直接跳过
bash "$root/mml-gui/vendor/prepare.sh"

# 直接调用 cargo 不会执行 Tauri 的 beforeBuildCommand，须先生成内嵌前端。
npm --prefix "$root/mml-vue" run build

cd "$root/mml-gui/src-tauri"
echo '正在构建 prerelease（thin LTO）...'

# 默认当前机器架构；MML_TARGETS 可指定多个三元组，例如：
#   MML_TARGETS="aarch64-apple-darwin x86_64-apple-darwin" bash build/build-prerelease.sh
if [ -n "${MML_TARGETS:-}" ]; then
  for target in $MML_TARGETS; do
    echo "  -> $target"
    # 必须带 custom-protocol：不带的话 tauri 把这次构建当成 dev，
    # 前端会连接 localhost:1420；未启动开发服务时无法加载页面。
    cargo build --profile prerelease --target "$target" --features custom-protocol,prerelease
  done
  mkdir -p "$root/target/prerelease"
  for target in $MML_TARGETS; do
    src="$root/target/$target/prerelease/mml-gui"
    [ -f "$src" ] || src="$root/target/$target/prerelease/mml-gui.exe"
    [ -f "$src" ] || { echo "找不到产物: $src" >&2; exit 1; }
    ext=""
    case "$src" in *.exe) ext=".exe" ;; esac
    cp -f "$src" "$root/target/prerelease/mml-gui-$target$ext"
  done
  echo
  echo "产物: $root/target/prerelease/mml-gui-<三元组>"
else
  # 必须带 custom-protocol：不带的话 tauri 把这次构建当成 dev，
  # 前端会连接 localhost:1420；未启动开发服务时无法加载页面。
  cargo build --profile prerelease --features custom-protocol,prerelease
  # 产物名按平台：Windows 带 .exe，macOS / Linux 没有
  bin="$root/target/prerelease/mml-gui"
  [ -f "$bin" ] || bin="$bin.exe"
  [ -f "$bin" ] || { echo "找不到产物: $root/target/prerelease/mml-gui" >&2; exit 1; }
  echo
  echo "产物: $bin"
fi
