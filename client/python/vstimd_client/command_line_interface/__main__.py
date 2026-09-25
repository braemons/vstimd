"""Allow ``python -m vstimd_client.command_line_interface`` as well as ``vstimctl``."""
from .main import main

raise SystemExit(main())
