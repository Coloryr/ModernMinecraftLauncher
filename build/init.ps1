# 初始化开发环境：装 mml-vue 的 npm 依赖，并确认 cargo tauri 可用。
# 只装依赖，不构建、不启动程序；克隆仓库后跑一次即可。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\init.ps1
# 对应的 bash 版：build/init.sh

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw '未找到 npm，请先安装 Node.js 20.19+ 或 22.12+。'
}
cargo tauri --version | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw '未找到 cargo tauri，请先执行 cargo install tauri-cli --version "^2" --locked。'
}

Write-Host '正在安装 mml-vue 的依赖...'
Push-Location (Join-Path $root 'mml-vue')
try {
    npm install
    if ($LASTEXITCODE -ne 0) { throw "mml-vue 依赖安装失败（退出码 $LASTEXITCODE）" }
}
finally {
    Pop-Location
}

# 还原 vendor 插件；否则第一次 cargo check 会因 path 依赖缺 Cargo.toml 直接失败
& (Join-Path $root 'mml-gui\vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { throw "vendor 插件还原失败（退出码 $LASTEXITCODE）" }

Write-Host '依赖安装完成。'
