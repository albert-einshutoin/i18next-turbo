#!/usr/bin/env python3
"""Local, disposable extraction comparison; never executes fixture application code."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import statistics
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[2]
ROOT = REPO.parent
OUT = REPO / 'target' / 'evaluation-20261013'
NODE = shutil.which('node')
TOOLS = {
    'cli': [NODE, str(ROOT / 'i18next-cli/dist/esm/cli.js')],
    'turbo': [str(REPO / 'target/release/i18next-turbo')],
    'turbo-wrapper': [NODE, str(REPO / 'bin/cli.js')],
}
LOCALES = ['en', 'ja', 'ru']
NAMESPACES = ['common', 'account', 'checkout', 'dashboard', 'settings', 'catalog']
ENV = dict(os.environ, CI='true', NO_COLOR='1',
           I18NEXT_TURBO_BINARY=str(REPO / 'target/release/i18next-turbo'))


def cpu_name():
    if sys.platform == 'darwin':
        return subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip()
    if sys.platform.startswith('linux'):
        return platform.processor() or None
    raise RuntimeError('Benchmark runners support Linux and macOS only')


def stop_process_group(process):
    # A wrapper may have exited while its descendants still own the session.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=5)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def flatten(value, prefix=''):
    if not isinstance(value, dict):
        return {prefix: value}
    result = {}
    for key, child in value.items():
        result.update(flatten(child, f'{prefix}.{key}' if prefix else key))
    return result


def catalog(project):
    result = {}
    for path in sorted((project / 'locales').glob('*/*.json')):
        result.update({f'{path.parent.name}/{path.stem}:{key}': value
                       for key, value in flatten(json.loads(path.read_text())).items()})
    return result


def differences(reference, actual):
    missing = sorted(reference.keys() - actual.keys())
    extra = sorted(actual.keys() - reference.keys())
    changed = [{'key': key, 'reference': reference[key], 'actual': actual[key]}
               for key in sorted(reference.keys() & actual.keys())
               if reference[key] != actual[key]]
    return {'equal': not (missing or extra or changed), 'missing': missing,
            'extra': extra, 'changed_values': changed}


def sources_digest(project):
    digest = hashlib.sha256()
    for path in sorted((project / 'src').rglob('*')):
        if path.is_file():
            digest.update(str(path.relative_to(project)).encode())
            digest.update(path.read_bytes())
    return digest.hexdigest()


def make_case(name, files, expected=None):
    case = OUT / 'fixtures' / name
    case.mkdir(parents=True, exist_ok=True)
    write_json(case / 'sources.json', files)
    for tool in TOOLS:
        project = case / tool
        project.mkdir(exist_ok=True)
        (project / '.evaluation-owned').write_text('disposable fixture\n')
        if (project / 'src').exists():
            shutil.rmtree(project / 'src')
        for relative, source in files.items():
            path = project / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(source)
        common = {'input': ['src/**/*.ts', 'src/**/*.tsx'],
                  'ignore': ['src/ignored/**'], 'functions': ['t', 'i18n.t'],
                  'extractFromComments': False, 'removeUnusedKeys': False,
                  'defaultValue': '', 'keySeparator': '.', 'nsSeparator': ':',
                  'primaryLanguage': 'en', 'generateBasePluralForms': False}
        write_json(project / 'i18next-turbo.json', dict(common, output='locales',
                   locales=LOCALES, defaultNamespace='common'))
        cli = {'locales': LOCALES, 'extract': dict(common, defaultNS='common',
               output='locales/{{language}}/{{namespace}}.json')}
        (project / 'i18next.config.cjs').write_text('module.exports = ' + json.dumps(cli) + ';\n')
        write_json(project / 'seed.json', {
            locale: {ns: {'existing': {'title': f'KEEP:{locale}:{ns}'},
                          'obsolete': {'keep': f'UNUSED:{locale}:{ns}'}}
                     for ns in NAMESPACES} for locale in LOCALES})
        reset_catalog(project)
    hashes = {tool: sources_digest(case / tool) for tool in TOOLS}
    assert len(set(hashes.values())) == 1, 'different source bytes across tools'
    write_json(case / 'manifest.json', {'source_files': len(files), 'source_sha256': hashes,
               'locales': LOCALES, 'namespaces': NAMESPACES,
               'expected': expected})
    return case


def reset_catalog(project):
    assert (project / '.evaluation-owned').is_file()
    path = project / 'locales'
    if path.exists():
        shutil.rmtree(path)
    for locale, namespaces in json.loads((project / 'seed.json').read_text()).items():
        for ns, values in namespaces.items():
            write_json(path / locale / f'{ns}.json', values)


def args_for(tool, watch=False):
    if tool == 'cli':
        return TOOLS[tool] + ['extract', '--config', 'i18next.config.cjs', '--quiet'] + (['--watch'] if watch else [])
    return TOOLS[tool] + ['--config', 'i18next-turbo.json', 'watch' if watch else 'extract']


def run_extract(case, tool, label, timeout=60):
    log_dir = OUT / 'logs' / case.name
    log_dir.mkdir(parents=True, exist_ok=True)
    start = time.perf_counter()
    with (log_dir / f'{tool}-{label}.stdout').open('wb') as stdout, \
            (log_dir / f'{tool}-{label}.stderr').open('wb') as stderr:
        process = subprocess.Popen(args_for(tool), cwd=case / tool, env=ENV,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            process.wait(timeout=timeout)
        finally:
            stop_process_group(process)
    elapsed = (time.perf_counter() - start) * 1000
    if process.returncode:
        raise RuntimeError(f'{case.name}/{tool}/{label} exited {process.returncode}; see {log_dir}')
    return elapsed


def stats(samples):
    return {'samples_ms': samples, 'median_ms': statistics.median(samples),
            'mean_ms': statistics.mean(samples), 'min_ms': min(samples), 'max_ms': max(samples)}


def verify_case(case, tools=TOOLS, additional_expected=None, snapshot=''):
    outputs = {tool: catalog(case / tool) for tool in tools}
    comparisons = {tool: differences(outputs['cli'], outputs[tool]) for tool in tools if tool != 'cli'}
    expected = json.loads((case / 'manifest.json').read_text())['expected']
    if expected is not None:
        expected.update(additional_expected or {})
    oracle = {tool: differences(expected, values) for tool, values in outputs.items()} if expected is not None else None
    preserved = {}
    for tool in tools:
        seed = json.loads((case / tool / 'seed.json').read_text())
        preserved[tool] = all(outputs[tool].get(f'{locale}/{ns}:{key}') == value
                              for locale, namespaces in seed.items()
                              for ns, values in namespaces.items()
                              for key, value in flatten(values).items())
    result = {'key_value_counts': {tool: len(value) for tool, value in outputs.items()},
              'parity': comparisons, 'oracle': oracle, 'existing_values_preserved': preserved}
    snapshot_dir = case / snapshot
    write_json(snapshot_dir / 'comparison.json', result)
    for tool, value in outputs.items():
        write_json(snapshot_dir / f'{tool}-catalog.json', value)
    return result


def static_case(count):
    files, expected = {}, {}
    for locale in LOCALES:
        for ns in NAMESPACES:
            expected[f'{locale}/{ns}:existing.title'] = f'KEEP:{locale}:{ns}'
            expected[f'{locale}/{ns}:obsolete.keep'] = f'UNUSED:{locale}:{ns}'
    key_names = ['title', 'subtitle', 'save', 'cancel', 'empty', 'error',
                 'details.body', 'table.header', 'loading', 'help']
    for index in range(count):
        ns = NAMESPACES[index % len(NAMESPACES)]
        if index % 5 == 4:
            source = ('export type Row = {id: string; count: number; active: boolean};\n'
                      'export const normalize = (rows: Row[]) => rows.filter(row => row.active);\n') * 1
            source += '\n'.join(f'export const config{n} = {{enabled: true, limit: {n}}};' for n in range(30))
            files[f'src/lib/helper{index:04}.ts'] = source
            continue
        calls = '\n'.join(f'    {name.replace(".", "_")}: t({json.dumps(name)}, {{name}}),'
                          for name in key_names)
        source = f'''import {{useTranslation}} from 'react-i18next';
import i18n from 'i18next';
type Props = {{name: string; active: boolean; items: readonly string[]}};
export function Page{index}({{name, active, items}}: Props) {{
  const {{t}} = useTranslation('{ns}', {{keyPrefix: 'page{index}'}});
  const labels = {{
{calls}
  }};
  const visible = items.filter(item => item.length > 0);
  return <section data-active={{active}}><h1>{{labels.title}}</h1>
    <p>{{labels.details_body}}</p><button>{{labels.save}}</button>
    {{visible.map(item => <span key={{item}}>{{item}}</span>)}}</section>;
}}
export const shared{index} = i18n.t('{ns}:page{index}.shared');
'''
        files[f'src/features/{ns}/Page{index:04}.tsx'] = source
        for locale in LOCALES:
            for key in key_names + ['shared']:
                expected[f'{locale}/{ns}:page{index}.{key}'] = ''
    files['src/ignored/ignored.ts'] = "t('common:must_not_extract');\n"
    return make_case(f'static-{count}', files, expected)


def probe_case():
    files = {
        'src/scopes.tsx': """import {useTranslation, Trans} from 'react-i18next';
