# CHud::SetHelpMessage

| Campo               | Valor                                                                |
|---------------------|----------------------------------------------------------------------|
| Dirección (VA)      | `0x00588BE0`                                                         |
| File offset         | `0x00187FE0`                                                         |
| Firma (16 bytes)    | `A0 C0 AE BA 00 84 C0 0F 85 4F 01 00 00 A0 14 C0`                 |
| Convención          | `__cdecl`, 4 argumentos                                              |
| Nivel de confianza  | **ALTA**                                                             |
| Prueba en juego     | Pendiente                                                             |

## Prototipo

```rust
fn SetHelpMessage(text: *const u8, quick: u8, permanent: u8, beep: u8);
```

## Evidencia

La función comprueba el estado del HUD, copia el texto recibido a sus buffers
internos y guarda los tres flags recibidos. Los accesos a la pila, después de
salvar registros, corresponden a los cuatro argumentos:

```asm
0x00588CA6: MOV BL, [ESP+0x18] ; tercer argumento
0x00588CEF: MOV AL, [ESP+0x1C] ; cuarto argumento
0x00588D0E: MOV AL, [ESP+0x14] ; segundo argumento
0x00588D05: PUSH ESI          ; primer argumento, texto
0x00588D3C: RET
```

El caller del juego resuelve primero el texto mediante `0x6A0050` y después
actualiza el estado de ayuda. El setter acepta el texto ya resuelto y cuatro
argumentos cdecl.

`0x0058A330` no es el setter: su único caller (`0x0058FC53`) lo invoca sin
argumentos y corresponde al procesamiento/renderizado del estado del HUD.
Invocarlo como una función de cuatro argumentos retorna correctamente, pero no
registra el texto para mostrarlo.

## Corrección respecto al análisis anterior

La dirección `0x58A330` estaba identificada incorrectamente como setter porque
su prólogo parecía una función de HUD. El análisis de callers y accesos a la
pila confirma que el setter usado por el juego es `0x588BE0`.
