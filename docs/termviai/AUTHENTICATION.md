# Terminal authentication recovery

The existing Edit Host form and its Password / SSH Key / Identity controls are
unchanged. Credentials entered in the different sections are retained as separate
Host slots. Selecting one section no longer removes the others.

Terminal connections try configured credentials in this order:

1. SSH key: the selected Keychain entry or explicit private-key path.
2. Identity: the selected Keychain identity's username and password.
3. Manual username and saved password.

Unavailable references and rejected credentials move to the next alternative.
Each alternative uses a fresh SSH transport, so changing usernames does not reuse
an already authenticated session or exhaust one transport's MaxAuthTries budget.
Network errors and host fingerprint mismatches are reported separately; they do
not become misleading password failures. Existing agent/default-key discovery
remains available for legacy quick connections without explicit credentials.

When no alternative succeeds, the same terminal pane displays the recovery page.
Choose Password, SSH Key (Keychain or path), or Identity and Continue & Save.
Encrypted-key passphrases and additional authentication challenges are entered
inside this pane. Other panes remain usable and broadcast skips unauthenticated
panes. The recovery page scrolls within its pane, with a fixed action footer.

After SSH accepts the selection, only that Host slot is updated. The selected
editor section follows the successful method. Other Host credentials, shared
Keychain keys and shared identities are not changed. Manual and key usernames
are stored independently. Passwords are staged in memory during attempts and
persisted through Windows Credential Manager only after acceptance; host JSON
contains references, not passwords. Failed attempts do not overwrite saved data.
A concurrent Host change or save failure leaves the connection usable and shows
a pane-local error with Retry saving. An unsuccessful authentication cannot save.

This recovery workflow applies to interactive terminal connections. Monitor and
Port Forwarding retain their existing authentication UX; they do not present a
terminal recovery page.

## Regression checks

Run on the Windows release toolchain:

```
cargo test --locked --release -p mux --lib ssh:: -- --test-threads=1
cargo test --locked --release -p wezterm-ssh --lib -- --test-threads=1
cargo test --locked --release -p wezterm-gui -- --test-threads=1
cargo build --locked --release -p wezterm-gui
```

A synthetic SSH integration test requires Python with `paramiko`, a disposable
fixture directory, and a free loopback port 22371. Start the fixture on the same
computer as the test:

```
python mux/tests/fixtures/authentication_server.py <fixture-directory>
```

Set `TERMVIAI_AUTH_FIXTURE` to that directory and run:

```
cargo test --locked --release -p mux --lib loopback_authentication_recovery -- --ignored --test-threads=1
```

The fixture listens only on 127.0.0.1 and generates synthetic keys/passwords. The
test runs against both libssh and libssh2. It checks early success, Key rejection followed by Identity success, both rejected
followed by manual success, and all rejected followed by encrypted-key recovery.
Stop it after testing. Never use its test credentials or host keys in production.

Manual trial checks: compare the approved terminal-auth UX at Windows 100% DPI;
check grouped panes, zoom, narrow/short windows, scrolling, tab navigation and
three themes; verify a real host's key, identity, password and MFA behavior.
Automated layout checks do not establish native pixel-for-pixel visual parity.
