Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# 脚本目录下的实验 crate 目录。
$labDir = Join-Path $PSScriptRoot "tg-rcore-tutorial-scheduler-lab"

# 进入实验目录执行 release 运行，保证结果可复现。
Push-Location $labDir
try {
    cargo run --release
} finally {
    # 无论成功失败都回到原目录。
    Pop-Location
}
