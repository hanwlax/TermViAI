"""Collector fixtures and signal safety checks; no SSH, NPU, or termination signal.
Native compatibility probes use only signal 0 on the test process itself.
"""
import ctypes
import errno
import types
import importlib.util
import pathlib
import unittest
from unittest.mock import patch, Mock, mock_open

spec = importlib.util.spec_from_file_location('collector', pathlib.Path(__file__).parents[1] / 'src/monitor_collect.py')
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)

TABLE = '''
+---------+---------+--------------------------------------------------+
| NPU Name | Health | Power(W) Temp(C) Hugepages-Usage(page) |
| Chip | Bus-Id | AICore(%) Memory-Usage(MB) HBM-Usage(MB) |
| 4 Ascend910 | OK | 100 44 0 / 0 |
| 2 | 0000:41:00.0 | 72 100 / 1024 52428 / 65536 |
| 3 | 0000:42:00.0 | 64 100 / 1024 52000 / 65536 |
| 7 Ascend910 | OK | 100 44 0 / 0 |
| 0 | 0000:81:00.0 | N/A 100 / 1024 N/A / N/A |
| NPU Chip | Process id | Process name | Process memory(MB) |
| 4 2 | 18420 | python3 | 51500 |
| 4 3 | 18420 | python3 | 51000 |
| 4 2 | 20510 | python3 | 100 |
'''

