@echo off
rem Cong cu binh (DanCard tren VectorCraft, ban cua IN Vinh Phat): cai vao AppData cua nguoi dung
rem nay, tu cap nhat tu Releases cua fork. Bam dup de cai lan dau, hoac de cai lai ban moi nhat.
rem Khong can quyen admin. Phan PowerShell nam sau dong danh dau ben duoi.
set "CCB_SELF=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$s=[IO.File]::ReadAllText($env:CCB_SELF,[Text.Encoding]::UTF8); $i=$s.IndexOf('#'+'POWERSHELL'); Invoke-Expression $s.Substring($i)"
echo.
pause
exit /b
#POWERSHELL
# Cài "Công cụ bình" cho người dùng này: %LOCALAPPDATA%\CongCuBinh\app, lối tắt trên Desktop và
# Start, mục "Open with" cho .ai .eps .pdf .svg .vectorcraft (không đổi app mặc định của loại file
# nào). App cài ở đây tự tải bản mới (apps/vectorcraft/src/dancard_update.rs).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$name = 'Công cụ bình'
$repo = 'Phanloc31012002/vectorcraft-vinhphat'
$downloads = "https://github.com/$repo/releases/download/"
$root = Join-Path $env:LOCALAPPDATA 'CongCuBinh'
$app = Join-Path $root 'app'
$work = Join-Path $root 'install'

# Khóa registry `path`, tạo khi chưa có (New-Item -Force trên khóa có sẵn xóa hết những gì trong nó).
function Ensure-Key([string] $path) {
  if (-not (Test-Path -LiteralPath $path)) { New-Item -Path $path -Force | Out-Null }
}

# Đổi tên/chuyển thư mục `from` thành `to` (chưa có), thử lại vài lần: phần mềm diệt virus có thể
# đang quét file vừa giải nén.
function Move-WithRetry([string] $from, [string] $to) {
  for ($i = 1; ; $i++) {
    try { Move-Item -LiteralPath $from -Destination $to -ErrorAction Stop; return }
    catch { if ($i -ge 10) { throw }; Start-Sleep -Milliseconds (300 * $i) }
  }
}

try {
  New-Item -ItemType Directory -Force -Path $work | Out-Null
  Write-Host 'Dang lay thong tin ban moi nhat...'
  $json = Join-Path $work 'latest.json'
  Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/$repo/releases/latest/download/latest.json" -OutFile $json
  $latest = Get-Content -LiteralPath $json -Raw -Encoding UTF8 | ConvertFrom-Json
  if (-not ($latest.url -is [string]) -or -not $latest.url.StartsWith($downloads) -or $latest.url.Contains('..')) {
    throw 'latest.json khong hop le'
  }
  if (-not ([string]$latest.sha256 -match '^[0-9a-fA-F]{64}$')) { throw 'latest.json khong co SHA-256' }
  $zip = Join-Path $work 'CongCuBinh.zip'
  Write-Host "Dang tai ban $($latest.build)..."
  Invoke-WebRequest -UseBasicParsing -Uri $latest.url -OutFile $zip
  if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne ([string]$latest.sha256).ToUpperInvariant()) {
    throw 'File tai ve bi loi (SHA-256 khong khop): chay lai file nay.'
  }
  $running = Get-Process -Name vectorcraft, vectorcraft-cli -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($app, [StringComparison]::OrdinalIgnoreCase) }
  if ($running) { throw 'App dang mo: dong app roi chay lai file nay.' }
  # Dấu của các lần cập nhật trước (bản đã tải, bản đang thử, bản bị trả lại) thuộc về bản cũ.
  $update = Join-Path $root 'update'
  if (Test-Path -LiteralPath $update) { Remove-Item -LiteralPath $update -Recurse -Force }
  $unpack = Join-Path $work 'unpack'
  if (Test-Path -LiteralPath $unpack) { Remove-Item -LiteralPath $unpack -Recurse -Force }
  Expand-Archive -LiteralPath $zip -DestinationPath $unpack -Force
  $exe = Get-ChildItem -LiteralPath $unpack -Filter 'vectorcraft.exe' -Recurse | Select-Object -First 1
  if (-not $exe) { throw 'Trong file tai ve khong co vectorcraft.exe' }
  # Bản đang cài sang một bên trước, bản mới vào chỗ, rồi mới xóa bản cũ: hỏng giữa chừng thì bản cũ
  # quay lại, máy không bao giờ mất app.
  $prev = Join-Path $root 'app.prev'
  if (Test-Path -LiteralPath $prev) { Remove-Item -LiteralPath $prev -Recurse -Force }
  if (Test-Path -LiteralPath $app) { Move-WithRetry $app $prev }
  try {
    Move-WithRetry $exe.DirectoryName $app
  } catch {
    if (Test-Path -LiteralPath $prev) { Move-WithRetry $prev $app }
    throw
  }
  Set-Content -LiteralPath (Join-Path $app 'build.txt') -Value $latest.build -Encoding Ascii
  Remove-Item -LiteralPath $prev, $unpack, $zip, $json -Recurse -Force -ErrorAction SilentlyContinue
  $target = Join-Path $app 'vectorcraft.exe'

  # Lối tắt trên Desktop và trong Start.
  $shell = New-Object -ComObject WScript.Shell
  foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {
    if (-not $dir) { continue }
    $lnk = $shell.CreateShortcut((Join-Path $dir "$name.lnk"))
    $lnk.TargetPath = $target
    $lnk.WorkingDirectory = $app
    $lnk.IconLocation = "$target,0"
    $lnk.Description = 'DanCard trên VectorCraft (tự cập nhật)'
    $lnk.Save()
  }

  # "Open with" (chuột phải › Open with), chỉ cho người dùng này.
  $classes = 'HKCU:\Software\Classes'
  $prog = "$classes\CongCuBinh.Document"
  Ensure-Key "$prog\shell\open\command"
  Ensure-Key "$prog\DefaultIcon"
  Set-Item -LiteralPath $prog -Value $name
  Set-Item -LiteralPath "$prog\DefaultIcon" -Value "`"$target`",0"
  Set-Item -LiteralPath "$prog\shell\open\command" -Value "`"$target`" `"%1`""
  foreach ($ext in '.ai', '.eps', '.pdf', '.svg', '.vectorcraft') {
    Ensure-Key "$classes\$ext\OpenWithProgids"
    New-ItemProperty -LiteralPath "$classes\$ext\OpenWithProgids" -Name 'CongCuBinh.Document' -Value '' -PropertyType String -Force | Out-Null
  }
  $apps = "$classes\Applications\vectorcraft.exe"
  Ensure-Key "$apps\shell\open\command"
  New-ItemProperty -LiteralPath $apps -Name 'FriendlyAppName' -Value $name -PropertyType String -Force | Out-Null
  Set-Item -LiteralPath "$apps\shell\open\command" -Value "`"$target`" `"%1`""

  Write-Host ''
  Write-Host "Da cai ban $($latest.build) vao $app"
  Write-Host 'Co loi tat tren Desktop. Tu nay app tu tai ban moi: dong roi mo lai la dung ban moi.'
  Start-Process -FilePath $target
} catch {
  Write-Host ''
  Write-Host "LOI: $($_.Exception.Message)" -ForegroundColor Red
}
