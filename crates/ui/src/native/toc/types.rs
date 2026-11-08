use truff_core::markdown::facts::DocFacts;

/// Scope of the Table of Contents hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TocScope {
    #[default]
    ActiveFile,
    Folder,
    Vault,
}

impl TocScope {
    pub const ALL: [Self; 3] = [Self::ActiveFile, Self::Folder, Self::Vault];

    pub fn label(&self) -> &'static str {
        match self {
            Self::ActiveFile => "Активная заметка",
            Self::Folder => "Текущая папка",
            Self::Vault => "Всё хранилище",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::ActiveFile => Self::Folder,
            Self::Folder => Self::Vault,
            Self::Vault => Self::ActiveFile,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::ActiveFile => Self::Vault,
            Self::Folder => Self::ActiveFile,
            Self::Vault => Self::Folder,
        }
    }
}

/// The specific payload of a TOC tree item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TocItemKind {
    Folder {
        path: String,
    },
    Note {
        path: String,
    },
    Heading {
        level: u8,
        text: String,
        line: usize,
        byte_offset: usize,
    },
    Block {
        id: String,
        line: usize,
        byte_offset: usize,
    },
    Related {
        path: String,
    },
}

/// A single row / item in the hierarchical TOC tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocItem {
    pub id: u64,
    pub title: String,
    pub level: usize,
    pub kind: TocItemKind,
    pub line: Option<usize>,
    pub path: Option<String>,
    pub is_collapsed: bool,
    pub has_children: bool,
    pub parent_idx: Option<usize>,
    pub is_active: bool,
}

impl TocItem {
    pub fn new_heading(
        id: u64,
        level: u8,
        text: String,
        line: usize,
        byte_offset: usize,
        path: Option<String>,
    ) -> Self {
        let display_level = level.saturating_sub(1) as usize;
        Self {
            id,
            title: text.clone(),
            level: display_level,
            kind: TocItemKind::Heading {
                level,
                text,
                line,
                byte_offset,
            },
            line: Some(line),
            path,
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_block(
        id: u64,
        block_id: String,
        line: usize,
        byte_offset: usize,
        path: Option<String>,
        parent_level: usize,
    ) -> Self {
        Self {
            id,
            title: format!("^{block_id}"),
            level: parent_level + 1,
            kind: TocItemKind::Block {
                id: block_id,
                line,
                byte_offset,
            },
            line: Some(line),
            path,
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_note(id: u64, name: String, path: String, level: usize) -> Self {
        Self {
            id,
            title: name,
            level,
            kind: TocItemKind::Note { path: path.clone() },
            line: None,
            path: Some(path),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_folder(id: u64, name: String, path: String, level: usize) -> Self {
        Self {
            id,
            title: name,
            level,
            kind: TocItemKind::Folder { path: path.clone() },
            line: None,
            path: Some(path),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_related(id: u64, target: String, level: usize) -> Self {
        Self {
            id,
            title: format!("[[{target}]]"),
            level,
            kind: TocItemKind::Related {
                path: target.clone(),
            },
            line: None,
            path: Some(target),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }
}

/// Computes the sticky parent chain (breadcrumbs) for the active item.
pub fn compute_sticky_chain(all_items: &[TocItem], active_idx: usize) -> Vec<String> {
    if active_idx >= all_items.len() {
        return Vec::new();
    }
    let mut chain = Vec::new();
    let mut curr_idx = Some(active_idx);

    while let Some(idx) = curr_idx {
        let item = &all_items[idx];
        chain.push(item.title.clone());
        curr_idx = item.parent_idx;
    }
    chain.reverse();
    chain
}

/// Identifies the active heading index in `facts` based on current cursor byte offset.
pub fn find_active_heading_index(facts: &DocFacts, caret_byte: usize) -> Option<usize> {
    if facts.headings.is_empty() {
        return None;
    }
    // Find the last heading whose starting byte range is <= caret_byte
    let mut active = None;
    for (idx, heading) in facts.headings.iter().enumerate() {
        if heading.range.start <= caret_byte {
            active = Some(idx);
        } else {
            break;
        }
    }
    active
}
