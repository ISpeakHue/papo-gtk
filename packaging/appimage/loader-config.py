# EN-US: Relocate loader paths into the cache; the mounted AppImage remains read-only.
# PT-BR: Ajusta os caminhos dos carregadores no cache; o AppImage montado permanece somente leitura.
"""Relocate Glycin's immutable configuration without modifying the mounted image."""
import configparser
import os
from pathlib import Path
import tempfile

root = Path(os.environ['PAPO_APPDIR'])
target = Path(os.environ['GLYCIN_DATA_DIR']) / 'glycin-loaders/2+/conf.d'
target.mkdir(parents=True, exist_ok=True, mode=0o700)
for source in (root / 'usr/share/glycin-loaders/2+/conf.d').glob('*.conf'):
    config = configparser.ConfigParser(interpolation=None)
    config.optionxform = str
    config.read(source)
    for section in config:
        if 'Exec' in config[section]:
            original = config[section]['Exec']
            if not original.startswith('/usr/libexec/glycin-loaders/'):
                raise RuntimeError('Unexpected packaged image loader path')
            config[section]['Exec'] = str(root / original.lstrip('/'))
    with tempfile.NamedTemporaryFile(mode='w', dir=target, delete=False) as staged:
        config.write(staged)
    os.replace(staged.name, target / source.name)
