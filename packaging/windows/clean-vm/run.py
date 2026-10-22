#!/usr/bin/env python3
"""EN-US: Boot only the prepared guest disk, with no network or host filesystem sharing.
PT-BR: Inicia apenas o disco preparado, sem rede nem compartilhamento do sistema de arquivos do host.
"""
import argparse
import json
from pathlib import Path
import signal
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', required=True, type=Path)
    parser.add_argument('--install', action='store_true', help='Boot the official DVD once and accept its keyboard prompt')
    parser.add_argument('--sockets', type=Path, default=Path('/tmp/papo-windows-clean-vm'))
    args = parser.parse_args()
    directory = args.directory.resolve()
    sockets = args.sockets.resolve()
    sockets.mkdir(mode=0o700, exist_ok=True)
    if (sockets / 'qmp.sock').exists():
        raise SystemExit('QMP socket already exists; do not start a second guest on this disk.')
    manifest = json.loads((directory / 'VM-INPUTS.json').read_text(encoding='utf-8'))
    tpm_state = directory / 'tpm'
    tpm_state.mkdir(exist_ok=True)
    tpm = subprocess.Popen(['swtpm', 'socket', '--tpm2', '--tpmstate', f'dir={tpm_state}',
                            '--ctrl', f'type=unixio,path={sockets / "tpm.sock"}',
                            '--log', f'file={directory / "tpm.log"},level=1'])
    guest = None
    try:
        for _ in range(100):
            if (sockets / 'tpm.sock').exists():
                break
            if tpm.poll() is not None:
                raise RuntimeError('TPM emulator failed to start')
            time.sleep(0.05)
        command = ['qemu-system-x86_64', '-name', 'papo-clean-windows', '-machine', 'q35,smm=on',
                   '-accel', 'kvm', '-cpu', 'host', '-smp', str(manifest['cpus']), '-m', str(manifest['memory_mib']),
                   '-global', 'driver=cfi.pflash01,property=secure,value=on',
                   '-drive', f'if=pflash,format={manifest["firmware"]["executable"]["format"]},readonly=on,file={manifest["firmware"]["executable"]["filename"]}',
                   '-drive', f'if=pflash,format={manifest["firmware"]["variable_format"]},file={manifest["firmware"]["variables"]}',
                   '-drive', f'file={directory / "os.qcow2"},format=qcow2,if=ide,index=0',
                   '-drive', f'file={manifest["iso"]},media=cdrom,readonly=on,if=ide,index=2',
                   '-drive', f'file={directory / "input.iso"},media=cdrom,readonly=on,if=ide,index=3',
                   '-device', 'qemu-xhci,id=usb',
                   '-drive', f'if=none,id=results,format=raw,file={directory / "results.img"}',
                   '-device', 'usb-tablet,bus=usb.0',
                   # EN-US: USB input remains available after Windows disables legacy PS/2 input.
                   # PT-BR: A entrada USB continua disponível quando o Windows desativa o PS/2 antigo.
                   '-device', 'usb-kbd,bus=usb.0',
                   '-chardev', f'socket,id=chrtpm,path={sockets / "tpm.sock"}',
                   '-tpmdev', 'emulator,id=tpm0,chardev=chrtpm', '-device', 'tpm-crb,tpmdev=tpm0',
                   '-vga', 'none', '-device', 'VGA,vgamem_mb=64,xres=1280,yres=800',
                   '-display', 'none', '-nic', 'none', '-boot', 'order=c,once=d,menu=on' if args.install else 'order=c,menu=on',
                   '-qmp', f'unix:{sockets / "qmp.sock"},server=on,wait=off', '-monitor', 'none',
                   '-rtc', 'base=utc,clock=host']
        if not args.install:
            command.extend(['-device', 'usb-storage,bus=usb.0,drive=results,removable=on'])
        # EN-US: Persist exact device/path arguments; all writable disks are task-owned images.
        # PT-BR: Registra dispositivos e caminhos exatos; todos os discos graváveis pertencem à tarefa.
        (directory / 'QEMU-COMMAND.json').write_text(json.dumps(command, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
        with (directory / 'qemu.log').open('a', encoding='utf-8') as log:
            guest = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            (directory / 'processes.json').write_text(json.dumps({'qemu': guest.pid, 'swtpm': tpm.pid}) + '\n')
            if args.install:
                for _ in range(200):
                    if (sockets / 'qmp.sock').exists() or guest.poll() is not None:
                        break
                    time.sleep(0.05)
                time.sleep(2)
                for _ in range(4):
                    subprocess.run([sys.executable, str(Path(__file__).with_name('control.py')), '--socket', str(sockets / 'qmp.sock'), '--key', 'spc'], check=True, stdout=log, stderr=log)
                    time.sleep(1)
            attached = not args.install
            while guest.poll() is None:
                # EN-US: Delay the removable disk until Windows owns C:, avoiding an installer drive-letter conflict.
                # PT-BR: Adia o disco removível até o Windows ocupar C:, evitando conflito de letras na instalação.
                if not attached and (directory / 'os.qcow2').stat().st_size > 1024 ** 3:
                    subprocess.run([sys.executable, str(Path(__file__).with_name('control.py')), '--socket', str(sockets / 'qmp.sock'), '--attach-results'], check=True, stdout=log, stderr=log)
                    (directory / 'RESULTS-ATTACHED.json').write_text(json.dumps({'after_guest_disk_bytes': (directory / 'os.qcow2').stat().st_size, 'utc_epoch': time.time()}) + '\n')
                    attached = True
                time.sleep(1)
            result = guest.returncode
            if result:
                raise SystemExit(f'QEMU exited with status {result}; inspect qemu.log.')
        print('QEMU stopped. Read the results disk only after Windows has completed shutdown.')
    finally:
        if guest is not None and guest.poll() is None:
            guest.terminate()
            try:
                guest.wait(timeout=10)
            except subprocess.TimeoutExpired:
                guest.kill()
                guest.wait()
        if tpm.poll() is None:
            tpm.send_signal(signal.SIGTERM)
        try:
            tpm.wait(timeout=10)
        except subprocess.TimeoutExpired:
            tpm.kill()
            tpm.wait()
        for name in ('tpm.sock', 'qmp.sock'):
            (sockets / name).unlink(missing_ok=True)


if __name__ == '__main__':
    main()
