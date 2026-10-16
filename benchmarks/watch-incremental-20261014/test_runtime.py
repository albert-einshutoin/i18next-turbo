"""Failure-path checks for the disposable POSIX benchmark runners."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('evaluation', REPO / 'benchmarks/evaluation-20261013/run.py')
evaluation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluation)


@unittest.skipUnless(sys.platform.startswith(('linux', 'darwin')), 'Linux/macOS process groups required')
class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.case = self.root / 'case'
        (self.case / 'turbo').mkdir(parents=True)
        self.addCleanup(self.recover_group)

    def recover_group(self):
        pid_file = self.case / 'turbo/parent.pid'
        if pid_file.exists():
            try:
                os.killpg(int(pid_file.read_text()), signal.SIGKILL)
            except ProcessLookupError:
                pass

    def assert_stopped(self, pid):
        for _ in range(500):
            try:
                os.kill(pid, 0)
                # An exited orphan can remain a zombie until Linux PID 1 reaps it.
                if sys.platform.startswith('linux'):
                    state = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[0]
                    if state == 'Z':
                        return
            except ProcessLookupError:
                return
            except FileNotFoundError:
                pass  # Recheck process existence after a possible exit race.
            time.sleep(0.01)
        self.fail(f'owned descendant {pid} remained running after cleanup')

    def tree_source(self, exit_leader=False):
        child = "import signal,time; from pathlib import Path; signal.signal(signal.SIGTERM, signal.SIG_IGN); Path('child.ready').touch(); time.sleep(60)"
        return f'''import os,signal,subprocess,sys,time
from pathlib import Path
Path('parent.pid').write_text(str(os.getpid()))
child = subprocess.Popen([sys.executable, '-c', {child!r}])
Path('child.pid').write_text(str(child.pid))
while not Path('child.ready').exists(): time.sleep(0.01)
print('stdout before failure', flush=True)
print('stderr before failure', file=sys.stderr, flush=True)
{'sys.exit(0)' if exit_leader else 'signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)'}
'''

    def test_linux_cpu_metadata_does_not_require_sysctl(self):
        with patch.object(sys, 'platform', 'linux'), patch.object(evaluation.platform, 'processor', return_value='Linux CPU'), \
                patch.object(evaluation.subprocess, 'check_output', side_effect=AssertionError('sysctl must not run')):
            self.assertEqual(evaluation.cpu_name(), 'Linux CPU')

    def test_extract_nonzero_saves_both_logs(self):
        source = "import sys; print('out', flush=True); print('err', file=sys.stderr, flush=True); sys.exit(7)"
        with patch.object(evaluation, 'OUT', self.root), patch.dict(evaluation.TOOLS, turbo=[sys.executable, '-c', source]):
            with self.assertRaisesRegex(RuntimeError, 'exited 7'):
                evaluation.run_extract(self.case, 'turbo', 'failure')
        self.assertEqual((self.root / 'logs/case/turbo-failure.stdout').read_text(), 'out\n')
        self.assertEqual((self.root / 'logs/case/turbo-failure.stderr').read_text(), 'err\n')

    def test_extract_timeout_saves_logs_and_kills_descendant(self):
        with patch.object(evaluation, 'OUT', self.root), \
                patch.dict(evaluation.TOOLS, turbo=[sys.executable, '-c', self.tree_source()]):
            with self.assertRaises(subprocess.TimeoutExpired):
                evaluation.run_extract(self.case, 'turbo', 'timeout', timeout=2)
        self.assertIn('stdout before failure', (self.root / 'logs/case/turbo-timeout.stdout').read_text())
        self.assertIn('stderr before failure', (self.root / 'logs/case/turbo-timeout.stderr').read_text())
        self.assert_stopped(int((self.case / 'turbo/child.pid').read_text()))

    def test_cleanup_kills_descendant_after_leader_exits(self):
        project = self.case / 'turbo'
        with (project / 'process.log').open('w') as log:
            process = subprocess.Popen([sys.executable, '-c', self.tree_source(True)], cwd=project,
                                       stdout=log, stderr=log, start_new_session=True)
            try:
                self.assertEqual(process.wait(timeout=10), 0)
                child = int((project / 'child.pid').read_text())
                os.kill(child, 0)
                evaluation.stop_process_group(process)
                self.assert_stopped(child)
            finally:
                self.recover_group()
                process.wait(timeout=5)

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux waitid/WNOWAIT required')
    def test_stopped_check_rejects_running_child_and_accepts_zombie(self):
        process = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
        try:
            with patch.object(time, 'sleep'), self.assertRaisesRegex(AssertionError, 'owned descendant'):
                self.assert_stopped(process.pid)
            process.kill()
            # WNOWAIT observes exit without reaping, leaving a real zombie.
            info = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOWAIT)
            self.assertEqual(info.si_code, os.CLD_KILLED)
            self.assertEqual(info.si_status, signal.SIGKILL)
            os.kill(process.pid, 0)
            self.assert_stopped(process.pid)
        finally:
            process.kill()
            process.wait(timeout=5)

    def test_runner_failure_is_nonzero_and_saves_failed_report(self):
        # Only the reference CLI is replaced: its deliberate error must stop the
        # real runner before any fake tool can be mistaken for a parity result.
        repo = self.root / 'repo'
        files = ['benchmarks/evaluation-20261013/run.py', 'benchmarks/watch-incremental-20261014/run.py',
                 'benchmarks/watch-incremental-20261014/cli-lock.json', 'benchmarks/watch-incremental-20261014/cli-install.json',
                 'src/watcher.rs', 'src/finite_values.rs', 'src/extractor.rs', 'src/config.rs', 'src/json_sync.rs', 'bin/cli.js']
        for name in files:
            destination = repo / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / name, destination)
        toolchain = repo / 'target/watch-import-evaluation-20261013/toolchain'
        cli = toolchain / 'node_modules/i18next-cli'
        (cli / 'dist/esm').mkdir(parents=True)
        (cli / 'package.json').write_text(json.dumps({'version': '1.75.3'}))
        (cli / 'dist/esm/cli.js').write_text("console.log('CLI failure stdout'); console.error('CLI failure stderr'); process.exit(7);\n")
        shutil.copyfile(REPO / 'benchmarks/watch-incremental-20261014/cli-lock.json', toolchain / 'package-lock.json')
        binary = repo / 'target/release/i18next-turbo'
        binary.parent.mkdir()
        binary.write_text('not executed')
        subprocess.run(['git', 'init', '-q', str(repo)], check=True)
        subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture', '--allow-empty'], cwd=repo, check=True)
        process = subprocess.run([sys.executable, str(repo / 'benchmarks/watch-incremental-20261014/run.py'),
                                  '--regression', '--sizes', '300', '--runs', '1'], cwd=repo,
                                 capture_output=True, text=True, timeout=30)
        self.assertNotEqual(process.returncode, 0)
        reports = list((repo / 'target/watch-import-evaluation-20261013/runs').glob('*/results.json'))
        self.assertEqual(len(reports), 1)
        report = json.loads(reports[0].read_text())
        self.assertEqual(report['status'], 'failed')
        self.assertIn('exited 7', report['error'])
        logs = reports[0].parent / 'logs/full-300-baseline'
        self.assertEqual((logs / 'cli-full-0.stdout').read_text(), 'CLI failure stdout\n')
        self.assertEqual((logs / 'cli-full-0.stderr').read_text(), 'CLI failure stderr\n')


if __name__ == '__main__':
    unittest.main()
