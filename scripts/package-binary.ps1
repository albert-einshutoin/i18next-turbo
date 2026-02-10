param(
  [Parameter(Mandatory = $true)][string]$Target,
  [Parameter(Mandatory = $true)][string]$Artifact,
  [Parameter(Mandatory = $true)][string]$BinaryName
)

$ErrorActionPreference = 'Stop'

New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item "target\\$Target\\release\\$BinaryName" "dist\\$BinaryName"
Compress-Archive -Path "dist\\$BinaryName" -DestinationPath "dist\\$Artifact.zip" -Force
