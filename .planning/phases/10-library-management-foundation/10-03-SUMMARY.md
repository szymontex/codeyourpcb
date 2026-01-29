---
phase: 10-library-management-foundation
plan: 03
subsystem: library
status: complete
tags: [library, fts5, search, bm25, ranking]

dependencies:
  requires:
    - phase: 10
      plan: 01
      reason: Uses Component models, ComponentId, SearchFilters, and schema.rs functions
  provides:
    - fts5-search-api
    - search-components-function
    - field-specific-search
  affects:
    - phase: 10
      plan: 04
      why: Search Manager will use these functions as backend
    - phase: 10
      plan: 05
      why: Library UI will display search results

tech-stack:
  added:
    - none: "All dependencies already present from Plan 01"
  patterns:
    - fts5-bm25-ranking: "ORDER BY rank (ascending) gives best matches first (BM25 negative scores)"
    - dynamic-sql-parameters: "Build SQL with optional filters, convert params to Vec<&dyn ToSql>"
    - field-validation: "Whitelist allowed fields to prevent SQL injection via field names"

key-files:
  created:
    - crates/cypcb-library/src/search.rs: "FTS5 search engine with 4 public functions + 7 tests"
  modified:
    - crates/cypcb-library/src/lib.rs: "Added pub mod search"
    - crates/cypcb-library/src/models.rs: "Fixed SearchFilters::default() to use limit=50"

decisions:
  - decision: "Use dynamic SQL with parameterized optional filters"
    rationale: "Allows flexible search with any combination of filters (source, category, manufacturer) without writing separate functions"
    alternatives: "Separate function per filter combination would require 2^3 = 8 functions"
    impact: "More complex parameter handling but cleaner API"

  - decision: "Validate field names against whitelist for field-specific search"
    rationale: "FTS5 field syntax (field:value) doesn't support parameterized field names, must build dynamically. Whitelist prevents SQL injection."
    alternatives: "Could use match statement on field enum, but less flexible for future field additions"
    impact: "Adding new searchable fields requires updating whitelist in search_by_field"

  - decision: "Fixed SearchFilters::default() to manually implement Default trait"
    rationale: "#[derive(Default)] doesn't respect #[serde(default = ...)] attributes, sets limit to 0 instead of 50"
    alternatives: "Could require explicit limit in all calls, but makes API harder to use"
    impact: "SearchFilters::default() now correctly returns limit=50"

metrics:
  - name: "lines-of-code"
    value: 415
    unit: "lines"
    context: "search.rs including 7 comprehensive tests"

  - name: "test-coverage"
    value: 7
    unit: "tests"
    context: "Resistor search, source filter, field search, empty query, invalid field, count, rebuild"

  - name: "duration"
    value: 3
    unit: "minutes"
    context: "From start to commit with all tests passing"

completed: 2026-01-29
---

# Phase 10 Plan 03: FTS5 Search Engine Summary

**One-liner:** FTS5 full-text search with BM25 ranking, field-specific queries, and optional filters for source/category/manufacturer.

## What Was Built

Implemented the FTS5-based search engine that all library management features depend on. Provides search_components function with BM25 relevance ranking and flexible filtering.

**Core deliverables:**

1. **search_components function:**
   - Takes query string (plain text or FTS5 syntax)
   - Applies optional SearchFilters (source, category, manufacturer, limit)
   - Returns Vec<SearchResult> ordered by BM25 rank (best matches first)
   - Handles empty queries gracefully (returns empty vec)
   - Supports prefix matching (query ending with *)
   - Supports field-specific queries (field:value syntax)

2. **search_by_field function:**
   - Field-specific search with validation
   - Whitelisted fields: source, name, category, description, manufacturer, mpn, value, package
   - Returns NotSupported error for invalid fields
   - Uses FTS5 field:value syntax internally

3. **rebuild_index function:**
   - Rebuilds FTS5 index on demand
   - Useful after bulk operations or if index becomes corrupted
   - Single SQL command: INSERT INTO components_fts(components_fts) VALUES('rebuild')

4. **component_count function:**
   - Simple utility to get total component count
   - Used for library statistics and pagination

5. **Bug Fix: SearchFilters::default():**
   - Fixed limit defaulting to 0 instead of 50
   - Manually implemented Default trait (derive doesn't respect serde attributes)
   - Critical fix - without this, all searches returned 0 results

**Test Coverage:**
- test_search_resistor: Plain text search returns 2 resistor components
- test_search_with_source_filter: Filter by source works correctly
- test_search_by_field_manufacturer: Field-specific query "manufacturer:TI"
- test_empty_query: Empty string returns empty results
- test_invalid_field: Invalid field returns NotSupported error
- test_component_count: Returns correct total
- test_rebuild_index: Index rebuild works, search still functional after

## Requirements Satisfied

| Requirement | Status | Evidence |
|-------------|--------|----------|
| **LIB-01:** Search by name/MPN/value/category | ✅ Complete | search_components searches across all indexed fields with BM25 ranking |
| **LIB-12:** Unified search across sources | ✅ Complete | Single search_components function queries all sources, optional source filter |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] SearchFilters::default() returned limit=0**

- **Found during:** Test execution - all searches returned 0 results despite FTS5 having data
- **Issue:** #[derive(Default)] doesn't respect #[serde(default = "default_limit")] attribute, set limit to 0
- **Fix:** Manually implemented Default trait with limit=50
- **Files modified:** crates/cypcb-library/src/models.rs
- **Commit:** bcb51b2 (included in main commit)

This was a critical bug that prevented search from working at all. The plan specified using SearchFilters with defaults, but the derived Default implementation didn't work correctly. Fixed immediately per Rule 1.

