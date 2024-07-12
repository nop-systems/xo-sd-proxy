use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Vm {
    pub(crate) name_label: String,
    pub(crate) tags: Vec<String>,
    #[serde(rename = "mainIpAddress")]
    pub(crate) main_ip_address: Option<String>,
    #[allow(dead_code)]
    pub(crate) href: String,
}
