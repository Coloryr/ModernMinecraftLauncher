# 预发布构建：thin LTO + 16 个代码生成单元，能并行，出包快很多。
# 带 prerelease feature：主页面会提示「预发布版」，见 mml-gui/src-tauri/src/build_info.rs。
# 只产出可执行文件，不打安装包；Windows 默认三种架构，集中在 <仓库>\target\prerelease\：
#   mml-gui-x64.exe / mml-gui-x86.exe / mml-gui-arm64.exe
# 只需要一种时用 -Arch：-Arch x64 / -Arch arm64。
# 用的是根 Cargo.toml 里的 [profile.prerelease]，各架构各有 target 子目录，
# 两个模式来回切不会互相刷缓存。正式发布用 build\build-release.ps1。
#
# 交叉编译前提同 build-release：rustup target 与对应架构的 MSVC 工具链。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\build-prerelease.ps1
# 对应的 bash 版：build/build-prerelease.sh

param(
    # 要构建的架构，逗号分隔：x64,x86,arm64
    [string]$Arch = 'x64,x86,arm64'
)

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

$all = @(
    @{ arch = 'x64';   triple = 'x86_64-pc-windows-msvc' },
    @{ arch = 'x86';   triple = 'i686-pc-windows-msvc' },
    @{ arch = 'arm64'; triple = 'aarch64-pc-windows-msvc' }
)
$want = $Arch.Split(',') | ForEach-Object { $_.Trim().ToLower() } | Where-Object { $_ }
$targets = $all | Where-Object { $want -contains $_.arch }
if (-not $targets) {
    throw "没有匹配的架构：$Arch（可选 x64 / x86 / arm64）"
}

# ---- 构建期临时目录 ----
# 主修复在 .cargo\config.toml 的 [env]；这里再设一次是为了**非 cargo 的子进程**。
# 起因：系统 %TEMP% 会让 MSVC 的 lib.exe / link.exe 报 LNK1104
# （无法打开 ...\lnk{GUID}.tmp）。口径见 build\build-release.ps1 的同一段注释。
$buildTmp = Join-Path $root 'target'
New-Item -ItemType Directory -Force -Path $buildTmp | Out-Null
$env:MML_BUILD_TMP = $buildTmp
$env:TMP = $buildTmp
$env:TEMP = $buildTmp

# 版本以 build/version 为准，先同步到应用清单
node (Join-Path $root 'build\set-version.mjs')
if ($LASTEXITCODE -ne 0) { throw '版本同步失败，已中止构建。' }

# vendor 插件要先还原（cargo 解析依赖图时就要读它的 Cargo.toml，早于构建脚本）；
# 脚本幂等，已还原过会直接跳过
powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'mml-gui\vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { throw 'vendor 插件还原失败，已中止构建。' }

foreach ($t in $targets) {
    rustup target add $t['triple'] | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "安装 Rust 目标失败：$($t['triple'])。请先安装 Rust，或手动执行 rustup target add $($t['triple'])。"
    }
}

# 直接调用 cargo 不会执行 Tauri 的 beforeBuildCommand，须先生成内嵌前端。
Push-Location (Join-Path $root 'mml-vue')
try {
    npm run build
    if ($LASTEXITCODE -ne 0) { throw '前端构建失败，已中止预发布构建。' }
}
finally {
    Pop-Location
}

Push-Location (Join-Path $root 'mml-gui\src-tauri')
try {
    foreach ($t in $targets) {
        Write-Host "正在构建 prerelease $($t['arch'])（$($t['triple'])）..."
        # 必须带 custom-protocol：不带的话 tauri 把这次构建当成 dev，
        # 前端会连接 localhost:1420；未启动开发服务时无法加载页面。
        cargo build --profile prerelease --target $t['triple'] --features custom-protocol,prerelease
        if ($LASTEXITCODE -ne 0) {
            throw "构建失败：$($t['arch'])（退出码 $LASTEXITCODE）。x86 / arm64 交叉编译需要对应架构的 MSVC 工具链。"
        }
    }
}
finally {
    Pop-Location
}

# 按架构收拢到 target\prerelease\，文件名带架构，三种可以并排放
$outDir = Join-Path $root 'target\prerelease'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
foreach ($t in $targets) {
    $src = Join-Path $root "target\$($t['triple'])\prerelease\mml-gui.exe"
    if (-not (Test-Path $src)) { throw "找不到产物：$src" }
    Copy-Item $src (Join-Path $outDir "mml-gui-$($t['arch']).exe") -Force
}

Write-Host '产物:'
foreach ($t in $targets) {
    Write-Host "  $(Join-Path $outDir "mml-gui-$($t['arch']).exe")"
}

if (-not [Console]::IsInputRedirected -and -not $env:CI) { Read-Host '按回车关闭' }
