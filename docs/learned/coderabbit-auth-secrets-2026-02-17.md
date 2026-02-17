## CodeRabbit CLI auth failure: ERR_SECRETS_PLATFORM_ERROR (Linux Secret Service)

Date: 2026-02-17

Observed error during `coderabbit auth login`:
- `Authentication failed: unable to store credentials in secured storage`
- `Error [ERR_SECRETS_PLATFORM_ERROR]: The name is not activatable (code: 2)`

Likely cause:
- No active Secret Service provider on the session D-Bus for `org.freedesktop.secrets`.

Key references:
- Secret Service API name/path (`org.freedesktop.secrets`, `/org/freedesktop/secrets`):  
  https://specifications.freedesktop.org/secret-service-spec/latest-single/
- `gnome-keyring-daemon` supports the `secrets` component:  
  https://man.archlinux.org/man/gnome-keyring-daemon.1.en
- KeePassXC can provide Secret Service and may replace other providers on D-Bus:  
  https://keepassxc.org/docs/KeePassXC_UserGuide#_freedesktop_secret_service_integration
- CodeRabbit CLI docs (auth command flow; WSL context):  
  https://docs.coderabbit.ai/cli/installation/wsl

Practical remediation:
- Install/start a Secret Service provider (`gnome-keyring` or KeePassXC Secret Service).
- Ensure a user session D-Bus exists.
- Retry `coderabbit auth login` after provider is active.
