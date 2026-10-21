"""EN-US: Bound PE parsing and verify recursive collection rejects unresolved/native-driver inputs.
PT-BR: Limita a leitura PE e verifica se a coleta recursiva rejeita dependências ausentes/drivers.
"""
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'packaging/windows'))
from pe import imports, is_system
from bundle import Bundle


def pe_image(names, delay=()):
    data = bytearray(4096)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 128)
    data[128:132] = b'PE\0\0'
    struct.pack_into('<HHIIIHH', data, 132, 0x8664, 1, 0, 0, 0, 240, 0)
    struct.pack_into('<H', data, 152, 0x20b)
    struct.pack_into('<Q', data, 152 + 24, 0x140000000)
    struct.pack_into('<I', data, 152 + 108, 16)
    struct.pack_into('<IIII', data, 392 + 8, 3584, 4096, 3584, 512)
    strings = 2048
    for values, table, index, stride, name_index in ((names, 512, 1, 20, 3), (delay, 1024, 13, 32, 1)):
        if not values:
            continue
        struct.pack_into('<II', data, 152 + 112 + index * 8, 4096 + table - 512, (len(values) + 1) * stride)
        for i, name in enumerate(values):
            entry = [0] * (stride // 4)
            if index == 13:
                entry[0] = 1
            entry[name_index] = 4096 + strings - 512
            struct.pack_into('<' + 'I' * len(entry), data, table + i * stride, *entry)
            encoded = name.encode() + b'\0'
            data[strings:strings + len(encoded)] = encoded
            strings += len(encoded)
    return data


class WindowsBundle(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_import_and_delay_import_dependencies_are_both_retained(self):
        image = self.root / 'app.exe'
        image.write_bytes(pe_image(['KERNEL32.dll', 'libgtk-4-1.dll'], ['libssl-3-x64.dll']))
        self.assertEqual(imports(image), ['kernel32.dll', 'libgtk-4-1.dll', 'libssl-3-x64.dll'])

    def test_non_amd64_truncated_and_path_imports_are_rejected(self):
        for data in (b'MZ', b'not executable', pe_image(['../private.dll'])):
            path = self.root / 'bad.dll'
            path.write_bytes(data)
            with self.assertRaises((ValueError, UnicodeError)):
                imports(path)
        data = pe_image(['kernel32.dll'])
        struct.pack_into('<H', data, 132, 0x14c)
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'AMD64'):
            imports(path)

    def test_system_contracts_do_not_classify_gpu_or_msys_dlls_as_windows(self):
        for name in ('kernel32.dll', 'api-ms-win-crt-runtime-l1-1-0.dll', 'd3d11.dll'):
            self.assertTrue(is_system(name))
        for name in ('nvcuda.dll', 'nvcuvid.dll', 'msys-2.0.dll', 'libgtk-4-1.dll', 'libpython3.14.dll'):
            self.assertFalse(is_system(name))

    def test_closure_copies_transitive_dependencies_and_rejects_missing_ones(self):
        prefix = self.root / 'ucrt64'
        (prefix / 'bin').mkdir(parents=True)
        (prefix / 'bin/first.dll').write_bytes(pe_image(['second.dll']))
        (prefix / 'bin/second.dll').write_bytes(pe_image(['kernel32.dll']))
        stage = self.root / 'package'
        (stage / 'runtime/bin').mkdir(parents=True)
        (stage / 'runtime/bin/app.exe').write_bytes(pe_image(['first.dll']))
        inventory = 'first /ucrt64/bin/first.dll\nsecond /ucrt64/bin/second.dll'
        with patch('bundle.run', side_effect=['/ucrt64', inventory]):
            bundler = Bundle(stage, prefix)
        bundler.closure()
        self.assertTrue((stage / 'runtime/bin/second.dll').is_file())
        self.assertEqual(bundler.dependencies['runtime/bin/first.dll'], ['second.dll'])
        (stage / 'runtime/bin/broken.dll').write_bytes(pe_image(['nvcuda.dll']))
        with self.assertRaisesRegex(RuntimeError, 'Unresolved non-system DLL nvcuda'):
            bundler.closure()


if __name__ == '__main__':
    unittest.main()
