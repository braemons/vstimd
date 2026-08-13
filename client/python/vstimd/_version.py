"""Single source of truth for the package version.

Kept import-free so setuptools can read it statically at build time
(`[tool.setuptools.dynamic]`) without importing the package — which would
require pyzmq and the generated protobuf stubs to already be present.

The client version is independent of the server's: the server's comes from the
git tag at compile time, and the two are released on their own cadences.
"""

__version__ = "0.1.0rc1"
