#![doc = include_str!("../README.md")]

#[cxx::bridge]
pub mod ffi {
    extern "C++" {
        include!("routingkit_cch_wrapper.h");

        type CCH; // CustomizableContractionHierarchy
        type CCHMetric; // CustomizableContractionHierarchyMetric
        type CCHQuery; // CustomizableContractionHierarchyQuery
        type CCHPartial; // CustomizableContractionHierarchyPartialCustomization

        /// Build a Customizable Contraction Hierarchy.
        /// Arguments:
        /// * order: node permutation (size = number of nodes)
        /// * tail/head: directed arc lists (same length)
        /// * filter_always_inf_arcs: if true, arcs with infinite weight placeholders are stripped
        unsafe fn cch_new(
            order: &[u32],
            tail: &[u32],
            head: &[u32],
            log_message: fn(&str),
            filter_always_inf_arcs: bool,
        ) -> UniquePtr<CCH>;

        /// Create a metric (weights binding) for an existing CCH.
        /// Keeps pointer to weights in CCHMetric; weights length must equal arc count.
        unsafe fn cch_metric_new(cch: &CCH, weights: &[u32]) -> UniquePtr<CCHMetric>;

        /// Run customization to compute upward/downward shortcut weights.
        /// Must be called after creating a metric and before queries.
        /// Cost: Depends on separator quality; usually near-linear in m * small constant; may allocate temporary buffers.
        unsafe fn cch_metric_customize(metric: Pin<&mut CCHMetric>);

        /// Rebind the metric to a new weight vector (same CCH), reusing the internal
        /// shortcut-weight buffers. A customize call must follow before further queries.
        unsafe fn cch_metric_reset(metric: Pin<&mut CCHMetric>, weights: &[u32]);

        /// Parallel customization; thread_count==0 picks an internal default (#procs if OpenMP, else 1).
        unsafe fn cch_metric_parallel_customize(metric: Pin<&mut CCHMetric>, thread_count: u32);

        /// Allocate a new reusable partial customization helper bound to a CCH.
        unsafe fn cch_partial_new(cch: &CCH) -> UniquePtr<CCHPartial>;

        /// Reset the partial updater to empty state (no updated arcs).
        unsafe fn cch_partial_reset(partial: Pin<&mut CCHPartial>);

        /// Mark an arc as updated (weight changed) for the next partial customize call.
        /// The actual weight change must already happened in the metric's weight vector.
        unsafe fn cch_partial_update_arc(partial: Pin<&mut CCHPartial>, arc: u32);

        /// Run partial customization to update shortcut weights affected by the marked arcs.
        unsafe fn cch_partial_customize(partial: Pin<&mut CCHPartial>, metric: Pin<&mut CCHMetric>);

        /// Allocate a new reusable query object bound to a metric.
        unsafe fn cch_query_new(metric: &CCHMetric) -> UniquePtr<CCHQuery>;

        /// Reset a query so it can be reused with the (same or updated) metric.
        /// Clears internal state, sources/targets, distance labels.
        unsafe fn cch_query_reset(query: Pin<&mut CCHQuery>, metric: &CCHMetric);

        /// Add a source node with initial distance (0 for standard shortest path).
        /// Multiple sources allowed (multi-source query).
        unsafe fn cch_query_add_source(query: Pin<&mut CCHQuery>, s: u32, dist: u32);

        /// Add a target node with initial distance (0 typical).
        /// Multiple targets allowed (multi-target query).
        unsafe fn cch_query_add_target(query: Pin<&mut CCHQuery>, t: u32, dist: u32);

        /// Execute the shortest path search (multi-source / multi-target if several added).
        /// Must be called after adding at least one source & target.
        unsafe fn cch_query_run(query: Pin<&mut CCHQuery>);

        /// Get shortest distance after run(). Undefined if run() not called.
        unsafe fn cch_query_distance(query: &CCHQuery) -> u32;

        /// Extract the node path for the current query result.
        /// Reconstructs path; may traverse parent pointers.
        unsafe fn cch_query_node_path(query: &CCHQuery) -> Vec<u32>;

        /// Extract the arc (edge) path corresponding to the shortest path.
        /// Each entry is an original arc id (after shortcut unpacking).
        unsafe fn cch_query_arc_path(query: &CCHQuery) -> Vec<u32>;

        /// Pin a fixed set of targets for one-to-many queries.
        /// The query must be in a freshly constructed/reset state.
        unsafe fn cch_query_pin_targets(query: Pin<&mut CCHQuery>, targets: &[u32]);

        /// Run the one-to-many search from the added sources to all pinned targets.
        unsafe fn cch_query_run_to_pinned_targets(query: Pin<&mut CCHQuery>);

        /// Distances to the pinned targets (in pinning order); inf_weight = unreachable.
        /// Must be called after run_to_pinned_targets.
        unsafe fn cch_query_distances_to_targets(query: &CCHQuery) -> Vec<u32>;

        /// Clear the sources and distance labels while keeping the pinned targets,
        /// so another one-to-many run can be performed.
        unsafe fn cch_query_reset_source(query: Pin<&mut CCHQuery>);

        /// Pin a fixed set of sources for many-to-one queries.
        /// The query must be in a freshly constructed/reset state.
        unsafe fn cch_query_pin_sources(query: Pin<&mut CCHQuery>, sources: &[u32]);

        /// Run the many-to-one search from all pinned sources to the added targets.
        unsafe fn cch_query_run_to_pinned_sources(query: Pin<&mut CCHQuery>);

        /// Distances from the pinned sources (in pinning order); inf_weight = unreachable.
        /// Must be called after run_to_pinned_sources.
        unsafe fn cch_query_distances_to_sources(query: &CCHQuery) -> Vec<u32>;

        /// Clear the targets and distance labels while keeping the pinned sources,
        /// so another many-to-one run can be performed.
        unsafe fn cch_query_reset_target(query: Pin<&mut CCHQuery>);

        /// Compute a high-quality nested dissection order using inertial flow separators.
        /// Inputs:
        /// * node_count: number of nodes
        /// * tail/head: directed arcs (treated as undirected for ordering)
        /// * latitude/longitude: per-node coords (len = node_count)
        /// Returns: order permutation (position -> node id).
        unsafe fn cch_compute_order_inertial(
            node_count: u32,
            tail: &[u32],
            head: &[u32],
            latitude: &[f32],
            longitude: &[f32],
        ) -> Vec<u32>;

        /// Fast fallback order: nodes sorted by (degree, id) ascending.
        /// Lower quality than nested dissection but zero extra data needed.
        unsafe fn cch_compute_order_degree(node_count: u32, tail: &[u32], head: &[u32])
        -> Vec<u32>;
    }
}

