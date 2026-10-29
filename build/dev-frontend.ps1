# 只启动前端：mml-vue 的 vite 开发服务器，端口 1420。
# 用于 Rust 后端以 debug 模式单独运行（cargo run）时配合前端热更新；
# cargo tauri dev 已自带 vite，不要同时运行这个脚本。
# 浏览器直连只能看到无 IPC 数据的空状态（?window=<kind> 取值见 mml-vue/src/windows/registry.ts）。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\dev-frontend.ps1
# 对应的 bash 版：build/dev-frontend.sh

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $root 'mml-vue')
try {
    Write-Host '正在启动前端开发服务器: http://localhost:1420 ...'
    npm run dev
    if ($LASTEXITCODE -ne 0) { throw "前端开发服务器异常退出（退出码 $LASTEXITCODE）" }
}
finally {
    Pop-Location
}
