# 预发布构建：thin LTO + 16 个代码生成单元，能并行，出包快很多。
# 带 prerelease feature：主页面会提示「预发布版」，见 mml-gui/src-tauri/src/build_info.rs。
# 只产出可执行文件，不打安装包，产物在 <仓库>\target\prerelease\mml-gui.exe。
# 用的是根 Cargo.toml 里的 [profile.prerelease]，与 release 各有独立 target 子目录，
# 两个模式来回切不会互相刷缓存。正式发布用 build\build-release.ps1。
#
# 用法：powershell -NoProfile -ExecutionPolicy Bypass -File build\build-prerelease.ps1
# 对应的 bash 版：build/build-prerelease.sh

$ErrorActionPreference = 'Stop'

# 仓库根：本脚本在 build\ 下，往上一层
$root = Split-Path -Parent $PSScriptRoot

# ---- 构建期临时目录 ----
# 主修复在 .cargo\config.toml 的 [env]；这里再设一次是为了**非 cargo 的子进程**。
# 起因：系统 %TEMP% 会让 MSVC 的 lib.exe / link.exe 报 LNK1104
# （无法打开 ...\lnk{GUID}.tmp）。口径见 build\build-release.ps1 的同一段注释。
$buildTmp = Join-Path $root 'target'
New-Item -ItemType Directory -Force -Path $buildTmp | Out-Null
$env:MML_BUILD_TMP = $buildTmp
$env:TMP = $buildTmp
$env:TEMP = $buildTmp

# vendor 插件要先还原（cargo 解析依赖图时就要读它的 Cargo.toml，早于构建脚本）；
# 脚本幂等，已还原过会直接跳过
powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'mml-gui\vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { throw 'vendor 插件还原失败，已中止构建。' }

Push-Location (Join-Path $root 'mml-gui\src-tauri')
try {
    Write-Host '正在构建 prerelease（thin LTO）...'
    # 必须带 custom-protocol：不带的话 tauri 把这次构建当成 dev，
    # 前端去连 localhost:1420 而不是内嵌 dist，窗口永远不出来（见 Cargo.toml 的 [features]）
    cargo build --profile prerelease --features custom-protocol,prerelease
    if ($LASTEXITCODE -ne 0) { throw "构建失败（退出码 $LASTEXITCODE）" }
}
finally {
    Pop-Location
}

Write-Host "产物: $(Join-Path $root 'target\prerelease\mml-gui.exe')"

if (-not [Console]::IsInputRedirected) { Read-Host '按回车关闭' }