// Thread-safety markers
// Safety rationale:
// - CCH (CustomizableContractionHierarchy) after construction is immutable.
// - CCHMetric after successful customize() is read-only for queries; underlying RoutingKit code
//   does not mutate metric during query operations (queries keep their own state).
// - CCHQuery holds mutable per-query state and must not be shared across threads concurrently;
//   we allow moving it between threads (Send) but not Sync.
// If RoutingKit later introduces internal mutation or lazy caching inside CCHMetric, these impls
// would need re-audit.
unsafe impl Send for ffi::CCH {}
unsafe impl Sync for ffi::CCH {}
unsafe impl Send for ffi::CCHMetric {}
unsafe impl Sync for ffi::CCHMetric {}
unsafe impl Send for ffi::CCHQuery {}
// (No Sync for CCHQuery)

// Rust wrapper over FFI
use cxx::UniquePtr;
use ffi::*;
pub use ffi::{
    cch_compute_order_degree as compute_order_degree_unchecked,
    cch_compute_order_inertial as compute_order_inertial_unchecked,
};

/// RoutingKit's internal "infinity" weight (`RoutingKit::inf_weight`).
/// A distance equal to this value means "unreachable"; input weights must not exceed it.
pub const INF_WEIGHT: u32 = 2_147_483_647;

fn is_permutation(arr: &[u32]) -> bool {
    let n = arr.len();
    let mut seen = vec![false; n];
    for &val in arr {
        if (val as usize) >= n || seen[val as usize] {
            return false;
        }
        seen[val as usize] = true;
    }
    true
}

