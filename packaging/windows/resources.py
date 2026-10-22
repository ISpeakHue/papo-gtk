"""EN-US: Generate matching Windows resources and stable shell identity from shared metadata.
PT-BR: Gera recursos Windows e identidade estável do shell a partir de metadados compartilhados.
"""
import argparse
import json
from pathlib import Path
import re
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def generate(output, version, original):
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('Windows resources require a three-component release version')
    if original not in {'Papo.exe', 'Papo.com', 'papo-gtk.exe', 'papo-shell.exe'}:
        raise ValueError('Unknown application executable')
    output.mkdir(parents=True, exist_ok=True)
    identity = json.loads((ROOT / 'data/windows/identity.json').read_text())
    images = [(size, (ROOT / f'assets/icons/hicolor/{size}x{size}/apps/br.com.papo.gtk.png').read_bytes()) for size in [128, 256]]
    icon = struct.pack('<HHH', 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for size, data in images:
        icon += struct.pack('<BBBBHHII', size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    (output / 'papo.ico').write_bytes(icon + b''.join(data for _, data in images))
    full = version + '.0'
    # EN-US: GTK defaults to system DPI awareness; do not force its opt-in per-monitor mode.
    # PT-BR: GTK usa DPI do sistema por padrão; não força seu modo por monitor opcional.
    manifest = f'''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
 <assemblyIdentity version="{full}" processorArchitecture="amd64" name="{identity['app_id']}" type="win32"/>
 <description>Papo</description>
 <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges></security></trustInfo>
 <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application><supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/></application></compatibility>
 <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings><dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware><dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">system</dpiAwareness></windowsSettings></application>
</assembly>
'''
    (output / 'papo.manifest').write_text(manifest, encoding='utf-8')
    native = output / 'identity.h'
    native.write_text(''.join(f'#define PAPO_{key.upper()} L{json.dumps(identity[key])}\n' for key in ['app_id', 'name', 'protocol', 'toast_clsid']), encoding='ascii')
    resource = f'''#include <winver.h>
1 ICON "{(output / 'papo.ico').resolve().as_posix()}"
1 24 "{(output / 'papo.manifest').resolve().as_posix()}"
1 VERSIONINFO
FILEVERSION {full.replace('.', ',')}
PRODUCTVERSION {full.replace('.', ',')}
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904B0"
  BEGIN
   VALUE "CompanyName", "Papo Contributors\\0"
   VALUE "FileDescription", "Papo\\0"
   VALUE "FileVersion", "{full}\\0"
   VALUE "InternalName", "Papo\\0"
   VALUE "OriginalFilename", "{original}\\0"
   VALUE "ProductName", "Papo\\0"
   VALUE "ProductVersion", "{version}\\0"
   VALUE "LegalCopyright", "GPL-3.0-only; Papo Contributors\\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
'''
    (output / 'papo.rc').write_text(resource, encoding='utf-8')
    return output / 'papo.rc'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--version', required=True)
    parser.add_argument('--original', required=True)
    parser.add_argument('--windres', default='windres')
    args = parser.parse_args()
    source = generate(args.output.resolve(), args.version, args.original)
    subprocess.run([args.windres, '-i', str(source), '-o', str(source.with_suffix('.o')), '-O', 'coff'], check=True)


if __name__ == '__main__':
    main()
