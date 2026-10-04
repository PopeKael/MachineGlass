# Render our original vector geometry at each Windows icon size. No external tools.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$assetDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'assets'
$frames = [System.Collections.Generic.List[byte[]]]::new()
$sizes = @(16, 32, 48, 64, 128, 256)
foreach ($size in $sizes) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.ScaleTransform($size / 64.0, $size / 64.0)
    $navy = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#101720'))
    $blue = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#62a7f4'))
    $tealPen = [System.Drawing.Pen]::new([System.Drawing.ColorTranslator]::FromHtml('#3fd5b7'), 5)
    $whitePen = [System.Drawing.Pen]::new([System.Drawing.ColorTranslator]::FromHtml('#f2f6fa'), 5)
    $tealPen.StartCap = $tealPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $whitePen.StartCap = $whitePen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $back = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $back.AddArc(1,1,28,28,180,90); $back.AddArc(35,1,28,28,270,90)
    $back.AddArc(35,35,28,28,0,90); $back.AddArc(1,35,28,28,90,90); $back.CloseFigure()
    $graphics.FillPath($navy, $back)
    $ear = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $ear.AddLine(17,10,12,14); $ear.AddLine(12,14,12,24)
    $ear.AddBezier(12,24,12,41,38,41,38,24)
    $ear.AddLine(38,24,38,14); $ear.AddLine(38,14,33,10)
    $graphics.DrawPath($tealPen,$ear)
    $tube = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $tube.AddLine(25,37,25,44); $tube.AddBezier(25,44,25,58,52,58,52,43); $tube.AddLine(52,43,52,33)
    $graphics.DrawPath($tealPen,$tube)
    $graphics.DrawLine($whitePen,17,10,20,12); $graphics.DrawLine($whitePen,33,10,30,12)
    $graphics.FillEllipse($blue,44,19,16,16); $graphics.FillEllipse($navy,48,23,8,8)
    $stream = [System.IO.MemoryStream]::new()
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $frames.Add($stream.ToArray())
    if ($size -eq 64) {
        $bitmap.Save((Join-Path $assetDir 'stethoscope.png'), [System.Drawing.Imaging.ImageFormat]::Png)
        $rgba = [byte[]]::new(64 * 64 * 4)
        for ($y = 0; $y -lt 64; $y++) { for ($x = 0; $x -lt 64; $x++) {
            $pixel = $bitmap.GetPixel($x, $y); $index = ($y * 64 + $x) * 4
            $rgba[$index] = $pixel.R; $rgba[$index+1] = $pixel.G; $rgba[$index+2] = $pixel.B; $rgba[$index+3] = $pixel.A
        } }
        [IO.File]::WriteAllBytes((Join-Path $assetDir 'stethoscope.rgba'), $rgba)
    }
    foreach ($owned in @($stream,$back,$ear,$tube,$tealPen,$whitePen,$navy,$blue,$graphics,$bitmap)) { $owned.Dispose() }
}
$iconStream = [IO.File]::Create((Join-Path $assetDir 'machineglass.ico'))
$writer = [IO.BinaryWriter]::new($iconStream)
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $dimension = if ($sizes[$i] -eq 256) { 0 } else { $sizes[$i] }
        $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
        $writer.Write([byte]0); $writer.Write([byte]0); $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$i].Length); $writer.Write([uint32]$offset)
        $offset += $frames[$i].Length
    }
    foreach ($frame in $frames) { $writer.Write($frame) }
} finally { $writer.Dispose() }
