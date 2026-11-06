param([string]$Path, [double]$Delay = 1.0, [double]$Hold = 0.6, [string]$Log = 'D:\w\lock.log')
Start-Sleep -Milliseconds ([int]($Delay * 1000))
$dir = Split-Path $Path
$f = [System.IO.File]::Open($Path, 'Open', 'Read', 'None')
"locked $Path at $((Get-Date).ToString('HH:mm:ss.fff'))" | Out-File -Append $Log
$seen = @{}
Get-ChildItem $dir | ForEach-Object { $seen[$_.Name] = 1 }
$end = (Get-Date).AddMilliseconds([int]($Hold * 1000))
while ((Get-Date) -lt $end) {
  Get-ChildItem $dir | ForEach-Object {
    if (-not $seen.ContainsKey($_.Name)) {
      $seen[$_.Name] = 1
      "  new file $($_.Name) seen at $((Get-Date).ToString('HH:mm:ss.fff')) while locked" | Out-File -Append $Log
    }
  }
  Start-Sleep -Milliseconds 20
}
$f.Close()
"released at $((Get-Date).ToString('HH:mm:ss.fff'))" | Out-File -Append $Log