class CollectorTests(unittest.TestCase):
    def test_non_contiguous_topology_and_processes(self):
        data = collector.parse_table(TABLE)
        self.assertEqual([(d['npu'],d['die']) for d in data['dies']],[(4,2),(4,3),(7,0)])
        self.assertEqual(data['dies'][0]['used_mb'],52428)
        self.assertEqual(data['dies'][0]['total_mb'],65536)
        self.assertEqual(data['dies'][2]['core'],None)
        self.assertEqual(len(data['processes']),3)

    def test_reference_25_5_physical_ids_are_not_die_ids(self):
        # Fixture from the user's npus-usage/test-parser.js (ASCEND_25).
        data = collector.parse_table((pathlib.Path(__file__).parent / 'fixtures/npu-smi-25.5.1.txt').read_text())
        self.assertEqual([(d['npu'], d['die']) for d in data['dies']], [(0,0),(0,1),(1,0),(1,1),(2,0)])
        self.assertEqual([d['used_mb'] for d in data['dies']], [46904,46582,3123,2875,3120])
        self.assertEqual([d['core'] for d in data['dies']], [0]*5)
        self.assertEqual([(p['npu'],p['die']) for p in data['processes']],[(0,0),(0,0),(1,1)])
        self.assertEqual(data['processes'][2]['pid'],3102668)

    def test_reference_25_6_single_die_with_blank_and_na_cells(self):
        # Preserve leading empty columns and ignore the container PID column.
        data = collector.parse_table((pathlib.Path(__file__).parent / 'fixtures/npu-smi-25.6-single-die.txt').read_text())
        self.assertEqual([(d['npu'], d['die']) for d in data['dies']], [(0,0),(1,0),(6,0),(7,0)])
        self.assertEqual([d['used_mb'] for d in data['dies']], [6632,6711,66466,66461])
        self.assertEqual(data['dies'][1]['core'],5)
        self.assertEqual(len(data['processes']),3)
        self.assertTrue(all(p['npu']==6 and p['die']==0 for p in data['processes']))
        self.assertEqual(data['processes'][0]['pid'],3621066)

    def test_absent_process_table_is_unavailable(self):
        data=collector.parse_table(TABLE.split('| NPU Chip')[0])
        self.assertFalse(data['processes_available'])
        self.assertEqual(data['processes'],[])

    def test_empty_or_duplicate_table_is_rejected(self):
        for text in ['permission denied', '', TABLE.replace('| 3 |','| 2 |')]:
            with self.assertRaises(RuntimeError): collector.parse_table(text)

    def test_missing_hbm_is_not_ddr(self):
        data=collector.parse_table(TABLE)
        self.assertIsNone(data['dies'][2]['used_mb'])
        self.assertIsNone(data['dies'][2]['total_mb'])

    def test_identity_uses_start_time_after_parenthesized_comm(self):
        fields=['S']+['0']*18+['890']+['0']*5
        files=[Mock(),Mock()]
        files[0].__enter__=Mock(return_value=Mock(read=Mock(return_value='42 (a tricky) name) '+' '.join(fields))))
        files[1].__enter__=Mock(return_value=Mock(read=Mock(return_value='boot-id\n')))
        for f in files:f.__exit__=Mock(return_value=False)
        with patch('builtins.open',side_effect=files):self.assertEqual(collector.identity(42),'boot-id:890')

    def stop_context(self):
        return patch.multiple(collector,process_control=Mock(side_effect=lambda: (collector.Pidfd(), '')),namespace_safe=Mock(return_value=True),run_smi=Mock(return_value={'processes_available':True,'processes':[{'pid':42}]}),identity=Mock(return_value='boot:123'))

    def sample_process(self, cwd='/workspace', error=None, tokens=('boot:123', 'boot:123')):
        # One PID appears on two DIEs. Its metadata must only be collected once.
        data = {'processes': [{'pid':42, 'die':0}, {'pid':42, 'die':1}]}
        with patch.multiple(collector, run_smi=Mock(return_value=data), process_control=Mock(return_value=(Mock(), '')), namespace_safe=Mock(return_value=True), identity=Mock(side_effect=tokens)), \
             patch.object(collector.os, 'readlink', return_value=cwd, side_effect=error) as readlink, \
             patch.object(collector.os, 'stat', return_value=Mock(st_uid=1000)), \
             patch.object(collector.os, 'geteuid', return_value=1000), \
             patch.object(collector.os, 'pidfd_open', create=True), \
             patch.object(collector.signal, 'pidfd_send_signal', create=True), \
             patch.object(collector.pwd, 'getpwuid', return_value=Mock(pw_name='tester')), \
             patch('builtins.open', mock_open(read_data=b'python3\0train.py\0')):
            result = collector.sample()
            readlink.assert_called_once_with('/proc/42/cwd')
        return result['processes']

    def test_working_directory_is_shared_across_dies_and_preserves_path(self):
        for cwd in ['/workspace', '/workspace/project name/训练 (deleted)']:
            for process in self.sample_process(cwd):
                self.assertEqual(process['cwd'], cwd)
                self.assertEqual(process['command'], 'python3 train.py')
                self.assertTrue(process['controllable'])

    def test_unreadable_working_directory_does_not_discard_other_metadata(self):
        for error in [PermissionError('denied'), FileNotFoundError('exited')]:
            for process in self.sample_process(error=error):
                self.assertEqual(process['cwd'], '')
                self.assertEqual(process['command'], 'python3 train.py')
                self.assertEqual(process['token'], 'boot:123')
                self.assertTrue(process['controllable'])

    def test_reused_pid_does_not_publish_working_directory(self):
        for process in self.sample_process(tokens=('boot:123', 'boot:456')):
            self.assertEqual(process['cwd'], '')
            self.assertFalse(process['controllable'])

    def test_stale_pid_is_never_signalled(self):
        with self.stop_context(),patch.object(collector.os,'pidfd_open',return_value=20,create=True),patch.object(collector.os,'close'),patch.object(collector.signal,'pidfd_send_signal',create=True) as send:
            result=collector.terminate([{'pid':42,'token':'boot:old'}])
            send.assert_not_called();self.assertIn('identity changed',result[0]['status'])

    def test_other_processes_are_never_signalled(self):
        with self.stop_context(),patch.object(collector.os,'pidfd_open',create=True) as opened,patch.object(collector.signal,'pidfd_send_signal',create=True) as send:
            result=collector.terminate([{'pid':43,'token':'boot:123'}])
            send.assert_not_called();opened.assert_not_called();self.assertIn('No longer',result[0]['status'])

    def test_permission_failure_and_pending_are_distinct(self):
        with self.stop_context(),patch.object(collector.os,'pidfd_open',return_value=20,create=True),patch.object(collector.os,'close'),patch.object(collector.signal,'pidfd_send_signal',side_effect=PermissionError('denied'),create=True):
            self.assertEqual(collector.terminate([{'pid':42,'token':'boot:123'}])[0]['status'],'denied')
        with self.stop_context(),patch.object(collector.os,'pidfd_open',return_value=20,create=True),patch.object(collector.os,'close'),patch.object(collector.signal,'pidfd_send_signal',create=True) as send,patch.object(collector.select,'select',return_value=([],[],[])):
            self.assertIn('waiting',collector.terminate([{'pid':42,'token':'boot:123'}])[0]['status'])
            self.assertEqual(send.call_args.args[1],collector.signal.SIGTERM)

    def test_container_control_is_rejected(self):
        with patch.object(collector,'namespace_safe',return_value=False),patch.object(collector,'run_smi') as read:
            with self.assertRaises(RuntimeError):collector.terminate([{'pid':42,'token':'boot:123'}])
            read.assert_not_called()


