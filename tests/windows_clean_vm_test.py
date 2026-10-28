"""EN-US: Check guest provisioning rules and preserve any existing VM disk.
PT-BR: Verifica regras de provisionamento do guest e preserva discos de VMs existentes.
"""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('windows_vm_prepare', ROOT / 'packaging/windows/clean-vm/prepare.py')
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)
NS = {'u': prepare.NS}


class WindowsCleanVm(unittest.TestCase):
    def test_answer_uses_valid_partition_order_and_standard_acceptance_account(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'Autounattend.xml'
            prepare.answer_file(path)
            root = ET.parse(path).getroot()
            disk = root.find('.//u:DiskConfiguration/u:Disk', NS)
            self.assertEqual(disk.findtext('u:DiskID', namespaces=NS), '0')
            modify = disk.findall('u:ModifyPartitions/u:ModifyPartition', NS)
            orders = [int(item.findtext('u:Order', namespaces=NS)) for item in modify]
            self.assertEqual(orders, list(range(1, len(orders) + 1)))
            self.assertEqual([item.findtext('u:PartitionID', namespaces=NS) for item in modify], ['1', '3'])
            account_name = root.findtext('.//u:AutoLogon/u:Username', namespaces=NS)
            accounts = {item.findtext('u:Name', namespaces=NS): item.findtext('u:Group', namespaces=NS)
                        for item in root.findall('.//u:LocalAccounts/u:LocalAccount', NS)}
            self.assertEqual(accounts[account_name], 'Users')
            self.assertEqual(account_name, 'PapoTester')

    def test_preparation_refuses_existing_disk_before_running_native_tools(self):
        with tempfile.TemporaryDirectory() as directory:
            disk = Path(directory) / 'os.qcow2'
            disk.write_bytes(b'Existing VM must be preserved')
            arguments = ['prepare.py', '--iso', 'missing.iso', '--iso-sha256', '0' * 64,
                         '--archive', 'missing.zip', '--directory', directory]
            with patch.object(sys, 'argv', arguments), patch.object(prepare, 'run') as native:
                with self.assertRaisesRegex(SystemExit, 'Refusing to overwrite'):
                    prepare.main()
                native.assert_not_called()
            self.assertEqual(disk.read_bytes(), b'Existing VM must be preserved')


if __name__ == '__main__':
    unittest.main()
