use crate::{
    CCH, CCHManyToOne, CCHMetric, CCHMetricPartialUpdater, CCHOneToMany, CCHQuery, CCHQueryResult,
    compute_order_degree, compute_order_inertial,
};
use pyo3::prelude::*;
use std::collections::HashMap;

unsafe fn extend_lifetime<'a, T>(r: &'a T) -> &'static T {
    unsafe { std::mem::transmute::<&'a T, &'static T>(r) }
}

unsafe fn extend_lifetime_mut<'a, T>(r: &'a mut T) -> &'static mut T {
    unsafe { std::mem::transmute::<&'a mut T, &'static mut T>(r) }
}

#[pyfunction]
#[pyo3(name = "compute_order_degree")]
fn py_compute_order_degree(node_count: u32, tail: Vec<u32>, head: Vec<u32>) -> Vec<u32> {
    compute_order_degree(node_count, &tail, &head)
}

#[pyfunction]
#[pyo3(name = "compute_order_inertial")]
fn py_compute_order_inertial(
    node_count: u32,
    tail: Vec<u32>,
    head: Vec<u32>,
    latitude: Vec<f32>,
    longitude: Vec<f32>,
) -> Vec<u32> {
    compute_order_inertial(node_count, &tail, &head, &latitude, &longitude)
}

#[pyclass(frozen)]
#[pyo3(name = "CCH")]
struct PyCCH(CCH);

#[pymethods]
impl PyCCH {
    #[new]
    fn new(order: Vec<u32>, tail: Vec<u32>, head: Vec<u32>, filter_always_inf_arcs: bool) -> Self {
        Self(CCH::new(
            &order,
            &tail,
            &head,
            |_| {},
            filter_always_inf_arcs,
        ))
    }
}

#[pyclass]
#[pyo3(name = "CCHMetric")]
struct PyCCHMetric {
    inner: CCHMetric<'static>, // should drop before cch
    _cch: Py<PyCCH>,
    query_count: usize,
}

#[pymethods]
impl PyCCHMetric {
    #[new]
    fn new(py: Python, cch: Py<PyCCH>, weights: Vec<u32>) -> Self {
        let cch_static = unsafe { extend_lifetime(&cch.borrow(py).0) };
        Self {
            inner: CCHMetric::new(cch_static, weights),
            _cch: cch,
            query_count: 0,
        }
    }

    #[getter]
    fn weights(&self) -> Vec<u32> {
        self.inner.weights().to_vec()
    }

    /// Rebind the metric to a new weight vector and re-customize,
    /// reusing internal buffers (cheaper than building a new CCHMetric).
    fn reset(&mut self, py: Python, weights: Vec<u32>) {
        assert!(
            self.query_count == 0,
            "cannot reset weights while there are active queries using the metric"
        );
        let inner = &mut self.inner;
        py.detach(|| inner.reset(weights));
    }
}

#[pyclass(unsendable)]
#[pyo3(name = "CCHMetricPartialUpdater")]
struct PyCCHMetricPartialUpdater {
    inner: CCHMetricPartialUpdater<'static>,
    _cch: Py<PyCCH>,
}

#[pymethods]
impl PyCCHMetricPartialUpdater {
    #[new]
    fn new(py: Python, cch: Py<PyCCH>) -> Self {
        let cch_static = unsafe { extend_lifetime(&cch.borrow(py).0) };
        Self {
            inner: CCHMetricPartialUpdater::new(cch_static),
            _cch: cch,
        }
    }

    fn apply(&mut self, py: Python, metric: Py<PyCCHMetric>, updates: HashMap<u32, u32>) {
        let mut metric_ref = metric.borrow_mut(py);
        assert!(
            metric_ref.query_count == 0,
            "cannot apply updates while there are active CCHQuerys using the metric"
        );
        let a = unsafe { extend_lifetime_mut(&mut metric_ref.inner) };
        self.inner.apply(a, &updates);
    }
}

#[pyclass(unsendable)]
#[pyo3(name = "CCHOneToMany")]
struct PyCCHOneToMany {
    inner: CCHOneToMany<'static>, // should drop before metric
    _metric: Py<PyCCHMetric>,
}

#[pymethods]
impl PyCCHOneToMany {
    #[new]
    fn new(py: Python, metric: Py<PyCCHMetric>, targets: Vec<u32>) -> Self {
        let metric_static = unsafe { extend_lifetime(&metric.borrow(py).inner) };
        // Construct first so a panic (invalid target id) cannot leak the query_count.
        let inner = CCHOneToMany::new(metric_static, &targets);
        metric.borrow_mut(py).query_count += 1;
        Self {
            inner,
            _metric: metric,
        }
    }

    /// Number of pinned targets (= length of the returned distance lists).
    #[getter]
    fn target_count(&self) -> usize {
        self.inner.target_count()
    }

    /// Replace the pinned target set, reusing internal buffers
    /// (cheaper than constructing a new CCHOneToMany).
    fn repin_targets(&mut self, targets: Vec<u32>) {
        self.inner.repin_targets(&targets);
    }

    /// Distances from `source` to every pinned target (None = unreachable).
    fn distances_from(&mut self, py: Python, source: u32) -> Vec<Option<u32>> {
        let inner = &mut self.inner;
        py.detach(|| inner.distances_from(source))
    }

    /// Multi-source variant taking `(source, initial_dist)` pairs.
    fn distances_from_multi(&mut self, py: Python, sources: Vec<(u32, u32)>) -> Vec<Option<u32>> {
        let inner = &mut self.inner;
        py.detach(|| inner.distances_from_multi(&sources))
    }
}

