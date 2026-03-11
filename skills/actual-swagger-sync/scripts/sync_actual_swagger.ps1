[CmdletBinding()]
param(
    [string]$OutputPath = "crates/api/actual/swagger.json",
    [string]$Image = "jhonderson/actual-http-api:26.3.0",
    [int]$PreferredPort = 5007,
    [int]$TimeoutSeconds = 60,
    [int]$MaxPortAttempts = 10
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$containerName = "actual-swagger-sync-$PID"
$containerPort = 5007
$startedContainer = $false

function Remove-ContainerIfExists {
    param([string]$Name)

    $existing = docker ps -aq --filter "name=^$Name$"
    if ($LASTEXITCODE -ne 0) {
        throw "failed to query docker containers"
    }

    if ($existing) {
        docker rm -f $Name | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "failed to remove existing container '$Name'"
        }
    }
}

function Get-SwaggerBody {
    param([string]$Url, [int]$TimeoutSeconds)

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 5
            if ($response.StatusCode -eq 200 -and $response.Content) {
                return $response.Content
            }
        }
        catch {
            Start-Sleep -Milliseconds 500
        }
    }

    return $null
}

function New-PortCandidates {
    param([int]$Primary, [int]$Attempts)

    if ($Attempts -lt 1) {
        throw "MaxPortAttempts must be at least 1"
    }

    $ports = [System.Collections.Generic.List[int]]::new()
    $null = $ports.Add($Primary)

    while ($ports.Count -lt $Attempts) {
        $candidate = Get-Random -Minimum 20000 -Maximum 60001
        if (-not $ports.Contains($candidate)) {
            $null = $ports.Add($candidate)
        }
    }

    return $ports
}

try {
    Get-Command docker | Out-Null

    $body = $null
    $lastSwaggerUrl = $null
    $portCandidates = New-PortCandidates -Primary $PreferredPort -Attempts $MaxPortAttempts

    Remove-ContainerIfExists -Name $containerName

    foreach ($hostPort in $portCandidates) {
        $swaggerUrl = "http://127.0.0.1:$hostPort/api-docs/swagger.json"
        $lastSwaggerUrl = $swaggerUrl

        # Reuse an already-running API endpoint when available.
        if ($hostPort -eq $PreferredPort) {
            $body = Get-SwaggerBody -Url $swaggerUrl -TimeoutSeconds 3
            if ($body) {
                break
            }
        }

        $runOutput = docker run -d --name $containerName -p "$hostPort`:$containerPort" -e ACTUAL_SERVER_URL=localhost -e ACTUAL_SERVER_PASSWORD=pass -e API_KEY=pass $Image 2>&1
        if ($LASTEXITCODE -ne 0) {
            $combinedOutput = ($runOutput | Out-String)
            if ($combinedOutput -match "port is already allocated") {
                continue
            }
            throw "failed to start docker container from image '$Image': $combinedOutput"
        }

        $startedContainer = $true
        $body = Get-SwaggerBody -Url $swaggerUrl -TimeoutSeconds $TimeoutSeconds

        if ($body) {
            break
        }

        Remove-ContainerIfExists -Name $containerName
        $startedContainer = $false
        throw "timed out after $TimeoutSeconds seconds waiting for $swaggerUrl"
    }

    if (-not $body) {
        throw "could not retrieve swagger after trying $MaxPortAttempts ports (last url: $lastSwaggerUrl)"
    }

    $null = $body | ConvertFrom-Json

    $absoluteOutput = Join-Path (Get-Location) $OutputPath
    $outputDirectory = Split-Path -Parent $absoluteOutput
    if (-not (Test-Path $outputDirectory)) {
        New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
    }

    $body | Set-Content -Encoding UTF8 -NoNewline $absoluteOutput
    Write-Host "Wrote swagger schema to $absoluteOutput"
}
finally {
    try {
        if ($startedContainer) {
            Remove-ContainerIfExists -Name $containerName
        }
    }
    catch {
        Write-Warning "cleanup failed for container '$containerName': $($_.Exception.Message)"
    }
}