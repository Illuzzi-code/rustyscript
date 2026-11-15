#!/usr/bin/env python3
"""
analyze_natives2.py – Verificar las direcciones restantes más importantes:
  - CPlaceable::GetPosition (0x404460) — solo LEA, no es una función de lectura útil
  - CEntity::SetPosn (0x40E460) — 83 3D... parece que ya tiene otro inicio
  - CClock globals (OK — escritura directa)
  - CWeather::ForceWeatherNow (0x578EA0) — 0 callers, buscar la real
  - CPed::AddWeapon (0x4474A0) — 0 callers, buscar la real
  - SetWantedLevel function
  - CPlayerPed get/set coordinates
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

def search_bytes(pattern, max_results=10):
    results = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name: continue
        seg = data[raw_off:raw_off + raw_size]
        pos = 0
        while True:
            idx = seg.find(pattern, pos)
            if idx == -1: break
            va = BASE + vaddr + idx
            ctx = seg[max(0,idx-4):idx+16]
            results.append((va, ctx))
            if len(results) >= max_results: return results
            pos = idx + 1
    return results

# ══════════════════════════════════════════════════════════════════════════════
print("=" * 70)
print("1. CPlaceable::GetPosition — 0x404460 lea eax,[ecx+44] ret")
print("=" * 70)
# 8D 41 44 C3 = LEA EAX,[ECX+0x44]; RET
# This is a 1-liner that returns a pointer to the position in the matrix.
# Perfect for use: call with ped ptr in ECX, returns ptr to CVector (X,Y,Z floats)
# Then read [ptr+0], [ptr+4], [ptr+8]
bs = read_va(0x404460, 8)
print(f"  0x404460: {hexb(bs,8)}")
print(f"  Decoded: 8D 41 44 = LEA EAX,[ECX+0x44]; C3 = RET")
print(f"  → returns ptr to CVector at ped+0x44")
print(f"  → X = *(ped+0x44), Y = *(ped+0x48), Z = *(ped+0x4C)")
callers = find_callers(0x404460)
print(f"  Callers: {len(callers)}")
for c in callers[:5]: print(f"    0x{c:08X}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("2. CEntity::SetPosn — verify 0x40E460 (shows 83 3D... = CMP [mem], imm)")
print("=" * 70)
bs = read_va(0x40E460, 32)
print(f"  0x40E460: {hexb(bs,16)}")
print(f"           {hexb(bs[16:32])}")
# 83 3D 78 4B 8E 00 02 = CMP [0x8E4B78], 2
# This is NOT a typical function prologue. It's jumping into the middle or
# it's a very unusual pattern. Check callers.
callers = find_callers(0x40E460)
print(f"  Callers: {len(callers)}")
for c in callers[:5]: print(f"    0x{c:08X}")

# Try nearby: 0x40E450 or 0x40E480
for try_va in [0x40E440, 0x40E450, 0x40E480, 0x40E490]:
    bs2 = read_va(try_va, 8)
    print(f"  0x{try_va:08X}: {hexb(bs2,8)}")

# Search for a function that writes CVector at ped+0x44
# Pattern: FSTP [reg+0x44] where reg comes from [0xB74490]
# From analyze_natives: 0x4A50C1: 8B 4C 24 04 D9 58 44 ...
bs3 = read_va(0x4A50C0, 32)
print(f"\n  Near 0x4A50C0 (FSTP [ECX+0x44] context): {hexb(bs3)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("3. CWeather::ForceWeatherNow — find the real function")
print("=" * 70)
# 0x578EA0 had 0 callers. Let's read it more carefully.
bs = read_va(0x578EA0, 32)
print(f"  0x578EA0: {hexb(bs,16)}")
print(f"           {hexb(bs[16:32])}")
# CE = INTO? No, this is likely in the middle of another function.
# Try 0x578F20, 0x578E80 etc. OR search for ForceWeatherNow differently.
# Actually CWeather has: ms_OldWeatherType, ms_NewWeatherType globals
# ForceWeatherNow: sets both to weather_id.
# Search for a function that writes to both 0xC81320 and 0xC81324

PAT_OLD = struct.pack("<I", 0xC81320)
PAT_NEW = struct.pack("<I", 0xC81324)

hits_combined = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name: continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_OLD, pos)
        if idx == -1: break
        # Check if PAT_NEW also appears within 50 bytes
        window = seg[idx:idx+80]
        if PAT_NEW in window:
            va = BASE + vaddr + idx
            ctx = seg[max(0,idx-8):idx+30]
            hits_combined.append((va, ctx))
        pos = idx + 1

print(f"\n  Functions writing both 0xC81320 and 0xC81324: {len(hits_combined)}")
for va, ctx in hits_combined[:5]:
    print(f"  0x{va:08X}: {hexb(ctx)}")
    # Walk back to find function start
    off = va_to_off(va)
    if off:
        for back in range(1, 300):
            test = off - back
            if test < 1: break
            bp = data[test-1]
            b0 = data[test]
            b1 = data[test+1] if test+1 < len(data) else 0
            if b0 in (0x55, 0x56, 0x57, 0x53) and bp in (0xCC, 0x90, 0xC3, 0xC2):
                fn_va = va - back
                print(f"    → fn start: 0x{fn_va:08X}")
                bs_fn = data[test:test+16]
                print(f"       {hexb(bs_fn)}")
                callers_fn = find_callers(fn_va, max_results=5)
                print(f"       callers: {len(callers_fn)}")
                for c in callers_fn[:3]: print(f"         0x{c:08X}")
                break
            if b0 == 0x83 and b1 == 0xEC and bp in (0xCC, 0x90, 0xC3, 0xC2):
                fn_va = va - back
                print(f"    → fn start (sub esp): 0x{fn_va:08X}")
                bs_fn = data[test:test+16]
                print(f"       {hexb(bs_fn)}")
                callers_fn = find_callers(fn_va, max_results=5)
                print(f"       callers: {len(callers_fn)}")
                for c in callers_fn[:3]: print(f"         0x{c:08X}")
                break

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("4. CPed::AddWeapon — find real address (0x4474A0 had 0 callers)")
print("=" * 70)
# 0x4474A0: 00 89 81 50 C0 96 00 — this is in the middle of code!
# Search for the real CPed::AddWeapon by looking for SetCurrentWeapon patterns
# In GTA SA, giving a weapon to player involves:
# - Loading ammo
# - Setting weapon slot
# Common pattern: push ammo_count, push weapon_id, call CPed::GiveWeapon

# Try nearby addresses
for try_va in [0x4474A0, 0x447530, 0x447600, 0x447900, 0x4479E0]:
    bs2 = read_va(try_va, 16)
    c = find_callers(try_va, max_results=3)
    print(f"  0x{try_va:08X}: {hexb(bs2,12)}  callers={len(c)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("5. CWanted::SetWantedLevel — find function near ped+0x484")
print("=" * 70)
# ped+0x484 = pointer to CWanted
# CWanted::m_nWantedLevel is at CWanted+0x08 (u32)
# Or: search for a function that takes (ped*, level) and sets wanted
# In plugin-sdk: CWanted::SetWantedLevel @ 0x60FE20 had weird bytes
# Let's search for functions near that range

for try_va in [0x60FE20, 0x60FE00, 0x60FDE0, 0x60FEC0, 0x60FF00]:
    bs2 = read_va(try_va, 16)
    c = find_callers(try_va, max_results=3)
    print(f"  0x{try_va:08X}: {hexb(bs2,12)}  callers={len(c)}")

# A simpler approach: write directly to [ped+0x484]+0x08
# But first confirm the offset. Search for [reg+0x484] access
PAT_484 = bytes([0x84, 0x04, 0x00, 0x00])
hits = search_bytes(PAT_484)
print(f"\n  [reg+0x484] patterns:")
for va, ctx in hits[:5]: print(f"    0x{va:08X}: {hexb(ctx)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("6. CClock globals confirmed — read and write pattern")
print("=" * 70)
# From xrefs found: 0x439527 writes to both 0xB70153 and 0xB70152
# Let's read that code
bs = read_va(0x439520, 32)
print(f"  0x439520: {hexb(bs,16)}")
print(f"           {hexb(bs[16:32])}")
# 74 13 88 0D 53 01 B7 00 88 0D 52 01 B7 00 = JZ +0x13; MOV [0xB70153], CL; MOV [0xB70152], CL
# Simple: just write bytes to these addresses directly
print(f"\n  To set time: write u8 to 0xB70153 (hours) and 0xB70152 (minutes)")
print(f"  Range: hours 0-23, minutes 0-59")
# Verify the read pattern
bs = read_va(0x4410C1, 16)
print(f"\n  0x4410C1 (read hours): {hexb(bs,12)}")
# A0 52 01 B7 00 = MOV AL, [0xB70152]  (read minutes)

print()
print("=" * 70)
print("Análisis 2 completado.")
print("=" * 70)
