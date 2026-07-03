use rand::{Rng, SeedableRng, rngs::StdRng};
use routingkit_cch::{CCH, CCHManyToOne, CCHMetric, CCHOneToMany, CCHQuery, compute_order_degree};

/// Build a layered DAG so that both reachable and unreachable pairs exist.
fn build_test_graph() -> (u32, Vec<u32>, Vec<u32>, Vec<u32>) {
    const L: usize = 12; // layers
    const W: usize = 15; // width per layer
    let node_count = (L * W) as u32;
    let mut tail = Vec::new();
    let mut head = Vec::new();
    let mut weights = Vec::new();
    let mut rng = StdRng::seed_from_u64(0xBEEF);

    for layer in 0..L - 1 {
        for i in 0..W {
            for j in 0..W {
                if (i + 2 * j) % 5 == 0 {
                    tail.push((layer * W + i) as u32);
                    head.push(((layer + 1) * W + j) as u32);
                    weights.push(1 + ((i * j) % 13) as u32);
                }
            }
        }
    }
    // some layer-skipping arcs
    for _ in 0..(L * W / 3) {
        let layer = rng.gen_range(0..L - 2);
        tail.push((layer * W + rng.gen_range(0..W)) as u32);
        head.push(((layer + 2) * W + rng.gen_range(0..W)) as u32);
        weights.push(rng.gen_range(5..40));
    }
    (node_count, tail, head, weights)
}

fn point_to_point(metric: &CCHMetric, s: u32, t: u32) -> Option<u32> {
    let mut q = CCHQuery::new(metric);
    q.add_source(s, 0);
    q.add_target(t, 0);
    q.run().distance()
}

#[test]
fn one_to_many_matches_point_to_point() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);

    let mut rng = StdRng::seed_from_u64(42);
    let targets: Vec<u32> = (0..50).map(|_| rng.gen_range(0..node_count)).collect();
    let mut one_to_many = CCHOneToMany::new(&metric, &targets);
    assert_eq!(one_to_many.target_count(), targets.len());

    // Reuse the same pinned object across several sources.
    for _ in 0..20 {
        let s = rng.gen_range(0..node_count);
        let batched = one_to_many.distances_from(s);
        assert_eq!(batched.len(), targets.len());
        for (&t, &d) in targets.iter().zip(batched.iter()) {
            assert_eq!(d, point_to_point(&metric, s, t), "s={s} t={t}");
        }
    }
}

#[test]
fn one_to_many_multi_source_with_offsets() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);

    let mut rng = StdRng::seed_from_u64(7);
    let targets: Vec<u32> = (0..30).map(|_| rng.gen_range(0..node_count)).collect();
    let mut one_to_many = CCHOneToMany::new(&metric, &targets);

    let sources: Vec<(u32, u32)> = (0..5)
        .map(|_| (rng.gen_range(0..node_count), rng.gen_range(0..100)))
        .collect();
    let batched = one_to_many.distances_from_multi(&sources);

    for (&t, &batched_d) in targets.iter().zip(batched.iter()) {
        let expected = sources
            .iter()
            .filter_map(|&(s, off)| point_to_point(&metric, s, t).map(|d| d + off))
            .min();
        assert_eq!(batched_d, expected, "t={t}");
    }
}

