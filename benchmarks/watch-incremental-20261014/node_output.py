"""Bounded local output/exit checks; fixture code never accesses a service."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import sys

if sys.version_info < (3, 11):
    raise SystemExit("Python 3.11 or later is required")

repo = Path(__file__).resolve().parents[2]
out = repo / 'target/watch-import-evaluation-20261013' / f'node-output-{time.time_ns()}'
out.mkdir(parents=True)
size = 262144
workers = 8
before = subprocess.check_output(['git', 'show', '260ed4ea8e5634352984c445be35e3ca37263cbb:bin/cli.js'], cwd=repo)
assert hashlib.sha256(before).hexdigest() == '7550dcf5566e8b6fe8b32682f0c3f133db9348094ef27af5a2ad3bfbe60f1fd3'

def check(wrapper, root, kind, index):
    project = root / f'{kind}-{index}'
    project.mkdir()
    child = project/'child.cjs'
    child.write_text(f"#!/usr/bin/env node\nprocess.stdout.write('x'.repeat({size}));\nprocess.stderr.write('y'.repeat({size}));\nprocess.exitCode = {7 if kind == 'child_error' else 0};\n")
    child.chmod(0o755)
    message = 'DIAGNOSTIC-BEGIN:' + 'z'*size + ':DIAGNOSTIC-END'
    plugin = project/'plugin.cjs'
    if kind == 'plugin_error':
        plugin.write_text('module.exports={afterSync(){throw new Error('+json.dumps(message)+');}};\n')
    else:
        plugin.write_text("module.exports={afterSync(){require('fs').writeFileSync('.after-sync','done');}};\n")
    config = {'locales':['en'], 'pluginErrorMode':'fail-fast', 'plugins':[{'resolve':'./plugin.cjs'}],
              'extract':{'input':'src/**/*.ts','output':'locales/{{language}}/{{namespace}}.json'}}
    (project/'i18next.config.cjs').write_text('module.exports='+json.dumps(config)+';\n')
    start = time.monotonic()
    result = subprocess.run(['node', str(wrapper), '--config','i18next.config.cjs','extract'],
                            cwd=project, env=dict(os.environ, I18NEXT_TURBO_BINARY=str(child)),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
    expected_code = {'success':0, 'child_error':7, 'plugin_error':1}[kind]
    stdout_equal = result.stdout == b'x'*size
    stderr_equal = result.stderr == b'y'*size if kind != 'plugin_error' else (
        result.stderr.startswith(b'y'*size) and message.encode() in result.stderr
        and b'plugin hook "afterSync" failed' in result.stderr
        and result.stderr.endswith(b':DIAGNOSTIC-END\n'))
    hook_ran = (project/'.after-sync').exists()
    passed = stdout_equal and stderr_equal and result.returncode == expected_code and hook_ran == (kind == 'success')
    return {'case':kind, 'trial':index, 'passed':passed, 'exit_code':result.returncode,
            'expected_exit_code':expected_code, 'stdout_bytes':len(result.stdout),
            'stderr_bytes':len(result.stderr), 'stdout_equal':stdout_equal, 'stderr_equal':stderr_equal,
            'after_sync_ran':hook_ran, 'elapsed_seconds':time.monotonic()-start}

report = {'workers':workers, 'trials_per_condition':8, 'child_stream_bytes':size,
          'wrapper_before_sha256':hashlib.sha256(before).hexdigest(),
          'wrapper_after_sha256':hashlib.sha256((repo/'bin/cli.js').read_bytes()).hexdigest(),
          'node_version':subprocess.check_output(['node','--version'],text=True).strip(), 'results':{}}
# Place the byte-identical old entry beside cli.js to keep module resolution identical.
with tempfile.NamedTemporaryFile(prefix='.node-parallel-before-',suffix='.js',dir=repo/'bin') as old:
    old.write(before); old.flush()
    for label,wrapper in [('before',Path(old.name)), ('after',repo/'bin/cli.js')]:
        with tempfile.TemporaryDirectory(prefix='turbo-node-output-') as temp:
            with ThreadPoolExecutor(max_workers=workers) as pool:
                futures = [pool.submit(check, wrapper, Path(temp), kind, trial)
                           for kind in ['success','child_error','plugin_error'] for trial in range(8)]
                report['results'][label] = [f.result() for f in futures]
        print(label, sum(r['passed'] for r in report['results'][label]), '/24 passed', flush=True)
        (out/'parallel-output.json').write_text(json.dumps(report,indent=2)+'\n')
assert any(not r['passed'] for r in report['results']['before'])
assert all(r['passed'] for r in report['results']['after'])
