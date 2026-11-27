"""Screen-free mode: the modules that touch the screen are never loaded.

``tools/civ6_hotseat.py --manual-lobby`` is for people who are not this
project's operator. They cannot be asked for Screen Recording or Accessibility
permission, Pillow, ``cliclick`` or a Swift compiler, and the screen automation
is macOS-only, so in that mode nothing may click, type, capture, OCR or move a
window. Saying so in each function is a promise; this makes it a fact.

``engage()`` puts a finder at the front of ``sys.meta_path``. From then on each
module in ``SCREEN_MODULES`` imports as a stand-in whose every attribute is a
``Refused`` object: binding it, or reading a constant from it at import time
(``civ6_play`` reads ``popup_clear.GAME_PROCESS`` that way), costs nothing,
and calling anything on it raises ``ScreenAutomationOff``. Pillow is refused
outright, so a ``try: from PIL import Image`` reads exactly as a machine
without Pillow. The real modules are never imported, so their Swift helpers
are never compiled and ``cliclick`` is never looked for.

The switch is the ``CIVVIS_SCREEN_FREE`` environment variable, read by
``civ6_control/__init__.py`` before any submodule loads, so it covers the
harness and every decision worker it starts (children inherit it). Unset --
every existing caller -- nothing here runs.
"""

from __future__ import annotations

import importlib.abc
import importlib.machinery
import os
import sys
import types

ENV = "CIVVIS_SCREEN_FREE"

#: Everything that clicks, types, captures, reads the screen or moves a window,
#: and everything that imports those at load (`popup_clear`, `vision`).
SCREEN_MODULES = (
    "civ6_control.macos_capture",
    "civ6_control.macos_input",
    "civ6_control.macos_ocr",
    "civ6_control.macos_window",
    "civ6_control.popup_clear",
    "civ6_control.vision",
)
#: Refused as absent, not stood in for: callers already handle "no Pillow".
REFUSED_PACKAGES = ("PIL",)


class ScreenAutomationOff(RuntimeError):
    """Something tried to use the screen in screen-free mode."""


class Refused:
    """Any name read off a stand-in module: inert until used."""

    def __init__(self, name: str):
        object.__setattr__(self, "_name", name)

    def __call__(self, *_args, **_kwargs):
        raise ScreenAutomationOff(f"{self._name}: screen automation is off in this mode")

    def __getattr__(self, attr: str):
        if attr.startswith("__"):
            raise AttributeError(attr)
        return Refused(f"{self._name}.{attr}")

    def __bool__(self) -> bool:
        raise ScreenAutomationOff(f"{self._name}: screen automation is off in this mode")

    def __repr__(self) -> str:
        return f"<refused {self._name}>"


class StandIn(types.ModuleType):
    """What a screen module imports as while the guard is engaged."""

    def __getattr__(self, attr: str):
        if attr.startswith("__"):
            raise AttributeError(attr)
        return Refused(f"{self.__name__}.{attr}")


class _Finder(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    def find_spec(self, name, path=None, target=None):  # noqa: ANN001
        if any(name == p or name.startswith(p + ".") for p in REFUSED_PACKAGES):
            raise ModuleNotFoundError(f"{name} is not loaded in screen-free mode", name=name)
        if name in SCREEN_MODULES:
            return importlib.machinery.ModuleSpec(name, self)
        return None

    def create_module(self, spec):  # noqa: ANN001
        return StandIn(spec.name)

    def exec_module(self, module) -> None:  # noqa: ANN001
        pass


def engaged() -> bool:
    return any(isinstance(finder, _Finder) for finder in sys.meta_path)


def loaded_for_real() -> list[str]:
    """Screen modules (or Pillow) imported as themselves in this process."""
    real = [name for name in SCREEN_MODULES
            if name in sys.modules and not isinstance(sys.modules[name], StandIn)]
    real += [name for name in sys.modules
             if any(name == p or name.startswith(p + ".") for p in REFUSED_PACKAGES)]
    return real


def engage() -> None:
    """Refuse the screen modules from now on, here and in every child process.

    Raises if one was already imported for real: screen-free must hold from
    the first import, or it is not a construction but a hope.
    """
    real = loaded_for_real()
    if real:
        raise RuntimeError(f"screen-free mode engaged after {', '.join(real)} loaded")
    os.environ[ENV] = "1"
    if not engaged():
        sys.meta_path.insert(0, _Finder())
