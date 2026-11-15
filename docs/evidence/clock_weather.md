# CClock y CWeather — globals de tiempo y clima

## CClock::ms_nGameClockHours

| Campo               | Valor                     |
|---------------------|---------------------------|
| Dirección (VA)      | `0x00B70153`              |
| Tipo                | `u8` (0–23)               |
| Nivel de confianza  | **ALTA**                  |
| Xrefs               | 5 en .text                |

## CClock::ms_nGameClockMinutes

| Campo               | Valor                     |
|---------------------|---------------------------|
| Dirección (VA)      | `0x00B70152`              |
| Tipo                | `u8` (0–59)               |
| Nivel de confianza  | **ALTA**                  |
| Xrefs               | 5 en .text                |

### Evidencia de los globales de reloj
En `0x439527`:
```
88 0D 53 01 B7 00    MOV [0xB70153], CL   ; write hours
88 0D 52 01 B7 00    MOV [0xB70152], CL   ; write minutes
66 89 0D 50 01 B7 00 MOV [0xB70150], CX  ; write packed clock word
C3                   RET
```
Escritura directa a los bytes de horas y minutos. Confirmado con 5 xrefs.

---

## CWeather::ms_OldWeatherType

| Campo               | Valor                     |
|---------------------|---------------------------|
| Dirección (VA)      | `0x00C81320`              |
| Tipo                | `u16`                     |
| Nivel de confianza  | **ALTA**                  |
| Xrefs               | 5 en .text                |

## CWeather::ms_NewWeatherType

| Campo               | Valor                     |
|---------------------|---------------------------|
| Dirección (VA)      | `0x00C81324`              |
| Tipo                | `u16`                     |
| Nivel de confianza  | **ALTA**                  |
| Xrefs               | 5 en .text                |

### IDs de clima (u16)
| ID | Clima           |
|----|-----------------|
| 0  | Soleado extra   |
| 1  | Soleado         |
| 2  | Nublado         |
| 3  | Lluvioso        |
| 4  | Tormenta        |
| 9  | Niebla          |
| 10 | Nublado extra   |
| 11 | Desierto soleado|
| 12 | Noche borrosa   |
| 17 | Nieva (never)   |

### Evidencia de los globales de clima
En `0x72A48A`:
```
66 A3 1C 13 C8 00    MOV [0xC8131C], AX
66 A3 20 13 C8 00    MOV [0xC81320], AX   ; OldWeatherType = AX
66 A3 14 13 C8 00    MOV [0xC81314], AX
```
La función `0x72A3C0` (1 caller en `0x53C086`) escribe a múltiples globales de
clima en un mismo bloque — es la función de forzado de clima.

### Uso en Rust
```rust
// Cambiar el clima:
*(0xC81320 as *mut u16) = weather_id;  // OldWeatherType
*(0xC81324 as *mut u16) = weather_id;  // NewWeatherType (fuerza el cambio inmediato)
// Cambiar la hora:
*(0xB70153 as *mut u8) = hour;         // 0-23
*(0xB70152 as *mut u8) = minute;       // 0-59
```

### NOTA: CWeather::ForceWeatherNow @ 0x578EA0
La función en `0x578EA0` **no es** ForceWeatherNow — el byte inicial `CE` = INTO
indica que estamos en medio de otra función. La escritura directa a los globales
es la forma más confiable y verificada.
