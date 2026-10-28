"""EN-US: Validate icon/manifest identity and exercise the native WinRT ABI without registration.
PT-BR: Valida identidade de ícone/manifesto e testa ABI WinRT sem registrar o aplicativo.
"""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'packaging/windows'))
from resources import generate
from pe import imports, is_system


class WindowsResources(unittest.TestCase):
    def test_gtk_process_leaves_dpi_selection_to_gdk(self):
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)
            generate(out,'1.0.2','papo-gtk.exe')
            manifest=ET.parse(out/'papo.manifest')
            self.assertIsNone(manifest.find('.//{http://schemas.microsoft.com/SMI/2016/WindowsSettings}dpiAwareness'))
            self.assertIsNone(manifest.find('.//{http://schemas.microsoft.com/SMI/2005/WindowsSettings}dpiAware'))
            self.assertEqual(manifest.find('.//{http://schemas.microsoft.com/SMI/2016/WindowsSettings}longPathAware').text,'true')

    def test_resources_match_version_icon_and_gtk_dpi_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            out=Path(tmp); rc=generate(out,'1.0.2','Papo.exe').read_text()
            self.assertIn('FILEVERSION 1,0,2,0',rc)
            self.assertIn('"OriginalFilename", "Papo.exe',rc)
            manifest=ET.parse(out/'papo.manifest')
            self.assertEqual(manifest.find('.//{urn:schemas-microsoft-com:asm.v3}requestedExecutionLevel').get('level'),'asInvoker')
            self.assertEqual(manifest.find('.//{http://schemas.microsoft.com/SMI/2016/WindowsSettings}dpiAwareness').text,'system')
            self.assertEqual(manifest.find('.//{http://schemas.microsoft.com/SMI/2016/WindowsSettings}longPathAware').text,'true')
            icon=(out/'papo.ico').read_bytes();self.assertEqual(struct.unpack_from('<HHH',icon),(0,1,2))
            for index,size in enumerate([128,256]):
                width,height,_,_,_,_,length,offset=struct.unpack_from('<BBBBHHII',icon,6+16*index)
                self.assertEqual((width,height),(size%256,size%256));self.assertEqual(icon[offset:offset+length],(ROOT/f'assets/icons/hicolor/{size}x{size}/apps/br.com.papo.gtk.png').read_bytes())
            for bad in ['1.2','1.2.3-rc1','1.2.3;command']:
                with self.assertRaises(ValueError):generate(out,bad,'Papo.exe')

    def test_identity_is_version_independent_and_matches_shared_application(self):
        identity=json.loads((ROOT/'data/windows/identity.json').read_text())
        self.assertEqual(identity['app_id'],'br.com.papo.gtk')
        self.assertIn('"'+identity['app_id']+'"',(ROOT/'src/app/branding.rs').read_text())
        self.assertIn(identity['protocol']+'://open/',(ROOT/'src/platform/activation.rs').read_text())
        self.assertRegex(identity['toast_clsid'],r'^\{[0-9A-F-]{36}\}$')


@unittest.skipUnless(os.name=='nt' and shutil.which('g++'),'Native Windows compiler required')
class WindowsShell(unittest.TestCase):
    def test_native_resource_and_winrt_prototype_without_registry_changes(self):
        with tempfile.TemporaryDirectory(prefix='Papo-identity-') as tmp:
            out=Path(tmp)
            runtime=out/'runtime/bin';runtime.mkdir(parents=True)
            rc=generate(out,'1.0.2','papo-shell.exe')
            subprocess.run(['windres','-i',str(rc),'-o',str(out/'papo.o'),'-O','coff'],check=True)
            binary=runtime/'papo-shell.exe'
            subprocess.run(['g++','-std=c++17','-Os','-static','-municode','-Wall','-Wextra','-Werror','-I',str(out),str(ROOT/'packaging/windows/shell.cpp'),str(out/'papo.o'),'-lole32','-lshell32','-lpropsys','-lshlwapi','-lruntimeobject','-luuid','-o',str(binary)],check=True)
            self.assertTrue(all(is_system(name) for name in imports(binary)))
            result=subprocess.run([str(binary),'--self-check'],check=True,capture_output=True,text=True,timeout=30)
            self.assertEqual(json.loads(result.stdout),{'winrt_abi':True})
            ticket='70b0d588-40ac-4314-86bf-130ad1c9b2ee'
            payload=f'<toast activationType="protocol" launch="papo-notification://open/{ticket}"><visual><binding template="ToastGeneric"><text>Papo</text><text>&lt;prévia &amp; teste&gt;</text></binding></visual><audio silent="true"/></toast>'
            result=subprocess.run([str(binary),'--check-notification',ticket,'0123456789abcdef'],input=payload,check=True,capture_output=True,text=True,encoding='utf-8',timeout=30)
            self.assertEqual(json.loads(result.stdout),{'winrt_toast':True})
            for invalid in [payload.replace(ticket,'00000000-0000-0000-0000-000000000000'),'x'*65537,'<invalid>']:
                result=subprocess.run([str(binary),'--check-notification',ticket,'0123456789abcdef'],input=invalid,capture_output=True,text=True,encoding='utf-8',timeout=30)
                self.assertNotEqual(result.returncode,0)


if __name__=='__main__':unittest.main()
