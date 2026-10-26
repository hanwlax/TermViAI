"""TermViAI's ephemeral, read-only NPU sampler; explicit stop payloads use pidfds.

Run over SSH with python3. No agent installation or files are written remotely.
The npu-smi table is parsed by column, never by assumed card/Die counts.
"""
import base64
import json
import os
import pwd
import re
import select
import shutil
import signal
import subprocess
import sys


def number(value):
    try:
        result = float(value)
        return result if 0 <= result < 1e15 else None
    except ValueError:
        return None


def parse_table(text):
    dies, processes = [], []
    card = None
    process_table = False
    single_die = False
    physical_ids = bool(re.search(r'Phy-?Id', text, re.IGNORECASE))
    memory_index = 0
    for line in text.splitlines():
        # Keep empty leading cells: 25.6 single-Die metric rows have blank IDs.
        cells = [c.strip() for c in line.split('|')[1:-1]]
        if not cells:
            continue
        if re.search(r'Process\s+id\b', line, re.IGNORECASE):
            process_table = True
            continue
        if re.search(r'AICore\(%\)|NPU Util\(%\)', line):
            memory_index = 1 if 'HBM-Usage' in line and 'Memory-Usage' in line else 0
        if cells[0] == 'NPU ID' and len(cells) > 1 and cells[1] == 'Name':
            single_die = True
            continue
        if process_table:
            tokens = cells[0].split()
            if (len(cells) >= 4 and tokens and all(t.isdigit() for t in tokens)
                    and len(tokens) == (1 if single_die else 2) and cells[1].isdigit()):
                npu, die = (int(tokens[0]), 0) if single_die else map(int, tokens)
                processes.append(dict(npu=npu, die=die, pid=int(cells[1]),
                                      name=cells[2][:256], memory_mb=number(cells[3])))
            continue
        if len(cells) < 3:
            continue
        metric_text = None
        if single_die:
            if cells[0].isdigit() and cells[1]:
                card = int(cells[0])
                continue
            if card is not None and not cells[0] and not cells[1] and len(cells) >= 4:
                npu, die = card, 0
                metric_text = ' '.join(cells[3:])
                card = None
        else:
            tokens = cells[0].split()
            if len(tokens) >= 2 and tokens[0].isdigit() and not tokens[1].isdigit():
                card = int(tokens[0])
                continue
            if (card is not None and tokens and len(tokens) <= 2
                    and all(t.isdigit() for t in tokens)
                    and re.search(r'[0-9a-fA-F]{2}:[0-9a-fA-F]{2}', cells[1])):
                # Chip + Phy-Id: retain the parent NPU and Chip/Die number.
                # Phy-Id is not the NPU ID used by the process table.
                npu = card if physical_ids or len(tokens) == 1 else int(tokens[0])
                die = int(tokens[0]) if physical_ids or len(tokens) == 1 else 0
                metric_text = ' '.join(cells[2:])
        if metric_text is not None:
            pairs = re.findall(r'(\d+|N/A|-)\s*/\s*(\d+|N/A|-)', metric_text)
            pair = pairs[memory_index] if len(pairs) > memory_index else None
            core = number(metric_text.split()[0]) if metric_text.split() else None
            dies.append(dict(npu=npu, die=die, used_mb=number(pair[0]) if pair else None,
                             total_mb=(number(pair[1]) or None) if pair else None,
                             core=core if core is not None and core <= 100 else None))
    ids = {(d['npu'], d['die']) for d in dies}
    if not dies or len(ids) != len(dies):
        raise RuntimeError('Unsupported npu-smi device table; no reliable topology was returned.')
    if any((p['npu'], p['die']) not in ids for p in processes):
        raise RuntimeError('npu-smi process IDs do not match the device topology.')
    # Missing process section is unknown, not an empty/idle device.
    return dict(dies=dies, processes=processes, processes_available=process_table)


