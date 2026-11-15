#!/usr/bin/env python3
"""
analyze5.py – Investigar la estructura real de CWorld::Players.
Pregunta: ¿qué hay en 0xB74490?
  (a) ¿Es un puntero a CPlayerInfo struct? → Players[0] = *(0xB74490)
  (b) ¿Es el inicio de CPlayerInfo inline? → Players[0] está en 0xB74490

Para responder, leer el código que accede a 0xB74490 y ver qué hace
con el valor leído.
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

def read_va(va, n=64):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

PLAYERS = 0xB74490
PAT = struct.pack("<I", PLAYERS)

# ── Analizar los primeros 20 xrefs a 0xB74490 ───────────────────────────────
print("=" * 70)
print("Accesos a CWorld::Players (0xB74490) — primeros 25 xrefs")
print("=" * 70)

xrefs = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT, pos)
        if idx == -1: break
        va = BASE + vaddr + idx
        # read the full instruction: typically the pattern appears at offset 1 or 2
        # of the instruction. Read 12 bytes before to get the full context.
        ctx_start = max(0, idx - 4)
        ctx = seg[ctx_start : idx + 8]
        xrefs.append((va - 4, ctx))  # rough instruction start
        pos = idx + 1

print(f"Total xrefs: {len(xrefs)}")
print()

# Show the first 25, focusing on how the value is USED after loading
for i, (va, ctx) in enumerate(xrefs[:25]):
    print(f"  [{i:2d}] near 0x{va:08X}: {hexb(ctx)}")
    # Decode what instruction loads from 0xB74490
    # Common patterns:
    # A1 90 44 B7 00       MOV EAX, [0xB74490]   → EAX = *(0xB74490) = pointer to ped
    # 8B 3D 90 44 B7 00    MOV EDI, [0xB74490]   → pointer
    # 8B 2D 90 44 B7 00    MOV EBP, [0xB74490]   → pointer
    # 8B 0D 90 44 B7 00    MOV ECX, [0xB74490]   → pointer
    # C7 05 90 44 B7 00    MOV [0xB74490], imm32  → write (initialization)
    # Identify opcode
    raw = ctx
    if len(raw) >= 5:
        b0 = raw[0] if va % 1 == 0 else 0
        # Try to find the actual instruction by looking at what byte precedes 90 44 B7 00
        # The pattern 90 44 B7 00 starts at offset 4 in ctx (ctx_start = va-4+4 = va)
        # Actually ctx[4:8] should be 90 44 B7 00
        # So the instruction opcode(s) are in ctx[0:4]
        # Most common: ctx[2] = modrm, ctx[0:2] = opcode
        for j in range(len(raw)-4):
            if raw[j:j+4] == bytes([0x90, 0x44, 0xB7, 0x00]):
                instr_start = j - 1  # ModRM byte before the address
                if instr_start >= 1:
                    opc = raw[instr_start-1]
                    modrm = raw[instr_start]
                    if opc == 0xA1:
                        print(f"       → A1: MOV EAX, [0xB74490]  (EAX = value at players)")
                    elif opc == 0x8B:
                        reg = (modrm >> 3) & 7
                        regs = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                        print(f"       → 8B {modrm:02X}: MOV {regs[reg]}, [0xB74490]")
                    elif opc == 0x03:
                        reg = (modrm >> 3) & 7
                        regs = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                        print(f"       → 03 {modrm:02X}: ADD {regs[reg]}, [0xB74490]")
                    elif raw[instr_start-1:instr_start+1] == bytes([0xC7, 0x05]):
                        print(f"       → C7 05: MOV [0xB74490], imm32  (WRITE/INIT)")
                break

print()

# ── Focus on the most common pattern: A1 (MOV EAX) ─────────────────────────
print("=" * 70)
print("Instrucciones A1 (MOV EAX, [0xB74490]) — ¿qué hace el código después?")
print("=" * 70)

PAT_A1 = bytes([0xA1, 0x90, 0x44, 0xB7, 0x00])  # MOV EAX, [0xB74490]
count = 0
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_A1, pos)
        if idx == -1: break
        va = BASE + vaddr + idx
        # Read 24 bytes of context AFTER the A1 instruction
        after = seg[idx+5 : idx+5+32]
        print(f"\n  MOV EAX,[0xB74490] @ 0x{va:08X}")
        print(f"  After: {hexb(after, 24)}")
        # Decode next instruction
        if after[0] == 0x85:  # TEST
            print(f"         85 {after[1]:02X}: TEST reg,reg  → checking if pointer is null")
        elif after[0] == 0x8B:  # MOV
            modrm = after[1]
            mod = (modrm >> 6) & 3
            reg = (modrm >> 3) & 7
            rm  = modrm & 7
            if mod == 1:  # [reg + disp8]
                disp = after[2]
                regs = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                r2   = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                print(f"         8B {modrm:02X} {disp:02X}: MOV {regs[reg]}, [{r2[rm]}+0x{disp:02X}]")
                print(f"         → reads [EAX+0x{disp:02X}] = reads field at offset 0x{disp:02X} of *players")
            elif mod == 2:  # [reg + disp32]
                disp = struct.unpack_from("<I", after, 2)[0]
                regs = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                r2   = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                print(f"         8B {modrm:02X}: MOV {regs[reg]}, [{r2[rm]}+0x{disp:08X}]")
                print(f"         → reads [EAX+0x{disp:08X}] = reads field at offset 0x{disp:08X}")
            elif mod == 0 and rm == 0:  # [EAX]
                print(f"         8B 00: MOV EAX, [EAX]  → double dereference: EAX = *(*(0xB74490))")
        count += 1
        if count >= 15: break
        pos = idx + 1
    if count >= 15: break

# ── Compare with plugin-sdk documentation ────────────────────────────────────
print()
print("=" * 70)
print("Búsqueda: ¿es 0xB74490 un puntero o el inicio de la struct inline?")
print("=" * 70)
# If code does: MOV EAX, [0xB74490]; MOV ECX, [EAX + N] → it's a pointer TO CPlayerInfo
# If code does: MOV EAX, [0xB74490]; MOV ECX, [EAX]     → EAX itself is the ped pointer
# If code does: MOV EAX, 0xB74490; MOV ECX, [EAX + N]   → it's inline struct (LEA pattern)

# Search for LEA EAX, [0xB74490] or similar
PAT_LEA = bytes([0xB8, 0x90, 0x44, 0xB7, 0x00])  # MOV EAX, 0xB74490 (imm, not mem)
hits_lea = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_LEA, pos)
        if idx == -1: break
        va = BASE + vaddr + idx
        after = seg[idx+5:idx+5+16]
        hits_lea.append((va, after))
        pos = idx + 1

print(f"\n  MOV EAX, 0xB74490 (treat as address): {len(hits_lea)} hits")
for va, after in hits_lea[:5]:
    print(f"    0x{va:08X}: after={hexb(after,12)}")

# Also search for direct offset access like [0xB74490 + offset]
# e.g. MOV EAX, [0xB744A8] where 0xB744A8 = 0xB74490 + 0x18
# This would indicate Players IS a struct at that address
print(f"\n  Acceso a 0xB74490+offset (struct inline):")
for off_test in [0x08, 0x0C, 0x10, 0x14, 0x18, 0x20, 0xB8]:
    tgt = PLAYERS + off_test
    pat = struct.pack("<I", tgt)
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name:
            continue
        seg = data[raw_off:raw_off + raw_size]
        idx = seg.find(pat)
        if idx != -1:
            va = BASE + vaddr + idx
            ctx = seg[max(0,idx-2):idx+8]
            print(f"    [0xB74490+0x{off_test:02X}] = 0x{tgt:08X} @ 0x{va:08X}: {hexb(ctx)}")
            break

print()
print("=" * 70)
print("Análisis 5 completado.")
print("=" * 70)