impl Drop for PyCCHOneToMany {
    fn drop(&mut self) {
        Python::attach(|py| self._metric.borrow_mut(py).query_count -= 1);
    }
}

#[pyclass(unsendable)]
#[pyo3(name = "CCHManyToOne")]
struct PyCCHManyToOne {
    inner: CCHManyToOne<'static>, // should drop before metric
    _metric: Py<PyCCHMetric>,
}

#[pymethods]
impl PyCCHManyToOne {
    #[new]
    fn new(py: Python, metric: Py<PyCCHMetric>, sources: Vec<u32>) -> Self {
        let metric_static = unsafe { extend_lifetime(&metric.borrow(py).inner) };
        // Construct first so a panic (invalid source id) cannot leak the query_count.
        let inner = CCHManyToOne::new(metric_static, &sources);
        metric.borrow_mut(py).query_count += 1;
        Self {
            inner,
            _metric: metric,
        }
    }

    /// Number of pinned sources (= length of the returned distance lists).
    #[getter]
    fn source_count(&self) -> usize {
        self.inner.source_count()
    }

    /// Replace the pinned source set, reusing internal buffers
    /// (cheaper than constructing a new CCHManyToOne).
    fn repin_sources(&mut self, sources: Vec<u32>) {
        self.inner.repin_sources(&sources);
    }

    /// Distances from every pinned source to `target` (None = unreachable).
    fn distances_to(&mut self, py: Python, target: u32) -> Vec<Option<u32>> {
        let inner = &mut self.inner;
        py.detach(|| inner.distances_to(target))
    }

    /// Multi-target variant taking `(target, initial_dist)` pairs.
    fn distances_to_multi(&mut self, py: Python, targets: Vec<(u32, u32)>) -> Vec<Option<u32>> {
        let inner = &mut self.inner;
        py.detach(|| inner.distances_to_multi(&targets))
    }
}

impl Drop for PyCCHManyToOne {
    fn drop(&mut self) {
        Python::attach(|py| self._metric.borrow_mut(py).query_count -= 1);
    }
}

#[pyclass(unsendable)]
#[pyo3(name = "CCHQuery")]
struct PyCCHQuery {
    inner: CCHQuery<'static>,
    _metric: Py<PyCCHMetric>,
}

#[pymethods]
impl PyCCHQuery {
    #[new]
    fn new(py: Python, metric: Py<PyCCHMetric>) -> Self {
        metric.borrow_mut(py).query_count += 1;
        let a = unsafe { extend_lifetime(&metric.borrow(py).inner) };
        Self {
            inner: CCHQuery::new(a),
            _metric: metric,
        }
    }

    fn run(self_: Py<Self>, py: Python, source: u32, target: u32) -> PyCCHQueryResult {
        let mut_q_static = unsafe { extend_lifetime_mut(&mut self_.borrow_mut(py).inner) };
        assert!(
            mut_q_static.state.iter().all(|x| !x),
            "drop the CCHQueryResult before running a new one"
        );
        let result = py.detach(|| {
            mut_q_static.add_source(source, 0);
            mut_q_static.add_target(target, 0);
            mut_q_static.run()
        });
        PyCCHQueryResult {
            inner: result,
            _query: self_,
        }
    }

    fn run_multi(
        self_: Py<Self>,
        py: Python,
        sources: Vec<(u32, u32)>,
        target: Vec<(u32, u32)>,
    ) -> PyCCHQueryResult {
        let mut_q_static = unsafe { extend_lifetime_mut(&mut self_.borrow_mut(py).inner) };
        assert!(
            mut_q_static.state.iter().all(|x| !x),
            "drop current CCHQueryResult before running a new one"
        );
        let result = py.detach(|| {
            for (s, d) in sources {
                mut_q_static.add_source(s, d);
            }
            for (t, d) in target {
                mut_q_static.add_target(t, d);
            }
            mut_q_static.run()
        });
        PyCCHQueryResult {
            inner: result,
            _query: self_,
        }
    }
}

impl Drop for PyCCHQuery {
    fn drop(&mut self) {
        Python::attach(|py| self._metric.borrow_mut(py).query_count -= 1);
    }
}

#[pyclass(unsendable)]
#[pyo3(name = "CCHQueryResult")]
struct PyCCHQueryResult {
    inner: CCHQueryResult<'static, 'static>,
    _query: Py<PyCCHQuery>,
}

#[pymethods]
impl PyCCHQueryResult {
    #[getter]
    fn distance(&self) -> Option<u32> {
        self.inner.distance()
    }

    #[getter]
    fn node_path(&self) -> Vec<u32> {
        self.inner.node_path().to_vec()
    }

    #[getter]
    fn arc_path(&self) -> Vec<u32> {
        self.inner.arc_path().to_vec()
    }
}

#[pymodule]
mod routingkit_cch {
    #[pymodule_export]
    use super::PyCCH;
    #[pymodule_export]
    use super::PyCCHManyToOne;
    #[pymodule_export]
    use super::PyCCHMetric;
    #[pymodule_export]
    use super::PyCCHMetricPartialUpdater;
    #[pymodule_export]
    use super::PyCCHOneToMany;
    #[pymodule_export]
    use super::PyCCHQuery;
    #[pymodule_export]
    use super::PyCCHQueryResult;
    #[pymodule_export]
    use super::py_compute_order_degree;
    #[pymodule_export]
    use super::py_compute_order_inertial;
}
