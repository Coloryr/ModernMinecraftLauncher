# 正式发布构建：fat LTO + 单代码生成单元，体积最小、运行最快，但出包慢 ——
# 末尾的跨 crate 优化是单线程的，会看到只有一个核在跑；产出安装包（tauri 打包链）。
# 预发布 / 日常打包用 build\build-prerelease.ps1。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\build-release.ps1
# 对应的 bash 版：build/build-release.sh

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

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

Push-Location (Join-Path $root 'mml-gui')
try {
    Write-Host '正在构建 release（fat LTO，含安装包）...'
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "构建失败（退出码 $LASTEXITCODE）" }
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