/// Lightweight fill-in reducing order heuristic: sort nodes by (degree, id) ascending.
/// Fast but lower quality than nested dissection. Use if no coordinates or other data available.
/// Panics if tail/head have inconsistent lengths or contain invalid node ids.
pub fn compute_order_degree(node_count: u32, tail: &[u32], head: &[u32]) -> Vec<u32> {
    assert!(
        tail.iter()
            .chain(head)
            .max()
            .map_or(true, |&v| v < node_count),
        "tail/head contain node ids outside valid range"
    );
    assert!(
        tail.len() == head.len(),
        "tail and head arrays must have the same length"
    );
    unsafe { cch_compute_order_degree(node_count, tail, head) }
}

/// High-quality nested dissection order using inertial flow separators.
/// Requires per-node coordinates (latitude/longitude) as input.
/// Panics if tail/head have inconsistent lengths or contain invalid node ids,
/// or if latitude/longitude lengths do not match node_count.
pub fn compute_order_inertial(
    node_count: u32,
    tail: &[u32],
    head: &[u32],
    latitude: &[f32],
    longitude: &[f32],
) -> Vec<u32> {
    assert!(
        tail.iter()
            .chain(head)
            .max()
            .map_or(true, |&v| v < node_count),
        "tail/head contain node ids outside valid range"
    );
    assert!(
        tail.len() == head.len(),
        "tail and head arrays must have the same length"
    );
    assert!(
        latitude.len() == (node_count as usize) && longitude.len() == (node_count as usize),
        "latitude/longitude length must equal node count"
    );
    unsafe { cch_compute_order_inertial(node_count, tail, head, latitude, longitude) }
}

/// Immutable Customizable Contraction Hierarchy index.
pub struct CCH {
    inner: UniquePtr<ffi::CCH>,
    edge_count: usize,
    node_count: usize,
}

impl CCH {
    /// Construct a new immutable Customizable Contraction Hierarchy index.
    ///
    /// Parameters:
    /// * `order` – permutation of node ids (length = node count) produced by a fill‑in reducing
    ///   nested dissection heuristic (e.g. [`compute_order_inertial`]) or a lightweight fallback.
    /// * `tail`, `head` – parallel arrays encoding each directed arc `i` as `(tail[i], head[i])`.
    ///   The ordering routine treats them as undirected; here they stay directed for queries.
    /// * `log_message` – callback for logging progress messages during construction.
    ///   May be `|_| {}` if you do not want any messages.
    /// * `filter_always_inf_arcs` – if `true`, arcs whose weight will always be interpreted as
    ///   an application defined 'infinity' placeholder may be removed during construction to
    ///   reduce index size. (Typically keep `false` unless you prepared such a list.)
    ///
    /// Cost: preprocessing is more expensive than a single customization but usually far cheaper
    /// than building a full classical CH of the same quality. Construction copies the input
    /// slices; you may drop them afterwards.
    ///
    /// Thread-safety: resulting object is `Send + Sync` and read-only.
    ///
    /// Panics if `order` is not a valid permutation of node ids,
    /// or if `tail`/`head` have inconsistent lengths or contain invalid node ids.
    pub fn new(
        order: &[u32],
        tail: &[u32],
        head: &[u32],
        log_message: fn(&str),
        filter_always_inf_arcs: bool,
    ) -> Self {
        assert!(
            is_permutation(order),
            "order array is not a valid permutation"
        );
        assert!(
            tail.len() == head.len(),
            "tail and head arrays must have the same length"
        );
        assert!(
            tail.iter()
                .chain(head)
                .max()
                .map_or(true, |&v| (v as usize) < order.len()),
            "tail/head contain node ids outside valid range"
        );
        unsafe { Self::new_unchecked(order, tail, head, log_message, filter_always_inf_arcs) }
    }

    pub unsafe fn new_unchecked(
        order: &[u32],
        tail: &[u32],
        head: &[u32],
        log_message: fn(&str),
        filter_always_inf_arcs: bool,
    ) -> Self {
        let cch = unsafe { cch_new(order, tail, head, log_message, filter_always_inf_arcs) };
        CCH {
            inner: cch,
            edge_count: tail.len(),
            node_count: order.len(),
        }
    }
}

