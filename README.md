# Psst

A not-so-ugly replacement for your pinentry and keyring prompter for wayland using a layer-shell overlay.

<p align="center">
  <img src="assets/demo.png" alt="psst asking for a new keyring password" width="520">
</p>

Psst provides:

- **`psst-pinentry`**: the dialog GnuPG uses to ask for your key passphrase or smartcard PIN (a *pinentry* program for `gpg-agent`).
- **`psst-keyring-prompter`**: the dialog that unlocks your GNOME keyring, replacing the default gnome keyring prompt.

## Setup

Build the programs:

```sh
cargo build --release
```

All binaries land in `target/release/`.


### GnuPG

Update your `gpg-agent` configuration to use `psst-pinentry` as your pinentry program. To do that, add the following to `~/.gnupg/gpg-agent.conf`:

```sh
pinentry-program /path/to/psst/target/release/psst-pinentry
```

<p align="center">
  <img src="assets/gnupg.png" alt="psst-pinentry unlocking a smartcard key" width="480">
</p>

### GNOME Keyring

To use `psst-keyring-prompter`, add the following to your compositor's autostart:

```sh
psst-keyring-prompter
```

and reload your agent:

```sh
gpg-connect-agent reloadagent /bye
```

It takes over keyring unlock prompts for as long as it's running.

<p align="center">
  <img src="assets/unlock_keyring.png" alt="psst unlocking the GNOME keyring" width="480">
  <img src="assets/new_keyring.png" alt="psst creating a new keyring" width="480">
</p>

## Theming

Every color, font, size, border, and radius is themeable through a [KDL](https://kdl.dev) file at `~/.config/psst/theme.kdl` (or `$XDG_CONFIG_HOME/psst/theme.kdl`). Anything you omit keeps its default, and an invalid theme is ignored with a warning rather than blocking a prompt.

The [`default-theme.kdl`](crates/theme/src/default-theme.kdl) file lists every available
option at its default value, copy it to `~/.config/psst/theme.kdl` and edit to taste.

## License

[MPL-2.0](LICENSE). You can use psst in anything, but if you redistribute the
covered files (modified or not), their source must stay open under the MPL.
