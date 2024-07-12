use std::collections::HashMap;
use std::fs;
use reqwest::header::{COOKIE, HeaderMap};
use rocket::{Build, catch, catchers, get, launch, Rocket, routes, State};
use rocket::http::Status;
use rocket::request::FromRequest;
use rocket::response::content;
use rocket::serde::json::Json;
use rocket_dyn_templates::{context, Template};
use tracing::{debug, info, instrument, trace};
use tracing_subscriber;

use crate::sd::Target;

mod sd;
mod xo;

#[derive(Debug, Clone)]
pub struct Config {
    xoa_url: String,
    xoa_token: String,
}

impl Config {
    pub fn new_from_env() -> Self {
        Config {
            xoa_url: std::env::var("XOA_URL").expect("XOA_URL must be set"),
            xoa_token: fs::read_to_string(
                std::env::var("XOA_TOKEN_FILE").expect("XOA_TOKEN_FILE must be set"),
            )
                .expect("Failed to read XOA token file")
                .trim()
                .parse()
                .unwrap(),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Config {
    type Error = ();

    async fn from_request(request: &'r rocket::Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        let config = request.rocket().state::<Config>().unwrap();
        rocket::request::Outcome::Success((*config).clone())
    }
}

#[instrument]
#[launch]
async fn launch() -> Rocket<Build> {
    tracing_subscriber::fmt::init();

    rocket::build()
        .mount("/", routes![get_sd_targets])
        .register("/", catchers![get_target_list])
        .attach(Template::fairing())
        .manage(Config::new_from_env())
}


#[instrument]
#[get("/targets/<job_name>")]
async fn get_sd_targets(job_name: &str, config: &State<Config>) -> (Status, Json<Vec<Target>>) {
    let endpoints = build_sd_targets(config.inner()).await.unwrap();
    trace!("{}", serde_json::to_string(&endpoints).unwrap());

    info!("Requesting targets for job: {}", job_name);

    if endpoints.contains_key(job_name) {
        return (
            Status::Ok,
            Json((*endpoints.get(job_name).unwrap().clone()).to_owned()),
        );
    }

    (Status::NotFound, Json(Vec::new()))
}

#[instrument]
#[catch(404)]
async fn get_target_list(status: Status, req: &rocket::Request<'_>) -> (Status, content::RawHtml<Template>) {
    let config = req.guard::<Config>().await.unwrap();
    let endpoints = build_sd_targets(&config).await.unwrap();

    (
        Status::NotFound,
        content::RawHtml(Template::render(
            "targets",
            context! {jobs: &endpoints.keys().map(|x| x.to_string()).collect::<Vec<String>>()},
        )),
    )
}

#[instrument]
async fn get_vms(config: &Config) -> Result<Vec<xo::Vm>, reqwest::Error> {
    let full_url = format!(
        "{}/rest/v0/vms?fields=name_label,tags,main_ip_address",
        config.xoa_url
    );

    let mut request_header = HeaderMap::new();
    request_header.insert(
        COOKIE,
        format!("authenticationToken={}", config.xoa_token)
            .parse()
            .unwrap(),
    );

    debug!("Requesting VMs from XOA: {}", full_url);
    let client = reqwest::Client::new();
    let response_full = client.get(full_url).headers(request_header).send().await?;

    let response = response_full.json::<Vec<xo::Vm>>().await?;

    Ok(response)
}

#[instrument]
async fn build_sd_targets(config: &Config) -> Result<HashMap<String, Vec<sd::Target>>, reqwest::Error> {
    let mut endpoints: HashMap<String, Vec<sd::Target>> = HashMap::new();
    // Get VMs from XOA and assign them to targets based on the tags of format "prometheus:job:<job_name>"
    let vms = get_vms(config).await?;

    for vm in vms {
        debug!("Processing VM: {}", vm.name_label);

        let mut probes: HashMap<String, sd::Target> = HashMap::new();
        let mut global_labels: HashMap<String, String> = HashMap::new();

        if vm.main_ip_address.is_none() {
            continue;
        }

        for tag in vm.tags {
            if tag.starts_with("prom:") {
                let job_name = tag.split(":").nth(1).unwrap().split("=").next().unwrap();
                let label = tag.split("=").next().unwrap();
                let label_key = label.split(":").last().unwrap();
                let label_value = tag.split("=").last().unwrap();

                trace!(
                    "VM: {}, Job: {}, Label: {}, Value: {}",
                    vm.name_label,
                    job_name,
                    label_key,
                    label_value
                );

                if job_name.is_empty() {
                    global_labels.insert(label_key.to_string(), label_value.to_string());
                    continue;
                }

                if let Some(target) = probes.get_mut(job_name) {
                    if job_name == label_key {
                        target.targets = vec![format!(
                            "{}:{}",
                            vm.main_ip_address.clone().unwrap(),
                            label_value
                        )];
                        trace!(
                            "Setting target address: {}",
                            target.targets.first().unwrap()
                        );
                    } else {
                        target
                            .labels
                            .insert(label_key.to_string(), label_value.to_string());
                    }
                } else {
                    let mut target = sd::Target {
                        targets: vec![],
                        labels: HashMap::new(),
                    };
                    if job_name == label_key {
                        target.targets = vec![format!(
                            "{}:{}",
                            vm.main_ip_address.clone().unwrap(),
                            label_value
                        )];
                        trace!(
                            "Setting target address: {}",
                            target.targets.first().unwrap()
                        );
                    } else {
                        target
                            .labels
                            .insert(label_key.to_string(), label_value.to_string());
                    }
                    probes.insert(job_name.to_string(), target);
                }
            }
        }

        for (_, target) in &mut probes {
            target.labels.extend(global_labels.clone().into_iter());
            if target.targets.is_empty() {
                target.targets = vec![vm.main_ip_address.clone().unwrap()];
            }
        }

        trace!("Probes for {}: {:#?}", vm.name_label, probes);

        for probe in probes {
            if let Some(targets) = endpoints.get_mut(&probe.0) {
                targets.push(probe.1);
            } else {
                endpoints.insert(probe.0, vec![probe.1]);
            }
        }
    }

    Ok(endpoints)
}
