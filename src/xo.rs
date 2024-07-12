use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Vm {
    pub(crate) name_label: String,
    pub(crate) tags: Vec<String>,
    pub(crate) mainIpAddress: Option<String>,
    pub(crate) href: String,
}
