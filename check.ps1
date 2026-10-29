# 根目录 cargo check 入口：先还原 vendor 插件，再执行 cargo check
#
# Cargo 在解析依赖图时就要读 mml-gui/src-tauri 的 path 依赖
# target/vendor/tauri-plugin-decoration/Cargo.toml，这早于任何构建脚本，
# 所以裸 `cargo check --all` 没法自动准备。这个脚本把两步合成一步。
#
# 用法：.\check.ps1 --all
#       .\check.ps1 -p mml-gui
# 不传参数时执行 cargo check --workspace。

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot

& (Join-Path $root 'mml-gui\vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not $CargoArgs -or $CargoArgs.Count -eq 0) {
    $CargoArgs = @('--workspace')
}

Push-Location $root
$code = 0
try {
    & cargo check @CargoArgs
    $code = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $code
