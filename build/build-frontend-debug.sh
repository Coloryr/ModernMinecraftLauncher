#!/usr/bin/env bash
# 前端调试版构建：前端不压缩并带 source map，界面标「调试版」，按 F12 可打开控制台。
# 用 [profile.prerelease]（thin LTO）只出可执行文件，不打安装包；Linux / macOS 压成 tar.gz
# 收进 build_out/，文件名形如 mml-linux-x64-<版本>-frontend-debug.tar.gz。
#
# 与正式发布 / 预发布的关系：发版时 CI 会在同一台机器上再跑一遍本脚本，
# 调试版作为附加产物一起发布；本地需要单独出包时直接运行即可。
#
# 用法：bash build/build-frontend-debug.sh
# 对应的 PowerShell 版：build/build-frontend-debug.ps1

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

# 版本来自 docs/version，先同步到应用清单
node "$root/build/set-version.mjs"
version="$(node "$root/build/version.mjs")"

# vendor 插件与 SDL 裁剪：cargo 解析依赖图时就要读，必须先还原
bash "$root/build/vendor/tauri-plugin-decoration/prepare.sh"
node "$root/build/vendor/sdl3-src/prepare.mjs"

# 直接调用 cargo 不会执行 Tauri 的 beforeBuildCommand，前端由这里先生成
npm --prefix "$root/mml-vue" run build:frontend-debug

case "$(uname -s)" in
  Linux) os_name=linux ;;
  Darwin) os_name=macos ;;
  *) echo "不支持的系统：$(uname -s)，Windows 请用 build-frontend-debug.ps1" >&2; exit 1 ;;
esac

# 从三元组 / uname 取架构名，与发布产物命名一致
arch_of() {
  case "$1" in
    x86_64*|amd64*) echo x64 ;;
    aarch64*|arm64*) echo arm64 ;;
    *) echo "$1" ;;
  esac
}

cd "$root/mml-gui/src-tauri"
out_dir="$root/build_out"
mkdir -p "$out_dir"

# 打成一个 tar.gz：GitHub 下载裸二进制不保留可执行位，压缩包能保住
package_one() {
  bin="$1"
  arch="$2"
  [ -f "$bin" ] || { echo "找不到产物：$bin" >&2; exit 1; }
  dest="$out_dir/mml-$os_name-$arch-$version-frontend-debug.tar.gz"
  tar -czf "$dest" -C "$(dirname "$bin")" "$(basename "$bin")"
  echo "产物：$dest"
}

if [ -n "${MML_TARGETS:-}" ]; then
  for target in $MML_TARGETS; do
    echo "正在构建 frontend-debug（$target）..."
    # custom-protocol 不能漏：否则 tauri 把这次构建当成 dev，前端会去连 localhost:1420
    cargo build --profile prerelease --target "$target" --features custom-protocol,frontend-debug
    package_one "$root/target/$target/prerelease/mml-gui" "$(arch_of "$target")"
  done
else
  echo "正在构建 frontend-debug（当前架构）..."
  cargo build --profile prerelease --features custom-protocol,frontend-debug
  bin="$root/target/prerelease/mml-gui"
  [ -f "$bin" ] || bin="$bin.exe"
  package_one "$bin" "$(arch_of "$(uname -m)")"
fi
