# Sets the primary display to WIDTH x HEIGHT (default 1920 x 1080) and prints
# the size Windows reports afterwards. GitHub's Windows runners start at
# 1024 x 768; their virtual display offers 1920 x 1080 but not 2560 x 1440.
param([int]$Width = 1920, [int]$Height = 1080)
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
$m = New-Object Disp+DEVMODE
$m.dmSize = [Runtime.InteropServices.Marshal]::SizeOf($m)
[Disp]::EnumDisplaySettings($null, -1, [ref]$m) | Out-Null
$m.dmPelsWidth = $Width; $m.dmPelsHeight = $Height; $m.dmFields = 0x180000
$r = [Disp]::ChangeDisplaySettings([ref]$m, 0)
Add-Type -AssemblyName System.Windows.Forms
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
"display: asked ${Width}x${Height}, result code $r, now $($b.Width)x$($b.Height)"
if ($b.Width -ne $Width -or $b.Height -ne $Height) { exit 1 }