/// A customized metric (weight binding) for a given [`CCH`].
/// Keeps ownership of the weight vector so it can be mutated for partial updates.
/// Thread-safety: `Send + Sync`; read-only for queries.
pub struct CCHMetric<'a> {
    inner: UniquePtr<ffi::CCHMetric>,
    weights: Box<[u32]>, // The C++ side stores only a raw pointer; it is valid for the lifetime of `self`.
    cch: &'a CCH,
}

impl<'a> CCHMetric<'a> {
    /// Create and customize a metric (weight binding) for a given [`CCH`].
    /// Owns the weight vector so that future partial updates can safely mutate it.
    pub fn new(cch: &'a CCH, weights: Vec<u32>) -> Self {
        assert!(
            weights.len() == cch.edge_count,
            "weights length must equal arc count",
        );
        let boxed: Box<[u32]> = weights.into_boxed_slice();
        // Temporarily borrow as slice for FFI creation
        let metric = unsafe {
            let mut metric = cch_metric_new(&cch.inner, &boxed);
            cch_metric_customize(metric.as_mut().unwrap());
            metric
        };
        CCHMetric {
            inner: metric,
            weights: boxed,
            cch,
        }
    }

    /// Parallel customization variant.
    #[cfg(feature = "openmp")]
    pub fn parallel_new(cch: &'a CCH, weights: Vec<u32>, thread_count: u32) -> Self {
        assert!(
            weights.len() == cch.edge_count,
            "weights length must equal arc count",
        );
        let boxed: Box<[u32]> = weights.into_boxed_slice();
        let metric = unsafe {
            let mut metric = cch_metric_new(&cch.inner, &boxed);
            cch_metric_parallel_customize(metric.as_mut().unwrap(), thread_count);
            metric
        };
        CCHMetric {
            inner: metric,
            weights: boxed,
            cch,
        }
    }

    /// weights slice
    pub fn weights(&self) -> &[u32] {
        &self.weights
    }

    /// Rebind this metric to a new weight vector and re-run customization,
    /// reusing the internal shortcut-weight buffers instead of allocating a new metric.
    ///
    /// This is the intended cheap path for full re-customization (e.g. periodic
    /// traffic updates). Requires exclusive access, so no queries may be borrowing
    /// the metric (enforced by the borrow checker).
    ///
    /// Panics if `weights` length does not equal the arc count.
    pub fn reset(&mut self, weights: Vec<u32>) {
        assert!(
            weights.len() == self.cch.edge_count,
            "weights length must equal arc count",
        );
        self.weights = weights.into_boxed_slice();
        unsafe {
            cch_metric_reset(self.inner.as_mut().unwrap(), &self.weights);
            cch_metric_customize(self.inner.as_mut().unwrap());
        }
    }

    /// Like [`CCHMetric::reset`], but customizes in parallel.
    /// `thread_count == 0` picks an internal default.
    #[cfg(feature = "openmp")]
    pub fn parallel_reset(&mut self, weights: Vec<u32>, thread_count: u32) {
        assert!(
            weights.len() == self.cch.edge_count,
            "weights length must equal arc count",
        );
        self.weights = weights.into_boxed_slice();
        unsafe {
            cch_metric_reset(self.inner.as_mut().unwrap(), &self.weights);
            cch_metric_parallel_customize(self.inner.as_mut().unwrap(), thread_count);
        }
    }
}

/// Reusable partial customization helper. Construct once if you perform many small incremental
/// weight updates; this avoids reallocating O(m) internal buffers each call.
pub struct CCHMetricPartialUpdater<'a> {
    partial: UniquePtr<ffi::CCHPartial>,
    cch: &'a CCH,
}

impl<'a> CCHMetricPartialUpdater<'a> {
    /// Create a reusable partial updater bound to a given CCH. You can then apply it to any
    /// metric built from the same CCH (even if you rebuild metrics with different weight sets).
    pub fn new(cch: &'a CCH) -> Self {
        let partial = unsafe { cch_partial_new(cch.inner.as_ref().unwrap()) };
        CCHMetricPartialUpdater { partial, cch }
    }

