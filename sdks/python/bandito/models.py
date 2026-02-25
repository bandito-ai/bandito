"""SDK types: Arm, PullResult, and internal cache structures."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class Arm:
    """An arm returned to the user after pull(). Frozen for safety."""

    arm_id: int
    model_name: str
    model_provider: str
    system_prompt: str
    is_prompt_templated: bool

    @property
    def model(self) -> str:
        """Convenience alias for model_name."""
        return self.model_name

    @property
    def prompt(self) -> str:
        """Convenience alias for system_prompt."""
        return self.system_prompt


@dataclass(frozen=True)
class PullResult:
    """Returned by pull(), passed to update(). Frozen."""

    arm: Arm
    event_id: str
    bandit_id: int
    bandit_name: str
    scores: dict[int, float]
    _pull_time: float = 0.0  # perf_counter timestamp, internal

    @property
    def model(self) -> str:
        """Reach-through to arm.model_name."""
        return self.arm.model_name

    @property
    def prompt(self) -> str:
        """Reach-through to arm.system_prompt."""
        return self.arm.system_prompt


@dataclass
class _BanditCache:
    """Internal mutable cache for a bandit's state.

    Math state (theta, chol, feature matrix) now lives in the Rust
    BanditEngine. This cache only holds metadata needed by the Python
    client for arm lookup, budget warnings, and latency reporting.
    """

    bandit_id: int
    name: str
    arms: list[Arm]
    optimization_mode: str
    avg_latency_last_n: float | None
    budget: float | None = None
    total_cost: float | None = None
