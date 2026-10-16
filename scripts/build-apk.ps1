<#
.SYNOPSIS
  Build, align and sign the Launcher Lab APK locally (arm64-v8a, pure Rust).

.DESCRIPTION
  Reproduces what .github/workflows/android.yml does on a GitHub runner, but on the
  local Windows host:

    1. locate the Android SDK (build-tools + platform) and the NDK
    2. make the GNU host toolchain's link helpers (dlltool/as/ld) reachable
    3. point the aarch64-linux-android target at the NDK clang wrapper
    4. `cargo apk build --release --lib` -> cargo-apk runs aapt, zipalign and
       apksigner itself, signing with CARGO_APK_RELEASE_KEYSTORE
    5. re-verify with `apksigner verify`, inspect with `aapt2 dump badging`,
       and print the SHA-256

  The keystore is read from -KeystoreDir (default: C:\Users\raj\launcher-lab-keys)
  and is never written to the repository.

.EXAMPLE
  pwsh -File scripts\build-apk.ps1
  pwsh -File scripts\build-apk.ps1 -SkipSign   # compile the cdylib only, no APK
#>
[CmdletBinding()]
param(
    [string] $Sdk      = $env:ANDROID_HOME,
    [string] $Ndk      = $env:ANDROID_NDK_HOME,
    [string] $JavaHome = $env:JAVA_HOME,
    [string] $KeystoreDir = 'C:\Users\raj\launcher-lab-keys',
    [string] $Abi      = 'arm64-v8a',
    [string] $Target   = 'aarch64-linux-android',
    [int]    $MinSdk   = 29,
    [switch] $SkipSign
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

function Fail($msg) { Write-Error $msg; exit 1 }

# ---- 1. toolchain discovery -------------------------------------------------
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }

# The Rust host toolchain here is the GNU ABI target (x86_64-pc-windows-gnu), so host
# build scripts and proc-macros need `dlltool`/`as`/`ld` on PATH to link import libraries.
$mingwBin = if ($env:MINGW_BIN) { $env:MINGW_BIN } else { Join-Path $env:USERPROFILE 'tools\mingw64\bin' }
if (Test-Path (Join-Path $mingwBin 'dlltool.exe')) { $env:Path = "$mingwBin;$env:Path" }
else { Write-Warning "dlltool not found in $mingwBin - host-ABI builds may fail. Set MINGW_BIN." }

if (-not $Sdk -or -not (Test-Path $Sdk)) { $Sdk = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not (Test-Path $Sdk)) { Fail "Android SDK not found. Pass -Sdk <path>." }

$buildTools = Get-ChildItem (Join-Path $Sdk 'build-tools') -Directory -ErrorAction SilentlyContinue |
    Sort-Object { [version]($_.Name -replace '[^0-9.]', '') } -Descending | Select-Object -First 1
if (-not $buildTools) { Fail "No build-tools under $Sdk\build-tools." }
$bt = $buildTools.FullName

if (-not $Ndk -or -not (Test-Path $Ndk)) {
    $Ndk = Get-ChildItem (Join-Path $Sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $Ndk) { Fail "Android NDK not found. Pass -Ndk <path>." }

if (-not $JavaHome -or -not (Test-Path $JavaHome)) { $JavaHome = $env:JAVA_HOME }
if (-not $JavaHome) { Fail "JAVA_HOME is not set (needed by keytool/apksigner)." }

$toolchain = Get-ChildItem (Join-Path $Ndk 'toolchains\llvm\prebuilt') -Directory | Select-Object -First 1 -ExpandProperty FullName
$linker = Join-Path $toolchain "bin\$Target$MinSdk-clang.cmd"
if (-not (Test-Path $linker)) { Fail "NDK clang wrapper not found: $linker" }

$env:ANDROID_HOME     = $Sdk
$env:ANDROID_NDK_HOME = $Ndk
$env:ANDROID_NDK_ROOT = $Ndk
$env:JAVA_HOME        = $JavaHome
$env:Path             = "$bt;$env:JAVA_HOME\bin;$env:Path"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $linker

Write-Host "SDK        : $Sdk"
Write-Host "build-tools: $($buildTools.Name)"
Write-Host "NDK        : $Ndk"
Write-Host "linker     : $linker"

# ---- 2. rust target ---------------------------------------------------------
if (-not (rustup target list --installed | Select-String -Quiet '^aarch64-linux-android$')) {
    rustup target add aarch64-linux-android
}

Push-Location $repo
try {
    if ($SkipSign) {
        cargo build --release --lib --target $Target
        if ($LASTEXITCODE -ne 0) { Fail "cargo build failed." }
        Write-Host "SkipSign: compiled the cdylib only (no APK packaged)."
        exit 0
    }

    if (-not (Get-Command cargo-apk -ErrorAction SilentlyContinue)) {
        Fail "cargo-apk is missing. Install it with: cargo install cargo-apk"
    }

    # ---- 3. package + sign --------------------------------------------------
    # cargo-apk 0.10 has no unsigned release path: for a release profile it insists on a
    # keystore, then does aapt + zipalign + `apksigner sign` itself. The keystore holds a
    # single entry, so no --ks-key-alias is required.
    $keystore = Join-Path $KeystoreDir 'launcher-lab-release.keystore'
    $pwFile   = Join-Path $KeystoreDir 'keystore-password.txt'
    if (-not (Test-Path $keystore)) { Fail "Keystore not found: $keystore" }
    if (-not (Test-Path $pwFile))   { Fail "Password file not found: $pwFile" }
    $env:CARGO_APK_RELEASE_KEYSTORE          = $keystore
    $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = (Get-Content $pwFile -Raw).Trim()

    cargo apk build --release --lib
    if ($LASTEXITCODE -ne 0) { Fail "cargo apk build failed." }

    $apk = Join-Path $repo 'target\release\apk\launcher-lab.apk'
    if (-not (Test-Path $apk)) { Fail "Expected signed APK not produced: $apk" }

    # ---- 4. verify ----------------------------------------------------------
    & (Join-Path $bt 'zipalign.exe') -c -v 4 $apk | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "zipalign verification failed." }

    & (Join-Path $bt 'apksigner.bat') verify --print-certs --verbose $apk |
        Select-String -Pattern 'Verified using|Signer #1 certificate DN|Signer #1 certificate SHA-256'
    if ($LASTEXITCODE -ne 0) { Fail "apksigner verification failed." }

    & (Join-Path $bt 'aapt2.exe') dump badging $apk |
        Select-String -Pattern '^package|^sdkVersion|^targetSdkVersion|^application-label:|^native-code|^uses-permission'

    $hash = (Get-FileHash $apk -Algorithm SHA256).Hash.ToLower()
    Write-Host ""
    Write-Host "APK    : $apk"
    Write-Host "size   : $((Get-Item $apk).Length) bytes"
    Write-Host "sha256 : $hash"
}
finally {
    Pop-Location
}
