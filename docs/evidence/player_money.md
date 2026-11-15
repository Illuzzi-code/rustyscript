# CPlayerInfo::m_nMoney

| Campo               | Valor                                                     |
|---------------------|-----------------------------------------------------------|
| Offset en struct    | `+0xB8` desde el inicio de CPlayerInfo                   |
| VA en tiempo real   | `CWorld::Players (0xB74490) + 0xB8 = 0xB74548`           |
| Tipo                | `i32` (con signo, puede ser negativo)                     |
| Nivel de confianza  | **MEDIA** — pendiente prueba en juego                     |
| Prueba en juego     | Pendiente                                                 |

## ⚠️ Estado MEDIA — no activar hasta prueba

Esta dirección **no se usa en el mod** hasta que el usuario confirme en juego  
que leer/escribir `[0xB74490 + 0xB8]` produce el valor correcto de dinero.

## Evidencia

### Escrituras a [reg+0xB8]
Búsqueda de `C7 86 B8 00 00 00` (MOV [ESI+0xB8], imm32):
```
0x00510891: C7 86 B8 00 00 00 00 00 00 00   ; MOV [ESI+0xB8], 0  (reset money)
0x00528B56: C7 86 B8 00 00 00 00 00 80 3F   ; MOV [ESI+0xB8], float-looking value
```

Búsqueda de `89 86 B8 00 00 00` (MOV [ESI+0xB8], EAX):
```
0x004FD782: 89 86 B8 00 00 00    (transferencia de dinero)
0x005018C0: 89 86 B8 00 00 00    (transferencia de dinero)
0x00528B3D: 89 86 B8 00 00 00    (transferencia de dinero)
0x0057178D: 89 86 B8 00 00 00    (transferencia de dinero)
0x00607195: 89 86 B8 00 00 00    (transfer stats)
0x007D1D56: 89 86 B8 00 00 00    (save/load)
0x007D33B7: 89 86 B8 00 00 00    (save/load)
```

### Incertidumbre
El offset `0xB8` coincide con plugin-sdk para `CPlayerInfo::m_nMoney`, pero
la función `0x528B56` escribe un valor que parece `float` (`00 00 80 3F` = 1.0f),
lo que sugiere que podría ser otro campo en ese contexto.
La función `0x510891` que hace un reset a 0 es más convincente para dinero.

**Requiere prueba en juego antes de usar.**

### Referencia cruzada
plugin-sdk: `CPlayerInfo::m_nMoney` @ offset `0xB8`. Hipótesis a confirmar.
