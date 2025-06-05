pub mod evaluator;

use dashmap::DashMap;
use sqlparser::ast::{Assignment, AssignmentTarget, Expr, Statement, TableFactor};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

/**
Divergence from the state of the original table will be stored as follows:
(Expr, HashMap<column_name, updated_value>), provided all conditions stipulated in the set are satisfied.
TODO: Prevent conflict with multiple databases that have the same table names.
TODO: Add accommodations for non-equality parameters.
*/
pub type StateDifference = Vec<(Option<Expr>, HashMap<String, Option<String>>)>;

/**
Divergence stored in the following format:
DashMap<K, V>; where K : Table, V: StateDifference
*/
pub type StateDiffLog = Arc<DashMap<String, StateDifference>>;

const DIALECT: MySqlDialect = MySqlDialect {};

pub fn get_diff(map: &mut StateDiffLog, query: &str) {
    let ast = Parser::parse_sql(&DIALECT, query);

    if ast.is_err() {
        return;
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

                let processed_assignments = process_assignments(assignments);

                if processed_assignments.is_err() {
                    panic_on_unsupported_behaviour(processed_assignments.err().unwrap());
                    return;
                }

                println!("{:?}", &processed_assignments);

                update_diff_log(map, selection, &table_name, processed_assignments);
            } else {
                panic_on_unsupported_behaviour("Update query with non-relation table");
            }
        }
    }
}

fn update_diff_log(
    map: &mut StateDiffLog,
    selection: Option<Expr>,
    table_name: &String,
    processed_assignments: Result<HashMap<String, Option<String>>, &str>,
) {
    if !map.contains_key(table_name) {
        map.insert(table_name.clone(), StateDifference::default());
    }

    let mut state_difference = map.get_mut(table_name).unwrap();
    state_difference.push((selection, processed_assignments.unwrap()))
}

fn process_assignments(
    assignments: Vec<Assignment>,
) -> Result<HashMap<String, Option<String>>, &'static str> {
    let mut processed_assignments = HashMap::default();

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

            let updated_value = value.clone().into_string();
            println!("updated value: {:?} {:?}", value, updated_value);
            processed_assignments.insert(column_name, updated_value);
        } else {
            panic_on_unsupported_behaviour("Non-value based assignment in update statement!");
            return Err("Non-value based assignment in update statement!");
        }
    }

    Ok(processed_assignments)
}

fn panic_on_unsupported_behaviour(error: &str) {
    if env::var("PANIC_ON_UNSUPPORTED_QUERY").is_ok_and(|val| val == "true") {
        panic!("{}", error);
    } else {
        println!("Ignoring unsupported query: {}", error);
    }
}
