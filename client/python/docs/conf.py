import sys
import os

sys.path.insert(0, os.path.abspath(".."))

from vstimd._version import __version__  # noqa: E402  (needs the sys.path entry above)

project = "vstimd-client"
author = "Joscha Schmiedt"
release = __version__

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx_autodoc_typehints",
    "myst_parser",
    "sphinx_copybutton",
]

html_theme = "furo"
html_title = "vstimd-client"

autodoc_default_options = {
    "members": True,
    "undoc-members": True,
    "show-inheritance": True,
}
autodoc_typehints = "description"
autodoc_member_order = "bysource"
autodoc_type_aliases = {
    "StimulusHandle": "vstimd.StimulusHandle",
    "AnimationHandle": "vstimd.AnimationHandle",
}

myst_enable_extensions = ["colon_fence"]

exclude_patterns = ["_build"]
