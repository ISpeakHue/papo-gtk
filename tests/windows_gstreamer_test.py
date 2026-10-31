"""EN-US: Native Unicode executable paths must retain the separate plugin scanner.
PT-BR: Caminhos Unicode nativos devem preservar o scanner de plugins separado.
"""
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'packaging/windows'))
from devcheck import probe_job


@unittest.skipUnless(sys.platform == 'win32' and shutil.which('gcc'), 'Native Windows GStreamer compiler/runtime required')
class WindowsGStreamer(unittest.TestCase):
    def test_fresh_registry_from_unicode_executable_keeps_scanner(self):
        with tempfile.TemporaryDirectory(prefix='Papo-Gst-') as directory:
            root = Path(directory)
            source = root / 'probe.c'
            source.write_text('''#include <windows.h>
#include <gst/gst.h>
#include <stdio.h>
int main(void) {
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    if (getchar() != '1') return 6;
    if (!gst_init_check(NULL, NULL, NULL)) return 2;
    gunichar2 path[32768];
    DWORD n = GetModuleFileNameW(NULL, (WCHAR *)path, G_N_ELEMENTS(path));
    if (!n || n >= G_N_ELEMENTS(path)) return 3;
    gchar *utf8 = g_utf16_to_utf8(path, n, NULL, NULL, NULL);
    if (g_strcmp0(utf8, gst_get_main_executable_path())) return 4;
    g_free(utf8);
    GstElementFactory *factory = gst_element_factory_find("identity");
    if (!factory) return 5;
    gst_object_unref(factory);
    puts("{\\"status\\":\\"pass\\"}");
    return 0;
}
''', encoding='ascii')
            flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', 'gstreamer-1.0'], text=True))
            compiled = root / 'probe.exe'
            subprocess.run(['gcc', str(source), *flags, '-o', str(compiled)], check=True, timeout=30)
            unicode = root / 'á é ç'
            unicode.mkdir()
            binary = unicode / 'probe.exe'
            shutil.move(compiled, binary)
            env = dict(os.environ, PYTHONHOME=sys.prefix, PYTHONNOUSERSITE='1', PYTHONDONTWRITEBYTECODE='1',
                       GST_REGISTRY_1_0=str(root / 'registry.bin'), GST_DEBUG='GST_PLUGIN_LOADING:4',
                       GST_PLUGIN_LOADING_WHITELIST='coreelements')
            with subprocess.Popen([str(binary)], env=env, cwd=root, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  encoding='utf-8', errors='replace',
                                  creationflags=subprocess.CREATE_NO_WINDOW) as child:
                try:
                    with probe_job(child):
                        output, errors = child.communicate(input='1', timeout=30)
                finally:
                    child.wait(timeout=5)
            self.assertEqual(child.returncode, 0, errors[-6000:])
            self.assertEqual(json.loads(output)['status'], 'pass')
            self.assertNotIn("Couldn't build cmd string", errors)
            self.assertNotIn('External plugin loader failed', errors)
            self.assertTrue((root / 'registry.bin').is_file())


if __name__ == '__main__':
    unittest.main()
