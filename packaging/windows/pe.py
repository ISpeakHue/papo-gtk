"""EN-US: Inspect AMD64 imports without executing DLLs or depending on MSYS ldd.
PT-BR: Inspeciona imports AMD64 sem executar DLLs nem depender do ldd do MSYS.
"""
from pathlib import Path
import re
import struct

# EN-US: These are OS components, not redistributable GPU/vendor DLLs.
# PT-BR: Estes são componentes do sistema, não DLLs redistribuíveis de GPU/fabricantes.
SYSTEM_DLLS = set(('''advapi32 avrt bcrypt bcryptprimitives cabinet cfgmgr32 comctl32 comdlg32
    crypt32 cryptbase cryptnet cryptui d2d1 d3d9 d3d11 d3d12 d3dcompiler_47 dcomp dbghelp
    dhcpcsvc dnsapi dsound dwmapi dwrite dxgi dxva2 evr fwpuclnt gdi32 gdiplus glu32 hid imagehlp
    imm32 iphlpapi kernel32 ksuser mf mfplat mfreadwrite mfuuid mmdevapi mpr msacm32 msimg32
    msvcrt msvfw32 ncrypt netapi32 normaliz nsi ntdll ole32 oleacc oleaut32 opengl32 powrprof
    propsys psapi rasapi32 rpcrt4 secur32 setupapi shcore shell32 shlwapi strmiids ucrtbase
    user32 userenv usp10 uxtheme version vfw32 wevtapi windowscodecs winhttp wininet winmm
    winscard wintrust winusb wlanapi wldap32 ws2_32 wsock32 wtsapi32 wmvcore rtworkq msdmo
    dinput8 xinput1_4 resampledmo hnetcfg activeds adsiid ktmw32 urlmon win32u
    sensorsapi mfcore mfcaptureengine avicap32 oledlg authz tdh pdh''').split())
SYSTEM_DLLS = {name + '.dll' for name in SYSTEM_DLLS} | {'winspool.drv'}


def is_system(name):
    name = name.lower()
    return name in SYSTEM_DLLS or name.startswith(('api-ms-win-', 'ext-ms-win-'))


def imports(path):
    data = Path(path).read_bytes()

    def unpack(fmt, offset):
        size = struct.calcsize(fmt)
        if offset < 0 or offset + size > len(data):
            raise ValueError(f'Truncated PE: {path}')
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b'MZ':
        raise ValueError(f'Not a PE image: {path}')
    pe = unpack('<I', 0x3c)[0]
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError(f'Invalid PE signature: {path}')
    machine, sections, _, _, _, optional_size, _ = unpack('<HHIIIHH', pe + 4)
    optional = pe + 24
    if machine != 0x8664 or unpack('<H', optional)[0] != 0x20b:
        raise ValueError(f'Expected native AMD64 PE32+: {path}')
    image_base = unpack('<Q', optional + 24)[0]
    section_table = optional + optional_size
    regions = [unpack('<IIII', section_table + i * 40 + 8) for i in range(sections)]

    def offset(rva):
        for virtual_size, base, raw_size, raw in regions:
            if base <= rva < base + max(virtual_size, raw_size) and rva - base < raw_size:
                return raw + rva - base
        raise ValueError(f'Unmapped PE RVA {rva:x}: {path}')

    def dll_name(rva):
        start = offset(rva)
        end = data.find(b'\0', start, min(start + 512, len(data)))
        if end == -1:
            raise ValueError(f'Unterminated import name: {path}')
        name = data[start:end].decode('ascii')
        if not re.fullmatch(r'[A-Za-z0-9_.+-]+\.(dll|drv)', name, re.I):
            raise ValueError(f'Unsafe import {name!r}: {path}')
        return name.lower()

    found = set()
    directories = unpack('<I', optional + 108)[0]
    for index, entry_size, name_index in ((1, 20, 3), (13, 32, 1)):
        if directories <= index or 112 + 8 * (index + 1) > optional_size:
            continue
        rva, size = unpack('<II', optional + 112 + 8 * index)
        if not rva:
            continue
        base = offset(rva)
        for position in range(0, min(size, 1024 * 1024), entry_size):
            entry = unpack('<' + 'I' * (entry_size // 4), base + position)
            if not any(entry):
                break
            name_rva = entry[name_index]
            if index == 13 and not entry[0] & 1:
                name_rva -= image_base
            found.add(dll_name(name_rva))
        else:
            raise ValueError(f'Unterminated import table: {path}')
    return sorted(found)
