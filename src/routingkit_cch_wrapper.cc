#include "routingkit_cch_wrapper.h"
#include "rust/cxx.h" // rust::Slice definition

#include <routingkit/customizable_contraction_hierarchy.h>
#include <routingkit/nested_dissection.h>
#include <routingkit/constants.h>
#include <stdexcept>
#include <functional>

using namespace RoutingKit;

std::unique_ptr<CCH> cch_new(rust::Slice<const uint32_t> order,
                             rust::Slice<const uint32_t> tail,
                             rust::Slice<const uint32_t> head,
                             rust::Fn<void(rust::Str)> log_message,
                             bool filter_always_inf_arcs)
{
    // copy from rust::Slice to std::vector
    auto to_vec = [](rust::Slice<const uint32_t> s)
    {
        std::vector<unsigned> v;
        v.reserve(s.size());
        for (size_t i = 0; i < s.size(); ++i)
            v.push_back(s[i]);
        return v;
    };
    CustomizableContractionHierarchy cch(
        to_vec(order),
        to_vec(tail),
        to_vec(head),
        [log_message](const std::string &msg)
        { log_message(msg); },
        filter_always_inf_arcs);
    return std::unique_ptr<CCH>(new CCH(std::move(cch)));
}

std::unique_ptr<CCHMetric> cch_metric_new(const CCH &cch, rust::Slice<const uint32_t> weight)
{
    // Zero-copy: directly use pointer into Rust slice.
    CustomizableContractionHierarchyMetric metric(cch.inner, reinterpret_cast<const unsigned *>(weight.data()));
    return std::unique_ptr<CCHMetric>(new CCHMetric(std::move(metric)));
}

void cch_metric_customize(CCHMetric &metric)
{
    metric.inner.customize();
}

void cch_metric_reset(CCHMetric &metric, rust::Slice<const uint32_t> weights)
{
    // Rebinds the weight pointer while reusing the internal forward/backward buffers.
    metric.inner.reset(reinterpret_cast<const unsigned *>(weights.data()));
}

void cch_metric_parallel_customize(CCHMetric &metric, uint32_t thread_count)
{
    RoutingKit::CustomizableContractionHierarchyParallelization par(*metric.inner.cch);
    if (thread_count == 0)
    {
        par.customize(metric.inner); // internal picks #procs (or 1 without OpenMP)
    }
    else
    {
        par.customize(metric.inner, thread_count);
    }
}

std::unique_ptr<CCHQuery> cch_query_new(const CCHMetric &metric)
{
    CustomizableContractionHierarchyQuery q(metric.inner);
    return std::unique_ptr<CCHQuery>(new CCHQuery(std::move(q)));
}

void cch_query_reset(CCHQuery &query, const CCHMetric &metric)
{
    query.inner.reset(metric.inner);
}

void cch_query_add_source(CCHQuery &query, uint32_t s, uint32_t dist)
{
    query.inner.add_source(s, dist);
}

void cch_query_add_target(CCHQuery &query, uint32_t t, uint32_t dist)
{
    query.inner.add_target(t, dist);
}

void cch_query_run(CCHQuery &query)
{
    query.inner.run();
}

uint32_t cch_query_distance(const CCHQuery &query)
{
    auto &mut_query = const_cast<RoutingKit::CustomizableContractionHierarchyQuery &>(query.inner);
    return mut_query.get_distance();
}

rust::Vec<uint32_t> cch_query_node_path(const CCHQuery &query)
{
    auto &mut_query = const_cast<RoutingKit::CustomizableContractionHierarchyQuery &>(query.inner);
    auto path = mut_query.get_node_path();
    rust::Vec<uint32_t> out;
    out.reserve(path.size());
    for (auto x : path)
        out.push_back(static_cast<uint32_t>(x));
    return out;
}

rust::Vec<uint32_t> cch_query_arc_path(const CCHQuery &query)
{
    auto &mut_query = const_cast<RoutingKit::CustomizableContractionHierarchyQuery &>(query.inner);
    auto path = mut_query.get_arc_path();
    rust::Vec<uint32_t> out;
    out.reserve(path.size());
    for (auto x : path)
        out.push_back(static_cast<uint32_t>(x));
    return out;
}

namespace
{
    std::vector<unsigned> to_uvec_helper(rust::Slice<const uint32_t> s)
    {
        std::vector<unsigned> v;
        v.reserve(s.size());
        for (size_t i = 0; i < s.size(); ++i)
            v.push_back(s[i]);
        return v;
    }
}