    /// Apply a batch of (arc, new_weight) updates to the given metric and run partial customize.
    pub fn apply<T>(&mut self, metric: &mut CCHMetric<'a>, updates: &T)
    where
        T: for<'b> std::ops::Index<&'b u32, Output = u32>,
        for<'b> &'b T: IntoIterator<Item = (&'b u32, &'b u32)>,
    {
        assert!(
            std::ptr::eq(metric.cch, self.cch),
            "CCHMetricPartialUpdater must be used with metrics from the same CCH"
        );
        for (k, v) in updates {
            metric.weights[*k as usize] = *v; // safe: length invariant unchanged (Box<[u32]>)
        }
        unsafe {
            cch_partial_reset(self.partial.as_mut().unwrap());
            for (k, _) in updates {
                cch_partial_update_arc(self.partial.as_mut().unwrap(), *k);
            }
            cch_partial_customize(
                self.partial.as_mut().unwrap(),
                metric.inner.as_mut().unwrap(),
            );
        }
    }
}

/// A reusable shortest-path query object bound to a given [`CCHMetric`].
/// Thread-safety: `Send` but not `Sync`; holds mutable per-query state.
pub struct CCHQuery<'a> {
    inner: UniquePtr<ffi::CCHQuery>,
    metric: &'a CCHMetric<'a>,
    state: [bool; 2],
}

impl<'a> CCHQuery<'a> {
    /// Allocate a new reusable shortest-path query bound to a given customized [`CCHMetric`].
    ///
    /// The query object stores its own frontier / label buffers and can be reset and reused for
    /// many (s, t) pairs or multi-source / multi-target batches. You may have multiple query
    /// objects referencing the same metric concurrently (read-only access to metric data).
    pub fn new(metric: &'a CCHMetric<'a>) -> Self {
        let inner = unsafe { cch_query_new(&metric.inner) };
        CCHQuery {
            inner,
            metric,
            state: [false; 2],
        }
    }

    /// Add a source node with an initial distance (normally 0). Multiple calls allow a multi-
    /// source query. Distances let you model already-traversed partial paths.
    pub fn add_source(&mut self, s: u32, dist: u32) {
        assert!(
            (s as usize) < self.metric.cch.node_count,
            "source node id out of range",
        );
        unsafe {
            cch_query_add_source(self.inner.as_mut().unwrap(), s, dist);
        }
        self.state[0] = true;
    }

    /// Add a target node with an initial distance (normally 0). Multiple calls allow multi-target
    /// queries; the algorithm stops when the frontiers settle the optimal distance to any target.
    pub fn add_target(&mut self, t: u32, dist: u32) {
        assert!(
            (t as usize) < self.metric.cch.node_count,
            "target node id out of range",
        );
        unsafe {
            cch_query_add_target(self.inner.as_mut().unwrap(), t, dist);
        }
        self.state[1] = true;
    }

    /// Execute the forward/backward upward/downward search to settle the shortest path between the
    /// added sources and targets. Must be called after at least one source and one target.
    /// Returns a [`CCHQueryResult`] holding a mutable reference to the query.
    /// The query is automatically reset when the result is dropped.
    pub fn run<'b>(&'b mut self) -> CCHQueryResult<'b, 'a> {
        assert!(
            self.state.iter().all(|&x| x),
            "must add at least one source and one target before running the query"
        );
        unsafe {
            cch_query_run(self.inner.as_mut().unwrap());
        }
        CCHQueryResult { query: self }
    }
}

/// The result of a single execution of a [`CCHQuery`].
/// Holds a mutable reference to the query to ensure it is not reset or reused
/// while the result is still alive. So you need to drop the result before reusing the query.
/// When the result is dropped, the query is automatically reset.
pub struct CCHQueryResult<'b, 'a> {
    query: &'b mut CCHQuery<'a>,
}

impl<'b, 'a> CCHQueryResult<'b, 'a> {
    /// Return the shortest path distance result of [`CCHQuery::run`].
    /// Returns `None` if no target is reachable.
    pub fn distance(&self) -> Option<u32> {
        let res = unsafe { cch_query_distance(self.query.inner.as_ref().unwrap()) };
        if res == INF_WEIGHT {
            // internally distance equals `inf_weight` means unreachable
            None
        } else {
            Some(res)
        }
    }

    /// Reconstruct and return the node id sequence of the current best path.
    /// Returns empty vec if no target is reachable.
    pub fn node_path(&self) -> Vec<u32> {
        unsafe { cch_query_node_path(self.query.inner.as_ref().unwrap()) }
    }

