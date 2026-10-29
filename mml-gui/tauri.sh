#!/usr/bin/env bash
# cargo tauri 的统一入口：先还原 vendor 插件，再执行 cargo tauri
#
# cargo 在解析依赖图时就要读 target/vendor/tauri-plugin-decoration/Cargo.toml，
# 这早于 Tauri 的 beforeBuildCommand，所以没法把 prepare 挂到 Tauri 配置里。
# 直接跑 `cargo tauri` 前必须还原插件；这个脚本把两步合成一步。
#
# 用法：bash tauri.sh dev / bash tauri.sh build / bash tauri.sh icon <svg>

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
bash vendor/prepare.sh
exec cargo tauri "$@"
