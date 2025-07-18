#[cfg(test)]
mod mysql_update_query_parameters {
  use assert_cmd::prelude::*;
  use pretty_assertions::assert_eq;
  use std::env;
  use std::fs;
  use std::io::Write;
  use std::process::Command;
  use tempfile::tempdir;

  use test_utils::test_utils::TSString;
  use test_utils::{run_test, sandbox::TestConfig};

  #[rustfmt::skip]
run_test!(should_pick_query_params_from_single_row_of_values, TestConfig::new("mysql", true, None, None),

//// TS query ////
r#"
const someUpdateQuery = sql`
UPDATE items
JOIN inventory ON inventory.id = items.inventory_id
SET items.name = ?
WHERE inventory.id = ?
`
"#,

//// Generated TS interfaces ////
r#"
export type SomeUpdateQueryParams = [string, number];

export interface ISomeUpdateQueryResult {
    
}

export interface ISomeUpdateQueryQuery {
    params: SomeUpdateQueryParams;
    result: ISomeUpdateQueryResult;
}
"#);

  #[rustfmt::skip]
run_test!(should_pick_query_params_from_multiple_rows_of_values, TestConfig::new("mysql", true, None, None),

//// TS query ////
r#"
const someUpdateQuery = sql`
UPDATE items
JOIN inventory ON inventory.id = items.inventory_id
SET
    items.name = ?,
    inventory.quantity = ?
WHERE inventory.id = ?
`
"#,

//// Generated TS interfaces ////
r#"
export type SomeUpdateQueryParams = [string, number | null, number];

export interface ISomeUpdateQueryResult {
    
}

export interface ISomeUpdateQueryQuery {
    params: SomeUpdateQueryParams;
    result: ISomeUpdateQueryResult;
}
"#);
}
