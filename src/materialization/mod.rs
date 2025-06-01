use dashmap::DashMap;
use sqlparser::ast::{Assignment, AssignmentTarget, Expr, Statement, TableFactor};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use std::collections::BTreeSet;
use std::env;
use std::ops::DerefMut;
use std::sync::Arc;

/**
Divergence from the state of the original table will be stored as follows:
Set<(column_name, Expr)> -> updated_value, provided all conditions stipulated in the set are satisfied.
TODO: Prevent conflict with multiple databases that have the same table names.
TODO: Add accommodations for non-equality parameters.
*/
pub type StateDifference = BTreeSet<(String, (Expr, Option<String>))>;

/**
Divergence stored in the following format:
DashMap<K, V>; where K : Table, V: StateDifference
*/
pub type StateDifferenceMap = Arc<DashMap<String, StateDifference>>;

const DIALECT: MySqlDialect = MySqlDialect {};

pub fn get_diff(map: &mut StateDifferenceMap, query: &String) -> Option<StateDifference> {
    let ast = Parser::parse_sql(&DIALECT, &query);

    if ast.is_err() {
        return None;
    }

    let statement = ast.unwrap();

    for statement in statement {
        if let Statement::Update {
            table,
            assignments,
            selection,
            ..
        } = statement
        {
            if let TableFactor::Table { name, alias, .. } = table.relation {
                let table_name = name.0.last().unwrap().clone().to_string();
                // let table_alias = match alias {
                //     None => None,
                //     Some(alias) => Some(alias.name.value),
                // };

                let processed_assignments = process_assignments(assignments);

                if processed_assignments.is_err() {
                    panic_on_unsupported_behaviour(processed_assignments.err().unwrap());
                    return None;
                }

                {
                    if !map.contains_key(&table_name) {
                        map.insert(table_name.clone(), StateDifference::default());
                    }

                    let mut state_difference = map.get_mut(&table_name).unwrap().deref_mut();
                    for processed_assignment in processed_assignments.unwrap() {
                        state_difference.insert(processed_assignment.0, processed_assignment.1);
                        // .insert((processed_assignment.0, processed_assignment.1));
                    }
                }

                println!("{:?}", processed_assignments);
            } else {
                panic_on_unsupported_behaviour("Update query with non-relation table");
                return None;
            }
        }
    }

    None
}

fn process_assignments(
    assignments: Vec<Assignment>,
) -> Result<Vec<(String, Option<String>)>, &'static str> {
    let mut processed_assignments = Vec::default();

    for assignment in assignments {
        if let Expr::Value(value) = assignment.value {
            let assignment_target = match assignment.target {
                AssignmentTarget::Tuple(_) => {
                    panic_on_unsupported_behaviour("Tuple assignment targets are not supported!");
                    return Err("Tuple assignment targets are not supported!");
                }
                AssignmentTarget::ColumnName(name) => Some(name),
            };

            let column_name = assignment_target
                .unwrap()
                .0
                .last()
                .unwrap()
                .as_ident()
                .unwrap()
                .value
                .clone();

            let updated_value = value.into_string();
            processed_assignments.push((column_name, updated_value));
        } else {
            panic_on_unsupported_behaviour("Non-value based assignment in update statement!");
            return Err("Non-value based assignment in update statement!");
        }
    }

    Ok(processed_assignments)
}

fn process_selection(expr: Option<Expr>) -> Result<Vec<(String, String)>, &'static str> {
    let processed_selections = Vec::new();

    if let None = expr {
        return Ok(processed_selections);
    }

    match expr {}

    Ok(processed_selections)
}

fn panic_on_unsupported_behaviour(error: &str) {
    if env::var("PANIC_ON_UNSUPPORTED_QUERY").is_ok_and(|val| val == "true") {
        panic!("{}", error);
    }
}
