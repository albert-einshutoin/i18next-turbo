#!/usr/bin/env python3
"""Disposable import-watch comparison; fixture application code is never executed."""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[2])
parser.add_argument('--sizes', type=int, nargs='+', default=[300, 1200])
parser.add_argument('--runs', type=int, default=5)
args = parser.parse_args()
if sys.version_info < (3, 11):
    parser.error('Python 3.11 or later is required')
if not 1 <= args.runs <= 5 or any(n < 30 or n > 1200 for n in args.sizes):
    parser.error('runs must be 1..5; fixture sizes must be 30..1200')
repo = args.repo.resolve()
bundle = Path(__file__).resolve().parent
work = repo / 'target/watch-import-evaluation-20261013'
cli_package = work / 'toolchain/node_modules/i18next-cli'
package = json.loads((cli_package / 'package.json').read_text())
assert package['version'] == '1.75.3'
spec = importlib.util.spec_from_file_location('baseline', repo / 'benchmarks/evaluation-20261013/run.py')
baseline = importlib.util.module_from_spec(spec)
sys.dont_write_bytecode = True
spec.loader.exec_module(baseline)
run_id = str(time.time_ns())
baseline.OUT = work / 'runs' / run_id
baseline.TOOLS['cli'] = [baseline.NODE, str(cli_package / 'dist/esm/cli.js')]
deadline = 8.0
report_path = baseline.OUT / 'results.json'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def seed_for(project):
    return {f'{lang}/{ns}:existing.title': f'KEEP:{lang}:{ns}'
            for lang in baseline.LOCALES for ns in baseline.NAMESPACES}

def configure(case):
    for tool in baseline.TOOLS:
        project = case / tool
        cfg = json.loads((project / 'i18next-turbo.json').read_text())
        cfg['removeUnusedKeys'] = True
        baseline.write_json(project / 'i18next-turbo.json', cfg)
        cli = dict(cfg)
        cli.pop('locales')
        cli.pop('defaultNamespace')
        cli.update(defaultNS='common', output='locales/{{language}}/{{namespace}}.json')
        (project / 'i18next.config.cjs').write_text('module.exports = ' + json.dumps({'locales': baseline.LOCALES, 'extract': cli}) + ';\n')
        seed = {lang: {ns: {'existing': {'title': f'KEEP:{lang}:{ns}'}} for ns in baseline.NAMESPACES}
                for lang in baseline.LOCALES}
        baseline.write_json(project / 'seed.json', seed)
        baseline.reset_catalog(project)

