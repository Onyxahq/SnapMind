param(
  [string]$lines,      # UTF-8 text file, one line per row
  [string]$out,        # output PNG
  [int]$width = 640,
  [int]$height = 0,    # 0 = fit the lines
  [int]$size = 15,     # font size in px
  [int]$columns = 1,
  [int]$repeat = 1     # repeat the lines to fill the page
)
# Renders text lines into a PNG with ClearType-like antialiasing, like a real screenshot.
# Used to make OCR test images (ASCII-only script: PowerShell 5 reads scripts as ANSI).
Add-Type -AssemblyName System.Drawing
$text = Get-Content -Encoding UTF8 $lines | Where-Object { $_.Trim() -ne '' }
$all = @()
for ($i = 0; $i -lt $repeat; $i++) { $all += $text }
$lineH = [int]($size * 1.7)
$perCol = [Math]::Ceiling($all.Count / $columns)
if ($height -eq 0) { $height = $perCol * $lineH + 2 * $lineH }
$bmp = New-Object System.Drawing.Bitmap $width, $height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::White)
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
$font = New-Object System.Drawing.Font('Microsoft YaHei UI', $size, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$brush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 32, 33, 36))
$colW = [int]($width / $columns)
for ($i = 0; $i -lt $all.Count; $i++) {
  $col = [Math]::Floor($i / $perCol)
  $row = $i % $perCol
  $g.DrawString($all[$i], $font, $brush, [float]($col * $colW + 24), [float]($lineH + $row * $lineH))
}
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
"saved $out ${width}x${height} lines=$($all.Count)"