## Technical Achievements

**Pattern Implementations:**

1. **FTS5 BM25 Ranking:**
   - BM25 scores are NEGATIVE - lower (more negative) = better match
   - ORDER BY rank (ascending) gives best matches first
   - Verified in tests: all ranks are negative floats

2. **Dynamic SQL with Optional Filters:**
   - Build SQL string conditionally based on which filters are set
   - Build parameter vector in parallel
   - Convert Vec<String> to Vec<&dyn rusqlite::ToSql> for rusqlite
   - Supports any combination of filters without code duplication

3. **Field Validation Whitelist:**
   - FTS5 field syntax (field:value) requires field name in SQL string
   - Can't parameterize field names, must validate against whitelist
   - Prevents SQL injection via malicious field names
   - Returns clear error message listing valid fields

4. **Plain Text Query Sanitization:**
   - Escape double quotes for FTS5 safety: `query.replace('"', "\"\"")`
   - Pass through field-specific queries (contain ':')
   - Pass through prefix queries (end with '*')
   - Simple and effective approach

**Performance Characteristics:**
- FTS5 search: O(log n) with BM25 ranking
- 5-component test database: sub-millisecond searches
- Expected performance: <10ms for 100k components (per research)
- Limit parameter controls result size for pagination

**Error Handling:**
- Empty query → empty results (not an error)
- Invalid field → NotSupported error with helpful message
- Database errors propagated as LibraryError::Database
- Metadata deserialization errors mapped to rusqlite::Error

## Integration Points

**Upstream Dependencies:**
- Phase 10 Plan 01: Uses Component, ComponentId, ComponentMetadata, SearchFilters, SearchResult models
- Phase 10 Plan 01: Uses schema.rs functions (insert_component, insert_library) for test setup
- Phase 09 Plan 02: SQLite connection from Storage abstraction

**Downstream Dependencies:**
- Phase 10 Plan 04 (Search Manager): Will call search_components as backend
- Phase 10 Plan 05 (Library UI): Will display search results to user
- Future plans: Any feature needing component discovery

**API Surface:**
```rust
// Public functions from search.rs
pub fn search_components(
    conn: &Connection,
    query: &str,
    filters: &SearchFilters,
) -> Result<Vec<SearchResult>, LibraryError>;

pub fn search_by_field(
    conn: &Connection,
    field: &str,
    value: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, LibraryError>;

pub fn rebuild_index(conn: &Connection) -> Result<(), LibraryError>;

pub fn component_count(conn: &Connection) -> Result<usize, LibraryError>;
```

## Next Phase Readiness

**Blockers:** None

**Concerns:** None - search implementation is solid

**Recommendations:**
1. Plan 04 (Search Manager) should expose typeahead search with debouncing
2. Consider adding search_suggestions function for autocomplete (future enhancement)
3. Monitor search performance on large libraries (>10k components)
4. Add telemetry to track most-searched queries for cache optimization

**Missing Functionality (deferred to later plans):**
- Search Manager with debouncing (Plan 04)
- Search UI with result display (Plan 05)
- Search result caching (future optimization)
- Advanced FTS5 features (NEAR, phrase queries) - documented but not exposed in API

## Lessons Learned

**What Went Well:**
- FTS5 triggers from Plan 01 worked perfectly - no manual index management needed
- Test-driven debugging quickly identified SearchFilters::default() bug
- Plain text query sanitization simple and effective
- All 7 tests passed after bug fix

**What Could Improve:**
- Initial implementation didn't catch Default derive bug (no test called default())
- Should have verified SearchFilters::default() behavior before writing search logic
- Debug output helped identify issue quickly (SQL: "LIMIT ?2" Params: ["resistor", "0"])

**Reusable Patterns:**
- Dynamic SQL with optional filters pattern applicable to any complex query API
- Field validation whitelist prevents injection in dynamic SQL contexts
- Manual Default implementation when derive conflicts with serde attributes

## Knowledge Captured

**Critical Insights:**
1. **FTS5 BM25 scores are negative:** Lower = better match, ORDER BY rank ascending
2. **#[derive(Default)] ignores serde attributes:** Must manually implement Default if non-zero defaults needed
3. **FTS5 field syntax:** Can't parameterize field names, must validate with whitelist
4. **Vec<&dyn ToSql> conversion:** Convert owned values to refs for rusqlite params

**Research Validation:**
- Research recommended FTS5 with BM25 ranking - implemented exactly as documented
- Research warned about FTS5 negative scores - verified in tests
- Research Pattern 2 (FTS5 with triggers) continues to work flawlessly

**Next Phase Considerations:**
- Search Manager (Plan 04) should debounce typeahead queries (300ms recommended)
- UI should display BM25 scores for debugging (hidden from end users)
- Consider caching popular queries in future (resistor, capacitor, 0805, etc.)

## Documentation

**Files Created:**
- `/workspace/codeyourpcb/crates/cypcb-library/src/search.rs` - 415 lines including tests

**Files Modified:**
- `/workspace/codeyourpcb/crates/cypcb-library/src/lib.rs` - Added pub mod search
- `/workspace/codeyourpcb/crates/cypcb-library/src/models.rs` - Fixed SearchFilters::default()

**Total Impact:**
- 415 lines of new code (search.rs)
- 15 lines modified (lib.rs + models.rs)
- 7 passing tests
- 0 warnings, 0 errors

**Git History:**
```
bcb51b2 feat(10-03): implement FTS5 search engine with BM25 ranking
```

---

*Phase 10 Plan 03 completed successfully on 2026-01-29*
*FTS5 search engine ready for Search Manager and Library UI*
