param(
    [string]$GodotPath = 'E:\Godot\Godot_v4.7-stable_win64.exe',
    [switch]$Smoke
)

$ErrorActionPreference = 'Stop'
$projectDirectory = $PSScriptRoot
$workspaceDirectory = [IO.Path]::GetFullPath((Join-Path $projectDirectory '../..'))
$consoleExecutable = Join-Path (Split-Path $GodotPath) ([IO.Path]::GetFileNameWithoutExtension($GodotPath) + '_console.exe')
if (Test-Path -LiteralPath $consoleExecutable) { $GodotPath = $consoleExecutable }

Push-Location $workspaceDirectory
try {
    & cargo build -p godot-counter --offline
    if ($LASTEXITCODE -ne 0) { throw 'Failed to build the Godot extension.' }
    & $GodotPath --headless --path $projectDirectory --editor --import 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw 'Failed to import the Godot project.' }

    if ($Smoke) {
        $previousSmoke = $env:QFRAMEWORK_GODOT_SMOKE
        try {
            $env:QFRAMEWORK_GODOT_SMOKE = '1'
            $logPath = Join-Path $workspaceDirectory 'target/godot-counter-smoke.log'
            $output = & $GodotPath --headless --path $projectDirectory --fixed-fps 60 --quit-after 60 --log-file $logPath 2>&1
            $exitCode = $LASTEXITCODE
            $output | ForEach-Object { Write-Host $_ }
            $result = $output -join "`n"
            if (Test-Path -LiteralPath $logPath) { $result += "`n" + (Get-Content -LiteralPath $logPath -Raw) }
            if ($exitCode -ne 0 -or $result -notmatch 'QFRAMEWORK_GODOT_SMOKE_OK' -or
                $result -match 'SCRIPT ERROR:|ERROR:|panicked') {
                throw 'Godot Controller integration checks failed.'
            }
        } finally {
            $env:QFRAMEWORK_GODOT_SMOKE = $previousSmoke
        }
    } else {
        & $GodotPath --path $projectDirectory
        if ($LASTEXITCODE -ne 0) { throw 'Godot exited with an error.' }
    }
} finally {
    Pop-Location
}
