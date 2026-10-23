use std::ops::Range;
use truff_core::navigation::{RowId, RowItem};

struct TreePath {
    path: String,
    components_start: u32,
    components_end: u32,
}

impl TreePath {
    #[inline]
    fn components_range(&self) -> Range<usize> {
        (self.components_start as usize)..(self.components_end as usize)
    }

    fn sort_key<'a>(
        &'a self,
        components: &'a [(u32, u32)],
    ) -> impl Iterator<Item = (bool, &'a str)> {
        let range = self.components_range();
        let len = range.len();
        components[range]
            .iter()
            .enumerate()
            .map(move |(index, &(s, e))| (index + 1 == len, &self.path[(s as usize)..(e as usize)]))
    }
}

pub(super) fn build_tree_items(files: Vec<String>) -> Vec<RowItem> {
    // Parse once: repeatedly scanning separators during sorting dominates large catalogs.
    let mut components: Vec<(u32, u32)> = Vec::with_capacity(files.len().saturating_mul(3));
    let mut paths: Vec<TreePath> = files
        .into_iter()
        .map(|path| {
            let first = components.len() as u32;
            let mut start = 0usize;
            for (end, byte) in path.bytes().enumerate() {
                if byte == b'/' || byte == b'\\' {
                    if start < end {
                        components.push((start as u32, end as u32));
                    }
                    start = end + 1;
                }
            }
            if start < path.len() {
                components.push((start as u32, path.len() as u32));
            }
            TreePath {
                path,
                components_start: first,
                components_end: components.len() as u32,
            }
        })
        .collect();
    // Stable order preserves input order for paths equal after separator normalization.
    paths.sort_by(|a, b| {
        if let (Some(&(a_s, a_e)), Some(&(b_s, b_e))) = (
            components[a.components_range()].last(),
            components[b.components_range()].last(),
        ) && a.path[..(a_s as usize)] == b.path[..(b_s as usize)]
        {
            return a.path[(a_s as usize)..(a_e as usize)]
                .cmp(&b.path[(b_s as usize)..(b_e as usize)]);
        }
        a.sort_key(&components).cmp(b.sort_key(&components))
    });

    // Count exact number of emitted rows (folders + files) so `items` can be allocated once
    // with exact capacity, avoiding a 2x peak memory spike from `shrink_to_fit()`.
    let mut exact_count = 0usize;
    let mut count_dirs: Vec<(usize, Range<usize>)> = Vec::with_capacity(8);
    for (path_idx, tree_path) in paths.iter().enumerate() {
        let range = tree_path.components_range();
        for (depth, &(s, e)) in components[range.clone()].iter().enumerate() {
            let comp_range = (s as usize)..(e as usize);
            if depth + 1 < range.len() {
                let same = count_dirs.get(depth).is_some_and(|(prev_idx, prev_range)| {
                    paths[*prev_idx].path[prev_range.clone()] == tree_path.path[comp_range.clone()]
                });
                if !same {
                    count_dirs.truncate(depth);
                    count_dirs.push((path_idx, comp_range));
                    exact_count += 1;
                }
            } else {
                count_dirs.truncate(depth);
                exact_count += 1;
                break;
            }
        }
    }
    drop(count_dirs);

    let mut items: Vec<RowItem> = Vec::with_capacity(exact_count);
    let mut active_dirs: Vec<usize> = Vec::with_capacity(8);
    for tree_path in paths {
        let range = tree_path.components_range();
        let path = tree_path.path;
        for (depth, &(s, e)) in components[range.clone()].iter().enumerate() {
            let part = &path[(s as usize)..(e as usize)];
            if depth + 1 < range.len() {
                let current = active_dirs
                    .get(depth)
                    .map(|&index| items[index].title.as_str());
                if current != Some(part) {
                    active_dirs.truncate(depth);
                    active_dirs.push(items.len());
                    items.push(RowItem {
                        id: RowId::new(items.len() as u64),
                        title: part.to_owned(),
                        depth,
                        is_folder: true,
                        is_collapsed: true,
                        data: None,
                    });
                }
            } else {
                active_dirs.truncate(depth);
                items.push(RowItem {
                    id: RowId::new(items.len() as u64),
                    title: part.to_owned(),
                    depth,
                    is_folder: false,
                    is_collapsed: false,
                    data: Some(path),
                });
                break;
            }
        }
    }
    items
}