def run_smi():
    binary = shutil.which('npu-smi')
    if not binary:
        for path in ['/usr/local/bin/npu-smi', '/usr/local/Ascend/driver/tools/npu-smi']:
            if os.path.isfile(path):
                binary = path
                break
    if not binary:
        raise RuntimeError('npu-smi is not installed or not accessible to this SSH user.')
    result = subprocess.run([binary, 'info'], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=12, env=dict(os.environ, LC_ALL='C'), encoding='utf-8', errors='replace')
    if result.returncode:
        raise RuntimeError('npu-smi failed: ' + (result.stderr or result.stdout)[-1000:])
    if len(result.stdout) > 2_000_000:
        raise RuntimeError('npu-smi output exceeds the sampling limit.')
    return parse_table(result.stdout)


def identity(pid):
    with open('/proc/%d/stat' % pid) as f:
        # comm may contain spaces and parentheses. starttime is field 22.
        start = f.read().rsplit(')', 1)[1].split()[19]
    with open('/proc/sys/kernel/random/boot_id') as f:
        boot = f.read().strip()
    return boot + ':' + start


def namespace_safe():
    # npu-smi can expose host PIDs inside a container. Never signal that namespace.
    if os.path.exists('/.dockerenv') or os.path.exists('/run/.containerenv'):
        return False
    with open('/proc/self/status') as f:
        for line in f:
            if line.startswith('NSpid:') and len(line.split()) > 2:
                return False
    with open('/proc/self/cgroup') as f:
        if re.search(r'docker|kubepods|containerd|lxc', f.read()):
            return False
    return True


class Pidfd:
    """Prefer Python bindings; support builds made with older Linux headers.

    Both backends retain pidfd identity semantics. Never fall back to kill(pid).
    Raw syscall numbers are only used for verified Linux 64-bit ABIs:
    x86_64 syscall_64.tbl and aarch64 asm-generic/unistd.h (Linux v5.10).
    """
    def __init__(self):
        self.native_open = getattr(os, 'pidfd_open', None)
        self.native_send = getattr(signal, 'pidfd_send_signal', None)
        self.libc = None
        if self.native_open is not None and self.native_send is not None:
            return
        import ctypes
        import platform
        if sys.platform != 'linux':
            raise RuntimeError('Process control requires Linux pidfd support.')
        self.ctypes = ctypes
        self.libc = ctypes.CDLL(None, use_errno=True)
        self.libc_open = getattr(self.libc, 'pidfd_open', None)
        self.libc_send = getattr(self.libc, 'pidfd_send_signal', None)
        if self.libc_open is not None:
            self.libc_open.argtypes = [ctypes.c_int, ctypes.c_uint]
            self.libc_open.restype = ctypes.c_int
        if self.libc_send is not None:
            self.libc_send.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint]
            self.libc_send.restype = ctypes.c_int
        if ((self.native_open is None and self.libc_open is None)
                or (self.native_send is None and self.libc_send is None)):
            if (platform.machine().lower() not in ('x86_64', 'amd64', 'aarch64', 'arm64')
                    or ctypes.sizeof(ctypes.c_void_p) != 8 or ctypes.sizeof(ctypes.c_long) != 8):
                raise RuntimeError('This Python/libc lacks pidfd bindings for this CPU ABI.')
            self.libc.syscall.restype = ctypes.c_long

    def checked(self, result):
        if result == -1:
            code = self.ctypes.get_errno()
            raise OSError(code, os.strerror(code))
        return result

    def open(self, pid):
        if self.native_open is not None:
            return self.native_open(pid, 0)
        if self.libc_open is not None:
            return self.checked(self.libc_open(pid, 0))
        c = self.ctypes
        return self.checked(self.libc.syscall(c.c_long(434), c.c_int(pid), c.c_uint(0)))

    def send(self, fd, sig):
        if self.native_send is not None:
            return self.native_send(fd, sig, None, 0)
        if self.libc_send is not None:
            return self.checked(self.libc_send(fd, sig, None, 0))
        c = self.ctypes
        return self.checked(self.libc.syscall(c.c_long(424), c.c_int(fd), c.c_int(sig),
                                             c.c_void_p(), c.c_uint(0)))


