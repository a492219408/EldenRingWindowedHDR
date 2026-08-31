[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $LogPath,

    [Parameter(Mandatory)]
    [string] $Label,

    [string] $GameExePath
)

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ResolvedLogPath = (Resolve-Path -LiteralPath $LogPath).Path
$SafeLabel = ($Label.Trim() -replace '[^0-9A-Za-z._-]', '_')
if ([string]::IsNullOrWhiteSpace($SafeLabel)) {
    throw 'Label must contain at least one safe filename character'
}

$Timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$ResultRoot = Join-Path $RepositoryRoot "test-results\$Timestamp-$SafeLabel"
$null = New-Item -ItemType Directory -Force -Path $ResultRoot

Copy-Item -LiteralPath $ResolvedLogPath -Destination (Join-Path $ResultRoot 'EldenRingWindowedHDR.log')
$IniPath = [System.IO.Path]::ChangeExtension($ResolvedLogPath, '.ini')
if (Test-Path -LiteralPath $IniPath) {
    Copy-Item -LiteralPath $IniPath -Destination (Join-Path $ResultRoot 'EldenRingWindowedHDR.ini')
}

$SystemLines = [System.Collections.Generic.List[string]]::new()
$SystemLines.Add("Collected: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')")
$SystemLines.Add("Label: $Label")
$SystemLines.Add("Computer: $env:COMPUTERNAME")
$SystemLines.Add("OS: $([System.Environment]::OSVersion.VersionString)")
$SystemLines.Add("PowerShell: $($PSVersionTable.PSVersion)")

try {
    $OperatingSystem = Get-CimInstance Win32_OperatingSystem
    $SystemLines.Add("Windows caption: $($OperatingSystem.Caption)")
    $SystemLines.Add("Windows build: $($OperatingSystem.Version) build $($OperatingSystem.BuildNumber)")
}
catch {
    $SystemLines.Add("Windows CIM query failed: $($_.Exception.Message)")
}

try {
    foreach ($Gpu in Get-CimInstance Win32_VideoController) {
        $SystemLines.Add("GPU: $($Gpu.Name)")
        $SystemLines.Add("GPU driver: $($Gpu.DriverVersion)")
        $SystemLines.Add("GPU mode: $($Gpu.CurrentHorizontalResolution)x$($Gpu.CurrentVerticalResolution)@$($Gpu.CurrentRefreshRate) Hz, $($Gpu.CurrentBitsPerPixel) bpp")
    }
}
catch {
    $SystemLines.Add("GPU CIM query failed: $($_.Exception.Message)")
}

if (-not [string]::IsNullOrWhiteSpace($GameExePath)) {
    $ResolvedGameExe = (Resolve-Path -LiteralPath $GameExePath).Path
    $GameFile = Get-Item -LiteralPath $ResolvedGameExe
    $SystemLines.Add("Game EXE: $ResolvedGameExe")
    $SystemLines.Add("Game EXE size: $($GameFile.Length)")
    $SystemLines.Add("Game EXE version: $($GameFile.VersionInfo.FileVersion)")
    $SystemLines.Add("Game EXE SHA-256: $((Get-FileHash -LiteralPath $ResolvedGameExe -Algorithm SHA256).Hash)")
}
else {
    $SystemLines.Add('Game EXE: -GameExePath was not supplied; use the runtime size and SHA-256 recorded in EldenRingWindowedHDR.log')
}

$SystemLines | Set-Content -LiteralPath (Join-Path $ResultRoot 'system.txt') -Encoding utf8
@"
显示器型号：
连接方式（HDMI/DP）：
Windows“使用 HDR”状态：
游戏显示模式：
启动前预期保存的 HDR 状态：
进入设置页前观察到的 HDR/SDR 状态：
打开设置页后显示的 HDR 开关状态：
退出时的 HDR 开关状态：
HDR 选项是否灰显/可选择：
本次启动内执行的显示模式/HDR 切换：
本次是否加载角色存档：
测试场景与持续时间：
Alt+Tab 结果：
是否出现灰雾、过曝、黑位抬升、颜色异常、闪屏或黑屏：
OBS 是否关闭；若开启，请记录视频格式、色彩空间和游戏是否最小化：
同时加载的 MOD / Overlay：
其他备注：
"@ | Set-Content -LiteralPath (Join-Path $ResultRoot 'observations.txt') -Encoding utf8

Write-Host "Collected test evidence: $ResultRoot"