import i18n from 'i18next';
export function A(){const {t: translate}=useTranslation('account',{keyPrefix:'profile'});
return translate('name')}
const fixed = i18n.getFixedT('en', 'checkout', 'form'); fixed('submit');
const {t}=useTranslation('dashboard'); t($ => $.summary.title); t(`static.literal`);
const element=<Trans ns="common" i18nKey="notice" defaults="Welcome {{name}}">Welcome</Trans>;
""",
        'src/defaults.ts': "t('common:greeting', 'Hello'); t('common:save', {defaultValue: 'Save'});\n",
        'src/plurals.ts': "declare const count: number; t('catalog:items', {count}); t('catalog:rank', {count, ordinal: true});\n",
        'src/context.ts': "declare const male: boolean; t('account:friend', {context: male ? 'male' : 'female'});\n",
        'src/types.ts': "export type Status = 'pending' | 'done' | 'failed'; export const LABELS = {save: 'common:constant.save', cancel: 'common:constant.cancel'} as const;\n",
        'src/dynamic.ts': "import {Status, LABELS} from './types'; declare const state: Status; t(`account:state.${state}`); t(LABELS.save); t(LABELS.cancel);\n",
        'src/array.ts': "const states = ['open', 'closed'] as const; for(const state of states) {t(`checkout:order.${state}`)}\n",
        'src/objects.ts': "t('settings:options', {returnObjects: true});\n",
        'src/ignored/ignored.ts': "t('common:must_not_extract');\n",
    }
    case = make_case('feature-probes', files)
    for tool in TOOLS:
        seed_path = case / tool / 'seed.json'
        seed = json.loads(seed_path.read_text())
        for locale in LOCALES:
            seed[locale]['settings']['options'] = {'light': f'{locale}:Light', 'dark': f'{locale}:Dark'}
        write_json(seed_path, seed)
        reset_catalog(case / tool)
    return case


def preservation_case():
    expected = {f'{locale}/{ns}:{key}': value for locale in LOCALES for ns in NAMESPACES
                for key, value in {'existing.title': f'KEEP:{locale}:{ns}',
                                   'obsolete.keep': f'UNUSED:{locale}:{ns}'}.items()}
    return make_case('preservation-probe', {
        'src/App.ts': "import {t} from 'i18next'; t('common:existing.title', {defaultValue: 'Override attempt'});\n",
        'src/Profile.ts': "import {t} from 'i18next'; t('account:existing.title', {defaultValue: 'Override attempt'});\n",
    }, expected)


def measure_case(case, runs):
    results = {}
    for tool in TOOLS:
        reset_catalog(case / tool)
        run_extract(case, tool, 'warmup')
    samples = {tool: {'first_sync': [], 'repeat_extract': []} for tool in TOOLS}
    for mode in ['first_sync', 'repeat_extract']:
        for sample in range(runs):
            order = list(TOOLS)
            order = order[sample % len(order):] + order[:sample % len(order)]
            for tool in order:
                if mode == 'first_sync':
                    reset_catalog(case / tool)
                elapsed = run_extract(case, tool, f'{mode}-{sample}')
                samples[tool][mode].append(elapsed)
        # Compare the final catalog after each mode, not only elapsed times.
        parity = verify_case(case)
        if parity['oracle'] is not None:
            assert all(item['equal'] for item in parity['oracle'].values()), f'{case.name}: oracle mismatch'
        assert all(parity['existing_values_preserved'].values())
    for tool in TOOLS:
        results[tool] = {mode: stats(values) for mode, values in samples[tool].items()}
    return {'timings': results, 'comparison': parity,
            'manifest': json.loads((case / 'manifest.json').read_text())}


def wait_for_key(project, key, process, timeout=30):
    deadline = time.perf_counter() + timeout
    while time.perf_counter() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f'watch process exited {process.returncode}')
        try:
            if all(json.loads((project / 'locales' / locale / 'common.json').read_text())
                   .get('watch', {}).get(key) == '' for locale in LOCALES):
                return
        except (FileNotFoundError, json.JSONDecodeError):
            pass  # Files may be temporarily absent or mid-write; deadline is strict.
        time.sleep(0.005)
    raise TimeoutError(f'watch did not synchronize {key} within {timeout}s')


def measure_watch(case, runs):
    samples = {}
    expected = {f'{locale}/common:watch.{key}': '' for locale in LOCALES
                for key in ['ready'] + [f'edit{sample}' for sample in range(runs)]}
    for tool in ['cli', 'turbo']:
        project = case / tool
        reset_catalog(project)
        source = next((project / 'src/features/common').glob('*.tsx'))
        original = source.read_text()
        source.write_text(original + "\ni18n.t('common:watch.ready');\n")
        log_path = OUT / 'logs' / case.name / f'{tool}-watch.log'
        with log_path.open('w') as log:
            process = subprocess.Popen(args_for(tool, watch=True), cwd=project, env=ENV,
                                       stdout=log, stderr=log, start_new_session=True)
            try:
                wait_for_key(project, 'ready', process)
                # Let chokidar finish registering its initially enumerated files.
                time.sleep(1)
                values = []
                for sample in range(runs):
                    key = f'edit{sample}'
                    start = time.perf_counter()
                    with source.open('a') as stream:
                        stream.write(f"\ni18n.t('common:watch.{key}');\n")
                    wait_for_key(project, key, process)
                    values.append((time.perf_counter() - start) * 1000)
                samples[tool] = stats(values)
            finally:
                stop_process_group(process)
                source.write_text(original)
    return {'timings': samples, 'comparison': verify_case(case, ['cli', 'turbo'], expected, 'watch'),
            'additional_expected': expected,
            'metric': 'source append to all 3 locales containing the new key; includes debounce and 5ms polling',
            'runs': runs}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--phase', choices=['probe', 'timing', 'watch', 'all'], default='all')
    parser.add_argument('--runs', type=int, default=7)
    options = parser.parse_args()
    if sys.version_info < (3, 11):
        parser.error('Python 3.11 or later is required')
    if not 1 <= options.runs <= 15:
        parser.error('--runs must be between 1 and 15')
    report_path = OUT / 'report.json'
    report = {'run_id': str(time.time_ns()), 'selected_phase': options.phase,
              'status': 'in_progress', 'active_phase': 'report_write'}
    try:
        write_json(report_path, report)
        report['active_phase'] = 'environment'
        report['environment'] = {'node': subprocess.check_output([NODE, '--version'], text=True).strip(),
            'system': platform.platform(), 'machine': platform.machine(), 'cpu': cpu_name(),
            'cli_commit': subprocess.check_output(['git', '-C', str(ROOT/'i18next-cli'), 'rev-parse', 'HEAD'], text=True).strip(),
            'turbo_commit': subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', 'HEAD'], text=True).strip(),
            'commands': {tool: args_for(tool) for tool in TOOLS},
            'cli_entry_sha256': hashlib.sha256((ROOT/'i18next-cli/dist/esm/cli.js').read_bytes()).hexdigest(),
            'turbo_binary_sha256': hashlib.sha256((REPO/'target/release/i18next-turbo').read_bytes()).hexdigest()}
        if options.phase in ['probe', 'all']:
            report['active_phase'] = 'probe'
            for case in [static_case(12), probe_case(), preservation_case()]:
                for tool in TOOLS:
                    run_extract(case, tool, 'probe')
                result = verify_case(case)
                report[case.name] = result
                write_json(report_path, report)
                print(case.name, json.dumps({'counts': result['key_value_counts'], 'equal': {k:v['equal'] for k,v in result['parity'].items()}}, ensure_ascii=False), flush=True)
                if case.name == 'static-12':
                    assert all(item['equal'] for item in result['oracle'].values()), 'static smoke failed'
        if options.phase in ['timing', 'all']:
            report['active_phase'] = 'timing'
            for count in [300, 1200]:
                case = static_case(count)
                report[case.name] = measure_case(case, options.runs)
                write_json(report_path, report)
                print(case.name, json.dumps({tool:{mode: round(value['median_ms'], 2) for mode,value in modes.items()} for tool,modes in report[case.name]['timings'].items()}), flush=True)
            files = json.loads((OUT / 'fixtures/static-300/sources.json').read_text())
            files.update(json.loads((OUT / 'fixtures/feature-probes/sources.json').read_text()))
            case = make_case('mixed-308', files)
            report[case.name] = measure_case(case, options.runs)
            write_json(report_path, report)
            print(case.name, json.dumps({'counts': report[case.name]['comparison']['key_value_counts'],
                  'equal': {k:v['equal'] for k,v in report[case.name]['comparison']['parity'].items()}}), flush=True)
        if options.phase in ['watch', 'all']:
            report['active_phase'] = 'watch'
            case = OUT / 'fixtures/static-1200'
            assert case.is_dir(), 'run timing first'
            report['watch-1200'] = measure_watch(case, min(options.runs, 5))
            write_json(report_path, report)
            print('watch-1200', json.dumps(report['watch-1200']['timings']), flush=True)
        report['status'] = 'completed'
        report.pop('active_phase')
        report['completed_at'] = time.strftime('%Y-%m-%dT%H:%M:%S%z')
        write_json(report_path, report)
    except BaseException as error:
        report['status'] = 'failed'
        report['failed_phase'] = report.pop('active_phase', 'report_write')
        report.pop('completed_at', None)
        report['error'] = str(error)
        try:
            write_json(report_path, report)
        except OSError as report_error:
            error.add_note(f'Could not save failed-run report: {report_error}')
        raise


if __name__ == '__main__':
    main()
