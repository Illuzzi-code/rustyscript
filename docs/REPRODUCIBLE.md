# REPRODUCIBLE — Comandos exactos de análisis

Todos los comandos se ejecutan desde el directorio `C:\Users\Ericson\rustyscript`.  
El ejecutable analizado es una copia en `work\gta_sa.exe`.

## Verificación del ejecutable

```powershell
# Hash y tamaño
$f = "work\gta_sa.exe"
$h = Get-FileHash $f -Algorithm SHA256
$s = (Get-Item $f).Length
Write-Host "Size:   $s bytes"
Write-Host "SHA256: $($h.Hash)"

# Resultado esperado:
# Size:   14383616 bytes
# SHA256: F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC
```

## Análisis binario (Python 3.15)

```powershell
# Análisis inicial: secciones PE, firmas, búsqueda de strings
python tools\analyze.py work\gta_sa.exe

# Análisis profundo: callers, SetHelpMessage, cheat zone, offsets de ped
python tools\analyze2.py

# Análisis de stubs de cheats y tabla de hashes
python tools\analyze3.py

# Análisis de VEHICLE_CHEAT_CORE, offsets finales, confirmación de direcciones
python tools\analyze4.py
```

## Compilación del mod

```powershell
# Target instalado previamente con:
# rustup target add i686-pc-windows-msvc

# Build de desarrollo
cargo build --target i686-pc-windows-msvc

# Build de release (para distribución)
cargo build --release --target i686-pc-windows-msvc

# Output: target\i686-pc-windows-msvc\release\rustymod.dll
# Copiar a la carpeta del juego como rustymod.asi:
Copy-Item "target\i686-pc-windows-msvc\release\rustymod.dll" `
          "D:\GTA SA\GTA San Andreas\rustymod.asi" -Force
```

## Configuración de Ghidra headless (para análisis más profundo)

```powershell
# Crear proyecto y analizar
$GHIDRA = "C:\Users\Ericson\Downloads\Compressed\ghidra_12.2_PUBLIC\support\analyzeHeadless.bat"
$PROJECT_DIR = "C:\Users\Ericson\rustyscript\ghidra_project"
$EXE = "C:\Users\Ericson\rustyscript\work\gta_sa.exe"

New-Item -ItemType Directory -Force $PROJECT_DIR

& $GHIDRA $PROJECT_DIR gta_sa_project `
  -import $EXE `
  -overwrite `
  -processor x86:LE:32:default `
  -cspec windows `
  -postScript ExportFunctions.java `
  2>&1 | Tee-Object -FilePath "ghidra_analysis.log"
```

## Buscar una dirección específica manualmente

```python
# En cualquiera de los scripts tools/analyze*.py:
# Patrón de bytes en .text:
pattern = bytes([0x83, 0xEC, 0x0C, 0x53, 0x56, 0x57])  # prólogo típico
# Ver va_to_off() y la función de búsqueda en analyze.py
```

## Firma del binario analizado

| Campo   | Valor |
|---------|-------|
| SHA-256 | `F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC` |
| Tamaño  | 14 383 616 bytes |
| Versión | GTA San Andreas 1.0 US |
| Base    | `0x00400000` |
| ASLR    | No |
| Sección `.text` | VA `0x401000`, RawOff `0x400`, RawSize `0x455E00` |
| Sección `.data` | VA `0x4A4000`, RawOff `0x4A1C00`, RawSize `0x40000` |
