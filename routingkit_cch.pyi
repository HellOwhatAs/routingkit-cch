class CCH:
    def __init__(
        self,
        order: list[int],
        tail: list[int],
        head: list[int],
        filter_always_inf_arcs: bool,
    ) -> None: ...

class CCHMetric:
    def __init__(
        self,
        cch: CCH,
        weights: list[int],
    ) -> None:
        self.weights: list[int]

    def reset(self, weights: list[int]) -> None:
        """Rebind the metric to a new weight vector and re-customize,
        reusing internal buffers (cheaper than building a new CCHMetric).
        Requires that no CCHQuery / CCHOneToMany / CCHManyToOne is alive."""

class CCHMetricPartialUpdater:
    def __init__(self, cch: CCH) -> None: ...
    def apply(self, metric: CCHMetric, updates: dict[int, int]) -> None: ...

class CCHQueryResult:
    distance: int | None
    node_path: list[int]
    arc_path: list[int]

class CCHOneToMany:
    """One-to-many query with a fixed (pinned) set of targets.
    Answers distances from a source to all pinned targets in one sweep."""

    target_count: int
    def __init__(self, metric: CCHMetric, targets: list[int]) -> None: ...
    def repin_targets(self, targets: list[int]) -> None:
        """Replace the pinned target set, reusing internal buffers
        (cheaper than constructing a new CCHOneToMany)."""

    def distances_from(self, source: int) -> list[int | None]:
        """Distances from `source` to every pinned target (None = unreachable),
        aligned with the pinned target order."""

    def distances_from_multi(
        self, sources: list[tuple[int, int]]
    ) -> list[int | None]:
        """Multi-source variant taking `(source, initial_dist)` pairs."""

class CCHManyToOne:
    """Many-to-one query with a fixed (pinned) set of sources.
    Answers distances from all pinned sources to a target in one sweep."""

    source_count: int
    def __init__(self, metric: CCHMetric, sources: list[int]) -> None: ...
    def repin_sources(self, sources: list[int]) -> None:
        """Replace the pinned source set, reusing internal buffers
        (cheaper than constructing a new CCHManyToOne)."""

    def distances_to(self, target: int) -> list[int | None]:
        """Distances from every pinned source to `target` (None = unreachable),
        aligned with the pinned source order."""

    def distances_to_multi(self, targets: list[tuple[int, int]]) -> list[int | None]:
        """Multi-target variant taking `(target, initial_dist)` pairs."""

class CCHQuery:
    def __init__(self, metric: CCHMetric) -> None: ...
    def run(self, source: int, target: int) -> CCHQueryResult: ...
    def run_multi(
        self, sources: list[tuple[int, int]], targets: list[tuple[int, int]]
    ) -> CCHQueryResult:
        """run query with multiple sources and targets and distances."""

def compute_order_degree(
    node_count: int, tail: list[int], head: list[int]
) -> list[int]: ...
def compute_order_inertial(
    node_count: int,
    tail: list[int],
    head: list[int],
    latitude: list[float],
    longitude: list[float],
) -> list[int]: ...
