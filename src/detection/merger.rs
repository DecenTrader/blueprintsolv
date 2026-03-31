/// Collinear segment merging to reduce object count in the vector line map (FR-026).
///
/// Two `ArchitecturalElement`s of the same type are merged when their primary
/// source segments are:
///   - ≤ 2° of angular alignment, AND
///   - nearest endpoint gap ≤ 10 pixels
///
/// A union-find groups all transitively-eligible elements into one merged
/// object, replacing its constituents in both the segment list and the
/// element list.
use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::blueprint::{
    element::ArchitecturalElement, scale::LineSegment, BoundingBox, ImagePoint, WorldPoint,
};

const MERGE_ANGLE_TOL_DEG: f64 = 2.0;
const MERGE_GAP_PX: f64 = 10.0;

/// Merge collinear same-type segments to reduce object count (FR-026).
///
/// Returns updated `(segments, elements)` where eligible groups are replaced
/// by a single merged representative.  Elements and segments not involved in
/// any merge are returned unchanged.
pub fn merge_collinear_segments(
    segments: &[LineSegment],
    elements: &[ArchitecturalElement],
) -> (Vec<LineSegment>, Vec<ArchitecturalElement>) {
    if elements.is_empty() {
        return (segments.to_vec(), Vec::new());
    }

    // Build a fast lookup from segment id → segment reference.
    let seg_map: HashMap<Uuid, &LineSegment> = segments.iter().map(|s| (s.id, s)).collect();

    let n = elements.len();
    let mut parent: Vec<usize> = (0..n).collect();

    // Find pairs eligible for merging and union them.
    for i in 0..n {
        for j in (i + 1)..n {
            if elements[i].element_type != elements[j].element_type {
                continue;
            }
            let si = primary_segment(&elements[i], &seg_map);
            let sj = primary_segment(&elements[j], &seg_map);
            if let (Some(si), Some(sj)) = (si, sj) {
                if are_merge_eligible(si, sj) {
                    let ri = uf_find(&parent, i);
                    let rj = uf_find(&parent, j);
                    if ri != rj {
                        parent[rj] = ri;
                    }
                }
            }
        }
    }

    // Group element indices by their union-find root.
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        groups.entry(uf_find(&parent, i)).or_default().push(i);
    }

    let mut out_segments: Vec<LineSegment> = Vec::new();
    let mut out_elements: Vec<ArchitecturalElement> = Vec::new();
    let mut used_seg_ids: HashSet<Uuid> = HashSet::new();

    for (rep, group) in &groups {
        if group.len() == 1 {
            // No merge — preserve original element and its source segment(s).
            let elem = &elements[group[0]];
            out_elements.push(elem.clone());
            for seg_id in &elem.source_segment_ids {
                used_seg_ids.insert(*seg_id);
                if let Some(&seg) = seg_map.get(seg_id) {
                    out_segments.push(seg.clone());
                }
            }
        } else {
            // Merge all elements in the group into one.
            let mut all_points: Vec<ImagePoint> = Vec::new();
            let mut wall_spacing: Option<f64> = None;
            let mut best_confidence: f32 = 0.0;

            for &idx in group {
                let elem = &elements[idx];
                best_confidence = best_confidence.max(elem.confidence);
                for seg_id in &elem.source_segment_ids {
                    used_seg_ids.insert(*seg_id);
                    if let Some(&seg) = seg_map.get(seg_id) {
                        all_points.extend_from_slice(&seg.points);
                        if wall_spacing.is_none() {
                            wall_spacing = seg.wall_spacing;
                        }
                    }
                }
            }

            // Derive real-world-length scale from the representative element's segment.
            let scale_ratio = primary_segment(&elements[*rep], &seg_map)
                .map(|s| {
                    if s.length_pixels > 0.0 {
                        s.real_world_length / s.length_pixels
                    } else {
                        1.0
                    }
                })
                .unwrap_or(1.0);

            let new_seg_id = Uuid::new_v4();
            let length_pixels = span_length_pixels(&all_points);
            out_segments.push(LineSegment {
                id: new_seg_id,
                points: all_points,
                length_pixels,
                real_world_length: length_pixels * scale_ratio,
                wall_spacing,
            });

            // Merged bounding box spans all constituent elements.
            let merged_bounds = group
                .iter()
                .skip(1)
                .fold(elements[group[0]].bounds, |acc, &idx| {
                    union_bounds(acc, elements[idx].bounds)
                });

            let rep_elem = &elements[*rep];
            out_elements.push(ArchitecturalElement {
                id: Uuid::new_v4(),
                element_type: rep_elem.element_type.clone(),
                bounds: merged_bounds,
                source_segment_ids: vec![new_seg_id],
                confidence: best_confidence,
                is_interior: rep_elem.is_interior,
                wall_thickness_m: rep_elem.wall_thickness_m,
            });
        }
    }

    // Retain any segments that weren't referenced by any element (pass-through).
    for seg in segments {
        if !used_seg_ids.contains(&seg.id) {
            out_segments.push(seg.clone());
        }
    }

    (out_segments, out_elements)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn primary_segment<'a>(
    elem: &ArchitecturalElement,
    seg_map: &HashMap<Uuid, &'a LineSegment>,
) -> Option<&'a LineSegment> {
    elem.source_segment_ids.first().and_then(|id| seg_map.get(id).copied())
}

