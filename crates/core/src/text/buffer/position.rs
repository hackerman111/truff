use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Strongly-typed unique identifier for a text buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId(pub u64);

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

impl BufferId {
    /// Creates a `BufferId` with an explicit ID.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Allocates the next unique `BufferId`.
    pub fn next() -> Self {
        Self(
            NEXT_BUFFER_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("buffer identifier space exhausted"),
        )
    }

    /// Keeps persisted IDs out of the namespace used by subsequently allocated buffers.
    /// Returns `None` if the historical ID exhausts the identifier space.
    pub fn reserve_through(id: Self) -> Option<()> {
        let next = id.get().checked_add(1)?;
        NEXT_BUFFER_ID.fetch_max(next, Ordering::Relaxed);
        Some(())
    }

    /// Returns the raw numeric ID.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for BufferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strongly-typed byte offset within a text buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BytePos(pub usize);

impl BytePos {
    /// Position at offset 0.
    pub const ZERO: Self = Self(0);

    /// Constructs a `BytePos`.
    pub const fn new(pos: usize) -> Self {
        Self(pos)
    }

    /// Returns the raw byte offset.
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for BytePos {
    fn from(p: usize) -> Self {
        Self(p)
    }
}

impl From<BytePos> for usize {
    fn from(p: BytePos) -> Self {
        p.0
    }
}

impl std::ops::Add<usize> for BytePos {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }
}

impl std::ops::Sub<usize> for BytePos {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self {
        Self(self.0.saturating_sub(rhs))
    }
}

impl fmt::Display for BytePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
