use std::ops::Range;
use truff_core::navigation::{RowId, RowItem};

struct TreePath {
    path: String,
    components: Range<usize>,
}

impl TreePath {
    fn sort_key<'a>(
        &'a self,
        components: &'a [Range<usize>],
    ) -> impl Iterator<Item = (bool, &'a str)> {
        components[self.components.clone()]
            .iter()
            .enumerate()
            .map(|(index, range)| {
                (
                    index + 1 == self.components.len(),
                    &self.path[range.clone()],
                )
            })
    }
}

pub(super) fn build_tree_items(files: Vec<String>) -> Vec<RowItem> {
    // Parse once: repeatedly scanning separators during sorting dominates large catalogs.
    let mut components = Vec::new();
    let mut paths: Vec<TreePath> = files
        .into_iter()
        .map(|path| {
            let first = components.len();
            let mut start = 0;
            for (end, byte) in path.bytes().enumerate() {
                if byte == b'/' || byte == b'\\' {
                    if start < end {
                        components.push(start..end);
                    }
                    start = end + 1;
                }
            }
            if start < path.len() {
                components.push(start..path.len());
            }
            TreePath {
                path,
                components: first..components.len(),
            }
        })
        .collect();
    // Stable order preserves input order for paths equal after separator normalization.
    paths.sort_by(|a, b| {
        if let (Some(a_last), Some(b_last)) = (
            components[a.components.clone()].last(),
            components[b.components.clone()].last(),
        ) && a.path[..a_last.start] == b.path[..b_last.start]
        {
            return a.path[a_last.clone()].cmp(&b.path[b_last.clone()]);
        }
        a.sort_key(&components).cmp(b.sort_key(&components))
    });
    let mut items: Vec<RowItem> = Vec::with_capacity(paths.len() + paths.len() / 4);
    let mut active_dirs: Vec<usize> = Vec::with_capacity(8);
    for TreePath {
        path,
        components: range,
    } in paths
    {
        for (depth, component) in components[range.clone()].iter().enumerate() {
            let part = &path[component.clone()];
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
    items.shrink_to_fit();
    items
}
