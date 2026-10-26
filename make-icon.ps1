# Draws icon-source.png (1024 px): a chip with pins and an "X" inside, in the
# Skale blues on the near-black square shared by the Skale X apps.
# Then run:  npx tauri icon icon-source.png -o src-tauri/icons
param([string]$Out = (Join-Path $PSScriptRoot 'icon-source.png'))
Add-Type -AssemblyName System.Drawing
$size = 1024
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear([System.Drawing.Color]::Transparent)

function RoundRect($x, $y, $w, $h, $r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $p.AddArc($x, $y, $r, $r, 180, 90)
  $p.AddArc($x + $w - $r, $y, $r, $r, 270, 90)
  $p.AddArc($x + $w - $r, $y + $h - $r, $r, $r, 0, 90)
  $p.AddArc($x, $y + $h - $r, $r, $r, 90, 90)
  $p.CloseFigure()
  $p
}

$g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 12, 13, 18))), (RoundRect 32 32 960 960 230))

$soft = [System.Drawing.Color]::FromArgb(255, 143, 169, 238)
$brand = [System.Drawing.Color]::FromArgb(255, 81, 115, 214)

# Pins: four per side
$pin = New-Object System.Drawing.Pen $soft, 46
$pin.StartCap = 'Round'; $pin.EndCap = 'Round'
foreach ($i in 0..3) {
  $c = 362 + $i * 100
  $g.DrawLine($pin, $c, 170, $c, 250); $g.DrawLine($pin, $c, 774, $c, 854)
  $g.DrawLine($pin, 170, $c, 250, $c); $g.DrawLine($pin, 774, $c, 854, $c)
}

# Chip body
$body = New-Object System.Drawing.Pen $soft, 52
$g.DrawPath($body, (RoundRect 262 262 500 500 120))

# "X" inside
$x = New-Object System.Drawing.Pen $brand, 92
$x.StartCap = 'Round'; $x.EndCap = 'Round'
$g.DrawLine($x, 412, 412, 612, 612)
$g.DrawLine($x, 612, 412, 412, 612)

$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"saved $Out"