/// Returns `true` if `a` and `b` are within 2° of alignment and ≤ 10 px apart.
fn are_merge_eligible(a: &LineSegment, b: &LineSegment) -> bool {
    let (a_start, a_end) = match (a.points.first(), a.points.last()) {
        (Some(&s), Some(&e)) => (s, e),
        _ => return false,
    };
    let (b_start, b_end) = match (b.points.first(), b.points.last()) {
        (Some(&s), Some(&e)) => (s, e),
        _ => return false,
    };

    let angle_a = segment_angle(a_start, a_end);
    let angle_b = segment_angle(b_start, b_end);
    if angular_diff_deg(angle_a, angle_b) > MERGE_ANGLE_TOL_DEG {
        return false;
    }

    // Minimum gap between any pair of endpoints.
    let gap = [
        dist(a_end, b_start),
        dist(a_start, b_end),
        dist(a_end, b_end),
        dist(a_start, b_start),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);

    gap <= MERGE_GAP_PX
}

fn segment_angle(start: ImagePoint, end: ImagePoint) -> f64 {
    let dx = end.x as f64 - start.x as f64;
    let dy = end.y as f64 - start.y as f64;
    dy.atan2(dx).to_degrees()
}

/// Unsigned angle difference in degrees, normalised to [0, 90] (line direction is ambiguous).
fn angular_diff_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).abs() % 180.0;
    if diff > 90.0 { 180.0 - diff } else { diff }
}

fn dist(a: ImagePoint, b: ImagePoint) -> f64 {
    let dx = a.x as f64 - b.x as f64;
    let dy = a.y as f64 - b.y as f64;
    (dx * dx + dy * dy).sqrt()
}

/// Length between first and last points of a point sequence.
fn span_length_pixels(points: &[ImagePoint]) -> f64 {
    match (points.first(), points.last()) {
        (Some(&s), Some(&e)) => dist(s, e),
        _ => 0.0,
    }
}

fn union_bounds(a: BoundingBox, b: BoundingBox) -> BoundingBox {
    BoundingBox {
        min: WorldPoint { x: a.min.x.min(b.min.x), y: a.min.y.min(b.min.y) },
        max: WorldPoint { x: a.max.x.max(b.max.x), y: a.max.y.max(b.max.y) },
    }
}

