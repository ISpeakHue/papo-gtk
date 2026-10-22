#!/usr/bin/env python3
"""EN-US: Prepare isolated QEMU media for the clean Windows runtime acceptance test.
PT-BR: Prepara mídias QEMU isoladas para o teste de runtime em um Windows limpo.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
NS = 'urn:schemas-microsoft-com:unattend'
WCM = 'http://schemas.microsoft.com/WMIConfig/2002/State'
ET.register_namespace('', NS)
ET.register_namespace('wcm', WCM)


def child(parent, tag, text=None, **attributes):
    node = ET.SubElement(parent, f'{{{NS}}}{tag}', attributes)
    if text is not None:
        node.text = str(text)
    return node


def component(settings, name):
    return child(settings, 'component', name=name, processorArchitecture='amd64',
                 publicKeyToken='31bf3856ad364e35', language='neutral', versionScope='nonSxS')


def answer_file(path):
    root = ET.Element(f'{{{NS}}}unattend')
    pe = child(root, 'settings', pass_='windowsPE')
    pe.attrib['pass'] = pe.attrib.pop('pass_')
    language = component(pe, 'Microsoft-Windows-International-Core-WinPE')
    child(child(language, 'SetupUILanguage'), 'UILanguage', 'en-US')
    for key, value in [('InputLocale', 'en-US'), ('SystemLocale', 'en-US'), ('UILanguage', 'en-US'), ('UserLocale', 'en-US')]:
        child(language, key, value)
    setup = component(pe, 'Microsoft-Windows-Setup')
    user = child(setup, 'UserData')
    child(user, 'AcceptEula', 'true')
    child(user, 'FullName', 'Papo Runtime Test')
    disk_config = child(setup, 'DiskConfiguration')
    disk = child(disk_config, 'Disk', **{f'{{{WCM}}}action': 'add'})
    child(disk, 'DiskID', 0)
    child(disk, 'WillWipeDisk', 'true')
    create = child(disk, 'CreatePartitions')
    for order, kind, size in [(1, 'EFI', 260), (2, 'MSR', 16), (3, 'Primary', None)]:
        partition = child(create, 'CreatePartition', **{f'{{{WCM}}}action': 'add'})
        child(partition, 'Order', order)
        child(partition, 'Type', kind)
        child(partition, 'Size' if size else 'Extend', size if size else 'true')
    modify = child(disk, 'ModifyPartitions')
    for order, (partition_id, fmt, label, letter) in enumerate([(1, 'FAT32', 'System', None), (3, 'NTFS', 'Windows', 'C')], 1):
        partition = child(modify, 'ModifyPartition', **{f'{{{WCM}}}action': 'add'})
        child(partition, 'Order', order)
        child(partition, 'PartitionID', partition_id)
        child(partition, 'Format', fmt)
        child(partition, 'Label', label)
        if letter:
            child(partition, 'Letter', letter)
    child(disk_config, 'WillShowUI', 'OnError')
    install = child(child(setup, 'ImageInstall'), 'OSImage')
    metadata = child(child(install, 'InstallFrom'), 'MetaData', **{f'{{{WCM}}}action': 'add'})
    child(metadata, 'Key', '/IMAGE/INDEX')
    child(metadata, 'Value', 1)
    destination = child(install, 'InstallTo')
    child(destination, 'DiskID', 0)
    child(destination, 'PartitionID', 3)
    child(install, 'WillShowUI', 'OnError')
    specialize = child(root, 'settings', **{'pass': 'specialize'})
    shell = component(specialize, 'Microsoft-Windows-Shell-Setup')
    child(shell, 'ComputerName', 'PAPO-CLEAN-VM')
    child(shell, 'TimeZone', 'UTC')
    deployment = component(specialize, 'Microsoft-Windows-Deployment')
    command = child(child(deployment, 'RunSynchronous'), 'RunSynchronousCommand', **{f'{{{WCM}}}action': 'add'})
    child(command, 'Order', 1)
    child(command, 'Description', 'Stage offline Papo acceptance fixtures')
    child(command, 'Path', r'cmd.exe /c for %d in (D E F G H I J K L M N O P) do @if exist %d:\setup.ps1 powershell.exe -NoProfile -ExecutionPolicy Bypass -File %d:\setup.ps1')
    oobe = child(root, 'settings', **{'pass': 'oobeSystem'})
    language = component(oobe, 'Microsoft-Windows-International-Core')
    for key in ('InputLocale', 'SystemLocale', 'UILanguage', 'UserLocale'):
        child(language, key, 'en-US')
    shell = component(oobe, 'Microsoft-Windows-Shell-Setup')
    options = child(shell, 'OOBE')
    for key, value in [('HideEULAPage', 'true'), ('HideOnlineAccountScreens', 'true'), ('HideWirelessSetupInOOBE', 'true'), ('ProtectYourPC', 3)]:
        child(options, key, value)
    # EN-US: Fixture-only credentials belong to this offline disposable guest, never a real account.
    # PT-BR: Credenciais de teste pertencem apenas ao guest descartável offline, nunca a uma conta real.
    password = 'Papo-Offline-Test-2026!'
    accounts = child(child(shell, 'UserAccounts'), 'LocalAccounts')
    for name, group in [('PapoTestAdmin', 'Administrators'), ('PapoTester', 'Users')]:
        account = child(accounts, 'LocalAccount', **{f'{{{WCM}}}action': 'add'})
        child(account, 'Name', name)
        child(account, 'DisplayName', name)
        child(account, 'Group', group)
        secret = child(account, 'Password')
        child(secret, 'Value', password)
        child(secret, 'PlainText', 'true')
    logon = child(shell, 'AutoLogon')
    child(logon, 'Username', 'PapoTester')
    child(logon, 'Enabled', 'true')
    child(logon, 'LogonCount', 1)
    secret = child(logon, 'Password')
    child(secret, 'Value', password)
    child(secret, 'PlainText', 'true')
    ET.indent(root)
    ET.ElementTree(root).write(path, encoding='utf-8', xml_declaration=True)


def run(*arguments):
    subprocess.run([str(value) for value in arguments], check=True)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iso', required=True, type=Path)
    parser.add_argument('--iso-sha256', required=True)
    parser.add_argument('--archive', required=True, type=Path)
    parser.add_argument('--directory', type=Path, default=ROOT / 'target/windows-clean-vm')
    parser.add_argument('--test', choices=['runtime','identity','storage','media'], default='runtime')
    parser.add_argument('--firmware', type=Path, default=Path('/usr/share/qemu/firmware/30-edk2-ovmf-4m-qcow2-x64-sb-enrolled.json'))
    args = parser.parse_args()
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    if (directory / 'os.qcow2').exists():
        raise SystemExit('Refusing to overwrite an existing VM disk; select a new directory.')
    iso, archive = args.iso.resolve(), args.archive.resolve()
    if digest(iso) != args.iso_sha256.lower():
        raise SystemExit('Windows ISO checksum does not match the official published hash.')
    checksum = archive.with_suffix(archive.suffix + '.sha256')
    if digest(archive) != checksum.read_text(encoding='utf-8').split()[0].lower():
        raise SystemExit('Papo ZIP checksum mismatch.')
    media = directory / 'input'
    media.mkdir(exist_ok=True)
    answer_file(media / 'Autounattend.xml')
    for source in (archive, checksum, ROOT / 'packaging/windows/check-portable.ps1', ROOT / 'packaging/windows/check-identity.ps1', ROOT / 'packaging/windows/check-storage.ps1', ROOT / 'packaging/windows/check-media.ps1', Path(__file__).with_name('acceptance.ps1'), Path(__file__).with_name('identity-acceptance.ps1'), Path(__file__).with_name('storage-acceptance.ps1'), Path(__file__).with_name('media-acceptance.ps1')):
        if source.suffix == '.ps1':
            # EN-US: PowerShell 5.1 needs the BOM to preserve bilingual fixture text.
            # PT-BR: O PowerShell 5.1 precisa do BOM para preservar os textos bilíngues.
            (media / source.name).write_text(source.read_text(encoding='utf-8-sig'), encoding='utf-8-sig')
        else:
            shutil.copy2(source, media / source.name)
    (media / 'setup.ps1').write_text(r'''$ErrorActionPreference = 'Stop'
$destination = 'C:\PapoAcceptance'
New-Item -ItemType Directory -Force $destination | Out-Null
foreach ($pattern in @('acceptance.ps1', 'identity-acceptance.ps1', 'storage-acceptance.ps1', 'check-portable.ps1', 'check-identity.ps1', 'check-storage.ps1', 'check-media.ps1', 'media-acceptance.ps1', '*.zip', '*.sha256')) {
    Copy-Item (Join-Path $PSScriptRoot $pattern) $destination
}
$startup = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\PapoAcceptance.cmd"
'@powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\PapoAcceptance\ACCEPTANCE_ENTRY' | Set-Content $startup -Encoding ASCII
# EN-US: Only this disposable one-shot test entry must be removable by the standard fixture account.
# PT-BR: Só esta entrada descartável de teste deve poder ser removida pela conta padrão de teste.
& icacls.exe $startup /grant '*S-1-5-32-545:M' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Cannot make the fixture startup entry one-shot' }
'''.replace('ACCEPTANCE_ENTRY', {'runtime':'acceptance.ps1','identity':'identity-acceptance.ps1','storage':'storage-acceptance.ps1','media':'media-acceptance.ps1'}[args.test]), encoding='utf-8')
    run('xorriso', '-as', 'mkisofs', '-quiet', '-J', '-r', '-V', 'PAPO_INPUT', '-o', directory / 'input.iso', media)
    run('qemu-img', 'create', '-f', 'qcow2', directory / 'os.qcow2', '80G')
    results = directory / 'results.img'
    with results.open('xb') as output:
        output.truncate(256 * 1024 * 1024)
    run('mkfs.vfat', '-F', '32', '-n', 'PAPORESULTS', results)
    marker = directory / 'PAPO_RESULTS.marker'
    marker.write_text('Disposable Papo clean-VM acceptance results.\n', encoding='ascii')
    run('mcopy', '-i', results, marker, '::/PAPO_RESULTS.marker')
    firmware = json.loads(args.firmware.read_text(encoding='utf-8'))
    mapping = firmware['mapping']
    if not {'secure-boot', 'enrolled-keys', 'requires-smm'}.issubset(firmware['features']):
        raise SystemExit('Use a Secure Boot/SMM firmware descriptor with Microsoft keys enrolled.')
    variable_format = mapping['nvram-template']['format']
    variables = directory / f'uefi-vars.{variable_format}'
    shutil.copy2(mapping['nvram-template']['filename'], variables)
    manifest = {'schema': 1, 'iso': str(iso), 'iso_sha256': args.iso_sha256.lower(),
                'archive': archive.name, 'archive_sha256': digest(archive),
                'test': args.test, 'network': 'none', 'memory_mib': 6144, 'cpus': 4,
                'accounts': {'administrator': 'PapoTestAdmin', 'acceptance': 'PapoTester'},
                'firmware': {'executable': mapping['executable'], 'variables': str(variables), 'variable_format': variable_format, 'descriptor': str(args.firmware), 'code_sha256': digest(Path(mapping['executable']['filename']))},
                'hypervisor': subprocess.check_output(['qemu-system-x86_64', '--version'], text=True).splitlines()[0]}
    (directory / 'VM-INPUTS.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print('Prepared fresh Windows VM disks and read-only input media:', directory)


if __name__ == '__main__':
    main()
