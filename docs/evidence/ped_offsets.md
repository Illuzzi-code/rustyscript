# Offsets del CPed (jugador)

Todos los offsets están verificados mediante búsqueda de patrón en la sección `.text`
del binario (`gta_sa.exe`, SHA-256: `F01A00CE...`).

## CPed::m_fHealth

| Campo           | Valor            |
|-----------------|------------------|
| Offset          | `+0x540`         |
| Tipo            | `f32`            |
| Rango normal    | 0.0 – 100.0      |
| Rango god mode  | hasta 1000.0     |
| Hits en binario | 132              |
| Confianza       | **ALTA**         |
| Prueba en juego | ✅ `health 23.6 → 100.1`, `81.2 → 100.1` — valores reales confirmados |

### Evidencia
Búsqueda de `40 05 00 00` (representación little-endian del offset `0x540` como DWORD
en instrucciones de tipo `[reg+0x540]`): 132 hits en `.text`.

Ejemplos:
```
0x00444F0D: 89 86 40 05 00 00    MOV [ESI+0x540], EAX   (escritura de salud)
0x00442101: D9 9E 40 05 00 00    FSTP [ESI+0x540]        (escritura float FPU)
0x004420A8: D9 86 40 05 00 00    FLD [ESI+0x540]         (lectura float FPU)
```

La función `0x444E59` escribe tanto `0x540` como `0x544` y `0x548`, confirmando
que son los tres campos contiguos health/maxhealth/armor.

## CPed::m_fMaxHealth

| Campo           | Valor            |
|-----------------|------------------|
| Offset          | `+0x544`         |
| Tipo            | `f32`            |
| Rango normal    | 100.0            |
| Hits en binario | 27               |
| Confianza       | **ALTA**         |

### Evidencia
Búsqueda de `44 05 00 00`: 27 hits.

Ejemplos:
```
0x00444F07: 89 86 44 05 00 00    MOV [ESI+0x544], EAX   (escritura maxhealth)
0x0047E9A7: 8B 86 44 05 00 00    MOV EAX, [ESI+0x544]   (lectura maxhealth)
```

La función `0x444F07` escribe primero `0x544` y luego inmediatamente `0x540`,
lo que es exactamente el patrón "establecer maxhealth y luego health = maxhealth"
típico de un cheat de salud. ✓

## CPed::m_fArmor

| Campo           | Valor            |
|-----------------|------------------|
| Offset          | `+0x548`         |
| Tipo            | `f32`            |
| Rango normal    | 0.0 – 100.0      |
| Hits en binario | 57               |
| Confianza       | **ALTA**         |

### Evidencia
Búsqueda de `48 05 00 00`: 57 hits.

Ejemplos:
```
0x00438E80: 89 88 48 05 00 00    MOV [ECX+0x548], ECX   (escritura armadura)
0x004420FB: 89 9E 48 05 00 00    MOV [ESI+0x548], EBX   (escritura armadura)
0x004565B9: D9 9E 48 05 00 00    FSTP [ESI+0x548]       (escritura float FPU)
```
