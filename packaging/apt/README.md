# apt archive

Deployed rigs update with `apt update && apt upgrade` rather than being
re-flashed. Re-flashing discards `/etc/braemons` — the rig config and any saved
stimulus configs — plus `/var/lib/braemons`; an upgrade preserves them, because
`vstimd-rig-config.toml` is a dpkg conffile.

The release workflow builds a signed archive from the `.debs` it already
produces and publishes it to the `gh-pages` branch, served at
`https://braemons.github.io/vstimd/apt`. The output is a plain static tree, so
the same bytes work from GitHub Pages, an nginx docroot, or an `rsync` copy on a
lab server for rigs with no internet access.

Pre-releases (tags with a hyphen, e.g. `v0.2.0-alpha1`) publish to the
**`testing`** suite; plain tags publish to **`stable`**. Suites have separate
pools, so a rig tracking `stable` is never upgraded onto an alpha.

## One-time setup

Three manual steps are needed before the first publish. Until they are done the
`apt-repo` job fails loudly and images ship with no update source — nothing
silently publishes unsigned.

### 1. Generate a signing key

CI cannot type a passphrase, so the key must have none. Keep it for the life of
the archive: changing it means every deployed rig needs the new public key.

```bash
gpg --batch --passphrase '' --quick-generate-key \
    'braemons vstimd archive <you@example.org>' rsa4096 sign never
```

### 2. Store the private key as a repository secret

```bash
KEYID=$(gpg --list-secret-keys --with-colons | awk -F: '/^sec:/ {print $5; exit}')
gpg --armor --export-secret-keys "$KEYID" | gh secret set APT_SIGNING_KEY
```

### 3. Commit the public key

The image installs this into `/etc/apt/keyrings/braemons.asc` so a flashed rig
trusts the archive out of the box. It is public by definition — committing it is
correct.

```bash
gpg --armor --export "$KEYID" > packaging/apt/braemons-archive-keyring.asc
git add packaging/apt/braemons-archive-keyring.asc
git commit -m "packaging(apt): add the archive signing key"
```

Then enable GitHub Pages for the repository (Settings → Pages → deploy from
branch `gh-pages`, folder `/`). The branch is created by the first `apt-repo`
run, so publish once before enabling.

## Rig setup

Images built after the public key is committed are configured already. To point
an existing rig at the archive by hand:

```bash
sudo install -D -m 0644 braemons-archive-keyring.asc /etc/apt/keyrings/braemons.asc
sudo tee /etc/apt/sources.list.d/braemons.sources >/dev/null <<'EOF'
Types: deb
URIs: https://braemons.github.io/vstimd/apt
Suites: stable
Components: main
Architectures: arm64
Signed-By: /etc/apt/keyrings/braemons.asc
EOF
sudo apt update && sudo apt upgrade
```

Switch `Suites:` to `testing` to track pre-releases.

## Rigs with no internet

The archive is static files, so mirror it onto a lab server and point the closed
rigs there:

```bash
wget -mnH --cut-dirs=1 -R 'index.html*' https://braemons.github.io/vstimd/apt/
rsync -a apt/ lab-server:/var/www/html/vstimd-apt/
```

Rigs then use `URIs: http://lab-server/vstimd-apt`. Signatures still verify —
they cover the archive contents, not where it was fetched from — so the mirror
needs no key of its own.

## Building the archive locally

```bash
make deb                                   # produce the .debs
APT_SIGNING_KEY="$(gpg --armor --export-secret-keys "$KEYID")" make apt-repo
```

Output lands in `dist/apt-repo`. Without `APT_SIGNING_KEY` the archive is built
unsigned, which apt only accepts with `[trusted=yes]` — usable for a local test,
never for anything a rig points at. Override the suite with
`make apt-repo APT_SUITE=stable`.
