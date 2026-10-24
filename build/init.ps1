# 初始化开发环境：安装启动器两个前端工程的 npm 依赖。
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

foreach ($project in @('mml-vue', 'mml-gui')) {
    Write-Host "正在安装 $project 的依赖..."
    Push-Location (Join-Path $root $project)
    try {
        npm install
        if ($LASTEXITCODE -ne 0) { throw "$project 依赖安装失败（退出码 $LASTEXITCODE）" }
    }
    finally {
        Pop-Location
    }
}

Write-Host '依赖安装完成。'
