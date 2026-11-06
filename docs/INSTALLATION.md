# Installation

[Documentation](README.md) · [User guide](USER_GUIDE.md) · [Troubleshooting](TROUBLESHOOTING.md)

Reponiq 1.0 targets Windows 10/11 x64 and Linux x64. A Git installation on your PATH is required. The native app uses WebView2 on Windows and WebKitGTK 4.1 on Linux; it does not bundle Chromium or require a browser tab.

## Windows

Download `Reponiq-1.0.0-windows-x64.exe` and `SHA256SUMS.txt` from the same GitHub release. Verify the checksum in PowerShell:

```powershell
Get-FileHash .\Reponiq-1.0.0-windows-x64.exe -Algorithm SHA256
```

Compare the printed hash with the exact filename’s entry in `SHA256SUMS.txt`. Run the installer and use its per-user installation flow. The installer uses the WebView2 bootstrapper if WebView2 is absent, so a first install may require internet access. No Microsoft Authenticode certificate is configured for v1; the publisher is not digitally certified.

## Linux

Download the package for your distribution and verify it against the release checksum file:

```bash
sha256sum Reponiq-1.0.0-linux-x64.deb
```

Debian/Ubuntu:

```bash
sudo apt install ./Reponiq-1.0.0-linux-x64.deb
```

Fedora/RHEL family:

```bash
sudo dnf install ./Reponiq-1.0.0-linux-x64.rpm
```

AppImage:

```bash
chmod +x Reponiq-1.0.0-linux-x64.AppImage
./Reponiq-1.0.0-linux-x64.AppImage
```

The AppImage may require FUSE 2; consult your distribution’s package manager. A desktop Secret Service provider such as GNOME Keyring or KWallet is needed for saved GitHub tokens. Run the app as your normal desktop user. Wayland/X11 session integration and package dependencies vary by distribution; check the release’s tested-platform record.

## Updates and uninstall

Quit the app before installing a newer version. There is no automatic updater. Install the new package for your system; keep a copy of your local database if you need an independent backup. Uninstalling the application does not intentionally remove your local organization/history or OS credentials. Disconnect accounts in Settings before uninstalling if you want their saved tokens deleted.

Existing pre-rename installations keep their original database and credential namespace. New installations use the Reponiq names. See the [data compatibility record](ARCHITECTURE.md#data-compatibility).
