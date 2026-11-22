pub mod elevation;
pub mod forecast;
pub mod search;

use std::time::Duration;

use crate::error::Error;

const USER_AGENT: &str = concat!("montana/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(15);

fn get(url: &str, query: &[(&str, &str)]) -> Result<String, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build()
        .into();
    let mut request = agent.get(url);
    for (key, value) in query {
        request = request.query(key, value);
    }
    Ok(request.call()?.body_mut().read_to_string()?)
}
