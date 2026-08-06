# apt archive signing key

The archive itself lives in **[braemons/packages](https://github.com/braemons/packages)**,
served at <https://braemons.github.io/packages/>. It is shared across braemons
daemons rather than being per-project: one `sources.list` entry and one key on a
rig, however many daemons it runs.

This directory holds only the **public** half of the signing key, because the SD
image needs it at build time. `build-sd-image.sh` installs it to
`/etc/apt/keyrings/braemons.asc` so a freshly flashed rig trusts the archive
without any manual step. Kept ASCII-armored, which apt reads directly —
dearmoring would need `gpg(1)`, not guaranteed present on a Lite image.

Fingerprint `0435E6ED C19F085E F0F62F22 A2BCE0FF 045159C5`.

If the file is absent the image build prints `apt updates: DISABLED` and
continues, producing an image with no update source rather than one that trusts
an archive it cannot verify.

## vstimd publishes nothing

Releases here just attach `.deb`s to the GitHub Release. `braemons/packages`
pulls them in on a schedule and republishes the signed archive, so this repo
holds no signing key and needs no cross-repo credentials. Nothing to do at
release time beyond tagging.

To publish immediately rather than waiting for the schedule:

```bash
gh workflow run publish.yml --repo braemons/packages
```

## Rig setup

Images built from a recent release are configured already; see
[Deployment](../../docs/operations/deployment.md#updating-a-deployed-rig). For an
existing rig, or for rigs on a closed lab network, the instructions are in the
[archive README](https://github.com/braemons/packages#using-it).

Pre-releases land in the `testing` suite and plain releases in `stable`, chosen
from the `~` in the version — so a rig tracking `stable` is never offered an
alpha. Images built from a pre-release track `testing` to match.
