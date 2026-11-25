# 前端调试版构建：前端不压缩并带 source map，界面标「调试版」，按 F12 可打开控制台。
# 用 [profile.prerelease]（thin LTO）只出可执行文件，不打安装包；产物收进 build_out/：
#   mml-windows-<arch>-<版本>-frontend-debug.exe
# Windows 默认 x64 / arm64，只要一种时用 -Arch。
#
# 与正式发布 / 预发布的关系：正式发版时 CI 会在同一台机器上再跑一遍本脚本，
# 调试版作为附加产物一起发布；本地需要单独出包时直接运行即可。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\build-frontend-debug.ps1
# 对应的 bash 版：build/build-frontend-debug.sh

param(
    # 要构建的架构，逗号分隔：x64,arm64
    [string]$Arch = 'x64,arm64'
)

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

$all = @(
    @{ arch = 'x64';   triple = 'x86_64-pc-windows-msvc' },
    @{ arch = 'arm64'; triple = 'aarch64-pc-windows-msvc' }
)
$want = $Arch.Split(',') | ForEach-Object { $_.Trim().ToLower() } | Where-Object { $_ }
$targets = $all | Where-Object { $want -contains $_.arch }
if (-not $targets) {
    throw "没有匹配的架构：$Arch（可选 x64 / arm64）"
}

# ---- 构建期临时目录 ----
# 口径见 build\build-release.ps1 的同一段注释：非 cargo 的打包链拿不到 .cargo\config.toml 的 [env]
$buildTmp = Join-Path $root 'target'
New-Item -ItemType Directory -Force -Path $buildTmp | Out-Null
$env:MML_BUILD_TMP = $buildTmp
$env:TMP = $buildTmp
$env:TEMP = $buildTmp

# 版本来自 docs/version，先同步到应用清单
node (Join-Path $root 'build\set-version.mjs')
if ($LASTEXITCODE -ne 0) { throw '版本同步失败，已中止构建。' }
$version = (& node (Join-Path $root 'build\version.mjs')).Trim()
if ($LASTEXITCODE -ne 0) { throw '读取版本失败，已中止构建。' }

# vendor 插件与 SDL 裁剪：cargo 解析依赖图时就要读，必须先还原
powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'build\vendor\tauri-plugin-decoration\prepare.ps1')
if ($LASTEXITCODE -ne 0) { throw 'vendor 插件还原失败，已中止构建。' }
node (Join-Path $root 'build\vendor\sdl3-src\prepare.mjs')
if ($LASTEXITCODE -ne 0) { throw 'SDL 裁剪准备失败，已中止构建。' }

foreach ($t in $targets) {
    rustup target add $t['triple'] | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "安装 Rust 目标失败：$($t['triple'])。"
    }
}

# 直接调用 cargo 不执行 Tauri 的 beforeBuildCommand，前端由这里先生成
Push-Location (Join-Path $root 'mml-vue')
try {
    npm run build:frontend-debug
    if ($LASTEXITCODE -ne 0) { throw '前端调试版构建失败，已中止。' }
}
finally {
    Pop-Location
}

Push-Location (Join-Path $root 'mml-gui\src-tauri')
try {
    foreach ($t in $targets) {
        Write-Host "正在构建 frontend-debug $($t['arch'])（$($t['triple'])）..."
        # custom-protocol 不能漏：否则 tauri 把这次构建当成 dev，前端会去连 localhost:1420
        cargo build --profile prerelease --target $t['triple'] --features custom-protocol,frontend-debug
        if ($LASTEXITCODE -ne 0) {
            throw "构建失败：$($t['arch'])（退出码 $LASTEXITCODE）。arm64 交叉编译需要对应架构的 MSVC 工具链。"
        }
    }
}
finally {
    Pop-Location
}

# 收进 build_out/：与正式 / 预发布的安装包并排，文件名带 -frontend-debug
$outDir = Join-Path $root 'build_out'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
foreach ($t in $targets) {
    $src = Join-Path $root "target\$($t['triple'])\prerelease\mml-gui.exe"
    if (-not (Test-Path $src)) { throw "找不到产物：$src" }
    $dest = Join-Path $outDir "mml-windows-$($t['arch'])-$version-frontend-debug.exe"
    Copy-Item $src $dest -Force
    Write-Host "产物：$dest"
}
