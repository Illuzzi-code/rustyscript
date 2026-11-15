#!/usr/bin/env python3
"""Analyze the SetHelpMessage function at 0x588BE0 in the GTA SA binary."""
import struct
import sys

exe_path = r'c:\Users\Ericson\rustyscript\work\gta_sa.exe'

with open(exe_path, 'rb') as f:
    data = f.read()

# Parse PE headers
pe_offset = struct.unpack_from('<I', data, 0x3C)[0]
num_sections = struct.unpack_from('<H', data, pe_offset + 6)[0]
opt_hdr_size = struct.unpack_from('<H', data, pe_offset + 20)[0]
sections_offset = pe_offset + 24 + opt_hdr_size

print(f'PE offset: 0x{pe_offset:X}')
print(f'Number of sections: {num_sections}')
print(f'Sections:')

sections = []
for i in range(num_sections):
    off = sections_offset + i * 40
    name = data[off:off+8].rstrip(b'\x00').decode('ascii', errors='replace')
    vsize = struct.unpack_from('<I', data, off + 8)[0]
    vaddr = struct.unpack_from('<I', data, off + 12)[0]
    raw_size = struct.unpack_from('<I', data, off + 16)[0]
    raw_offset = struct.unpack_from('<I', data, off + 20)[0]
    sections.append((name, vaddr, vsize, raw_offset, raw_size))
    print(f'  {name:8s} VA=0x{vaddr:08X} VSize=0x{vsize:08X} RawOff=0x{raw_offset:08X} RawSize=0x{raw_size:08X}')

# Helper: convert VA to file offset
def va_to_offset(va):
    image_base = 0x400000
    rva = va - image_base
    for name, vaddr, vsize, raw_offset, raw_size in sections:
        if vaddr <= rva < vaddr + vsize:
            return raw_offset + (rva - vaddr)
    return None

# Examine SetHelpMessage at 0x588BE0
va = 0x588BE0
file_offset = va_to_offset(va)
if file_offset is None:
    print(f'ERROR: Could not map VA 0x{va:08X}')
    sys.exit(1)

print(f'\nVA 0x{va:08X} -> file offset 0x{file_offset:08X}')

# Read 128 bytes of the function
func_bytes = data[file_offset:file_offset+128]
print(f'Bytes: {" ".join(f"{b:02X}" for b in func_bytes)}')

# Detailed disassembly of first instructions
print(f'\n--- Disassembly of first bytes ---')
print(f'A0 C0 AE BA 00     MOV AL, [0x00BAAEC0]   ; HUD state guard')
print(f'84 C0              TEST AL, AL')
print(f'0F 85 4F 01 00 00 JNE  +0x14F            ; skip while unavailable')

# Check what 0x96A7CC is (the global loaded first)
global_va = 0x96A7CC
global_off = va_to_offset(global_va)
print(f'\nGlobal at 0x{global_va:08X}:')
if global_off is not None and global_off < len(data):
    gval = struct.unpack_from('<I', data, global_off)[0]
    print(f'  File offset 0x{global_off:08X}, initial value: 0x{gval:08X} ({gval})')
else:
    print(f'  In BSS/uninitialized section (file offset not mapped)')

# Also check CMessages::AddMessageJumpQ at 0x69F1E0
va2 = 0x69F1E0
off2 = va_to_offset(va2)
if off2 is not None:
    msg_bytes = data[off2:off2+32]
    print(f'\nAddMessageJumpQ @ 0x{va2:08X} (file offset 0x{off2:08X}):')
    print(f'Bytes: {" ".join(f"{b:02X}" for b in msg_bytes)}')

# Check the calling convention of SetHelpMessage more carefully
# Look for how arguments are accessed (ESP+offset = arguments on stack)
print(f'\n--- Looking for stack argument access patterns ---')
# After SUB ESP,0x34, arguments would be at ESP+0x34+4(ret addr)+0..N
# First arg (text ptr) at ESP+0x38, second at ESP+0x3C, third at ESP+0x40, fourth at ESP+0x44
# But with optimization, the compiler might use different offsets

# Let's also check who calls 0x58A330 (search for E8 rel32 calls)
print(f'\n--- Searching for calls to SetHelpMessage (0x588BE0) ---')
call_sites = []
for i in range(len(data) - 5):
    if data[i] == 0xE8:
        rel32 = struct.unpack_from('<i', data, i + 1)[0]
        # The call target = VA_of_next_instruction + rel32
        # VA_of_next_instruction = file_offset_to_VA(i + 5)
        # We need to convert file offset i to VA
        for name, vaddr, vsize, raw_offset, raw_size in sections:
            if raw_offset <= i < raw_offset + raw_size:
                call_va = vaddr + 0x400000 + (i - raw_offset)
                target_va = call_va + 5 + rel32
                if target_va == 0x588BE0:
                    call_sites.append(call_va)
                break

print(f'Found {len(call_sites)} call sites to 0x588BE0:')
for cs in call_sites[:20]:
    print(f'  0x{cs:08X}')
