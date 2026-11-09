param(
    [string]$Executable = (Join-Path $PSScriptRoot 'TermViAI-GPUI.exe'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'validation')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$reportPath = Join-Path $OutputDirectory 'render.json'
$stderrPath = Join-Path $OutputDirectory 'runtime-stderr.txt'
$stdoutPath = Join-Path $OutputDirectory 'runtime-stdout.txt'
$startInfo = New-Object System.Diagnostics.ProcessStartInfo
$startInfo.FileName = $Executable
$startInfo.Arguments = '--auto-check --report "' + $reportPath + '"'
$startInfo.UseShellExecute = $false
$startInfo.RedirectStandardError = $true
$startInfo.RedirectStandardOutput = $true
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $startInfo
[void]$process.Start()
$stderrTask = $process.StandardError.ReadToEndAsync()
$stdoutTask = $process.StandardOutput.ReadToEndAsync()
$started = Get-Date
$records = @()
try {
    while (-not $process.HasExited) {
        $process.Refresh()
        $seconds = ((Get-Date) - $started).TotalSeconds
        $records += [pscustomobject]@{
            elapsed_seconds = [Math]::Round($seconds,2)
            phase = $(if ($seconds -lt 5) {'hosts-idle'} elseif ($seconds -lt 40) {'four-streams'} else {'post-stream'})
            working_set_bytes = $process.WorkingSet64
            private_bytes = $process.PrivateMemorySize64
            cpu_seconds = $process.TotalProcessorTime.TotalSeconds
            handles = $process.HandleCount
        }
        if ($seconds -gt 90) {throw 'GPUI auto-check exceeded 90 seconds'}
        Start-Sleep -Milliseconds 1000
    }
    $process.WaitForExit()
    $stderrTask.Result | Set-Content -Encoding UTF8 $stderrPath
    $stdoutTask.Result | Set-Content -Encoding UTF8 $stdoutPath
    $records | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'process-samples.json')
    [pscustomobject]@{
        exit_code = $process.ExitCode
        peak_working_set_bytes = ($records | Measure-Object working_set_bytes -Maximum).Maximum
        peak_private_bytes = ($records | Measure-Object private_bytes -Maximum).Maximum
        logical_processors = [Environment]::ProcessorCount
        scope = 'GPUI UI process only; excludes synthetic producer children, ConPTY and shared GPU allocations'
        comparison = 'No baseline comparison; not a production terminal throughput benchmark'
    } | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'summary.json')
    if ($process.ExitCode -ne 0) {throw "GPUI exited with code $($process.ExitCode)"}
} finally {
    # Only the disposable process started by this script may be stopped.
    if (-not $process.HasExited) {Stop-Process -Id $process.Id}
}
