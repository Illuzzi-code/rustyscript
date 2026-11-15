#!/usr/bin/env python3
"""
analyze4.py – Análisis final enfocado:
  1. Las funciones pequeñas en 0x43A490-0x43A540 son PUSH model + CALL + RET
     → Son las funciones de cheat de spawn. Encontrar cuál es "VehicleCheat" (Rhino=432)
  2. Encontrar la tabla de hashes de cheats: buscar en .data
     la secuencia de dwords que apunta a las funciones 0x43A490+
  3. Confirmar AddMessageJumpQ callers (convención: push args, call, add esp,N)
  4. Buscar CPlayerInfo::money offset (para "dar dinero")
  5. Buscar CWorld::SetWorldOnFire / wanted level functions
  6. Buscar CHud::SetHelpMessage real function (not just the flag read)
"""

import sys
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

def find_callers(target_va, limit=20):
    result = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name:
            continue
        seg = data[raw_off:raw_off + raw_size]
        for i in range(len(seg) - 5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                call_va = BASE + vaddr + i
                tgt_va  = call_va + 5 + rel
                if tgt_va == target_va:
                    result.append(call_va)
                    if len(result) >= limit: return result
    return result

# ═══════════════════════════════════════════════════════════════════════════════
print("=" * 70)
print("1. Small cheat spawn functions around 0x43A490")
print("=" * 70)
# Pattern: 68 xx xx xx xx (PUSH model_id) E8 xx xx xx xx (CALL spawn_fn) 59 C3
# Already saw: 0x43A49F: 68 B0 01 00 00 (PUSH 432=Rhino) E8 06 FC FF FF CALL → ?
# Let's decode that CALL target
rhino_push_va = 0x43A49F
bs = read_va(rhino_push_va, 12)
print(f"  At 0x{rhino_push_va:08X}: {hexb(bs, 12)}")
# CALL rel @ offset 5: E8 06 FC FF FF
rel = struct.unpack_from("<i", bs, 6)[0]
tgt = rhino_push_va + 5 + 5 + rel  # instruction after push (5 bytes) + call offset
# Wait: push imm32 is 5 bytes (68 xx xx xx xx), then E8 xx xx xx xx
# So CALL is at rhino_push_va + 5
call_at = rhino_push_va + 5
bs2 = read_va(call_at, 6)
print(f"  CALL at 0x{call_at:08X}: {hexb(bs2, 6)}")
rel2 = struct.unpack_from("<i", bs2, 1)[0]
tgt2 = call_at + 5 + rel2
print(f"  → CALL target: 0x{tgt2:08X}")

# That target is the actual vehicle spawn function used by cheats
bs_spawn = read_va(tgt2, 32)
print(f"  Spawn function @ 0x{tgt2:08X}: {hexb(bs_spawn, 16)}")

# Now list all the small cheat stubs in this area
print(f"\n  Cheat stub list (68 xx xx xx xx E8 xx xx xx xx 59 C3):")
# Scan from 0x438000 to 0x43C000 for this pattern
scan_start = va_to_off(0x438000)
scan_end   = va_to_off(0x43C000)
if scan_start and scan_end:
    stubs = []
    for i in range(scan_start, scan_end - 11):
        if (data[i] == 0x68 and data[i+5] == 0xE8 and
            data[i+10] == 0x59 and data[i+11] == 0xC3):
            model = struct.unpack_from("<I", data, i+1)[0]
            rel_c = struct.unpack_from("<i", data, i+6)[0]
            fn_va = BASE + (i - va_to_off(BASE + 0x1000) + 0x1000)
            # Better: compute VA from file offset
            # fn_va calculation from file offset:
            rva = None
            for (sn, svaddr, svsize, sraw_off, sraw_size) in sections:
                if sraw_off <= i < sraw_off + sraw_size:
                    rva = svaddr + (i - sraw_off)
                    break
            if rva:
                fn_va_real = BASE + rva
                call_tgt = fn_va_real + 5 + 5 + rel_c
                stubs.append((fn_va_real, model, call_tgt))
    print(f"  Found {len(stubs)} stubs")
    for fn_va_r, model, ctgt in stubs[:30]:
        print(f"    0x{fn_va_r:08X}  model={model:4d} (0x{model:03X})  spawn_fn=0x{ctgt:08X}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("2. Cheat hash table in .data – find array pointing to stubs above")
print("=" * 70)
# We know stub VAs, so search .data for pointers to them
if stubs:
    first_stub = stubs[0][0]
    stub_vas   = set(s[0] for s in stubs)
    
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if 'data' not in sec_name.lower():
            continue
        seg = data[raw_off:raw_off + min(raw_size, vsize)]
        for i in range(0, len(seg) - 4, 4):
            dw = struct.unpack_from("<I", seg, i)[0]
            if dw in stub_vas:
                va_ref = BASE + vaddr + i
                # Found a pointer to a cheat stub. Walk back/forward to find the table.
                print(f"  Pointer to stub 0x{dw:08X} at data 0x{va_ref:08X}")
                # Print surrounding dwords
                start_i = max(0, i-32)
                ctx_seg = seg[start_i:i+48]
                print(f"  Context (dwords before/after):")
                for j in range(0, len(ctx_seg)-4, 8):
                    h = struct.unpack_from("<I", ctx_seg, j)[0]
                    p = struct.unpack_from("<I", ctx_seg, j+4)[0]
                    entry_va = BASE + vaddr + start_i + j
                    print(f"    0x{entry_va:08X}: hash=0x{h:08X}  val=0x{p:08X}")
                break
        else:
            continue
        break

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("3. CPlayerInfo::money offset")
print("=" * 70)
# In plugin-sdk for 1.0 US, CPlayerInfo is part of CWorld::Players array
# CPlayerInfo::m_nMoney is at offset 0xB8 from CPlayerInfo base
# CWorld::Players is array of CPlayerInfo (each ~0x13C bytes)
# So money VA at runtime = CWorld::Players[0] + 0xB8
# But CWorld::Players contains a pointer to CPlayerPed at offset 0
# Actually: CWorld::Players[0] IS a CPlayerInfo struct
# CPlayerInfo::m_pPed is at offset 0x0 (the first field is a pointer to the ped)
# CPlayerInfo::m_nMoney is at offset 0xB8

# Search for reference to known offset pattern for money write
# In cheats, HESOYAM sets health, money, armor. We already know the cheats
# use m_nMoney. Search for MOV [esi+0xB8], eax or similar.
PAT_MONEY = bytes([0xB8, 0x00, 0x00, 0x00])  # offset 0xB8 as dword
# This is too generic. Let's search more specifically:
# ADD [reg + 0xB8], something (money increment)
# or MOV [reg + 0xB8], imm (money set)
# C7 86 B8 00 00 00 (MOV [ESI+0xB8], imm32) or 89 86 B8 00 00 00
PAT_MOV_MONEY = bytes([0xC7, 0x86, 0xB8, 0x00, 0x00, 0x00])
PAT_MOV_MONEY2= bytes([0x89, 0x86, 0xB8, 0x00, 0x00, 0x00])
PAT_ADD_MONEY = bytes([0x01, 0x86, 0xB8, 0x00, 0x00, 0x00])  # ADD [ESI+0xB8], eax
PAT_MOV_MONEY3= bytes([0x89, 0x9E, 0xB8, 0x00, 0x00, 0x00])  # MOV [ESI+0xB8], ebx
PAT_ADD_MONEY2= bytes([0x01, 0x9E, 0xB8, 0x00, 0x00, 0x00])

for pname, pat in [("MOV [ESI+0xB8]",PAT_MOV_MONEY),("MOV [ESI+0xB8]_2",PAT_MOV_MONEY2),
                   ("ADD [ESI+0xB8]",PAT_ADD_MONEY),("MOV [ESI+0xB8]_3",PAT_MOV_MONEY3),
                   ("ADD [ESI+0xB8]_2",PAT_ADD_MONEY2)]:
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name:
            continue
        seg = data[raw_off:raw_off + raw_size]
        pos = 0
        while True:
            idx = seg.find(pat, pos)
            if idx == -1: break
            va = BASE + vaddr + idx
            ctx = seg[max(0,idx-4):idx+12]
            print(f"  {pname} @ 0x{va:08X}: {hexb(ctx)}")
            pos = idx + 1

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("4. SetHelpMessage real function – search for cdecl function near 0x58A330")
print("=" * 70)
# 0x58A330 callers: 0x58FC53
# Let's read 0x58A330 more carefully
bs = read_va(0x58A330, 48)
print(f"  0x58A330 prologue (48 bytes):")
for row in range(0, 48, 16):
    print(f"    0x{0x58A330+row:08X}: {hexb(bs[row:row+16])}")
print(f"  Callers: 0x58FC53 and others")
callers_help = find_callers(0x58A330)
print(f"  All callers ({len(callers_help)}):")
for c in callers_help[:10]:
    ctx = read_va(c, 24)
    print(f"    0x{c:08X}: {hexb(ctx, 20)}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("5. Buscar CGame::Process Idle caller function that hooks into game loop")
print("=" * 70)
# The log says: busqueda en Idle (0x53E920-0x53EC00)
# The call to CGame::Process @ 0x53E981 is within the game loop idle function
# Let's also confirm: is the function at 0x53E920 itself called by anything?
# (The hook should patch the CALL at 0x53E981 to redirect to our code)
fn_idle_start = 0x53E920  # approximate
bs_idle = read_va(fn_idle_start, 128)
print(f"  Idle function @ 0x{fn_idle_start:08X} (128 bytes):")
for row in range(0, 128, 16):
    print(f"    0x{fn_idle_start+row:08X}: {hexb(bs_idle[row:row+16])}")

# Find the exact call to 0x53BEE0 within this area
off_idle = va_to_off(0x53E920)
for i in range(100):
    if data[off_idle+i] == 0xE8:
        rel = struct.unpack_from("<i", data, off_idle+i+1)[0]
        tgt = 0x53E920 + i + 5 + rel
        if tgt == 0x53BEE0:
            print(f"\n  CALL to CGame::Process at 0x{0x53E920+i:08X} (file off 0x{off_idle+i:X})")
            # This is where the existing hook patches
            break

print("\n" + "=" * 70)
print("Análisis 4 completado.")
print("=" * 70)
