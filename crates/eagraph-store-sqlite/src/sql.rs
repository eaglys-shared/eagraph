pub const BRANCH_SCHEMA: &str = include_str!("../sql/branch_schema.sql");

pub const UPSERT_SYMBOL: &str = include_str!("../sql/queries/upsert_symbol.sql");
pub const UPSERT_EDGE: &str = include_str!("../sql/queries/upsert_edge.sql");
pub const SEARCH_SYMBOLS: &str = include_str!("../sql/queries/search_symbols.sql");
pub const SEARCH_SYMBOLS_WITH_KIND: &str =
    include_str!("../sql/queries/search_symbols_with_kind.sql");
pub const GET_NEIGHBORS_OUTGOING: &str =
    include_str!("../sql/queries/get_neighbors_outgoing.sql");
pub const GET_NEIGHBORS_INCOMING: &str =
    include_str!("../sql/queries/get_neighbors_incoming.sql");
pub const GET_NEIGHBORS_BOTH: &str = include_str!("../sql/queries/get_neighbors_both.sql");
pub const GET_SHORTEST_PATH: &str = include_str!("../sql/queries/get_shortest_path.sql");
pub const UPSERT_ANNOTATION: &str = include_str!("../sql/queries/upsert_annotation.sql");
pub const GET_ANNOTATIONS: &str = include_str!("../sql/queries/get_annotations.sql");
pub const UPSERT_FILE_RECORD: &str = include_str!("../sql/queries/upsert_file_record.sql");
pub const GET_FILE_RECORD: &str = include_str!("../sql/queries/get_file_record.sql");