    /// Reconstruct and return the original arc ids along the shortest path.
    /// Returns empty vec if no target is reachable.
    pub fn arc_path(&self) -> Vec<u32> {
        unsafe { cch_query_arc_path(self.query.inner.as_ref().unwrap()) }
    }
}

impl<'b, 'a> Drop for CCHQueryResult<'b, 'a> {
    /// reset the query
    fn drop(&mut self) {
        unsafe {
            cch_query_reset(
                self.query.inner.as_mut().unwrap(),
                self.query.metric.inner.as_ref().unwrap(),
            );
        }
        self.query.state.iter_mut().for_each(|x| *x = false);
    }
}

/// A reusable one-to-many query with a fixed (pinned) set of targets.
///
/// Pinning precomputes the search space of the targets once; afterwards each
/// [`CCHOneToMany::distances_from`] call answers distances from a source to *all*
/// pinned targets in a single elimination-tree sweep, which is much faster than
/// running one point-to-point query per target. Use this for distance tables /
/// matrices (one `CCHOneToMany` per row) or k-nearest style workloads.
///
/// Thread-safety: `Send` but not `Sync`; holds mutable per-query state.
pub struct CCHOneToMany<'a> {
    inner: UniquePtr<ffi::CCHQuery>,
    metric: &'a CCHMetric<'a>,
    target_count: usize,
}

impl<'a> CCHOneToMany<'a> {
    /// Create a one-to-many query bound to `metric` with a fixed set of pinned targets.
    /// Distances returned later are aligned with the order of `targets`
    /// (duplicates are allowed and answered per entry).
    ///
    /// Panics if any target node id is out of range.
    pub fn new(metric: &'a CCHMetric<'a>, targets: &[u32]) -> Self {
        assert!(!targets.is_empty(), "must provide at least one target");
        assert!(
            targets
                .iter()
                .all(|&t| (t as usize) < metric.cch.node_count),
            "target node id out of range",
        );
        let mut inner = unsafe { cch_query_new(&metric.inner) };
        unsafe { cch_query_pin_targets(inner.as_mut().unwrap(), targets) };
        CCHOneToMany {
            inner,
            metric,
            target_count: targets.len(),
        }
    }

    /// Number of pinned targets (= length of the distance vectors returned).
    pub fn target_count(&self) -> usize {
        self.target_count
    }

    /// Replace the pinned target set, reusing the query's internal O(n) label
    /// buffers (cheaper than constructing a new [`CCHOneToMany`]).
    ///
    /// Panics if any target node id is out of range.
    pub fn repin_targets(&mut self, targets: &[u32]) {
        assert!(!targets.is_empty(), "must provide at least one target");
        assert!(
            targets
                .iter()
                .all(|&t| (t as usize) < self.metric.cch.node_count),
            "target node id out of range",
        );
        // Validation done above: the FFI section below cannot panic.
        unsafe {
            cch_query_reset(
                self.inner.as_mut().unwrap(),
                self.metric.inner.as_ref().unwrap(),
            );
            cch_query_pin_targets(self.inner.as_mut().unwrap(), targets);
        }
        self.target_count = targets.len();
    }

    /// Compute the shortest distances from `source` to every pinned target.
    /// Result is aligned with the pinned target order; `None` = unreachable.
    pub fn distances_from(&mut self, source: u32) -> Vec<Option<u32>> {
        self.distances_from_multi(&[(source, 0)])
    }

    /// Multi-source variant: for each pinned target `t`, returns
    /// `min` over the given `(source, initial_dist)` pairs of `initial_dist + dist(source, t)`.
    ///
    /// Panics if `sources` is empty or contains node ids out of range.
    pub fn distances_from_multi(&mut self, sources: &[(u32, u32)]) -> Vec<Option<u32>> {
        assert!(!sources.is_empty(), "must provide at least one source");
        assert!(
            sources
                .iter()
                .all(|&(s, _)| (s as usize) < self.metric.cch.node_count),
            "source node id out of range",
        );
        // All validation is done above: the FFI section below cannot panic, so the
        // query always returns to the "targets pinned, no sources" state.
        unsafe {
            for &(s, d) in sources {
                cch_query_add_source(self.inner.as_mut().unwrap(), s, d);
            }
            cch_query_run_to_pinned_targets(self.inner.as_mut().unwrap());
            let dist = cch_query_distances_to_targets(self.inner.as_ref().unwrap());
            cch_query_reset_source(self.inner.as_mut().unwrap());
            dist.into_iter()
                .map(|d| if d == INF_WEIGHT { None } else { Some(d) })
                .collect()
        }
    }
}

