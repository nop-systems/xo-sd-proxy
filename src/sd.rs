use serde::Serialize;
use std::collections::HashMap;

// stuct representing Prometheus HTTP SD targets
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "rocket::serde")]
pub struct Target {
    pub(crate) targets: Vec<String>,
    pub(crate) labels: HashMap<String, String>,
}
