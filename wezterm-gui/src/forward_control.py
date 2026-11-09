"""Linux-only conflict inspection; explicit confirmation pins an SSH session by pidfd.
The shared process helpers are loaded without running the NPU sampler.
"""
def listeners(port):
    result = subprocess.run(['ss', '-H', '-ltnp', 'sport = :%d' % port],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            encoding='utf-8', errors='replace', timeout=5)
    if result.returncode or result.stderr.strip():
        raise RuntimeError(result.stderr.strip() or 'Cannot inspect remote listeners')
    return sorted(set(int(v) for v in re.findall(r'pid=(\d+)', result.stdout))), bool(result.stdout.strip())


def session_info(pid):
    token = identity(pid)
    with open('/proc/%d/cmdline' % pid, 'rb') as f:
        cmd = f.read(8192).replace(b'\0', b' ').decode('utf-8', 'replace').strip()
    exe = os.path.basename(os.readlink('/proc/%d/exe' % pid))
    # Never select the sshd daemon, terminal sessions, or arbitrary business processes.
    # OpenSSH's pure -N forwarder is named "sshd: user"; an exec session
    # without a PTY is "sshd: user@notty". Exclude [priv]/[listener] and PTYs.
    eligible = exe in ('sshd', 'sshd-session') and re.fullmatch(r'sshd(?:-session)?: [^\s@\[\]]+(?:@notty)?', cmd) is not None
    if token != identity(pid):
        raise RuntimeError('Listener changed during inspection; inspect again')
    return dict(pid=pid, token=token, command=cmd, eligible=eligible)


def forward_control(request):
    if request.get('register'):
        if not namespace_safe():
            raise RuntimeError('Cannot establish the host SSH session identity in this PID namespace')
        pid = os.getppid()
        for _ in range(16):
            info = session_info(pid)
            if info['eligible']:
                return {'session': info}
            with open('/proc/%d/status' % pid) as f:
                pid = int(next(v.split()[1] for v in f if v.startswith('PPid:')))
            if pid <= 1:
                break
        raise RuntimeError('Cannot identify this dedicated SSH session')
    port = int(request['port'])
    if not 1 <= port <= 65535:
        raise RuntimeError('Invalid port')
    pids, occupied = listeners(port)
    if request.get('automatic') and not occupied:
        return dict(released=False, occupied=False, controllable=False, reason='Port is free', owner=None)
    backend, reason = process_control()
    info = None
    if len(pids) == 1:
        try:
            info = session_info(pids[0])
            if info['eligible'] and backend:
                fd = backend.open(info['pid'])
                try:
                    backend.send(fd, 0)
                finally:
                    os.close(fd)
        except Exception as error:
            reason = str(error)
            backend = None
    if info is None:
        reason = reason or ('Multiple or hidden listener owners; choose another port.' if occupied else 'No listener found; check SSH forwarding policy.')
    controllable = bool(info and info['eligible'] and backend)
    if request.get('target'):
        target = request['target']
        if not controllable or info['pid'] != target['pid'] or info['token'] != target['token']:
            raise RuntimeError('Listener identity or permission changed; inspect again')
        fd = backend.open(info['pid'])
        try:
            current = session_info(info['pid'])
            if current['token'] != target['token'] or not current['eligible'] or listeners(port)[0] != [info['pid']]:
                raise RuntimeError('Listener changed; no signal sent')
            backend.send(fd, signal.SIGTERM)
        finally:
            os.close(fd)
        import time
        for _ in range(20):
            if not listeners(port)[1]:
                return dict(released=True, occupied=False, controllable=False, reason='Port released', owner=None)
            time.sleep(.1)
        raise RuntimeError('Termination requested, but the port is still occupied. Inspect before retrying; no request was repeated.')
    return dict(released=False, occupied=occupied, controllable=controllable,
                reason=reason or ('' if controllable else 'This listener is not an eligible SSH session.'), owner=info)


def main():
    try:
        print('TERMVIAI_FORWARD_V1=' + json.dumps(forward_control(json.loads(base64.b64decode(sys.argv[1])))))
    except Exception as error:
        print('TERMVIAI_FORWARD_V1=' + json.dumps({'error': str(error)[:2000]}))


if __name__ == '__main__':
    main()