def process_control():
    """Probe the actual kernel, not Python's version or attribute presence.

    Signal 0 checks support/permission for this collector only; no signal is sent.
    """
    import errno
    fd = None
    try:
        if not namespace_safe():
            return None, 'Process control is disabled in a container/PID namespace; connect to the host.'
        backend = Pidfd()
        fd = backend.open(os.getpid())
        backend.send(fd, 0)
        return backend, ''
    except OSError as error:
        if error.errno == errno.ENOSYS:
            return None, 'The host kernel does not provide the required pidfd system calls (or they are filtered).'
        if error.errno in (errno.EPERM, errno.EACCES):
            return None, 'Host security policy denied the process-control capability check: ' + str(error)
        return None, 'Process-control capability check failed: ' + str(error)
    except (ImportError, AttributeError, RuntimeError) as error:
        return None, 'Process-control compatibility unavailable: ' + str(error)
    finally:
        if fd is not None:
            os.close(fd)


def sample():
    data = run_smi()
    backend, data['stop_reason'] = process_control()
    safe = backend is not None
    details = {}
    for process in data['processes']:
        pid = process['pid']
        if pid not in details:
            item = dict(token='', command='', cwd='', owner='', controllable=False)
            try:
                token = identity(pid)
                # Equivalent to pwdx, without depending on the remote pwdx utility.
                # A restricted or vanished cwd must not discard other metadata.
                try:
                    cwd = os.readlink('/proc/%d/cwd' % pid)
                except OSError:
                    cwd = ''
                uid = os.stat('/proc/%d' % pid).st_uid
                item['owner'] = pwd.getpwuid(uid).pw_name
                with open('/proc/%d/cmdline' % pid, 'rb') as f:
                    item['command'] = f.read(8192).replace(b'\0', b' ').decode('utf-8', 'replace').strip()
                if token == identity(pid):
                    item['cwd'] = cwd
                    item['token'] = token
                    item['controllable'] = safe and pid > 1 and (os.geteuid() == 0 or uid == os.geteuid())
            except (OSError, KeyError, IndexError):
                pass
            details[pid] = item
        process.update(details[pid])
    return data


def terminate(targets):
    backend, reason = process_control()
    if backend is None:
        raise RuntimeError(reason)
    live = run_smi()
    if not live['processes_available']:
        raise RuntimeError('Cannot verify current NPU process membership.')
    members = {p['pid'] for p in live['processes']}
    if not targets or len({t.get('pid') for t in targets}) != len(targets):
        raise RuntimeError('Empty or duplicate process selection.')
    if any(type(t.get('pid')) is not int or t['pid'] <= 1 or not isinstance(t.get('token'), str) or not t['token'] for t in targets):
        raise RuntimeError('Invalid process identity in confirmation.')
    result, seen = [], set()
    for target in targets:
        pid, token = target['pid'], target['token']
        if type(pid) is not int or pid <= 1 or pid in seen or not isinstance(token, str) or not token:
            raise RuntimeError('Invalid process identity in confirmation.')
        seen.add(pid)
        fd = None
        try:
            if pid not in members:
                raise RuntimeError('No longer an NPU process; skipped')
            fd = backend.open(pid)
            if identity(pid) != token:
                raise RuntimeError('Process identity changed; skipped')
            backend.send(fd, signal.SIGTERM)
            exited = bool(select.select([fd], [], [], 0)[0])
            status = 'Exited' if exited else 'Exit requested; waiting for process'
        except (OSError, RuntimeError) as error:
            status = str(error)
        finally:
            if fd is not None:
                os.close(fd)
        result.append(dict(pid=pid, status=status))
    return result


def main():
    try:
        request = json.loads(base64.b64decode(sys.argv[1])) if len(sys.argv) > 1 else {}
        payload = {'results': terminate(request['targets'])} if 'targets' in request else {'snapshot': sample()}
        print('TERMVIAI_MONITOR_V1=' + json.dumps(payload, ensure_ascii=True))
    except Exception as error:
        print('TERMVIAI_MONITOR_V1=' + json.dumps({'error': str(error)[:2000]}))


if __name__ == '__main__':
    main()