void cch_query_pin_targets(CCHQuery &query, rust::Slice<const uint32_t> targets)
{
    query.inner.pin_targets(to_uvec_helper(targets));
}

void cch_query_run_to_pinned_targets(CCHQuery &query)
{
    query.inner.run_to_pinned_targets();
}

rust::Vec<uint32_t> cch_query_distances_to_targets(const CCHQuery &query)
{
    auto &mut_query = const_cast<RoutingKit::CustomizableContractionHierarchyQuery &>(query.inner);
    auto dist = mut_query.get_distances_to_targets();
    rust::Vec<uint32_t> out;
    out.reserve(dist.size());
    for (auto x : dist)
        out.push_back(static_cast<uint32_t>(x));
    return out;
}

void cch_query_reset_source(CCHQuery &query)
{
    query.inner.reset_source();
}

void cch_query_pin_sources(CCHQuery &query, rust::Slice<const uint32_t> sources)
{
    query.inner.pin_sources(to_uvec_helper(sources));
}

void cch_query_run_to_pinned_sources(CCHQuery &query)
{
    query.inner.run_to_pinned_sources();
}

rust::Vec<uint32_t> cch_query_distances_to_sources(const CCHQuery &query)
{
    auto &mut_query = const_cast<RoutingKit::CustomizableContractionHierarchyQuery &>(query.inner);
    auto dist = mut_query.get_distances_to_sources();
    rust::Vec<uint32_t> out;
    out.reserve(dist.size());
    for (auto x : dist)
        out.push_back(static_cast<uint32_t>(x));
    return out;
}

void cch_query_reset_target(CCHQuery &query)
{
    query.inner.reset_target();
}

rust::Vec<uint32_t> cch_compute_order_inertial(
    uint32_t node_count,
    rust::Slice<const uint32_t> tail,
    rust::Slice<const uint32_t> head,
    rust::Slice<const float> latitude,
    rust::Slice<const float> longitude)
{
    auto to_uvec = [](rust::Slice<const uint32_t> s)
    {
        std::vector<unsigned> v;
        v.reserve(s.size());
        for (size_t i = 0; i < s.size(); ++i)
            v.push_back(s[i]);
        return v;
    };
    std::vector<float> lat;
    lat.reserve(latitude.size());
    std::vector<float> lon;
    lon.reserve(longitude.size());
    for (size_t i = 0; i < latitude.size(); ++i)
        lat.push_back(latitude[i]);
    for (size_t i = 0; i < longitude.size(); ++i)
        lon.push_back(longitude[i]);
    auto order = RoutingKit::compute_nested_node_dissection_order_using_inertial_flow(
        node_count,
        to_uvec(tail),
        to_uvec(head),
        lat,
        lon,
        [](const std::string &) {});
    rust::Vec<uint32_t> out;
    out.reserve(order.size());
    for (auto x : order)
        out.push_back(static_cast<uint32_t>(x));
    return out;
}

rust::Vec<uint32_t> cch_compute_order_degree(
    uint32_t node_count,
    rust::Slice<const uint32_t> tail,
    rust::Slice<const uint32_t> head)
{
    std::vector<uint32_t> deg(node_count, 0);
    for (size_t i = 0; i < tail.size(); ++i)
    {
        auto u = tail[i];
        auto v = head[i];
        if (u < node_count)
            deg[u]++;
        if (v < node_count)
            deg[v]++;
    }
    std::vector<uint32_t> nodes(node_count);
    for (uint32_t i = 0; i < node_count; ++i)
        nodes[i] = i;
    std::sort(nodes.begin(), nodes.end(), [&](uint32_t a, uint32_t b)
              {
        if(deg[a] != deg[b]) return deg[a] < deg[b];
        return a < b; });
    rust::Vec<uint32_t> out;
    out.reserve(nodes.size());
    for (auto x : nodes)
        out.push_back(x);
    return out;
}

// -------- Partial customization wrappers --------
std::unique_ptr<CCHPartial> cch_partial_new(const CCH &cch)
{
    return std::unique_ptr<CCHPartial>(new CCHPartial(cch.inner));
}

void cch_partial_reset(CCHPartial &partial)
{
    partial.inner.reset();
}

void cch_partial_update_arc(CCHPartial &partial, uint32_t arc)
{
    partial.inner.update_arc(arc);
}

void cch_partial_customize(CCHPartial &partial, CCHMetric &metric)
{
    partial.inner.customize(metric.inner);
}
