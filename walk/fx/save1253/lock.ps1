# Walter #1253: hold a file open with no sharing, like a virus scanner would.
param([string]$Path, [double]$Delay = 1.0, [double]$Hold = 0.6)
Start-Sleep -Milliseconds ([int]($Delay * 1000))
$t0 = Get-Date
$f = [System.IO.File]::Open($Path, 'Open', 'Read', 'None')
"locked $Path at $($t0.ToString('HH:mm:ss.fff'))" | Out-File -Append D:\w\lock.log
Start-Sleep -Milliseconds ([int]($Hold * 1000))
$f.Close()
"released at $((Get-Date).ToString('HH:mm:ss.fff'))" | Out-File -Append D:\w\lock.log
