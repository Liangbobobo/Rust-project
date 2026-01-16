$ErrorActionPreference = "Stop"
$desktopPath = "C:\Users\liang\Desktop"
# Using a pattern to avoid encoding issues with Chinese characters in the script file
$filePattern = "*20260116171922_34_2.jpg"

$file = Get-ChildItem -Path $desktopPath -Filter $filePattern | Select-Object -First 1

if (-not $file) {
    Write-Error "Image file not found with pattern $filePattern in $desktopPath"
    exit 1
}

$imagePath = $file.FullName
Write-Host "Found image: $imagePath"

$outputPath = "C:\Users\liang\Desktop\2_inch_photos.docx"

# Dimensions: 3.5cm x 5.3cm
# 1 cm = 28.3465 points
$width = 3.5 * 28.3465
$height = 5.3 * 28.3465

try {
    $word = New-Object -ComObject Word.Application
    $word.Visible = $false
    
    $doc = $word.Documents.Add()
    $selection = $word.Selection
    
    # Page Setup A4
    $doc.PageSetup.PageWidth = 595.3
    $doc.PageSetup.PageHeight = 841.85
    $doc.PageSetup.LeftMargin = 28.35
    $doc.PageSetup.RightMargin = 28.35
    $doc.PageSetup.TopMargin = 28.35
    $doc.PageSetup.BottomMargin = 28.35

    # Insert 20 images
    for ($i = 0; $i -lt 20; $i++) {
        $shp = $selection.InlineShapes.AddPicture($imagePath)
        $shp.LockAspectRatio = 0 # msoFalse
        $shp.Width = $width
        $shp.Height = $height
        $selection.TypeText("  ")
    }
    
    $doc.SaveAs($outputPath)
    $doc.Close()
    Write-Host "Success: Document created at $outputPath"
} catch {
    Write-Error "Failed: $_"
} finally {
    if ($word) {
        $word.Quit()
        [System.Runtime.Interopservices.Marshal]::ReleaseComObject($word) | Out-Null
    }
}