# Install and run Looking Glass on Windows

Looking Glass is an unfinished, independently written Rust recreation using your own American McGee's Alice game data. All 39 visits have completed automated campaign routes, but original-game fidelity and broader device validation remain unfinished. The current launch opens the main menu; see [the campaign audit](CAMPAIGN_RUN_AUDIT.md) for exact evidence.

## Install the Windows preview

**[Download for Windows — ZIP](https://github.com/looking-glass-project/LookingGlass/releases/download/v0.32.0-preview.2-setup.1/LookingGlass-Windows-x64-v0.32.0-preview.2-setup.1.zip)**

This link downloads the one package you need. The other GitHub release files
are optional developer, verification or video files; you do not need them to play.

1. Right-click the Windows ZIP and choose **Extract All**. Keep the entire folder
   somewhere writable, such as `Documents\LookingGlass`. Do not run it inside the ZIP.
2. Open the extracted folder and double-click **Launch.cmd**.
3. On first launch, the guided window defaults to local game files. It looks for game
   files beside the game, in Downloads, and at a previously saved location.
   If none are found, use **Choose archive** or **Choose folder**. Setup does not
   automatically select or start a download.
4. Choose your **Language**.
5. Select **Set up**. Setup checks and imports
   the files, then opens the game. Choose **New Game**.
   Next time, double-click **Launch.cmd** or the optional desktop shortcut.

You do not need to type commands, install Rust or Python, install 7-Zip separately,
or run as administrator. The Windows ZIP includes the small extractor it needs.
Setup uses the Windows PowerShell and .NET components already supplied with Windows.
It changes no machine-wide execution-policy setting.

Allow **3 GB free space** for the game, its download and saved progress. The
optional download is about **933 MB**, so its duration depends on your connection.
You can cancel from the setup window. If a connection fails, retry or use an
existing file; partial downloads are discarded rather than treated as complete.

## Where to get the game files

Use an existing compatible folder or archive that you are entitled to use.
The supported data is the **English 2011 vanilla** package. Setup's optional download
fetches **Alice1_2011_vanilla.7z** from
[American McGee's Alice (2011) on Internet Archive](https://archive.org/details/alice_202106).
The [file listing](https://archive.org/download/alice_202106) and its archive
metadata were checked on **1 October 2026**. Use files you are entitled to use.
Original game data is downloaded separately and is never bundled in the engine ZIP.

To opt in, select **Download from Internet Archive**, then **Set up**. A separate
confirmation asks whether you are entitled to download and use the files and
want to continue. It defaults to **No**. Declining leaves setup open without
starting a download or changing your existing configuration. Confirmation is
required each time you start setup's download option, including a retry or reuse
of its cached download. Selecting local files does not require this confirmation.

The optional download is hosted by that third-party service, not by Looking Glass.
Its availability and setup's checks do not establish redistribution permission
or your entitlement to use the files. Only download or import data you are
authorised to use. EA and the respective rights holders retain the original
game rights; Looking Glass claims none of them. This project is not endorsed by
or affiliated with EA or its licensors. See the [rights notice](../LEGAL.md).

For a manual download, select that exact `.7z` file under **Show all**. You do not
need the torrent, pictures or metadata files. In setup, use **Choose archive** and
select the downloaded file. You can also drag it onto **Setup.cmd**.

Already have extracted files? Select **Choose folder**, then choose any of:

- `Alice1`, containing `bin\base`;
- its parent folder, containing `Alice1\bin\base`;
- `bin\base` itself, containing the five base PK3 files and a supported language pack.

Setup copies an external folder's PK3 files into `private/game-data`; the source
folder stays unchanged. ZIP archives with the same base-folder layout are also
supported. Compatibility with 2000 retail data and third-party texture mods is
not established.
Looking Glass runs independently; you do not need to edit Steam configuration,
change Alice: Madness Returns settings, or run the original executable.

## Changing files, updates and saves

Open **Setup.cmd** to choose a different archive or folder. Setup validates the
required game and language packs and the map layout before changing the
remembered path.
Downloaded files must also match the expected size and SHA-256 checksum. Local
folders are checked for archive structure and map format, not every asset's bytes.

For an archive, setup imports the required game and language packs to
`private/game-data`. It does not extract or run the original executables. Imports
are isolated, and a validated import can be reused. An interrupted extraction may
leave an unused import folder; the previously configured game and saves are kept.

All local settings, imported files, downloads and saves are under `private/`.
External folders are copied; setup does not modify them. Moving the whole Looking
Glass folder keeps imported data working; desktop shortcuts must be recreated
after a move.

For updates, extract the new release into a **new folder** and keep the old one.
Follow that release's save-compatibility notes before copying your `private/`
folder. Setup never resets your saves. Original-game saves are not compatible.
Removing the Looking Glass folder removes its local saves/imports, so keep a backup;
an external data folder is unaffected.

## Choosing or changing the language

English, German, French, Spanish and Dutch are supported. Dutch uses English
spoken dialogue. Choose **Language** during setup.

To change it later, open **Setup.cmd**, select another language and click
**Apply language**. If its files are unavailable, choose a compatible folder,
archive or **Download**. Your game files, mods and saved progress are kept.
Use **Launch.cmd** to return to the game.

## Troubleshooting

| Problem | What to do |
| --- | --- |
| Nothing starts from the ZIP | Choose **Extract All** first, then open **Launch.cmd** in the extracted folder. |
| No game files found / Set up is disabled | Use **Choose archive** or **Choose folder**. The optional **Download from Internet Archive** requires a separate entitlement confirmation. |
| Download fails or is unavailable | Retry, or download the named archive yourself and choose it in setup. No previous game setting is replaced. |
| Integrity check fails | The download is incomplete or different. Obtain the compatible archive again, or select an existing compatible folder. |
| Missing pack / unsupported map format | Select the English 2011 vanilla archive or its Alice1 folder. |
| Unpacking fails | Check free space and that the download is complete. Retry setup. |
| Access denied | Move the whole folder outside Program Files to a writable location such as Documents. |
| Extractor missing | Extract the complete Windows ZIP again. Source users can install [7-Zip](https://www.7-zip.org/) or select already extracted files. |
| Source folder needs Rust | Use the Windows binary package, or follow the developer steps below. |
| A save fails after changing game data | Return to the previous base data and mods. |

This is an **unsigned preview**. Windows may identify it as an unknown publisher.
Check the release source and checksum; do not disable antivirus or system security.
On a managed computer, ask its administrator if policy blocks PowerShell.
Report launch errors with the exact message, Windows version and release name.
Do not post original game files, private paths or personal saves publicly.

## Build from source

Install a Windows Rust toolchain and the required Microsoft C++ build tools using the [official Rust installation instructions](https://www.rust-lang.org/tools/install). The project's minimum Rust version is declared in Cargo.toml (currently 1.86).

From the project folder:

```powershell
cargo build --release --locked
.\Setup.cmd
.\Launch.cmd
```

The compiler downloads the locked open-source dependencies. It does not fetch Alice game files. Advanced data selection is also available without setup:

```powershell
.\target\release\looking-glass.exe --data "D:\Games\Alice1\bin\base" --map gvillage
```
