"""EN-US: Prove signing changes only PE checksum/security-table data, never executable content.
PT-BR: Prova que a assinatura só altera checksum/tabela de certificados, nunca o conteúdo executável.
"""
import struct


def fields(data):
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("Not a PE image")
    pe = struct.unpack_from("<I", data, 60)[0]
    if pe < 64 or pe + 24 > len(data) or data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Invalid PE header")
    optional = pe + 24
    size = struct.unpack_from("<H", data, pe + 20)[0]
    if optional + size > len(data) or size < 96:
        raise ValueError("Truncated optional header")
    magic = struct.unpack_from("<H", data, optional)[0]
    base = {0x10b: 96, 0x20b: 112}.get(magic)
    if base is None or size < base + 40 or struct.unpack_from("<I", data, optional + base - 4)[0] < 5:
        raise ValueError("Missing PE security directory")
    return optional + 64, optional + base + 32


def signing_only(before, after):
    checksum, security = fields(before)
    if fields(after) != (checksum, security) or before[security:security + 8] != b"\0" * 8:
        raise ValueError("Expected one previously unsigned PE image")
    offset, size = struct.unpack_from("<II", after, security)
    if (not size or offset < len(before) or offset - len(before) > 7 or offset % 8
            or size < 8 or offset + size != len(after) or any(after[len(before):offset])):
        raise ValueError("Invalid appended Authenticode certificate table")
    old, new = bytearray(before), bytearray(after[:len(before)])
    for index, length in ((checksum, 4), (security, 8)):
        old[index:index + length] = new[index:index + length] = b"\0" * length
    if old != new:
        raise ValueError("Signing changed executable content")
