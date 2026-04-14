import os
from pathlib import Path


def read_file(path: str) -> str:
    with open(path) as f:
        return f.read()


def ensure_dir(path: str):
    os.makedirs(path, exist_ok=True)


def resolve_path(base: str, relative: str) -> str:
    return str(Path(base) / relative)
