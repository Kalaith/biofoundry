<#
.SYNOPSIS
    Headless screenshot harness for Biofoundry.

.DESCRIPTION
    Thin wrapper around the shared macroquad-toolkit capture script. Builds
    the debug exe and drives it through the env-var capture hook
    (BIOFOUNDRY_CAPTURE_*) provided by macroquad_toolkit::capture in
    src/main.rs. Scenes: menu, warren, mine, factory, famine, raid, breeding, worm.

.EXAMPLE
    ./scripts/capture_ui.ps1
    ./scripts/capture_ui.ps1 -Scenes warren -Frames 60 -SkipBuild
#>
param(
    [string[]]$Scenes = @("menu", "warren", "mine", "factory", "famine", "raid", "breeding", "worm"),
    [int]$Frames = 150,
    [string]$OutputDir = "docs\verification",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$gameDir = Split-Path -Parent $PSScriptRoot
$shared = Join-Path (Split-Path -Parent $gameDir) "macroquad-toolkit\scripts\capture_ui.ps1"

& $shared -GameDir $gameDir -Prefix "BIOFOUNDRY" -Scenes $Scenes -Frames $Frames -OutputDir $OutputDir -SkipBuild:$SkipBuild
