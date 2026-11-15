#!/usr/bin/env python3
"""
analyze_spawn.py – Verificar VEHICLE_CHEAT_CORE @ 0x43A0B0.
El primer byte es 0x90 (NOP) seguido de E9 (JMP rel32).
Eso indica que 0x43A0B0 no es el inicio de la función — es un trampolín.
Seguimos el JMP para encontrar la función real.
"""

import struct
from pathlib import Path

EXE  = r"C:\Users\Ericson\rustyscript\work\gta_sa.exe"
BASE = 0x400000
data = Path(EXE).read_bytes()

pe_off  = struct.unpack_from("<I", data, 0x3C)[0]
num_sec = struct.unpack_from("<H", data, pe_off + 6)[0]
opt_sz  = struct.unpack_from("<H", data, pe_off + 20)[0]
sec_off = pe_off + 24 + opt_sz
sections = []
for i in range(num_sec):
    b = sec_off + i * 40
    name     = data[b:b+8].rstrip(b'\x00').decode('ascii', errors='replace')
    vsize    = struct.unpack_from("<I", data, b+8)[0]
    vaddr    = struct.unpack_from("<I", data, b+12)[0]
    raw_size = struct.unpack_from("<I", data, b+16)[0]
    raw_off  = struct.unpack_from("<I", data, b+20)[0]
    sections.append((name, vaddr, vsize, raw_off, raw_size))

def va_to_off(va):
    rva = va - BASE
    for (n, vaddr, vsize, raw_off, raw_size) in sections:
        if vaddr <= rva < vaddr + max(vsize, raw_size):
            return raw_off + (rva - vaddr)
    return None

def read_va(va, n=32):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

def find_callers(target_va, max_results=20):
    result = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name: continue
        seg = data[raw_off:raw_off + raw_size]
        for i in range(len(seg) - 5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                if BASE + vaddr + i + 5 + rel == target_va:
                    result.append(BASE + vaddr + i)
                    if len(result) >= max_results: return result
    return result

# ── 1. Seguir el JMP en 0x43A0B0 ─────────────────────────────────────────
print("=" * 70)
print("1. VEHICLE_CHEAT_CORE @ 0x43A0B0 — seguir trampolín")
print("=" * 70)

# Bytes: 90 E9 9B D7 FC FF ...
# 90 = NOP
# E9 = JMP rel32
bs = read_va(0x43A0B0, 8)
print(f"  0x43A0B0: {hexb(bs,8)}")

off = va_to_off(0x43A0B0)
if off and data[off] == 0x90 and data[off+1] == 0xE9:
    rel = struct.unpack_from("<i", data, off+2)[0]
    real_target = 0x43A0B0 + 1 + 5 + rel  # NOP(1) + JMP rel32(5)
    print(f"  NOP; JMP rel32={rel:+d} → 0x{real_target:08X}")
    bs2 = read_va(real_target, 32)
    print(f"  Real target @ 0x{real_target:08X}: {hexb(bs2,16)}")
    print(f"                              {hexb(bs2[16:32])}")
    # Check callers of real_target
    callers = find_callers(real_target)
    print(f"  Callers of 0x{real_target:08X}: {len(callers)}")
    for c in callers[:8]: print(f"    0x{c:08X}")
    # Save for later
    REAL_SPAWN = real_target
else:
    print("  Not a NOP+JMP pattern")
    REAL_SPAWN = 0x43A0B0

# ── 2. Leer la función real de spawn ─────────────────────────────────────
print()
print("=" * 70)
print(f"2. Función real de spawn @ 0x{REAL_SPAWN:08X}")
print("=" * 70)
bs = read_va(REAL_SPAWN, 48)
for row in range(0, 48, 16):
    print(f"  0x{REAL_SPAWN+row:08X}: {hexb(bs[row:row+16])}")

# Find internal CALLs
off2 = va_to_off(REAL_SPAWN)
if off2:
    for i in range(48):
        if data[off2+i] == 0xE8:
            rel = struct.unpack_from("<i", data, off2+i+1)[0]
            tgt = REAL_SPAWN + i + 5 + rel
            print(f"  CALL @ +{i} → 0x{tgt:08X}  ({hexb(read_va(tgt,8),8)})")

# ── 3. Verificar posición offsets con lectura + test ─────────────────────
print()
print("=" * 70)
print("3. Posición del ped — confirmar acceso [ped+0x44] en función de spawn")
print("=" * 70)
# La función de spawn necesita saber dónde está el jugador para colocar
# el vehículo junto a él. Debe acceder a [ped+0x44/0x48/0x4C].
# Buscar ese patrón dentro del área 0x43xxxx
PAT_X = bytes([0x40, 0x00, 0x00, 0x00])  # offset 0x44 as dword... no
# Better: D9 40 44 (FLD [EAX+0x44]) or D9 46 44 (FLD [ESI+0x44])
# but 0x44 fits in a byte so modrm mod=01
PAT_FLD_EAX44 = bytes([0xD9, 0x40, 0x44])
PAT_FLD_ESI44 = bytes([0xD9, 0x46, 0x44])
PAT_FLD_ECX44 = bytes([0xD9, 0x41, 0x44])
PAT_FLD_EBX44 = bytes([0xD9, 0x43, 0x44])

hits = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name: continue
    seg = data[raw_off:raw_off + raw_size]
    for pat in [PAT_FLD_EAX44, PAT_FLD_ESI44, PAT_FLD_ECX44, PAT_FLD_EBX44]:
        pos = 0
        while True:
            idx = seg.find(pat, pos)
            if idx == -1: break
            va = BASE + vaddr + idx
            # Only interested in the spawn area ~ 0x430000-0x450000
            if 0x430000 <= va <= 0x450000:
                ctx = seg[idx:idx+16]
                hits.append((va, ctx))
            pos = idx + 1

print(f"  FLD [reg+0x44] in cheat zone: {len(hits)}")
for va, ctx in hits[:10]: print(f"    0x{va:08X}: {hexb(ctx,12)}")

# ── 4. Scan for the actual spawn function used by vehicle cheats ──────────
print()
print("=" * 70)
print("4. Scan: función llamada por los 16 stubs (0x43A4A0–0x43A680)")
print("=" * 70)
# All stubs call the same function. We found it's at REAL_SPAWN via JMP.
# Let's also double-check by reading the CALL in a few stubs directly:
stub_vas = [0x43A4A0, 0x43A4B0, 0x43A530, 0x43A550]
for sv in stub_vas:
    off3 = va_to_off(sv)
    if off3 and data[off3] == 0x68:  # PUSH imm32
        model = struct.unpack_from("<I", data, off3+1)[0]
        if data[off3+5] == 0xE8:    # CALL rel32
            rel = struct.unpack_from("<i", data, off3+6)[0]
            tgt = sv + 5 + 5 + rel
            print(f"  stub 0x{sv:08X} model={model}: CALL → 0x{tgt:08X}")
