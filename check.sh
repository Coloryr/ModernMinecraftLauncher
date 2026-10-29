#!/usr/bin/env bash
# 根目录 cargo check 入口：先还原 vendor 插件，再执行 cargo check
#
# Cargo 在解析依赖图时就要读 mml-gui/src-tauri 的 path 依赖
# target/vendor/tauri-plugin-decoration/Cargo.toml，这早于任何构建脚本，
# 所以裸 `cargo check --all` 没法自动准备。这个脚本把两步合成一步。
#
# 用法：bash check.sh --all
#       bash check.sh -p mml-gui
# 不传参数时执行 cargo check --workspace。

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$root/mml-gui/vendor/prepare.sh"

cd "$root"
if [ "$#" -eq 0 ]; then
  exec cargo check --workspace
fi
exec cargo check "$@"
