#!/usr/bin/env python3
"""
analyze_natives.py – Verificar todas las funciones nativas del catálogo:
  - CPlayerPed::GetPos / SetPos (posición del jugador)
  - CPlayerInfo::m_nMoney (dinero) — confirmar offset exacto
  - CPlayerPed::SetWantedLevel
  - CWorld::GetCurrentHour / SetHour
  - CWeather::ForceWeatherNow
  - CGangElement / CCarCtrl::CreateCarForScript (spawn vehículo)
  - CWeapon::GiveWeaponToPlayer
  - CHud::SetHelpMessage (ya confirmado)
  - AddMessageJumpQ (ya confirmado)
  - CRenderer / DrawText funciones
"""

import struct, re
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
                call_va = BASE + vaddr + i
                if call_va + 5 + rel == target_va:
                    result.append(call_va)
                    if len(result) >= max_results: return result
    return result

def search_bytes(pattern, section_filter='.text', max_results=10):
    results = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if section_filter not in sec_name and section_filter.upper() not in sec_name: continue
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

def check_fn(va, expected_prologue_bytes, name):
    bs = read_va(va, 16)
    if bs is None:
        print(f"  {name} @ 0x{va:08X}: OUT OF RANGE")
        return False
    actual = list(bs[:len(expected_prologue_bytes)])
    expected = list(expected_prologue_bytes)
    ok = actual == expected
    print(f"  {name} @ 0x{va:08X}: {hexb(bs,16)}")
    print(f"    {'OK' if ok else 'MISMATCH'} (expected {hexb(bytes(expected))})")
    return ok

# ══════════════════════════════════════════════════════════════════════════════
print("=" * 70)
print("1. CPed position offsets — confirmar X/Y/Z")
print("=" * 70)
# In GTA SA, CPed inherits from CPlaceable.
# CPlaceable has a CMatrix at offset 0x14.
# The translation part (position) is at CMatrix+0x30 = CPed+0x44 (X), +0x48 (Y), +0x4C (Z)
# Actually plugin-sdk: CEntity::GetPosition returns [this+0x44] for the matrix translation.
# Let's verify: search for [reg+0x44] access patterns
# and [reg+0x48], [reg+0x4C] together

# More direct: search for the function GetPos or patterns that return XYZ near CWorld::Players
# CPlaceable::GetPosition in plugin-sdk @ 0x404460
BS_GETPOS = read_va(0x404460, 16)
print(f"  CPlaceable::GetPosition @ 0x404460: {hexb(BS_GETPOS)}")
# Expected: 8B 44 24 04 D9 40 44 ... (load arg from stack, then FLD [EAX+0x44])
# or: 55 8B EC ...

# Search for FLD [reg+0x44] pattern (reading X float)
print(f"\n  FLD [reg+0x44] patterns (X position):")
for va, ctx in search_bytes(bytes([0xD9, 0x40, 0x44]), max_results=5):
    print(f"    0x{va:08X}: {hexb(ctx)}")

# SetPos: FSTP [reg+0x44]  = D9 58 44  or D9 98 44 00 00 00
print(f"\n  FSTP [reg+0x44] patterns (write X position):")
for va, ctx in search_bytes(bytes([0xD9, 0x58, 0x44]), max_results=5):
    print(f"    0x{va:08X}: {hexb(ctx)}")

