"""Compatibilidade com imports anteriores; regras agora pertencem ao Earthion."""
from .validators.earthion import Analyzer, hms, log

__all__ = ["Analyzer", "hms", "log"]
