# routingkit-cch

[![Crates.io](https://img.shields.io/crates/v/routingkit-cch.svg)](https://crates.io/crates/routingkit-cch)
[![Pypi](https://img.shields.io/pypi/v/routingkit-cch.svg)](https://pypi.org/project/routingkit-cch/)
[![Docs](https://img.shields.io/docsrs/routingkit-cch)](https://docs.rs/routingkit-cch/)
[![License](https://img.shields.io/crates/l/routingkit-cch)](https://github.com/RoutingKit/RoutingKit/blob/master/LICENSE)

Rust/Python bindings for the Customizable Contraction Hierarchies (CCH) implementation from [RoutingKit](https://github.com/RoutingKit/RoutingKit). CCH is a three‑phase shortest path acceleration technique for large directed graphs (e.g. road networks) that allows fast re-weighting while keeping very low query latency.

## Why CCH?
CustomizableContractionHierarchies (CCH) are an index-based speedup technique for shortest paths in directed graphs that can quickly be adapted to new weights. CCHs use, contrary to regulars CHs, a three phase setup:

1. Preprocessing
2. Customization
3. Query

The preprocessing is slow but does not rely on the arc weights. The Customization introduces the weights and is reasonably fast. Finally, the actual paths are computed in the query phase. A common setup consists of doing the preprocessing once and the customization per user upon login. Further one can use a customization to incorporate live-traffic updates.

## Features
- Safe ergonomic Rust API on top of proven C++ core via [`cxx`](https://cxx.rs/).
- Build indices from raw edge lists (tail/head arrays).
- Ordering helpers: nested dissection (inertial separator heuristic) and degree fallback.
- Sequential & parallel customization, and partial weight update afterwards.
- Cheap full re-customization via `CCHMetric::reset` (reuses internal buffers).
- Reusable query object supporting multi-source / multi-target searches.
- Batched one-to-many / many-to-one queries with pinned targets/sources (`CCHOneToMany`, `CCHManyToOne`).
- Path extraction: node sequence & original arc id sequence.
- Thread-safe sharing of immutable structures (`CCH`, `CCHMetric`).

## Installation

---

Rust stable release from [crates.io](https://crates.io/crates/routingkit-cch):
```toml
[dependencies]
routingkit-cch = "0.1"
```
Or track the repository:
```toml
[dependencies]
routingkit-cch = { git = "https://github.com/HellOwhatAs/routingkit-cch" }
```

---

Python stable release from [pypi](https://pypi.org/project/routingkit-cch/):
```bash
pip install routingkit-cch
```
Or track the repository:
```bash
pip install git+https://github.com/HellOwhatAs/routingkit-cch
```

---

For the git form ensure the `RoutingKit` submodule is present:
```bash
git submodule update --init --recursive
```
Requirements: C++17 compiler (MSVC / gcc / clang).
OpenMP is enabled automatically by the build script; ensure your toolchain provides an OpenMP runtime (e.g. install libomp on macOS).
Without it the build may fail when `CCHMetric::parallel_new` is called.

## Quick Start
> For python examples, see [`examples`](./examples) folder.
```rust
use routingkit_cch::{CCH, CCHMetric, CCHQuery, compute_order_degree};

// Small toy graph: 0 -> 1 -> 2 -> 3
let tail = vec![0, 1, 2];
let head = vec![1, 2, 3];
let weights = vec![10, 5, 7]; // total 22 from 0 to 3
let node_count = 4u32;

// 1) Compute a (cheap) order; for real data prefer compute_order_inertial (requires lat,lon).
let order = compute_order_degree(node_count, &tail, &head);
let cch = CCH::new(&order, &tail, &head, |_| {}, false);

// 2) Bind weights & customize (done inside CCHMetric::new here).
let metric = CCHMetric::new(&cch, weights.clone());

// 3) Run a shortest path query 0 -> 3.
let mut q = CCHQuery::new(&metric);
q.add_source(0, 0);
q.add_target(3, 0);
let res = q.run();
assert_eq!(res.distance(), Some(22));
let node_path = res.node_path();
assert_eq!(node_path, vec![0, 1, 2, 3]);
let arc_path = res.arc_path();
assert_eq!(arc_path, vec![0, 1, 2]);
```

## Building an Order
For production use prefer the inertial nested dissection based order:
```rust,ignore
use routingkit_cch::compute_order_inertial;
let order = compute_order_inertial(node_count, &tail, &head, &latitude, &longitude);
```
Better separators -> faster customization & queries. External advanced orderers (e.g. FlowCutter) could be integrated offline; you only need to supply the permutation.

## (Parallel) Customization
```rust,ignore
use routingkit_cch::{CCH, CCHMetric};
let metric = CCHMetric::new(&cch, weights.clone()); // single thread
let metric = CCHMetric::parallel_new(&cch, weights.clone(), 0); // 0 -> auto threads
```
Use when graphs are large enough; for tiny graphs overhead may outweigh benefit.

## Full Re-Customization (Metric Reset)
When all (or most) weights change (e.g. periodic traffic refresh), rebind the existing metric
instead of building a new one; internal shortcut-weight buffers are reused:
```rust,ignore
let mut metric = CCHMetric::new(&cch, weights);
// ... later, new weights for the same CCH ...
metric.reset(new_weights); // re-customizes in place
// or, with the `openmp` feature:
metric.parallel_reset(new_weights, 0); // 0 -> auto threads
```
Requires exclusive access: drop all queries borrowing the metric first (enforced by the borrow checker).

## Incremental (Partial) Weight Updates
If only a small subset of arc weights change (e.g. traffic incidents), you can avoid a full re-customization:
```rust,ignore
let mut metric = CCHMetric::parallel_new(&cch, weights.clone(), 0);
let mut updater = CCHMetricPartialUpdater::new(&cch);
// ... run queries ...
// Update two arcs (id 12 -> 900, id 77 -> 450)
updater.apply(&mut metric, &BTreeMap::from_iter([(12, 900), (77, 450)]));
// New queries now see updated weights.
```

## Query
```rust,ignore
let mut q = CCHQuery::new(&metric);
q.add_source(0, 0);
q.add_target(3, 0);
{
    let res = q.run();
    printf("{:?}", res.distance());
} // drop res before reusing q since res takes &mut q
q.add_source(2, 0);
q.add_target(5, 0);
{
    let res = q.run();
    // ...
}
```
`CCHQuery` is not thread-safe; create one instance per thread and reuse it is far cheaper than constructing a new one.

## One-to-Many / Many-to-One (Batched) Queries
When you need distances from one node to many fixed destinations (distance tables, matrices,
k-nearest), pin the destination set once and query it in a single sweep — much faster than a
point-to-point query per destination:
```rust,ignore
use routingkit_cch::{CCHOneToMany, CCHManyToOne};

let mut otm = CCHOneToMany::new(&metric, &targets); // pin once (moderately expensive)
for s in sources {
    let dist: Vec<Option<u32>> = otm.distances_from(s); // aligned with `targets`; None = unreachable
}

// Mirror image: distances from many fixed sources to a target.
let mut mto = CCHManyToOne::new(&metric, &sources);
let dist = mto.distances_to(target); // aligned with `sources`
```
Multi-source/-target variants with initial distance offsets are available
(`distances_from_multi`, `distances_to_multi`). Note: batched queries return distances only
(no path reconstruction), matching the underlying RoutingKit API.

## Path Reconstruction
After `run() -> CCHQueryResult`:
- `CCHQueryResult::distance()` -> `Option<u32>` (None = unreachable)
- `CCHQueryResult::node_path()` -> `Vec<node_id>` (empty = unreachable)
- `CCHQueryResult::arc_path()` -> `Vec<original_arc_id>` (empty = unreachable)

## Thread Safety
| Type                      | Send | Sync | Notes                                             |
| ------------------------- | ---- | ---- | ------------------------------------------------- |
| `CCH`                     | yes  | yes  | Immutable after build                             |
| `CCHMetric`               | yes  | yes  | Read-only after customization / partial-update    |
| `CCHQuery`                | yes  | no   | Internal mutable labels; reuse it within thread   |
| `CCHQueryResult`          | yes  | no   | Runned state of `CCHQuery`, actually `&mut` of it |
| `CCHOneToMany`            | yes  | no   | Internal mutable labels; reuse it within thread   |
| `CCHManyToOne`            | yes  | no   | Internal mutable labels; reuse it within thread   |
| `CCHMetricPartialUpdater` | no   | no   | Should have nothing to do with parallel           |

Create separate queries per thread for parallel batch querying.
