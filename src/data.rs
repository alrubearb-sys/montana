pub mod elevation;
pub mod forecast;
pub mod release;
pub mod search;

use std::sync::LazyLock;
use std::time::Duration;

use crate::error::Error;

const USER_AGENT: &str = concat!("peakcast/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(15);

// One agent for every request, so that requests to the same server share the connection.
static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build()
        .into()
});

fn get(url: &str, query: &[(&str, &str)]) -> Result<String, Error> {
    let mut request = AGENT.get(url);
    for (key, value) in query {
        request = request.query(key, value);
    }
    Ok(request.call()?.body_mut().read_to_string()?)
}
