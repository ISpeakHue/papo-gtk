#!/usr/bin/env python3
"""EN-US: Bound one exact GTK test process and retain native abort/timeout output.
PT-BR: Limita um processo GTK exato e preserva a saída de falhas e timeouts nativos.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TEST = 'ui::root::tests::ui_smoke_window_has_header_controls_and_server_password'


def execute(command, environment, timeout):
    process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace',
                               start_new_session=os.name != 'nt')
    timed_out = False
    try:
        output, _ = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        if os.name == 'nt':
            subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], capture_output=True)
            if process.poll() is None:
                process.kill()
        else:
            os.killpg(process.pid, signal.SIGKILL)
        output, _ = process.communicate(timeout=15)
    passed = not timed_out and process.returncode == 0 and bool(re.search(
        r'^test result: ok\. 1 passed; 0 failed;', output, re.MULTILINE))
    return {'status': 'pass' if passed else 'fail', 'exit_code': process.returncode,
            'timed_out': timed_out, 'output': output}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('case')
    parser.add_argument('--timeout', type=float, default=240)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--output-dir', type=Path,
                        default=ROOT / ('target/windows-build/gui' if os.name == 'nt' else 'target/ci/gui'))
    args = parser.parse_args()
    if not re.fullmatch(r'[a-z0-9-]{1,80}', args.case) or not 0 < args.timeout <= 600:
        parser.error('case must be a plain selector; timeout must be in (0, 600] seconds')
    binary = args.binary
    if binary is None:
        built = subprocess.run(['cargo', 'test', '--locked', '--no-run', '--message-format=json'],
                               cwd=ROOT, stdout=subprocess.PIPE, text=True, check=True)
        executables = [r['executable'] for line in built.stdout.splitlines()
                       if (r := json.loads(line)).get('reason') == 'compiler-artifact'
                       and r.get('profile', {}).get('test') and r.get('executable')
                       and r['target']['name'] == 'papo-gtk']
        if len(executables) != 1:
            raise RuntimeError('Expected one instrumented GTK test executable')
        binary = Path(executables[0])
    binary = binary.resolve()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment['PAPO_UI_CASE'] = args.case
    print(f'Running bounded GTK case: {args.case} ({args.timeout:g}s)', flush=True)
    result = execute([str(binary), TEST, '--exact', '--ignored', '--test-threads=1', '--nocapture'],
                     environment, args.timeout)
    output = result.pop('output')
    log = args.output_dir / f'{args.case}.log'
    log.write_text(output, encoding='utf-8')
    print(output, end='' if output.endswith('\n') else '\n', flush=True)
    report = {'schema': 1, 'case': args.case, 'test': TEST, 'timeout_seconds': args.timeout,
              'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest(), **result}
    (args.output_dir / f'{args.case}.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    raise SystemExit(0 if result['status'] == 'pass' else 124 if result['timed_out'] else 1)


if __name__ == '__main__':
    main()
