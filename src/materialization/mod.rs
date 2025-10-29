pub mod evaluator;

use crate::util::cache::get_cache_ttl;
use dashmap::DashMap;
use log::{debug, error};
use sqlparser::ast::{Assignment, AssignmentTarget, Expr, Statement, TableFactor};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
#[cfg(feature = "replay")]
use std::sync::Mutex;
use ttl_cache::TtlCache;
use uuid::Uuid;

/**
Divergence from the state of the original table will be stored as follows:
(Expr, HashMap<column_name, updated_value>), provided all conditions stipulated in the set are satisfied.
TODO: Prevent conflict with multiple databases that have the same table names.
TODO: Add accommodations for non-equality parameters.
*/
pub type StateDifference = TtlCache<String, (Option<Expr>, HashMap<String, Option<String>>)>;

/**
Divergence stored in the following format:
DashMap<K, V>; where K : Table, V: StateDifference
*/
pub type StateDiffLog = Arc<DashMap<String, StateDifference>>;

#[cfg(feature = "replay")]
pub type ReplayLog = Option<Arc<Mutex<TtlCache<String, String>>>>;

pub fn get_diff(map: &mut StateDiffLog, ast: &Option<Vec<Statement>>) {
    if ast.is_none() {
        return;
    }

    let statement = ast.as_ref().unwrap();

    for statement in statement {
        if let Statement::Update {
            table,
            assignments,
            selection,
            ..
        } = statement
        {
            if let TableFactor::Table {
                name,
                alias: _alias,
                ..
            } = &table.relation
            {
                let table_name = name.0.last().unwrap().clone().to_string();

                let processed_assignments = process_assignments(&assignments);

                if processed_assignments.is_err() {
                    panic_on_unsupported_behaviour(processed_assignments.err().unwrap());
                    return;
                }

                debug!("{:?}", &processed_assignments);

                update_diff_log(map, selection, &table_name, processed_assignments);
            } else {
                panic_on_unsupported_behaviour("Update query with non-relation table");
            }
        }
    }
}

fn update_diff_log(
    map: &mut StateDiffLog,
    selection: &Option<Expr>,
    table_name: &String,
    processed_assignments: Result<HashMap<String, Option<String>>, &str>,
) {
    if !map.contains_key(table_name) {
        map.insert(table_name.clone(), StateDifference::new(usize::MAX));
    }

    let mut state_difference = map.get_mut(table_name).unwrap();

    // YUCKKKKKKK. THIS IS TRASH.

    state_difference.insert(
        Uuid::new_v4().to_string(),
        (selection.clone(), processed_assignments.unwrap()),
        get_cache_ttl(),
    );
}

fn process_assignments(
    assignments: &Vec<Assignment>,
) -> Result<HashMap<String, Option<String>>, &'static str> {
    let mut processed_assignments = HashMap::default();

    for assignment in assignments {
        if let Expr::Value(value) = assignment.value.clone() {
            let assignment_target = match assignment.target.clone() {
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
        error!("{}", error);
    } else {
        error!("Ignoring unsupported query: {}", error);
    }
}
