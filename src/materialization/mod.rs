use dashmap::DashMap;
use std::collections::BTreeSet;
use std::sync::Arc;

pub type StateDifferenceMap = Arc<DashMap<BTreeSet<(String, String)>, String>>;
