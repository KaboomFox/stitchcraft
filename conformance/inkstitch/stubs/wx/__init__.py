"""A stand-in for wxPython, the toolkit of Ink/Stitch's dialogs, for the Ink/Stitch differential.

Ink/Stitch's extensions import wxPython as its package loads, and writing a machine file never opens a
window. So every name resolves to an inert placeholder: a class that takes any arguments, any attribute and
any operation, and any `wx.*` submodule is a module of such names. `cargo xtask conformance --differential`
puts this folder first on Ink/Stitch's module path. Written for StitchCraft, under its licence.
"""
import importlib.abc
import importlib.machinery
import sys
import types


class _Inert:
    def __init__(self, *args, **kwargs):
        pass

    def __call__(self, *args, **kwargs):
        return _Inert()

    def __getattr__(self, name):
        return _Inert()

    def __iter__(self):
        return iter((_Inert, _Inert))

    def __or__(self, other):
        return 0

    __ror__ = __and__ = __rand__ = __add__ = __radd__ = __or__

    def __bool__(self):
        return False

    def __int__(self):
        return 0


class _Meta(type):
    def __getattr__(cls, name):
        return _Inert()

    def __or__(cls, other):
        return 0

    __ror__ = __or__

    def __iter__(cls):
        return iter((_Inert, _Inert))


class Inert(_Inert, metaclass=_Meta):
    pass


_MADE = {}


def _named(module, name):
    """A placeholder class of its own for each name, so that a class may derive from several."""
    key = (module, name)
    if key not in _MADE:
        _MADE[key] = _Meta(name, (Inert,), {})
    return _MADE[key]


def __getattr__(name):
    if name.startswith("__"):
        raise AttributeError(name)
    return _named("wx", name)


class _Finder(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    """Finds every `wx.*` submodule, as a module of placeholders."""

    def find_spec(self, fullname, path, target=None):
        if fullname.startswith("wx."):
            return importlib.machinery.ModuleSpec(fullname, self, is_package=True)
        return None

    def create_module(self, spec):
        module = types.ModuleType(spec.name)
        module.__path__ = []

        def attribute(name, _module=spec.name):
            if name.startswith("__"):
                raise AttributeError(name)
            return _named(_module, name)

        module.__getattr__ = attribute
        return module

    def exec_module(self, module):
        pass


sys.meta_path.insert(0, _Finder())
