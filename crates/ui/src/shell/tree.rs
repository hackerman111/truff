use truff_core::navigation::{RowId, RowItem};

#[derive(Debug)]
struct TreeNode {
    name: String,
    full_path: Option<String>,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn new_dir(name: String) -> Self {
        Self {
            name,
            full_path: None,
            children: Vec::new(),
        }
    }

    fn new_file(name: String, full_path: String) -> Self {
        Self {
            name,
            full_path: Some(full_path),
            children: Vec::new(),
        }
    }

    fn insert(&mut self, parts: &[&str], full_path: &str) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            self.children.push(TreeNode::new_file(
                parts[0].to_string(),
                full_path.to_string(),
            ));
        } else {
            let dir_name = parts[0];
            if let Some(pos) = self
                .children
                .iter()
                .position(|c| c.name == dir_name && c.full_path.is_none())
            {
                self.children[pos].insert(&parts[1..], full_path);
            } else {
                let mut dir_node = TreeNode::new_dir(dir_name.to_string());
                dir_node.insert(&parts[1..], full_path);
                self.children.push(dir_node);
            }
        }
    }

    fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            let a_is_dir = a.full_path.is_none();
            let b_is_dir = b.full_path.is_none();
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        for child in &mut self.children {
            child.sort();
        }
    }

    fn flatten(&self, depth: usize, next_id: &mut u64, items: &mut Vec<RowItem>) {
        for child in &self.children {
            let id = RowId::new(*next_id);
            *next_id += 1;
            let is_folder = child.full_path.is_none();
            items.push(RowItem {
                id,
                title: child.name.clone(),
                depth,
                is_folder,
                is_collapsed: is_folder,
                data: child.full_path.clone(),
            });
            if is_folder {
                child.flatten(depth + 1, next_id, items);
            }
        }
    }
}

pub(super) fn build_tree_items(files: &[String]) -> Vec<RowItem> {
    let mut root = TreeNode::new_dir(String::new());
    for file in files {
        let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        root.insert(&parts, file);
    }
    root.sort();
    let mut items = Vec::new();
    let mut next_id = 0;
    root.flatten(0, &mut next_id, &mut items);
    items
}
