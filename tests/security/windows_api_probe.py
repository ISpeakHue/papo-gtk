# EN-US: Diagnostic API compatibility only; full isolation assertions live in the packaged media-runtime check.
# PT-BR: Diagnóstico apenas da compatibilidade das APIs; as verificações completas estão no teste de runtime.
import ctypes
from ctypes import wintypes as w
import socket
import sys

print('handle API', flush=True)
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GetFinalPathNameByHandleW.argtypes = [w.HANDLE, w.LPWSTR, w.DWORD, w.DWORD]
buffer = ctypes.create_unicode_buffer(32768)
print('handle result', kernel.GetFinalPathNameByHandleW(w.HANDLE(555555), buffer, len(buffer), 0), flush=True)
print('credential API', flush=True)
cred = ctypes.WinDLL('advapi32', use_last_error=True)
cred.CredReadW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, ctypes.POINTER(ctypes.c_void_p)]
result = ctypes.c_void_p()
print('credential result', cred.CredReadW('Papo.Media.Nonexistent.Diagnostic', 1, 0, ctypes.byref(result)), flush=True)
print('socket API', flush=True)
for family, address in [(socket.AF_INET, ('127.0.0.1', 9)), (socket.AF_INET6, ('::1', 9))]:
    s = socket.socket(family, socket.SOCK_STREAM)
    s.settimeout(0.3)
    try:
        s.connect(address)
    except OSError:
        print('TCP denied', flush=True)
    finally:
        s.close()
print('API diagnostic completed', flush=True)