def fixture_states(size):
    initial = baseline.static_case(size)
    files = json.loads((initial / 'sources.json').read_text())
    expected = json.loads((initial / 'manifest.json').read_text())['expected']
    base_expected = {k: v for k, v in expected.items() if ':obsolete.keep' not in k}
    files['src/model/status.ts'] = "export type Status = 'open' | 'closed';\n"
    files['src/model/labels.ts'] = "export const LABELS = {action: 'common:actions.save'} as const;\n"
    files['src/retained.ts'] = "import i18n from 'i18next';\n" + '\n'.join(f"i18n.t('{ns}:existing.title');" for ns in baseline.NAMESPACES) + '\n'
    consumers = sorted(p for p in files if p.endswith('.tsx'))[:20]
    for path in consumers:
        ns = Path(path).parts[2]
        index = int(re.search(r'Page(\d+)', path)[1])
        files[path] = ("import {Status} from '../../model/status';\nimport {LABELS} from '../../model/labels';\n" + files[path]
                       + f"\ndeclare const state: Status;\ni18n.t(`{ns}:page{index}.state.${{state}}`);\ni18n.t(LABELS.action);\n")
    states = [('baseline', dict(files))]
    files['src/model/status.ts'] = "export type Status = 'open' | 'pending';\n"
    states.append(('type_change', dict(files)))
    files['src/model/labels.ts'] = "export const LABELS = {action: 'common:actions.publish'} as const;\n"
    states.append(('constant_change', dict(files)))
    added = 'src/features/dashboard/Added.tsx'
    files[added] = "import i18n from 'i18next';\nimport {Status} from '../../model/status';\nimport {LABELS} from '../../model/labels';\ndeclare const state: Status;\ni18n.t('dashboard:added.title');\ni18n.t(`dashboard:added.state.${state}`);\ni18n.t(LABELS.action);\n"
    states.append(('file_add', dict(files)))
    deleted = sorted(p for p in files if re.search(r'/Page\d+\.tsx$', p))[-1]
    ns = Path(deleted).parts[2]
    index = int(re.search(r'Page(\d+)', deleted)[1])
    del files[deleted]
    deleted_namespace, deleted_index = ns, index
    states.append(('file_delete', dict(files)))
    files['src/features/dashboard/AddedMoved.tsx'] = files.pop(added)
    states.append(('file_rename', dict(files)))
    files['src/model/labels-moved.ts'] = files.pop('src/model/labels.ts')
    files = {p: text.replace("from '../../model/labels'", "from '../../model/labels-moved'") for p, text in files.items()}
    states.append(('dependency_rename', dict(files)))
    # Rebuild every oracle from the literal workload contract, independently of source edits.
    with_oracles = []
    for stage, (name, sources) in enumerate(states):
        oracle = dict(base_expected)
        if stage >= 4:
            prefixes = tuple(f'{lang}/{deleted_namespace}:page{deleted_index}.' for lang in baseline.LOCALES)
            oracle = {k: v for k, v in oracle.items() if not k.startswith(prefixes)}
        statuses = ['open', 'closed'] if stage == 0 else ['open', 'pending']
        action = 'actions.save' if stage < 2 else 'actions.publish'
        for lang in baseline.LOCALES:
            oracle[f'{lang}/common:{action}'] = ''
            for consumer in consumers:
                ns = Path(consumer).parts[2]
                index = int(re.search(r'Page(\d+)', consumer)[1])
                for status in statuses:
                    oracle[f'{lang}/{ns}:page{index}.state.{status}'] = ''
            if stage >= 3:
                for key in ['added.title', 'added.state.open', 'added.state.pending']:
                    oracle[f'{lang}/dashboard:{key}'] = ''
        with_oracles.append((name, sources, oracle))
    assert not baseline.differences(with_oracles[1][2], with_oracles[0][2])['equal'], 'stale type results must fail'
    assert with_oracles[4][2] == with_oracles[5][2], 'pure file rename must preserve the catalog'
    return with_oracles

def source_delta(before, after):
    return {'added': sorted(after.keys() - before.keys()), 'removed': sorted(before.keys() - after.keys()),
            'modified': sorted(p for p in before.keys() & after.keys() if before[p] != after[p])}

def write_sources(project, sources):
    assert (project / '.evaluation-owned').is_file()
    source = project / 'src'
    if source.exists():
        shutil.rmtree(source)
    for name, text in sources.items():
        dest = project / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text)

def mutate(project, before, after):
    # Renames preserve bytes and expose filesystem rename events without a marker edit.
    removed = set(before) - set(after)
    added = set(after) - set(before)
    for old in sorted(removed):
        matching = [new for new in sorted(added) if before[old] == after[new]]
        if matching:
            new = matching[0]
            (project / new).parent.mkdir(parents=True, exist_ok=True)
            (project / old).rename(project / new)
            added.remove(new)
        else:
            (project / old).unlink()
    for new in sorted(added):
        (project / new).parent.mkdir(parents=True, exist_ok=True)
        (project / new).write_text(after[new])
    for name in sorted(set(before) & set(after)):
        if before[name] != after[name]:
            (project / name).write_text(after[name])

def log_count(path, tool):
    text = path.read_text(errors='replace') if path.exists() else ''
    return text.count('Extraction complete!') if tool == 'cli' else text.count('--- Ready ---') + text.count('--- Sync complete ---')

def wait_catalog(project, expected, process, log, tool, count, timeout):
    start = time.perf_counter()
    last = {}
    while time.perf_counter() - start < timeout:
        if process.poll() is not None:
            return 'process_exited', last
        if log_count(log, tool) > count:
            try:
                last = baseline.catalog(project)
                if last == expected:
                    return 'matched', last
            except (json.JSONDecodeError, FileNotFoundError):
                pass  # Only transient reads while disposable catalogs are being replaced.
        time.sleep(0.005)
    try:
        last = baseline.catalog(project)
    except (json.JSONDecodeError, FileNotFoundError):
        return 'unreadable_output', last
    return ('event_unobserved' if log_count(log, tool) == count else 'output_mismatch'), last

