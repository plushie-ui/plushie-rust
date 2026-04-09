//! Composable query pipeline: filter, sort, paginate.
//!
//! Build a query with chained methods, then call `run()` to
//! produce a `QueryResult` with the matching entries and
//! pagination metadata.

/// The result of running a query: a page of entries with metadata.
#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    /// The entries for the requested page.
    pub entries: Vec<T>,
    /// Total entries matching the filter (before pagination).
    pub total: usize,
    /// The page number (0-indexed).
    pub page: usize,
    /// The page size used.
    pub page_size: usize,
}

/// A composable query over a slice of records.
///
/// Chain `filter`, `sort`, `page`, and `page_size` in any order,
/// then call `run` to produce a `QueryResult`.
///
/// ```ignore
/// let result = Query::new(&items)
///     .filter(|item| item.active)
///     .sort(|a, b| a.name.cmp(&b.name))
///     .page(0)
///     .page_size(10)
///     .run();
/// ```
pub struct Query<'a, T> {
    records: &'a [T],
    filter: Option<Box<dyn Fn(&T) -> bool + 'a>>,
    sort: Option<Box<dyn Fn(&T, &T) -> std::cmp::Ordering + 'a>>,
    page: usize,
    page_size: usize,
}

impl<'a, T: Clone> Query<'a, T> {
    /// Start a query over the given records.
    /// Defaults to page 0, page size 25, no filter, no sort.
    pub fn new(records: &'a [T]) -> Self {
        Self {
            records,
            filter: None,
            sort: None,
            page: 0,
            page_size: 25,
        }
    }

    /// Only include records that match the predicate.
    pub fn filter(mut self, f: impl Fn(&T) -> bool + 'a) -> Self {
        self.filter = Some(Box::new(f));
        self
    }

    /// Sort the matching records by the given comparator.
    pub fn sort(mut self, f: impl Fn(&T, &T) -> std::cmp::Ordering + 'a) -> Self {
        self.sort = Some(Box::new(f));
        self
    }

    /// Which page to return (0-indexed).
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// How many entries per page.
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size;
        self
    }

    /// Execute the query: filter, sort, paginate, return results.
    pub fn run(self) -> QueryResult<T> {
        let mut filtered: Vec<T> = match &self.filter {
            Some(f) => self.records.iter().filter(|r| f(r)).cloned().collect(),
            None => self.records.to_vec(),
        };

        if let Some(sort_fn) = &self.sort {
            filtered.sort_by(|a, b| sort_fn(a, b));
        }

        let total = filtered.len();
        let start = self.page * self.page_size;
        let entries = if start < total {
            filtered[start..total.min(start + self.page_size)].to_vec()
        } else {
            Vec::new()
        };

        QueryResult {
            entries,
            total,
            page: self.page,
            page_size: self.page_size,
        }
    }
}
