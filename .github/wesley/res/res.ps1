$src = @"
using System; using System.Runtime.InteropServices;
public class Disp {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Ansi)]
  public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra; public int dmFields;
    public int dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string dmFormName;
    public short dmLogPixels; public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
  }
  [DllImport("user32.dll")] public static extern bool EnumDisplaySettings(string d, int n, ref DEVMODE m);
  [DllImport("user32.dll")] public static extern int ChangeDisplaySettings(ref DEVMODE m, int f);
}
"@
Add-Type $src
$m = New-Object Disp+DEVMODE; $m.dmSize = [Runtime.InteropServices.Marshal]::SizeOf($m)
$modes = @{}; $i = 0
while ([Disp]::EnumDisplaySettings($null, $i, [ref]$m)) { $modes["$($m.dmPelsWidth)x$($m.dmPelsHeight)"] = 1; $i++ }
"MODES: " + (($modes.Keys | Sort-Object { [int]($_ -split 'x')[0] }) -join ' ')
foreach ($wh in @(@(2560,1440), @(1920,1080))) {
  [Disp]::EnumDisplaySettings($null, -1, [ref]$m) | Out-Null
  $m.dmPelsWidth = $wh[0]; $m.dmPelsHeight = $wh[1]; $m.dmFields = 0x180000
  $r = [Disp]::ChangeDisplaySettings([ref]$m, 0)
  "TRY $($wh[0])x$($wh[1]) -> $r"
  if ($r -eq 0) { break }
}
Add-Type -AssemblyName System.Windows.Forms
"NOW: " + [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
