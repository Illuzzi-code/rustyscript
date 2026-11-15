# CWorld::Players

| Campo               | Valor                                               |
|---------------------|-----------------------------------------------------|
| Dirección (VA)      | `0x00B74490`                                        |
| Tipo                | Variable global estática (array de CPlayerInfo)     |
| Nivel de confianza  | **ALTA**                                            |
| Prueba en juego     | Pendiente                                           |

## Evidencia

### Referencias en código
Búsqueda del valor `0x00B74490` como DWORD en la sección `.text`:  
**376 referencias encontradas**.

Patrón dominante: `8B xx D 90 44 B7 00` = `MOV r, [0xB74490]`  
(lectura del puntero al ped del jugador).

Ejemplos representativos:
```
0x00404551: A1 90 44 B7 00     MOV EAX, [0xB74490]
0x0040524A: 8B 3D 90 44 B7 00  MOV EDI, [0xB74490]
0x00409D8A: A1 90 44 B7 00     MOV EAX, [0xB74490]
```

### Uso en código de cheats
En `0x00439B70` (zona de cheats):
```
8B 2D 90 44 B7 00    MOV EBP, [0xB74490]   ; cargar CPlayerInfo[0]
8B 5D 08             MOV EBX, [EBP+8]      ; ← posible puntero al ped
```

### Referencia cruzada
plugin-sdk: `CWorld::Players` @ `0xB74490`. Confirmado ✓.

## Estructura
```
// 0xB74490 contiene la dirección base de CPlayerInfo[0] (doble puntero)
*(0xB74490)        → dirección de CPlayerInfo struct
*(CPlayerInfo + 0) → puntero al CPed del jugador  ← [EBP+0x00] en 0x439BAA
*(CPlayerInfo + 4) → otro campo                   ← [EBP+0x04] en 0x439B90
*(CPlayerInfo + 8) → índice/handle de vehículo?   ← [EBP+0x08] en 0x439B74, × 0x7C4
```

### Confirmación definitiva — función 0x439B20
```asm
0x439B6E  MOV EBP, [0xB74490]       ; EBP = *(0xB74490) = CPlayerInfo base
0x439B74  MOV EBX, [EBP+0x08]       ; EBX = *(CPlayerInfo+8) → índice vehículo
0x439B77  TEST EBX, EBX
0x439B79  JZ  → salida
0x439B90  MOV EAX, [EBP+0x04]
0x439BAA  MOV EDI, [EBP+0x00]       ; EDI = *(CPlayerInfo+0) = puntero al CPed ✓
0x439BB5  MOV ECX, EDI
0x439BB7  CALL 0x5DF8F0              ; llama función de ped con this=EDI
0x439BE9  MOV EAX, [EDI+0x47C]      ; accede a campo del ped via EDI ✓
```

### Código correcto en Rust
```rust
// DOS dereferencias necesarias:
let player_info_base = (0xB74490usize as *const usize).read();  // = CPlayerInfo base
let ped_ptr = (player_info_base as *const usize).read();         // = CPed ptr
```
