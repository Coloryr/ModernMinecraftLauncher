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

# 版本来自 Git 标签，先同步到应用清单
node "$root/build/set-version.mjs"

# vendor 插件（tauri-plugin-decoration）要先还原：cargo 解析依赖图时就要读它的
# Cargo.toml，早于任何构建脚本。脚本幂等，已还原过会直接跳过。
bash "$root/mml-gui/vendor/prepare.sh"

cd "$root/mml-gui"

# 默认构建当前机器架构（Linux / macOS 的 CI 各跑各的）。
# 需要多架构时用 MML_TARGETS 指定三元组，空格分隔，例如：
#   MML_TARGETS="aarch64-apple-darwin x86_64-apple-darwin" bash build/build-release.sh
# Windows 的多架构（x64 / x86 / arm64）用 build-release.ps1。
#
# 打包类型默认交给 tauri 自己决定（Linux 上是 deb / rpm / AppImage）。交叉编译、
# 或该架构没有某个打包工具时用 MML_BUNDLES 限定，例如 MML_BUNDLES=deb,rpm。
echo '正在构建 release（fat LTO，含安装包）...'
if [ -n "${MML_TARGETS:-}" ]; then
  for target in $MML_TARGETS; do
    echo "  -> $target"
    if [ -n "${MML_BUNDLES:-}" ]; then
      cargo tauri build --target "$target" --bundles "$MML_BUNDLES"
    else
      cargo tauri build --target "$target"
    fi
  done
else
  cargo tauri build
fi

# 把 Tauri 的安装包按版本标签重命名后集中到 build_out/
echo '正在收集安装包到 build_out ...'
node "$root/build/package-release.mjs"
