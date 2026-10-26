# 正式发布构建：fat LTO + 单代码生成单元，体积最小、运行最快，但出包慢 ——
# 末尾的跨 crate 优化是单线程的，会看到只有一个核在跑；产出安装包（tauri 打包链）。
# 预发布 / 日常打包用 build\build-prerelease.ps1。
#
# Windows 默认构建三种架构：x64 / x86 / arm64，安装包集中在 build_out/：
#   mml-windows-x64-<版本>.msi / -setup.exe
#   mml-windows-x86-<版本>.msi / -setup.exe
#   mml-windows-arm64-<版本>-setup.exe（arm64 只打 NSIS）
# 只需要一种时用 -Arch：-Arch x64 / -Arch x64,x86 / -Arch arm64。
#
# 交叉编译前提：rustup target 里有对应三元组（脚本会自动 rustup target add），
# 且 Visual Studio 装了该架构的 MSVC 工具链。arm64 交叉编译要装
# 「MSVC v143 - VS 2022 C++ ARM64/ARM64EC 生成工具」；CI 上 arm64 用原生 arm runner。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\build-release.ps1
# 对应的 bash 版：build/build-release.sh

param(
    # 要构建的架构，逗号分隔：x64,x86,arm64
    [string]$Arch = 'x64,x86,arm64'
)

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

$all = @(
    @{ arch = 'x64';   triple = 'x86_64-pc-windows-msvc';  bundles = 'msi,nsis' },
    @{ arch = 'x86';   triple = 'i686-pc-windows-msvc';    bundles = 'msi,nsis' },
    # arm64 的 WiX 支持视 WiX 版本而定：只打 NSIS 安装包，避免整条构建因 MSI 失败
    @{ arch = 'arm64'; triple = 'aarch64-pc-windows-msvc'; bundles = 'nsis' }
)
$want = $Arch.Split(',') | ForEach-Object { $_.Trim().ToLower() } | Where-Object { $_ }
$targets = $all | Where-Object { $want -contains $_.arch }
if (-not $targets) {
    throw "没有匹配的架构：$Arch（可选 x64 / x86 / arm64）"
}

# ---- 构建期临时目录 ----
# 主修复在 .cargo\config.toml 的 [env]（把 TMP/TEMP 指到仓库的 target\，覆盖一切走 cargo
# 的构建：npm run tauri build / tauri dev / cargo build / cargo test）。
# 这里再设一次是为了**非 cargo 的子进程**：tauri 打包链（NSIS / WiX）不经过 cargo，
# 拿不到 [env] 注入的值。
# 起因：系统 %TEMP% 会让 MSVC 的 lib.exe / link.exe 报
#   LINK : fatal error LNK1104: 无法打开文件 "...\lnk{GUID}.tmp"（退出码 1104）。
$buildTmp = Join-Path $root 'target'
New-Item -ItemType Directory -Force -Path $buildTmp | Out-Null
$env:MML_BUILD_TMP = $buildTmp
$env:TMP = $buildTmp
$env:TEMP = $buildTmp

# 发布版本以 build/version 为准，先同步到应用清单
node (Join-Path $root 'build\set-version.mjs')
if ($LASTEXITCODE -ne 0) { throw '版本同步失败，已中止构建。' }

# vendor 插件（tauri-plugin-decoration）要先还原：它由 patch 打到上游源码上，
# 而 cargo 解析依赖图时就要读它的 Cargo.toml，早于任何构建脚本，必须在这里先跑。
# 脚本幂等，已还原过会直接跳过。
powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'mml-gui\vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { throw 'vendor 插件还原失败，已中止构建。' }

# 缺的三元组先补上；rustup target add 本身幂等，已装的不会重下
foreach ($t in $targets) {
    rustup target add $t['triple'] | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "安装 Rust 目标失败：$($t['triple'])。请先安装 Rust，或手动执行 rustup target add $($t['triple'])。"
    }
}

Push-Location (Join-Path $root 'mml-gui')
try {
    foreach ($t in $targets) {
        Write-Host "正在构建 release $($t['arch'])（$($t['triple'])）..."
        npm run tauri -- build --target $t['triple'] --bundles $t['bundles']
        if ($LASTEXITCODE -ne 0) {
            throw "构建失败：$($t['arch'])（退出码 $LASTEXITCODE）。x86 / arm64 交叉编译需要对应架构的 MSVC 工具链。"
        }
    }
}
finally {
    Pop-Location
}

# 把 Tauri 的安装包按 build/version 重命名后集中到 build_out/
Write-Host '正在收集安装包到 build_out ...'
node (Join-Path $root 'build\package-release.mjs')
if ($LASTEXITCODE -ne 0) { throw "安装包收集失败（退出码 $LASTEXITCODE）" }

# 右键「使用 PowerShell 运行」时窗口会在结束时关掉，停一下看输出；
# 非交互（管道 / 自动化）不问，免得卡住
if (-not [Console]::IsInputRedirected -and -not $env:CI) { Read-Host '按回车关闭' }