def stop(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)

def watch_once(case, tool, before, after, expected_before, expected_after, trial, label):
    project = case / tool
    write_sources(project, before)
    baseline.reset_catalog(project)
    log = baseline.OUT / 'logs' / case.name / f'{tool}-{label}-watch-{trial}.log'
    log.parent.mkdir(parents=True, exist_ok=True)
    command = baseline.args_for(tool, watch=True)
    if tool == 'cli':
        command.remove('--quiet')
    with log.open('w') as stream:
        process = subprocess.Popen(command, cwd=project, env=baseline.ENV, stdout=stream,
                                   stderr=stream, start_new_session=True)
        try:
            status, _ = wait_catalog(project, expected_before, process, log, tool, 0, 20)
            if status != 'matched':
                raise RuntimeError(f'watch preparation failed {case.name}/{tool}: {status}, {log}')
            time.sleep(1)  # Registration settling, outside the timed interval, same for all tools.
            count = log_count(log, tool)
            start = time.perf_counter()
            mutate(project, before, after)
            status, actual = wait_catalog(project, expected_after, process, log, tool, count, deadline)
            elapsed = (time.perf_counter() - start) * 1000
            result = {'status': status, 'elapsed_ms': elapsed if status == 'matched' else None,
                      'observation_ms': elapsed, 'completion_events': log_count(log, tool) - count,
                      'watch_vs_full': baseline.differences(expected_after, actual),
                      'seed_preserved': all(actual.get(k) == v for k, v in seed_for(project).items()),
                      'source_sha256': baseline.sources_digest(project),
                      'process_exit_before_cleanup': process.poll()}
            snapshot = baseline.OUT / 'snapshots' / case.name / label / str(trial)
            baseline.write_json(snapshot / f'{tool}-watch-catalog.json', actual)
            if status == 'matched':
                time.sleep(0.15)
                stable = baseline.catalog(project)
                result['stable_after_150ms'] = stable == expected_after
                if not result['stable_after_150ms']:
                    result['status'] = 'unstable_output'
                    result['elapsed_ms'] = None
            if tool != 'cli':
                result['incremental_extract_cycles'] = log.read_text().count('--- Sync complete ---')
                result['reparsed_files_per_cycle'] = [int(n) for n in re.findall(r'Reparsed files: (\d+)', log.read_text())]
                result['final_completion_events'] = log_count(log, tool) - count
                result['full_extract_cycles'] = max(0, log.read_text().count('--- Initial extraction ---') - 1)
                result['reported_cached_files'] = [int(n) for n in re.findall(r'Files: (\d+)', log.read_text())][1:]
            return result
        finally:
            stop(process)

report = {'run_id': run_id, 'status': 'in_progress', 'runs_requested': args.runs,
          'watch_timeout_seconds': deadline, 'poll_ms': 5, 'registration_settle_seconds': 1,
          'metric': 'source mutation start to completed watch cycle and all-locale full-catalog equality; includes debounce and polling',
          'environment': {'node': subprocess.check_output([baseline.NODE, '--version'], text=True).strip(),
                          'python': platform.python_version(), 'platform': platform.platform(),
                          'cpu': subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip(),
                          'turbo_source_sha256': {relative: sha(repo / relative) for relative in ['src/watcher.rs', 'src/finite_values.rs', 'src/extractor.rs', 'src/config.rs', 'src/json_sync.rs']},
                          'turbo_uncommitted_diff_sha256': hashlib.sha256(subprocess.check_output(['git', '-C', str(repo), 'diff', '--', 'src/watcher.rs', 'src/finite_values.rs', 'src/extractor.rs', 'docs/compatibility.md'])).hexdigest(),
                          'turbo_commit': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip(),
                          'cli_version': package['version'],
                          'cli_entry_sha256': sha(cli_package / 'dist/esm/cli.js'),
                          'toolchain_lock_sha256': sha(work / 'toolchain/package-lock.json'),
                          'cli_install_provenance': json.loads((bundle / 'cli-install.json').read_text()),
                          'turbo_binary_sha256': sha(repo / 'target/release/i18next-turbo'),
                          'wrapper_sha256': sha(repo / 'bin/cli.js')}, 'cases': {}}
