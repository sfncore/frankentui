use ftui_core::geometry::Rect;
use ftui_render::buffer::Buffer;
use ftui_render::cell::Cell;
use ftui_render::diff::BufferDiff;
use proptest::prelude::*;

#[test]
fn repro_dirty_span_off_by_one() {
    let width = 10;
    let height = 10;
    let old = Buffer::new(width, height);
    let mut new = Buffer::new(width, height);

    // CRITICAL: Clear dirty flags to enable dirty span tracking.
    // By default, new buffers are "full dirty", masking the bug.
    new.clear_dirty();

    // Make a single cell change
    new.set_raw(5, 5, Cell::from_char('X'));

    // Verify it's dirty
    assert!(new.is_row_dirty(5), "Row 5 should be dirty");

    // Compute diff
    let diff = BufferDiff::compute_dirty(&old, &new);

    // If the bug exists, this will be 0 because 5..5 is empty
    assert_eq!(
        diff.len(),
        1,
        "Diff should detect 1 change. Got {}",
        diff.len()
    );
    assert_eq!(diff.changes(), &[(5, 5)], "Change position incorrect");
}

#[test]
fn dirty_span_overflow_promotes_full_row() {
    let width = 256u16;
    let height = 4u16;
    let old = Buffer::new(width, height);
    let mut new = Buffer::new(width, height);
    new.clear_dirty();

    let max_spans = new.dirty_span_stats().max_spans_per_row as u16;
    let gap = 4u16;
    let mut x = 0u16;
    for _ in 0..=max_spans {
        if x >= width {
            break;
        }
        new.set_raw(x, 1, Cell::from_char('X'));
        x = x.saturating_add(gap);
    }

    let stats = new.dirty_span_stats();
    assert!(
        stats.overflows >= 1,
        "Expected span overflow to trigger full-row dirty. stats={stats:?}"
    );
    assert!(
        stats.rows_full_dirty >= 1,
        "Expected at least one full-dirty row after overflow. stats={stats:?}"
    );
    assert!(
        stats.span_coverage_cells >= width as usize,
        "Expected full-row coverage after overflow. stats={stats:?}"
    );

    let full = BufferDiff::compute(&old, &new);
    let dirty = BufferDiff::compute_dirty(&old, &new);
    assert_eq!(
        full.changes(),
        dirty.changes(),
        "Overflow path must match full diff. stats={stats:?}"
    );
}

#[test]
fn dirty_span_full_row_fill_matches_full_diff() {
    let width = 64u16;
    let height = 6u16;
    let old = Buffer::new(width, height);
    let mut new = Buffer::new(width, height);
    new.clear_dirty();

    new.fill(Rect::new(0, 3, width, 1), Cell::from_char('F'));

    let stats = new.dirty_span_stats();
    assert!(
        stats.rows_full_dirty >= 1,
        "Expected full-row dirty after fill. stats={stats:?}"
    );

    let full = BufferDiff::compute(&old, &new);
    let dirty = BufferDiff::compute_dirty(&old, &new);
    assert_eq!(
        full.changes(),
        dirty.changes(),
        "Full-row fill must match full diff. stats={stats:?}"
    );
}

#[test]
fn dirty_span_last_column_boundary() {
    let width = 17u16;
    let height = 5u16;
    let old = Buffer::new(width, height);
    let mut new = Buffer::new(width, height);
    new.clear_dirty();

    let x = width - 1;
    new.set_raw(x, 2, Cell::from_char('Z'));

    let full = BufferDiff::compute(&old, &new);
    let dirty = BufferDiff::compute_dirty(&old, &new);
    assert_eq!(
        full.changes(),
        dirty.changes(),
        "Last-column change must match full diff"
    );
    assert_eq!(dirty.changes(), &[(x, 2)]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn dirty_span_diff_equivalence_random_ops(
        width in 5u16..80,
        height in 5u16..40,
        ops in proptest::collection::vec((0u16..160, 0u16..160, any::<u8>()), 0..200),
    ) {
        let old = Buffer::new(width, height);
        let mut new = Buffer::new(width, height);
        new.clear_dirty();

        for (raw_x, raw_y, tag) in ops {
            let x = raw_x % width;
            let y = raw_y % height;
            let ch = char::from_u32('A' as u32 + (tag % 26) as u32).unwrap_or('A');
            new.set_raw(x, y, Cell::from_char(ch));
        }

        let full = BufferDiff::compute(&old, &new);
        let dirty = BufferDiff::compute_dirty(&old, &new);
        let stats = new.dirty_span_stats();
        prop_assert_eq!(
            full.changes(),
            dirty.changes(),
            "diff mismatch (w={}, h={}). stats={:?}",
            width,
            height,
            stats
        );
    }
}
