import struct

# Log reporta:
#   call site 0x53E981
#   bytes originales en memoria: E8 AA FA AB 67
#   on_game_process @ 0x69221000
#   bytes nuevos: E8 7A 26 CE 68

hook_site = 0x53E981
orig_mem  = bytes([0xE8, 0xAA, 0xFA, 0xAB, 0x67])
rel_orig  = struct.unpack_from("<i", orig_mem, 1)[0]
tgt_orig  = (hook_site + 5 + rel_orig) & 0xFFFFFFFF
print(f"Bytes en memoria antes del hook: {' '.join(f'{b:02X}' for b in orig_mem)}")
print(f"  rel32 = {rel_orig:+d}  (0x{rel_orig & 0xFFFFFFFF:08X})")
print(f"  apuntaban a: 0x{tgt_orig:08X}")

disk_bytes = bytes([0xE8, 0x5A, 0xD5, 0xFF, 0xFF])
rel_disk   = struct.unpack_from("<i", disk_bytes, 1)[0]
tgt_disk   = (hook_site + 5 + rel_disk) & 0xFFFFFFFF
print(f"\nBytes en disco:                  {' '.join(f'{b:02X}' for b in disk_bytes)}")
print(f"  rel32 = {rel_disk:+d}  apuntaban a: 0x{tgt_disk:08X}")
print(f"  CGame::Process esperado:         0x0053BEE0")
print(f"  Disco OK: {tgt_disk == 0x0053BEE0}")

new_bytes = bytes([0xE8, 0x7A, 0x26, 0xCE, 0x68])
rel_new   = struct.unpack_from("<i", new_bytes, 1)[0]
tgt_new   = (hook_site + 5 + rel_new) & 0xFFFFFFFF
print(f"\nBytes instalados por nuestro hook: {' '.join(f'{b:02X}' for b in new_bytes)}")
print(f"  rel32 = {rel_new:+d}")
print(f"  apuntan a: 0x{tgt_new:08X}")
print(f"  on_game_process reportado: 0x69221000")
print(f"  Hook OK: {tgt_new == 0x69221000}")

CGAME = 0x53BEE0
print(f"\nDiagnóstico del target original en memoria:")
print(f"  Apuntaba a 0x{tgt_orig:08X}, CGame::Process es 0x{CGAME:08X}")
if tgt_orig != CGAME:
    print(f"  → OTRO ASI patcheó ese CALL antes que nosotros.")
    print(f"    La 'trampoline' intermedia (0x{tgt_orig:08X}) era de otro mod.")
    print(f"    Nuestro hook saltó a on_game_process DIRECTAMENTE,")
    print(f"    sin pasar por esa trampoline. ¡El juego anterior no llamará a CGame::Process!")
    print(f"  ACCION REQUERIDA: guardar los bytes originales en memoria para restaurar la cadena.")
else:
    print(f"  → Apuntaba directamente a CGame::Process. Cadena correcta.")
