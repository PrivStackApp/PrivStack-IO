# Microsoft Store Assets

This folder should contain the required image assets for Microsoft Store submission.

## Required Assets

Create PNG images with the following specifications:

| Asset | Size | Filename | Purpose |
|-------|------|----------|---------|
| Store Logo | 50x50 | `StoreLogo.png` | Store listing icon |
| Small Tile | 71x71 | `SmallTile.png` | Small tile on Start |
| Square 44x44 | 44x44 | `Square44x44Logo.png` | Taskbar, Start all apps |
| Square 150x150 | 150x150 | `Square150x150Logo.png` | Start tile (medium) |
| Wide 310x150 | 310x150 | `Wide310x150Logo.png` | Start tile (wide) |
| Square 310x310 | 310x310 | `Square310x310Logo.png` | Start tile (large) |
| Splash Screen | 620x300 | `SplashScreen.png` | App launch splash |

## Optional (Recommended) Assets

For better Store presence, also create:

| Asset | Size | Filename |
|-------|------|----------|
| Badge Logo | 24x24 | `BadgeLogo.png` |
| Square 71x71 | 71x71 | `Square71x71Logo.png` |
| Target Size 16 | 16x16 | `Square44x44Logo.targetsize-16.png` |
| Target Size 24 | 24x24 | `Square44x44Logo.targetsize-24.png` |
| Target Size 32 | 32x32 | `Square44x44Logo.targetsize-32.png` |
| Target Size 48 | 48x48 | `Square44x44Logo.targetsize-48.png` |
| Target Size 256 | 256x256 | `Square44x44Logo.targetsize-256.png` |

## Store Listing Screenshots

For Partner Center submission, you'll also need:

- **Screenshots**: 1366x768 or 1920x1080 (PNG or JPG)
- **App Icon**: 300x300 (for Store listing)
- **Hero Image**: 1920x1080 (optional, for featured placement)
- **Promotional Images**: Various sizes for Store marketing

## Design Guidelines

1. **Background**: Use transparent or #0D1117 (PrivStack dark theme)
2. **Icon**: Simple, recognizable at small sizes
3. **Padding**: Leave ~12.5% padding around the icon
4. **Format**: PNG with transparency where appropriate

## Quick Generation

You can generate all required sizes from a single high-res source using ImageMagick:

```bash
# From a 1024x1024 source image
convert source.png -resize 50x50 StoreLogo.png
convert source.png -resize 71x71 SmallTile.png
convert source.png -resize 44x44 Square44x44Logo.png
convert source.png -resize 150x150 Square150x150Logo.png
convert source.png -resize 310x150 -gravity center -background "#0D1117" -extent 310x150 Wide310x150Logo.png
convert source.png -resize 310x310 Square310x310Logo.png
convert source.png -resize 620x300 -gravity center -background "#0D1117" -extent 620x300 SplashScreen.png
```

## Placeholder Generation

For testing, you can create placeholder assets:

```powershell
# PowerShell script to create colored placeholder PNGs
# Requires System.Drawing (built into Windows)
Add-Type -AssemblyName System.Drawing

$sizes = @{
    "StoreLogo.png" = @(50, 50)
    "SmallTile.png" = @(71, 71)
    "Square44x44Logo.png" = @(44, 44)
    "Square150x150Logo.png" = @(150, 150)
    "Wide310x150Logo.png" = @(310, 150)
    "Square310x310Logo.png" = @(310, 310)
    "SplashScreen.png" = @(620, 300)
}

foreach ($file in $sizes.Keys) {
    $size = $sizes[$file]
    $bmp = New-Object System.Drawing.Bitmap($size[0], $size[1])
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::FromArgb(13, 17, 23))  # #0D1117

    # Draw "PS" text
    $font = New-Object System.Drawing.Font("Arial", [Math]::Min($size[0], $size[1]) / 3)
    $brush = [System.Drawing.Brushes]::Cyan
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $rect = New-Object System.Drawing.RectangleF(0, 0, $size[0], $size[1])
    $g.DrawString("PS", $font, $brush, $rect, $sf)

    $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
    Write-Host "Created: $file"
}
```
