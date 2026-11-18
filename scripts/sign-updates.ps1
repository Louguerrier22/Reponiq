$ErrorActionPreference = 'Stop'
$config = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$keyDirectory = Join-Path $env:LOCALAPPDATA 'ReponiqSigning'
$publicKey = Get-Content (Join-Path $keyDirectory 'updater-key.pub') -Raw
if ($publicKey.Trim() -cne $config.plugins.updater.pubkey) {
    throw 'The local signing identity does not match the public key embedded in Reponiq.'
}

$assets = foreach ($target in @('windows-x64.exe', 'linux-x64.AppImage')) {
    $platform = $target.Split('.')[0]
    $name = "Reponiq-$($config.version)-$target"
    $directory = Join-Path '.work/release' $platform
    $file = Join-Path $directory $name
    $checksum = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ("$checksum  $name" -cnotin (Get-Content -LiteralPath (Join-Path $directory 'SHA256SUMS.txt'))) {
        throw "The checksum for $name does not match the collected release."
    }
    if (Test-Path -LiteralPath "$file.sig") { throw "A signature already exists for $name." }
    $file
}
if (Test-Path -LiteralPath '.work/release/latest.json') { throw 'An update manifest already exists.' }

$previousKey = $env:TAURI_SIGNING_PRIVATE_KEY
$previousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
$previousKeyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
$protected = Get-Content (Join-Path $keyDirectory 'updater-key.dpapi') -Raw | ConvertTo-SecureString
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = ConvertFrom-SecureString $protected -AsPlainText
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $null
    foreach ($asset in $assets) {
        npm run tauri signer sign -- --app-version $config.version $asset
        if ($LASTEXITCODE -ne 0) { throw "Could not sign $asset." }
    }
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY = $previousKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $previousPassword
    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $previousKeyPath
    $protected.Dispose()
}
node scripts/release.mjs update-manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