/// A reusable many-to-one query with a fixed (pinned) set of sources.
///
/// The mirror image of [`CCHOneToMany`]: pinning precomputes the search space of
/// the sources once; each [`CCHManyToOne::distances_to`] call answers distances
/// from *all* pinned sources to a target in a single sweep.
///
/// Thread-safety: `Send` but not `Sync`; holds mutable per-query state.
pub struct CCHManyToOne<'a> {
    inner: UniquePtr<ffi::CCHQuery>,
    metric: &'a CCHMetric<'a>,
    source_count: usize,
}

impl<'a> CCHManyToOne<'a> {
    /// Create a many-to-one query bound to `metric` with a fixed set of pinned sources.
    /// Distances returned later are aligned with the order of `sources`
    /// (duplicates are allowed and answered per entry).
    ///
    /// Panics if any source node id is out of range.
    pub fn new(metric: &'a CCHMetric<'a>, sources: &[u32]) -> Self {
        assert!(!sources.is_empty(), "must provide at least one source");
        assert!(
            sources
                .iter()
                .all(|&s| (s as usize) < metric.cch.node_count),
            "source node id out of range",
        );
        let mut inner = unsafe { cch_query_new(&metric.inner) };
        unsafe { cch_query_pin_sources(inner.as_mut().unwrap(), sources) };
        CCHManyToOne {
            inner,
            metric,
            source_count: sources.len(),
        }
    }

    /// Number of pinned sources (= length of the distance vectors returned).
    pub fn source_count(&self) -> usize {
        self.source_count
    }

    /// Replace the pinned source set, reusing the query's internal O(n) label
    /// buffers (cheaper than constructing a new [`CCHManyToOne`]).
    ///
    /// Panics if any source node id is out of range.
    pub fn repin_sources(&mut self, sources: &[u32]) {
        assert!(!sources.is_empty(), "must provide at least one source");
        assert!(
            sources
                .iter()
                .all(|&s| (s as usize) < self.metric.cch.node_count),
            "source node id out of range",
        );
        // Validation done above: the FFI section below cannot panic.
        unsafe {
            cch_query_reset(
                self.inner.as_mut().unwrap(),
                self.metric.inner.as_ref().unwrap(),
            );
            cch_query_pin_sources(self.inner.as_mut().unwrap(), sources);
        }
        self.source_count = sources.len();
    }

    /// Compute the shortest distances from every pinned source to `target`.
    /// Result is aligned with the pinned source order; `None` = unreachable.
    pub fn distances_to(&mut self, target: u32) -> Vec<Option<u32>> {
        self.distances_to_multi(&[(target, 0)])
    }

    /// Multi-target variant: for each pinned source `s`, returns
    /// `min` over the given `(target, initial_dist)` pairs of `dist(s, target) + initial_dist`.
    ///
    /// Panics if `targets` is empty or contains node ids out of range.
    pub fn distances_to_multi(&mut self, targets: &[(u32, u32)]) -> Vec<Option<u32>> {
        assert!(!targets.is_empty(), "must provide at least one target");
        assert!(
            targets
                .iter()
                .all(|&(t, _)| (t as usize) < self.metric.cch.node_count),
            "target node id out of range",
        );
        // All validation is done above: the FFI section below cannot panic, so the
        // query always returns to the "sources pinned, no targets" state.
        unsafe {
            for &(t, d) in targets {
                cch_query_add_target(self.inner.as_mut().unwrap(), t, d);
            }
            cch_query_run_to_pinned_sources(self.inner.as_mut().unwrap());
            let dist = cch_query_distances_to_sources(self.inner.as_ref().unwrap());
            cch_query_reset_target(self.inner.as_mut().unwrap());
            dist.into_iter()
                .map(|d| if d == INF_WEIGHT { None } else { Some(d) })
                .collect()
        }
    }
}

#[cfg(feature = "pyo3")]
mod python_binding;
