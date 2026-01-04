---
description: How to run cargo commands in this project
---

# Cargo Commands

PowerShell does not have cargo in PATH by default. Use one of these methods:

## Method 1: Full Path (Recommended)
```powershell
C:\Users\jessa\.cargo\bin\cargo.exe check
C:\Users\jessa\.cargo\bin\cargo.exe build
C:\Users\jessa\.cargo\bin\cargo.exe test
```

## Method 2: Add to Path for Session
```powershell
$env:Path = "$HOME\.cargo\bin;$env:Path"
cargo check
```

## Submodule Required
// turbo
Before building, ensure the submodule is initialized:
```powershell
git submodule update --init --recursive
```

## Build from Workspace Root
// turbo
Run builds from `c:\localdev\qdrant-rs`:
```powershell
cd c:\localdev\qdrant-rs
C:\Users\jessa\.cargo\bin\cargo.exe check
```

Or from the package directory:
```powershell
cd c:\localdev\qdrant-rs\qdrantrs-port
C:\Users\jessa\.cargo\bin\cargo.exe check
```