class PidfdTests(unittest.TestCase):
    def test_native_bindings_are_preferred(self):
        with patch.object(collector.os, 'pidfd_open', return_value=20, create=True) as opened, \
             patch.object(collector.signal, 'pidfd_send_signal', create=True) as sent, \
             patch('ctypes.CDLL', side_effect=AssertionError('unnecessary libc load')):
            backend = collector.Pidfd()
            self.assertEqual(backend.open(42), 20)
            backend.send(20, 0)
            opened.assert_called_once_with(42, 0)
            sent.assert_called_once_with(20, 0, None, 0)

    def test_missing_python_bindings_use_real_libc_on_self_only(self):
        for missing_send in [False, True]:
            with patch.object(collector.os, 'pidfd_open', None, create=True), \
                 patch.object(collector.signal, 'pidfd_send_signal',
                              None if missing_send else collector.signal.pidfd_send_signal, create=True):
                backend = collector.Pidfd()
                fd = backend.open(collector.os.getpid())
                try:
                    backend.send(fd, 0)
                    self.assertFalse(collector.os.get_inheritable(fd))
                finally:
                    collector.os.close(fd)

    def test_raw_syscalls_on_self_when_libc_wrappers_are_missing(self):
        libc = ctypes.CDLL(None, use_errno=True)
        old_libc = types.SimpleNamespace(syscall=libc.syscall)
        with patch.object(collector.os, 'pidfd_open', None, create=True), \
             patch.object(collector.signal, 'pidfd_send_signal', None, create=True), \
             patch('ctypes.CDLL', return_value=old_libc):
            backend = collector.Pidfd()
            fd = backend.open(collector.os.getpid())
            try:
                backend.send(fd, 0)
                self.assertFalse(collector.os.get_inheritable(fd))
            finally:
                collector.os.close(fd)

    def test_syscall_abi_and_errno_for_x86_64_and_aarch64(self):
        for machine in ['x86_64', 'aarch64']:
            call = Mock(return_value=20)
            with patch.object(collector.os, 'pidfd_open', None, create=True), \
                 patch.object(collector.signal, 'pidfd_send_signal', None, create=True), \
                 patch('platform.machine', return_value=machine), \
                 patch('ctypes.CDLL', return_value=types.SimpleNamespace(syscall=call)):
                backend = collector.Pidfd()
                self.assertEqual(backend.open(42), 20)
                self.assertEqual([v.value for v in call.call_args.args], [434, 42, 0])
                backend.send(20, 0)
                self.assertEqual([v.value for v in call.call_args.args], [424, 20, 0, None, 0])
                call.return_value = -1
                with patch('ctypes.get_errno', return_value=errno.ENOSYS):
                    with self.assertRaises(OSError) as error: backend.open(42)
                    self.assertEqual(error.exception.errno, errno.ENOSYS)

    def test_unknown_raw_syscall_abi_is_rejected(self):
        with patch.object(collector.os, 'pidfd_open', None, create=True), \
             patch.object(collector.signal, 'pidfd_send_signal', None, create=True), \
             patch('platform.machine', return_value='unknown'), \
             patch('ctypes.CDLL', return_value=types.SimpleNamespace(syscall=Mock())):
            with self.assertRaisesRegex(RuntimeError, 'CPU ABI'): collector.Pidfd()

    def test_probe_checks_self_with_signal_zero_and_closes_descriptor(self):
        backend = Mock()
        backend.open.return_value = 20
        with patch.object(collector, 'namespace_safe', return_value=True), \
             patch.object(collector, 'Pidfd', return_value=backend), \
             patch.object(collector.os, 'close') as close:
            self.assertEqual(collector.process_control(), (backend, ''))
            backend.open.assert_called_once_with(collector.os.getpid())
            backend.send.assert_called_once_with(20, 0)
            close.assert_called_once_with(20)

    def test_kernel_and_policy_failures_have_distinct_reasons(self):
        for error, text in [(errno.ENOSYS, 'kernel'), (errno.EPERM, 'security policy'),
                            (errno.EACCES, 'security policy'), (errno.EMFILE, 'check failed')]:
            for phase in ['open', 'send']:
                backend = Mock()
                backend.open.return_value = 20
                getattr(backend, phase).side_effect = OSError(error, 'test')
                with patch.object(collector, 'namespace_safe', return_value=True), \
                     patch.object(collector, 'Pidfd', return_value=backend), \
                     patch.object(collector.os, 'close') as close:
                    result, reason = collector.process_control()
                    self.assertIsNone(result)
                    self.assertIn(text, reason)
                    self.assertNotIn('Python 3.9', reason)
                    self.assertEqual(close.call_count, int(phase == 'send'))

    def test_container_does_not_attempt_compatibility_probe(self):
        with patch.object(collector, 'namespace_safe', return_value=False), \
             patch.object(collector, 'Pidfd') as backend:
            result, reason = collector.process_control()
            self.assertIsNone(result)
            self.assertIn('namespace', reason)
            backend.assert_not_called()

if __name__=='__main__': unittest.main()
