# T03: FTS5 Search Engine

**Slice:** S09 — **Milestone:** M001

## Description

Implement FTS5-based full-text search with BM25 ranking for component discovery.

Purpose: Core search engine enabling LIB-01 (search by name/MPN/value/category) and LIB-12 (unified search across sources).
Output: search.rs module with search_components function using FTS5 queries.

## Must-Haves

- [ ] "Full-text search returns ranked results across all indexed components"
- [ ] "Search supports field-specific queries (manufacturer:TI, category:Resistor)"
- [ ] "Search returns results in milliseconds, not seconds"

## Files

- `crates/cypcb-library/src/search.rs`
- `crates/cypcb-library/src/lib.rs`