#[test]
fn repin_matches_fresh_construction() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);

    let mut rng = StdRng::seed_from_u64(2024);
    let targets_a: Vec<u32> = (0..40).map(|_| rng.gen_range(0..node_count)).collect();
    let targets_b: Vec<u32> = (0..25).map(|_| rng.gen_range(0..node_count)).collect();

    let mut reused = CCHOneToMany::new(&metric, &targets_a);
    // Run a few queries so that internal labels are dirty before repinning.
    for _ in 0..5 {
        let s = rng.gen_range(0..node_count);
        reused.distances_from(s);
    }

    reused.repin_targets(&targets_b);
    assert_eq!(reused.target_count(), targets_b.len());

    let mut fresh = CCHOneToMany::new(&metric, &targets_b);
    for _ in 0..20 {
        let s = rng.gen_range(0..node_count);
        assert_eq!(reused.distances_from(s), fresh.distances_from(s), "s={s}");
    }

    // Same for many-to-one.
    let mut reused = CCHManyToOne::new(&metric, &targets_a);
    for _ in 0..5 {
        let t = rng.gen_range(0..node_count);
        reused.distances_to(t);
    }
    reused.repin_sources(&targets_b);
    assert_eq!(reused.source_count(), targets_b.len());

    let mut fresh = CCHManyToOne::new(&metric, &targets_b);
    for _ in 0..20 {
        let t = rng.gen_range(0..node_count);
        assert_eq!(reused.distances_to(t), fresh.distances_to(t), "t={t}");
    }
}

#[test]
fn many_to_one_matches_point_to_point() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);

    let mut rng = StdRng::seed_from_u64(43);
    let sources: Vec<u32> = (0..50).map(|_| rng.gen_range(0..node_count)).collect();
    let mut many_to_one = CCHManyToOne::new(&metric, &sources);
    assert_eq!(many_to_one.source_count(), sources.len());

    for _ in 0..20 {
        let t = rng.gen_range(0..node_count);
        let batched = many_to_one.distances_to(t);
        assert_eq!(batched.len(), sources.len());
        for (&s, &d) in sources.iter().zip(batched.iter()) {
            assert_eq!(d, point_to_point(&metric, s, t), "s={s} t={t}");
        }
    }
}

#[test]
fn metric_reset_matches_fresh_metric() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);

    let mut new_weights = weights.clone();
    let mut rng = StdRng::seed_from_u64(99);
    for w in new_weights.iter_mut() {
        *w = rng.gen_range(1..100);
    }

    // Reused metric: build with old weights, then reset to new ones.
    let mut metric = CCHMetric::new(&cch, weights);
    metric.reset(new_weights.clone());
    assert_eq!(metric.weights(), &new_weights[..]);

    // Reference metric built directly from the new weights.
    let reference = CCHMetric::new(&cch, new_weights);

    let mut rng = StdRng::seed_from_u64(1234);
    for _ in 0..200 {
        let s = rng.gen_range(0..node_count);
        let t = rng.gen_range(0..node_count);
        assert_eq!(
            point_to_point(&metric, s, t),
            point_to_point(&reference, s, t),
            "s={s} t={t}"
        );
    }

    // Pinned queries built after the reset must see the new weights.
    let targets: Vec<u32> = (0..20).map(|_| rng.gen_range(0..node_count)).collect();
    let mut otm = CCHOneToMany::new(&metric, &targets);
    let s = rng.gen_range(0..node_count);
    for (&t, &d) in targets.iter().zip(otm.distances_from(s).iter()) {
        assert_eq!(d, point_to_point(&reference, s, t), "s={s} t={t}");
    }
}

#[test]
#[should_panic(expected = "target node id out of range")]
fn one_to_many_rejects_invalid_target() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);
    let _ = CCHOneToMany::new(&metric, &[0, node_count]);
}

#[test]
#[should_panic(expected = "must provide at least one target")]
fn one_to_many_rejects_empty_targets() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);
    let _ = CCHOneToMany::new(&metric, &[]);
}

#[test]
#[should_panic(expected = "must provide at least one source")]
fn many_to_one_rejects_empty_sources_on_repin() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let metric = CCHMetric::new(&cch, weights);
    let mut many_to_one = CCHManyToOne::new(&metric, &[0]);
    many_to_one.repin_sources(&[]);
}

#[test]
#[should_panic(expected = "weights length must equal arc count")]
fn metric_reset_rejects_wrong_length() {
    let (node_count, tail, head, weights) = build_test_graph();
    let order = compute_order_degree(node_count, &tail, &head);
    let cch = CCH::new(&order, &tail, &head, |_| {}, false);
    let mut metric = CCHMetric::new(&cch, weights);
    metric.reset(vec![1, 2, 3]);
}
