$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path ([System.IO.Path]::GetTempPath()) ('grain-quality-' + [guid]::NewGuid())
$originalLocation = Get-Location
$names = @('RUSTUP_HOME', 'CARGO_HOME', 'CARGO_TARGET_DIR', 'HOME', 'USERPROFILE',
    'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME',
    'TEMP', 'TMP', 'TMPDIR', 'OPENAI_API_KEY', 'GRAIN_AI_KEY', 'ANTHROPIC_API_KEY',
    'CODEX_API_KEY', 'GRAIN_AI_PROVIDER', 'GRAIN_GENERATOR_CMD', 'OPENAI_BASE_URL',
    'GRAIN_OFFLINE', 'GRAIN_PREVIEW_BACKEND')
$saved = @{}
foreach ($name in $names) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $args failed with exit code $LASTEXITCODE"
    }
}

try {
    New-Item -ItemType Directory -Path $work | Out-Null
    # Copy project inputs only, never local credentials or sketch state.
    foreach ($path in @('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'LICENSE',
        'README.md', 'src', 'tests', 'examples', 'fixtures', 'docs', 'scripts', 'scenarios')) {
        $source = Join-Path $root $path
        if (Test-Path $source) {
            Copy-Item -Recurse -Path $source -Destination (Join-Path $work $path)
        }
    }

    # Preserve native Windows toolchain paths before isolating the user profile.
    if (-not $env:RUSTUP_HOME) { $env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup' }
    if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $env:USERPROFILE '.cargo' }
    if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $root 'target' }
    $env:HOME = Join-Path $work 'home'
    $env:USERPROFILE = $env:HOME
    $env:APPDATA = Join-Path $work 'appdata'
    $env:LOCALAPPDATA = Join-Path $work 'localappdata'
    $env:XDG_CONFIG_HOME = Join-Path $work 'config'
    $env:XDG_CACHE_HOME = Join-Path $work 'cache'
    $env:XDG_DATA_HOME = Join-Path $work 'data'
    $env:TEMP = Join-Path $work 'tmp'
    $env:TMP = $env:TEMP
    $env:TMPDIR = $env:TEMP
    foreach ($directory in @($env:HOME, $env:APPDATA, $env:LOCALAPPDATA,
        $env:XDG_CONFIG_HOME, $env:XDG_CACHE_HOME, $env:XDG_DATA_HOME, $env:TEMP)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
    foreach ($name in @('OPENAI_API_KEY', 'GRAIN_AI_KEY', 'ANTHROPIC_API_KEY',
        'CODEX_API_KEY', 'GRAIN_AI_PROVIDER', 'GRAIN_GENERATOR_CMD', 'OPENAI_BASE_URL')) {
        [Environment]::SetEnvironmentVariable($name, $null, 'Process')
    }
    $env:GRAIN_OFFLINE = '1'
    $env:GRAIN_PREVIEW_BACKEND = 'half-block'

    Set-Location $work
    Invoke-Cargo fmt --all -- --check
    Invoke-Cargo clippy --locked --all-targets --all-features -- -D warnings
    Invoke-Cargo run --locked --features internal-test-fixture --bin gen_fixture
    Invoke-Cargo test --locked --lib --bins -- --test-threads=1
    Invoke-Cargo test --locked --tests --features internal-test-fixture -- --test-threads=1
    Invoke-Cargo test --locked --doc
} finally {
    Set-Location $originalLocation
    foreach ($name in $names) {
        [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
    }
    if (Test-Path $work) { Remove-Item -Recurse -Force $work }
}