# plugin-sdk: CEntity::SetPosn @ 0x40E460
BS_SETPOS = read_va(0x40E460, 16)
print(f"\n  CEntity::SetPosn @ 0x40E460: {hexb(BS_SETPOS)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("2. CPlayerInfo::m_nMoney — confirmar offset exacto")
print("=" * 70)
# Already found MOV [ESI+0xB8], 0 at 0x510891 (reset money)
# Let's read that function to understand the struct layout
bs_money_reset = read_va(0x510880, 32)
print(f"  Near 0x510880 (money reset context): {hexb(bs_money_reset)}")

# Also check plugin-sdk value: CPlayerInfo::m_nMoney @ offset 0xB8
# The global CWorld::Players[0] is at 0xB74490 (a pointer to CPlayerInfo)
# So money address at runtime = *(0xB74490) + 0xB8
print(f"\n  Money offset from CPlayerInfo base: +0xB8")
print(f"  At runtime: *(0xB74490) + 0xB8")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("3. CWanted::SetMaximumWantedLevel / SetWantedLevel")
print("=" * 70)
# plugin-sdk: CWanted::SetWantedLevel @ 0x60FE20
# CPlayerPed::m_pWanted is at ped+0x484 (pointer to CWanted)
# CWanted::m_nWantedLevel is at CWanted+0x08
# Or direct function call: search for it

# Try @ 0x60FE20
bs = read_va(0x60FE20, 16)
print(f"  0x60FE20: {hexb(bs)}")

# Also search for the wanted-stars pattern:
# SetWantedLevel is __thiscall (ECX=CWanted*), arg: level
# CWanted* from CPlayerPed: at offset 0x484
PAT_WANTED = bytes([0x84, 0x04, 0x00, 0x00])  # offset 0x484
hits = search_bytes(PAT_WANTED, max_results=8)
print(f"\n  [reg+0x484] accesses (CWanted ptr in ped):")
for va, ctx in hits[:5]:
    print(f"    0x{va:08X}: {hexb(ctx)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("4. CClock::GetGameClockHours / SetGameClock")
print("=" * 70)
# plugin-sdk: CClock::ms_nGameClockHours @ 0xB70153 (u8 global)
# CClock::ms_nGameClockMinutes @ 0xB70152
# No function needed — write directly to globals

CLOCK_HOURS   = 0xB70153
CLOCK_MINUTES = 0xB70152
# Verify by finding references
PAT_H = struct.pack("<I", CLOCK_HOURS)
PAT_M = struct.pack("<I", CLOCK_MINUTES)
hits_h = search_bytes(PAT_H, max_results=5)
hits_m = search_bytes(PAT_M, max_results=5)
print(f"  CClock::ms_nGameClockHours @ 0xB70153: {len(hits_h)} xrefs")
for va, ctx in hits_h[:3]: print(f"    0x{va:08X}: {hexb(ctx)}")
print(f"  CClock::ms_nGameClockMinutes @ 0xB70152: {len(hits_m)} xrefs")
for va, ctx in hits_m[:3]: print(f"    0x{va:08X}: {hexb(ctx)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("5. CWeather::ForceWeatherNow")
print("=" * 70)
# plugin-sdk: CWeather::ForceWeatherNow @ 0x578EA0
bs = read_va(0x578EA0, 16)
print(f"  0x578EA0: {hexb(bs)}")
callers = find_callers(0x578EA0)
print(f"  Callers: {len(callers)}")
for c in callers[:5]: print(f"    0x{c:08X}")

# Also: CWeather::OldWeatherType @ 0xC81320 (u16)
#       CWeather::NewWeatherType @ 0xC81324 (u16)  
PAT_WEATHER = struct.pack("<I", 0xC81320)
hits_w = search_bytes(PAT_WEATHER, max_results=5)
print(f"\n  CWeather globals @ 0xC81320: {len(hits_w)} xrefs")
for va, ctx in hits_w[:3]: print(f"    0x{va:08X}: {hexb(ctx)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("6. CCarCtrl::CreateCarForScript — spawn vehículo por script")
print("=" * 70)
# plugin-sdk: CCarCtrl::CreateCarForScript @ 0x431F80
# args: (model_id: i32, x: f32, y: f32, z: f32, find_ground: bool) -> *mut CVehicle
bs = read_va(0x431F80, 16)
print(f"  0x431F80: {hexb(bs)}")
callers = find_callers(0x431F80, max_results=5)
print(f"  Callers: {len(callers)}")
for c in callers[:5]: print(f"    0x{c:08X}")

# VEHICLE_CHEAT_CORE @ 0x43A0B0 — confirm it calls CreateCarForScript
bs_core = read_va(0x43A0B0, 48)
print(f"\n  VEHICLE_CHEAT_CORE @ 0x43A0B0 (48b): {hexb(bs_core[:16])}")
print(f"                                       {hexb(bs_core[16:32])}")
print(f"                                       {hexb(bs_core[32:48])}")
# Find CALL targets within
off = va_to_off(0x43A0B0)
if off:
    for i in range(48):
        if data[off+i] == 0xE8:
            rel = struct.unpack_from("<i", data, off+i+1)[0]
            tgt = 0x43A0B0 + i + 5 + rel
            print(f"    CALL at 0x{0x43A0B0+i:08X} → 0x{tgt:08X}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("7. CWeapon::AddWeaponForScriptActor / GiveWeaponToPlayer")
print("=" * 70)
# plugin-sdk: CWorld::GiveWeaponToPlayer @ multiple, or
# CPed::AddWeaponModel: 0x4474A0 or similar
# Let's check: CWeaponInfo is in a table. 
# ScriptCommand: give weapon to actor uses a function near 0x47A480 or similar
# More reliable: search for the function that processes GIVE_WEAPON_TO_CHAR opcode
# For now let's try plugin-sdk value: CPed::AddWeapon @ 0x4474A0
bs = read_va(0x4474A0, 16)
print(f"  CPed::AddWeapon @ 0x4474A0: {hexb(bs)}")
callers_w = find_callers(0x4474A0, max_results=5)
print(f"  Callers: {len(callers_w)}")
for c in callers_w[:5]: print(f"    0x{c:08X}")

# Also try 0x47A480 (CRunningScript weapon give)
bs2 = read_va(0x47A480, 16)
print(f"\n  0x47A480: {hexb(bs2)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("8. SetHelpMessage @ 0x588BE0 — confirmar convención")
print("=" * 70)
check_fn(0x588BE0, bytes([0xA0, 0xC0, 0xAE, 0xBA, 0x00, 0x84, 0xC0, 0x0F]), "CHud::SetHelpMessage")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("9. CPed::GetCurrentRoom / CGame::currArea")
print("=" * 70)
# CGame::currArea @ 0x96C0D8 (u8): interior ID
PAT_AREA = struct.pack("<I", 0x96C0D8)
hits_area = search_bytes(PAT_AREA, max_results=5)
print(f"  CGame::currArea @ 0x96C0D8: {len(hits_area)} xrefs")
for va, ctx in hits_area[:3]: print(f"    0x{va:08X}: {hexb(ctx)}")

# ══════════════════════════════════════════════════════════════════════════════
print()
print("=" * 70)
print("10. Summary of confirmed vs pending natives")
print("=" * 70)
confirmed = [
    ("CGame::Process",        "0x53BEE0", "ALTA",  "hook"),
    ("AddMessageJumpQ",       "0x69F1E0", "ALTA",  "xrefs+callers"),
    ("SetHelpMessage",        "0x588BE0", "ALTA",  "xrefs"),
    ("CWorld::Players",       "0xB74490", "ALTA",  "in-game confirmed"),
    ("ped.health  +0x540",    "",         "ALTA",  "in-game confirmed 23.6→100.1"),
    ("ped.maxhealth +0x544",  "",         "ALTA",  "in-game confirmed 100.1"),
    ("ped.armor  +0x548",     "",         "ALTA",  "in-game confirmed 0.0→100.0"),
    ("CheatSpawnRhino",       "0x43A4A0", "ALTA",  "tabla cheats + stub pattern"),
    ("VehicleCheatCore",      "0x43A0B0", "MEDIA", "pending in-game"),
]
print(f"  {'Nativo':<30} {'VA':<12} {'Confianza':<8} {'Evidencia'}")
print(f"  {'-'*30} {'-'*12} {'-'*8} {'-'*30}")
for n, va, conf, ev in confirmed:
    print(f"  {n:<30} {va:<12} {conf:<8} {ev}")
