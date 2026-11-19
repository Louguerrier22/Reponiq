# Installation

[Documentation](README.md) · [User guide](USER_GUIDE.md) · [Troubleshooting](TROUBLESHOOTING.md)

Reponiq targets Windows 10/11 x64 and Linux x64. Install Git and download the installer and `SHA256SUMS.txt` from the [latest release](https://github.com/Louguerrier22/Reponiq/releases/latest). Substitute your downloaded filename in the examples below. The app uses WebView2 on Windows and WebKitGTK 4.1 on Linux.

## Windows

Choose the Windows `.exe` installer and verify its checksum in PowerShell:

```powershell
Get-FileHash .\Reponiq-1.1.0-windows-x64.exe -Algorithm SHA256
```

Compare the printed hash with the exact filename’s entry in `SHA256SUMS.txt`. Run the installer and select **Create desktop shortcut** on the final screen if you want one. To pin Reponiq, open the app, right-click its Windows taskbar icon, and choose **Pin to taskbar**. The installer uses the WebView2 bootstrapper if WebView2 is absent. No Microsoft Authenticode certificate is configured for v1.

## Linux

Download the package for your distribution and verify it against the release checksum file:

```bash
sha256sum Reponiq-1.1.0-linux-x64.deb
```

Debian/Ubuntu:

```bash
sudo apt install ./Reponiq-1.1.0-linux-x64.deb
```

Fedora/RHEL family:

```bash
sudo dnf install ./Reponiq-1.1.0-linux-x64.rpm
```

AppImage:

```bash
chmod +x Reponiq-1.1.0-linux-x64.AppImage
./Reponiq-1.1.0-linux-x64.AppImage
```

The AppImage may require FUSE 2; consult your distribution’s package manager. A desktop Secret Service provider such as GNOME Keyring or KWallet is needed for saved GitHub tokens. Run the app as your normal desktop user. Wayland/X11 session integration and package dependencies vary by distribution; check the release’s tested-platform record.

## Updates and uninstall

Install 1.1 or later manually the first time if you're running a 1.0 build. On Windows and Linux AppImage, Reponiq checks its GitHub release feed on launch and prepares a newer signed package in the background. The sidebar shows **Update ready** when the download and signature check finish. Open it and choose **Restart and update**, or fully quit from the window or tray and reopen once the installer finishes. If Reponiq is busy, let the workspace operation finish and retry. A quit during download cancels that attempt and the next launch checks again. If installation fails, retry from the update dialog or quit to leave it for the next launch. Packages installed as DEB or RPM are updated through their package manager.

Quit the app before manually installing a newer version. Keep an independent backup of the local database if needed. Uninstalling the application does not intentionally remove your local organization/history or OS credentials. Disconnect accounts in Settings before uninstalling if you want their saved tokens deleted.

Existing pre-rename installations keep their original database and credential namespace. New installations use the Reponiq names. See the [data compatibility record](ARCHITECTURE.md#data-compatibility).
