# cargo tauri 的统一入口：先还原 vendor 插件，再执行 cargo tauri
#
# cargo 在解析依赖图时就要读 target/vendor/tauri-plugin-decoration/Cargo.toml，
# 这早于 Tauri 的 beforeBuildCommand，所以没法把 prepare 挂到 Tauri 配置里。
# 直接跑 `cargo tauri` 前必须还原插件；这个脚本把两步合成一步。
#
# 用法：.\tauri.ps1 dev / .\tauri.ps1 build / .\tauri.ps1 icon <svg>

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TauriArgs
)

$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'vendor\prepare.ps1')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Push-Location $PSScriptRoot
$code = 0
try {
    & cargo tauri @TauriArgs
    $code = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $code
