"""Conflict-control tests. All listener/process/signal operations are mocked."""
import pathlib
import types
import unittest
from unittest.mock import Mock, patch, mock_open

src = pathlib.Path(__file__).parents[1] / 'src'
control = types.ModuleType('forward_control_test')
exec(compile((src / 'monitor_collect.py').read_text(), 'monitor_collect.py', 'exec'), control.__dict__)
exec(compile((src / 'forward_control.py').read_text(), 'forward_control.py', 'exec'), control.__dict__)


class ForwardControlTests(unittest.TestCase):
    def test_only_noninteractive_ssh_sessions_are_eligible(self):
        for exe, cmd, eligible in [
            ('sshd', b'sshd: alice@notty', True),
            ('sshd', b'sshd: alice', True),
            ('sshd-session', b'sshd-session: alice@notty', True),
            ('sshd', b'sshd: alice@pts/0', False),
            ('sshd', b'/usr/sbin/sshd -D', False),
            ('python3', b'sshd: alice@notty', False),
            ('sshd', b'sshd: alice [priv]', False),
            ('sshd', b'sshd: [listener]', False),
        ]:
            with self.subTest(cmd=cmd), patch('builtins.open', mock_open(read_data=cmd)), \
                 patch.object(control.os, 'readlink', return_value='/usr/sbin/' + exe), \
                 patch.object(control, 'identity', return_value='boot:1'):
                self.assertEqual(control.session_info(42)['eligible'], eligible)

    def test_listener_parse_and_nonzero_exit(self):
        with patch.object(control.subprocess, 'run', return_value=Mock(returncode=0, stdout='LISTEN users:(("sshd",pid=42,fd=6))\nLISTEN users:(("sshd",pid=42,fd=7))', stderr='')) as run:
            self.assertEqual(control.listeners(18080), ([42], True))
            self.assertEqual(run.call_args.args[0][-1], 'sport = :18080')
        with patch.object(control.subprocess, 'run', return_value=Mock(returncode=1, stderr='permission denied')):
            with self.assertRaisesRegex(RuntimeError, 'permission denied'):
                control.listeners(18080)
        # ss can report a netlink failure on stderr while still exiting zero.
        with patch.object(control.subprocess, 'run', return_value=Mock(returncode=0, stderr='Cannot open netlink socket: Operation not permitted', stdout='')):
            with self.assertRaisesRegex(RuntimeError, 'netlink'):
                control.listeners(18080)

    def fixture(self, token='boot:1', eligible=True):
        return dict(pid=42, token=token, command='sshd: alice@notty', eligible=eligible)

    def test_inspection_never_sends_termination_signal(self):
        backend = Mock(open=Mock(return_value=55))
        with patch.object(control, 'listeners', return_value=([42], True)), \
             patch.object(control, 'session_info', return_value=self.fixture()), \
             patch.object(control, 'process_control', return_value=(backend, '')), \
             patch.object(control.os, 'close'):
            result = control.forward_control({'port': 18080})
            self.assertTrue(result['controllable'])
            backend.send.assert_called_once_with(55, 0)

    def test_ambiguous_business_or_missing_pidfd_cannot_be_forced(self):
        for pids, eligible, supported in [([42,43], True, True), ([], True, True), ([42], False, True), ([42], True, False)]:
            backend = Mock()
            with self.subTest(pids=pids, eligible=eligible, supported=supported), \
                 patch.object(control, 'listeners', return_value=(pids, True)), \
                 patch.object(control, 'session_info', return_value=self.fixture(eligible=eligible)), \
                 patch.object(control, 'process_control', return_value=(backend if supported else None, '')), \
                 patch.object(control.os, 'close'):
                with self.assertRaises(RuntimeError):
                    control.forward_control({'port': 18080, 'target': self.fixture()})
                backend.send.assert_not_called()

    def test_changed_identity_or_owner_refuses_signal(self):
        for infos, owners in [
            ([self.fixture('boot:2')], [([42], True)]),
            ([self.fixture(), self.fixture('boot:2')], [([42], True)]),
            ([self.fixture(), self.fixture()], [([42], True), ([43], True)]),
        ]:
            backend = Mock(open=Mock(return_value=55))
            with patch.object(control, 'listeners', side_effect=owners), \
                 patch.object(control, 'session_info', side_effect=infos), \
                 patch.object(control, 'process_control', return_value=(backend, '')), \
                 patch.object(control.os, 'close'):
                with self.assertRaises(RuntimeError):
                    control.forward_control({'port':18080, 'target':self.fixture()})
                self.assertTrue(all(c.args[1] == 0 for c in backend.send.call_args_list))

    def test_confirmed_target_sends_once_and_waits_for_release(self):
        backend = Mock(open=Mock(return_value=55))
        with patch.object(control, 'listeners', side_effect=[([42], True), ([42], True), ([], False)]), \
             patch.object(control, 'session_info', return_value=self.fixture()), \
             patch.object(control, 'process_control', return_value=(backend, '')), \
             patch.object(control.os, 'close'):
            self.assertTrue(control.forward_control({'port':18080, 'target':self.fixture()})['released'])
            self.assertEqual([c.args[1] for c in backend.send.call_args_list], [0, control.signal.SIGTERM])

    def test_invalid_port_does_not_query_remote(self):
        with patch.object(control, 'listeners') as listeners:
            for port in [0, -1, 65536, '80;rm']:
                with self.assertRaises((RuntimeError, ValueError)):
                    control.forward_control({'port': port})
            listeners.assert_not_called()

    def test_registration_identifies_ancestor_session_not_a_port_owner(self):
        with patch.object(control, 'namespace_safe', return_value=True), \
             patch.object(control.os, 'getppid', return_value=99), \
             patch.object(control, 'session_info', side_effect=[dict(eligible=False), self.fixture()]) as info, \
             patch('builtins.open', mock_open(read_data='PPid:\t42\n')), \
             patch.object(control, 'listeners') as listeners:
            self.assertEqual(control.forward_control({'register':True}), {'session':self.fixture()})
            self.assertEqual([c.args[0] for c in info.call_args_list], [99,42])
            listeners.assert_not_called()

    def test_registration_refuses_ambiguous_namespace(self):
        with patch.object(control, 'namespace_safe', return_value=False), \
             patch.object(control, 'session_info') as info:
            with self.assertRaisesRegex(RuntimeError, 'namespace'):
                control.forward_control({'register':True})
            info.assert_not_called()

    def test_automatic_recovery_never_signals_when_the_port_is_free(self):
        with patch.object(control, 'listeners', return_value=([],False)), \
             patch.object(control, 'process_control') as backend:
            result = control.forward_control({'port':18080,'automatic':True,'target':self.fixture()})
            self.assertFalse(result['occupied'])
            self.assertFalse(result['released'])
            backend.assert_not_called()

    def test_automatic_recovery_refuses_another_or_reused_session(self):
        backend = Mock(open=Mock(return_value=55))
        for changed in [dict(self.fixture(), pid=43), self.fixture('new-boot:1'), self.fixture('boot:2')]:
            with patch.object(control, 'listeners', return_value=([changed['pid']],True)), \
                 patch.object(control, 'session_info', return_value=changed), \
                 patch.object(control, 'process_control', return_value=(backend,'')), \
                 patch.object(control.os, 'close'):
                with self.assertRaisesRegex(RuntimeError,'identity'):
                    control.forward_control({'port':18080,'automatic':True,'target':self.fixture()})
                self.assertTrue(all(c.args[1]==0 for c in backend.send.call_args_list))

if __name__ == '__main__':
    unittest.main()
