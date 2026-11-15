# CMessages::AddMessageJumpQ

| Campo               | Valor                                                                 |
|---------------------|-----------------------------------------------------------------------|
| Dirección (VA)      | `0x0069F1E0`                                                          |
| File offset         | `0x29E5E0`                                                            |
| Firma (16 bytes)    | `81 EC 20 03 00 00 56 8B B4 24 28 03 00 00 68 90`                    |
| Convención          | `__cdecl`, 4 argumentos                                               |
| Nivel de confianza  | **ALTA**                                                              |
| Prueba en juego     | Pendiente                                                             |

## Prototipo
```rust
fn AddMessageJumpQ(text: *const u8, time_ms: u16, flag: i16, unk: i32);
```

## Evidencia

### Firma de bytes
```
81 EC 20 03 00 00    sub esp, 320h      ; reserva 800 bytes en la pila (buffer de texto)
56                   push esi
8B B4 24 28 03 00 00 mov esi, [esp+328h]  ; primer arg = puntero a texto
```
El `sub esp, 320h` es característico de las funciones de mensajes que necesitan un buffer grande para el texto en pantalla.

### Callers encontrados
| Caller VA    | Contexto resumido                                        |
|--------------|----------------------------------------------------------|
| `0x004681BE` | `push 0; push ecx; push eax; push edi; call 0x69F1E0; add esp,10h` → 4 args, cdecl ✓ |
| `0x005B187E` | `push 1; push 1; push edx; push eax; call 0x69F1E0` → 4 args, cdecl ✓ |

El `add esp, 10h` (16 = 4×4) después del call confirma 4 argumentos de 4 bytes = `__cdecl`.

### Variantes encontradas
Hay múltiples funciones con el mismo prólogo `81 EC 20 03 00 00` en la zona `0x69Exxx`:
- `0x69E363`, `0x69E5F8`, `0x69E6E8` — otras variantes (AddMessageQ, AddBigMessage, etc.)
- `0x69F1E0` es la que tiene exactamente el mismo caller pattern que el mod existente registró.

### Referencia cruzada
plugin-sdk: `CMessages::AddMessageJumpQ` @ `0x69F1E0`. Confirmado ✓.