preserved = {str(repo / name): sha(repo / name) for name in ['src/watcher.rs', 'src/finite_values.rs', 'src/extractor.rs', 'src/config.rs', 'src/json_sync.rs', 'bin/cli.js', 'benchmarks/evaluation-20261013/run.py']}
initial_files = {p: sha(Path(p)) for p in preserved}
assert initial_files == preserved
try:
    baseline.write_json(report_path, report)
    for size in args.sizes:
        states = fixture_states(size)
        full = {}
        for name, sources, oracle in states:
            case = baseline.make_case(f'full-{size}-{name}', sources)
            configure(case)
            full[name] = {'timings': {}, 'catalogs': {}, 'source_sha256': {}}
            for tool in baseline.TOOLS:
                samples = []
                for trial in range(args.runs):
                    baseline.reset_catalog(case / tool)
                    samples.append(baseline.run_extract(case, tool, f'full-{trial}'))
                    actual = baseline.catalog(case / tool)
                    comparison = baseline.differences(oracle, actual)
                    if not comparison['equal']:
                        baseline.write_json(baseline.OUT / f'{size}-{name}-{tool}-oracle-failure.json', comparison)
                        raise RuntimeError(f'full extraction differs from independent oracle: {size}/{name}/{tool}')
                full[name]['timings'][tool] = baseline.stats(samples)
                full[name]['catalogs'][tool] = actual
                full[name]['source_sha256'][tool] = baseline.sources_digest(case / tool)
                baseline.write_json(baseline.OUT / 'snapshots' / str(size) / name / f'{tool}-full-catalog.json', actual)
            assert len(set(full[name]['source_sha256'].values())) == 1
        result = {'base_workload_files': size, 'initial_input_files': len(states[0][1]) - 1,
                  'finite_dependency_consumers': 20, 'retained_translations': 18, 'steps': {}}
        report['cases'][str(size)] = result
        for index in range(1, len(states)):
            name, after, oracle = states[index]
            before = states[index-1][1]
            case = baseline.make_case(f'watch-{size}-{name}', before)
            configure(case)
            step = {'source_delta': source_delta(before, after), 'full_key_value_count': len(oracle),
                    'full_extraction_timings': full[name]['timings'], 'full_source_sha256': full[name]['source_sha256'],
                    'full_cross_tool_parity': {tool: baseline.differences(full[name]['catalogs']['cli'], catalog)
                                              for tool, catalog in full[name]['catalogs'].items() if tool != 'cli'},
                    'watch': {tool: [] for tool in baseline.TOOLS}}
            result['steps'][name] = step
            for trial in range(args.runs):
                order = list(baseline.TOOLS)
                order = order[trial % 3:] + order[:trial % 3]
                for tool in order:
                    if step['watch'][tool] and step['watch'][tool][-1]['status'] != 'matched':
                        continue  # A failed condition has no latency sample; don't retry it to obtain a success.
                    measurement = watch_once(case, tool, before, after, states[index-1][2], full[name]['catalogs'][tool], trial, name)
                    assert measurement['source_sha256'] == full[name]['source_sha256'][tool]
                    step['watch'][tool].append(measurement)
                    print(size, name, tool, trial, measurement['status'], round(measurement['observation_ms'], 2), flush=True)
                    baseline.write_json(report_path, report)
            for tool, measurements in step['watch'].items():
                samples = [m['elapsed_ms'] for m in measurements if m['status'] == 'matched']
                step.setdefault('watch_timings', {})[tool] = baseline.stats(samples) if samples else None
        baseline.write_json(report_path, report)
    assert {p: sha(Path(p)) for p in preserved} == initial_files
    report['source_files_unchanged'] = True
    report['status'] = 'completed'
    report['completed_at'] = time.strftime('%Y-%m-%dT%H:%M:%S%z')
    baseline.write_json(report_path, report)
    print('REPORT', report_path, flush=True)
except BaseException as error:
    report['status'] = 'failed'
    report['error'] = f'{type(error).__name__}: {error}'
    try:
        baseline.write_json(report_path, report)
    except OSError as report_error:
        error.add_note(f'Could not save failed-run report: {report_error}')
    raise
