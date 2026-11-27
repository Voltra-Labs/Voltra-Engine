//! The order a staged scene spawns in: parents before children.

/// Every index once, each after its parent — but for a loop of links,
/// which is entered wherever it is first met.
pub(super) fn parents_first(parent: &[Option<usize>]) -> Vec<usize> {
    let mut placed = vec![false; parent.len()];
    let mut in_chain = vec![false; parent.len()];
    let mut order = Vec::with_capacity(parent.len());
    let mut chain = Vec::new();
    for start in 0..parent.len() {
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            if placed[index] || in_chain[index] {
                break;
            }
            in_chain[index] = true;
            chain.push(index);
            cursor = parent[index];
        }
        for index in chain.drain(..).rev() {
            in_chain[index] = false;
            placed[index] = true;
            order.push(index);
        }
    }
    order
}