/// Simple union-find `find` (no path compression — n is small).
fn uf_find(parent: &[usize], mut i: usize) -> usize {
    while parent[i] != i {
        i = parent[i];
    }
    i
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{element::ElementType, WorldPoint};

    fn make_seg(id: Uuid, points: Vec<ImagePoint>, rw_len: f64) -> LineSegment {
        LineSegment {
            id,
            length_pixels: rw_len, // reuse field as length for tests
            real_world_length: rw_len,
            wall_spacing: None,
            points,
        }
    }

    fn make_elem(
        seg_id: Uuid,
        et: ElementType,
        min: (f64, f64),
        max: (f64, f64),
    ) -> ArchitecturalElement {
        ArchitecturalElement {
            id: Uuid::new_v4(),
            element_type: et,
            bounds: BoundingBox {
                min: WorldPoint { x: min.0, y: min.1 },
                max: WorldPoint { x: max.0, y: max.1 },
            },
            source_segment_ids: vec![seg_id],
            confidence: 0.9,
            is_interior: None,
            wall_thickness_m: None,
        }
    }

    #[test]
    fn collinear_walls_within_tolerance_are_merged() {
        // Two horizontal walls separated by a 5-pixel gap — should merge.
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let seg_a = make_seg(
            id_a,
            vec![
                ImagePoint { x: 0, y: 10 },
                ImagePoint { x: 50, y: 10 },
            ],
            50.0,
        );
        let seg_b = make_seg(
            id_b,
            vec![
                ImagePoint { x: 55, y: 10 }, // 5-px gap from seg_a end
                ImagePoint { x: 100, y: 10 },
            ],
            45.0,
        );
        let elem_a = make_elem(id_a, ElementType::Wall, (0.0, 0.0), (5.0, 0.5));
        let elem_b = make_elem(id_b, ElementType::Wall, (5.5, 0.0), (10.0, 0.5));

        let (segs, elems) =
            merge_collinear_segments(&[seg_a, seg_b], &[elem_a, elem_b]);
        assert_eq!(elems.len(), 1, "two collinear walls should merge into one");
        assert_eq!(segs.len(), 1, "merged segment count should be 1");
    }

    #[test]
    fn parallel_walls_too_far_apart_not_merged() {
        // Two horizontal walls 50 pixels apart vertically — must NOT merge.
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let seg_a = make_seg(
            id_a,
            vec![
                ImagePoint { x: 0, y: 0 },
                ImagePoint { x: 100, y: 0 },
            ],
            100.0,
        );
        let seg_b = make_seg(
            id_b,
            vec![
                ImagePoint { x: 0, y: 50 }, // 50-px perpendicular gap
                ImagePoint { x: 100, y: 50 },
            ],
            100.0,
        );
        let elem_a = make_elem(id_a, ElementType::Wall, (0.0, 0.0), (10.0, 0.0));
        let elem_b = make_elem(id_b, ElementType::Wall, (0.0, 5.0), (10.0, 5.0));

        let (segs, elems) =
            merge_collinear_segments(&[seg_a, seg_b], &[elem_a, elem_b]);
        assert_eq!(elems.len(), 2, "parallel walls with large gap should stay separate");
        assert_eq!(segs.len(), 2);
    }

    #[test]
    fn cross_type_segments_not_merged() {
        // A Wall and a Door that are otherwise collinear must NOT merge.
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let seg_a = make_seg(
            id_a,
            vec![
                ImagePoint { x: 0, y: 10 },
                ImagePoint { x: 50, y: 10 },
            ],
            50.0,
        );
        let seg_b = make_seg(
            id_b,
            vec![
                ImagePoint { x: 52, y: 10 }, // only 2-px gap
                ImagePoint { x: 100, y: 10 },
            ],
            48.0,
        );
        let elem_a = make_elem(id_a, ElementType::Wall, (0.0, 0.0), (5.0, 0.5));
        let elem_b = make_elem(id_b, ElementType::Door, (5.2, 0.0), (10.0, 0.5));

        let (segs, elems) =
            merge_collinear_segments(&[seg_a, seg_b], &[elem_a, elem_b]);
        assert_eq!(elems.len(), 2, "Wall and Door must not be merged even if collinear");
        assert_eq!(segs.len(), 2);
    }
}
