//! Weights along one axis: the two children whose thresholds bracket the
//! input, mixed linearly between them.

/// Writes into `out` each of `n` children's weight for the input `at`,
/// child `i` sitting at `key(i)`.
///
/// Between two thresholds, the two children there mix linearly; at one,
/// it alone plays; below the lowest or above the highest, the end child
/// does (clamped: the input does not extrapolate). Children at one
/// threshold share its weight equally. A threshold that is not a finite
/// number weighs nothing; an input that is not is read as zero. The
/// weights sum to one unless no threshold is finite, when all are zero.
pub(crate) fn weights(n: usize, key: impl Fn(usize) -> f32, at: f32, out: &mut [f32]) {
    out.fill(0.0);
    let at = if at.is_finite() { at } else { 0.0 };
    // The nearest threshold at or below the input, and at or above.
    let (mut below, mut above): (Option<f32>, Option<f32>) = (None, None);
    for k in (0..n).map(&key).filter(|k| k.is_finite()) {
        if k <= at && below.is_none_or(|b| k > b) {
            below = Some(k);
        }
        if k >= at && above.is_none_or(|a| k < a) {
            above = Some(k);
        }
    }
    let (low, high) = match (below, above) {
        (Some(low), Some(high)) => (low, high),
        (None, Some(end)) | (Some(end), None) => (end, end),
        (None, None) => return,
    };
    if low == high {
        share(n, &key, low, 1.0, out);
        return;
    }
    let t = (at - low) / (high - low);
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.5
    };
    share(n, &key, low, 1.0 - t, out);
    share(n, &key, high, t, out);
}

/// Adds `weight` to the children at `threshold`, split equally.
fn share(n: usize, key: &impl Fn(usize) -> f32, threshold: f32, weight: f32, out: &mut [f32]) {
    let count = (0..n).filter(|&i| key(i) == threshold).count();
    if count == 0 {
        return;
    }
    for (i, w) in out.iter_mut().enumerate().take(n) {
        if key(i) == threshold {
            *w += weight / count as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(thresholds: &[f32], input: f32) -> Vec<f32> {
        let mut out = vec![f32::NAN; thresholds.len()];
        weights(thresholds.len(), |i| thresholds[i], input, &mut out);
        out
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6)
    }

    #[test]
    fn at_a_threshold_its_child_alone_and_between_two_a_linear_mix() {
        let idle_walk_run = [0.0, 2.0, 4.0];
        assert!(close(&at(&idle_walk_run, 0.0), &[1.0, 0.0, 0.0]));
        assert!(close(&at(&idle_walk_run, 2.0), &[0.0, 1.0, 0.0]));
        assert!(close(&at(&idle_walk_run, 3.0), &[0.0, 0.5, 0.5]));
        assert!(close(&at(&idle_walk_run, 0.5), &[0.75, 0.25, 0.0]));
    }

    #[test]
    fn outside_the_thresholds_the_end_child_plays() {
        let unordered = [4.0, 0.0, 2.0];
        assert!(close(&at(&unordered, -3.0), &[0.0, 1.0, 0.0]));
        assert!(close(&at(&unordered, 9.0), &[1.0, 0.0, 0.0]));
    }

    #[test]
    fn one_child_shared_thresholds_and_bad_numbers_stay_finite() {
        assert!(close(&at(&[5.0], -1.0), &[1.0]));
        assert!(close(&at(&[1.0, 1.0, 3.0], 1.0), &[0.5, 0.5, 0.0]));
        assert!(close(&at(&[1.0, 1.0, 3.0], 2.0), &[0.25, 0.25, 0.5]));
        assert!(close(&at(&[f32::NAN, 2.0], 0.0), &[0.0, 1.0]));
        assert!(close(&at(&[0.0, 2.0], f32::NAN), &[1.0, 0.0]), "read as 0");
        assert!(close(&at(&[f32::INFINITY], 0.0), &[0.0]));
        assert!(at(&[], 1.0).is_empty());
        let huge = at(&[-f32::MAX, f32::MAX], 0.0);
        assert!(huge.iter().all(|w| w.is_finite()), "{huge:?}");
    }
}
