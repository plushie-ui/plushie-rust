#![allow(clippy::type_complexity)] // Query closures are inherently complex types

//! Composable query pipeline: filter, search, sort, paginate, group.
//!
//! Build a query with chained methods, then call `run()` to
//! produce a `QueryResult` with the matching entries,
//! pagination metadata, and optional grouping.
//!
//! Pipeline order: filter -> search -> sort -> paginate -> group.

/// Sort direction for multi-field sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Ascending order (smallest first).
    Asc,
    /// Descending order (largest first).
    Desc,
}

/// The result of running a query: a page of entries with metadata.
#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    /// The entries for the requested page.
    pub entries: Vec<T>,
    /// Total entries matching the filter (before pagination).
    pub total: usize,
    /// The page number (1-indexed; first page is 1).
    pub page: usize,
    /// The page size used.
    pub page_size: usize,
    /// Grouped entries (if `group` was called).
    pub groups: Option<std::collections::HashMap<String, Vec<T>>>,
}

/// A composable query over a slice of records.
///
/// Chain `filter`, `search`, `sort`/`sort_by`, `page`, `page_size`,
/// and `group` in any order, then call `run`.
///
/// ```ignore
/// let result = Query::new(&items)
///     .filter(|item| item.active)
///     .search("alice", |item| vec![&item.name, &item.email])
///     .sort_by(vec![
///         (SortDir::Asc, Box::new(|a, b| a.name.cmp(&b.name))),
///     ])
///     .page(1)
///     .page_size(10)
///     .group(|item| item.category.clone())
///     .run();
/// ```
pub struct Query<'a, T> {
    records: &'a [T],
    filter: Option<Box<dyn Fn(&T) -> bool + 'a>>,
    search: Option<Box<dyn Fn(&T) -> bool + 'a>>,
    sort: Option<Box<dyn Fn(&T, &T) -> std::cmp::Ordering + 'a>>,
    group: Option<Box<dyn Fn(&T) -> String + 'a>>,
    page: usize,
    page_size: usize,
}

impl<'a, T: Clone> Query<'a, T> {
    /// Start a query over the given records.
    /// Defaults to page 1, page size 25, no filter, no sort.
    pub fn new(records: &'a [T]) -> Self {
        Self {
            records,
            filter: None,
            search: None,
            sort: None,
            group: None,
            page: 1,
            page_size: 25,
        }
    }

    /// Only include records that match the predicate.
    pub fn filter(mut self, f: impl Fn(&T) -> bool + 'a) -> Self {
        self.filter = Some(Box::new(f));
        self
    }

    /// Case-insensitive substring search across fields.
    ///
    /// `fields_fn` extracts searchable text from a record. The query
    /// string is matched case-insensitively against each returned field.
    ///
    /// ```ignore
    /// query.search("alice", |item| vec![&item.name, &item.email])
    /// ```
    pub fn search(mut self, query: &'a str, fields_fn: impl Fn(&T) -> Vec<&str> + 'a) -> Self {
        let q = query.to_lowercase();
        self.search = Some(Box::new(move |record: &T| {
            fields_fn(record)
                .iter()
                .any(|field| field.to_lowercase().contains(&q))
        }));
        self
    }

    /// Sort the matching records by the given comparator.
    pub fn sort(mut self, f: impl Fn(&T, &T) -> std::cmp::Ordering + 'a) -> Self {
        self.sort = Some(Box::new(f));
        self
    }

    /// Sort by multiple criteria with direction.
    ///
    /// Each spec is a `(SortDir, comparator)` pair. Records are compared
    /// by the first spec; ties are broken by subsequent specs.
    ///
    /// ```ignore
    /// query.sort_by(vec![
    ///     (SortDir::Asc, Box::new(|a, b| a.name.cmp(&b.name))),
    ///     (SortDir::Desc, Box::new(|a, b| a.age.cmp(&b.age))),
    /// ])
    /// ```
    pub fn sort_by(
        mut self,
        specs: Vec<(SortDir, Box<dyn Fn(&T, &T) -> std::cmp::Ordering + 'a>)>,
    ) -> Self {
        self.sort = Some(Box::new(move |a: &T, b: &T| {
            for (dir, cmp_fn) in &specs {
                let cmp = cmp_fn(a, b);
                if cmp != std::cmp::Ordering::Equal {
                    return match dir {
                        SortDir::Asc => cmp,
                        SortDir::Desc => cmp.reverse(),
                    };
                }
            }
            std::cmp::Ordering::Equal
        }));
        self
    }

    /// Group paginated results by a key extracted from each record.
    ///
    /// The `key_fn` returns a string group key for each record.
    /// The result's `groups` field will contain a map of key to entries.
    pub fn group(mut self, key_fn: impl Fn(&T) -> String + 'a) -> Self {
        self.group = Some(Box::new(key_fn));
        self
    }

    /// Which page to return (1-indexed; first page is 1).
    ///
    /// Values less than 1 are clamped to 1 at `run()` time.
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// How many entries per page.
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size;
        self
    }

    /// Execute the query: filter, search, sort, paginate, group.
    pub fn run(self) -> QueryResult<T> {
        // Filter
        let mut results: Vec<T> = match &self.filter {
            Some(f) => self.records.iter().filter(|r| f(r)).cloned().collect(),
            None => self.records.to_vec(),
        };

        // Search (applied after filter)
        if let Some(search_fn) = &self.search {
            results.retain(|r| search_fn(r));
        }

        // Sort
        if let Some(sort_fn) = &self.sort {
            results.sort_by(|a, b| sort_fn(a, b));
        }

        // Paginate (1-based: page 1 is the first page)
        let total = results.len();
        let page = self.page.max(1);
        let start = (page - 1).saturating_mul(self.page_size);
        let entries = if start < total {
            results[start..total.min(start.saturating_add(self.page_size))].to_vec()
        } else {
            Vec::new()
        };

        // Group (applied to the paginated entries)
        let groups = self.group.map(|key_fn| {
            let mut map: std::collections::HashMap<String, Vec<T>> =
                std::collections::HashMap::new();
            for entry in &entries {
                map.entry(key_fn(entry)).or_default().push(entry.clone());
            }
            map
        });

        QueryResult {
            entries,
            total,
            page,
            page_size: self.page_size,
            groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Query;

    #[test]
    fn extreme_page_does_not_overflow() {
        let items = vec![1, 2, 3];
        let result = Query::new(&items).page(usize::MAX).page_size(2).run();
        assert!(result.entries.is_empty());
        assert_eq!(result.total, 3);
        assert_eq!(result.page, usize::MAX);
    }

    #[test]
    fn extreme_page_size_does_not_overflow() {
        let items = vec![1, 2, 3];
        let result = Query::new(&items)
            .page(usize::MAX)
            .page_size(usize::MAX)
            .run();
        assert!(result.entries.is_empty());
        assert_eq!(result.total, 3);
        assert_eq!(result.page_size, usize::MAX);
    }
}
