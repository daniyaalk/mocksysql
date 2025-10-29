use std::env;
use std::sync::RwLock;
use std::time::Duration;

static CACHE_TTL: RwLock<Option<u64>> = RwLock::new(None);

pub fn get_cache_ttl() -> Duration {
    let ttl = match *CACHE_TTL.read().unwrap() {
        Some(ttl_config) => get_duration_from_ttl(ttl_config),
        None => {
            let ttl_config = env::var("DIFF_TTL")
                .unwrap_or("0".to_string())
                .parse()
                .unwrap_or(0);
            get_duration_from_ttl(ttl_config)
        }
    };

    CACHE_TTL.write().unwrap().replace(ttl.as_secs());

    Duration::from_secs(ttl.as_secs())
}

fn get_duration_from_ttl(ttl: u64) -> Duration {
    if ttl == 0 {
        Duration::from_secs(10000000)
    } else {
        Duration::from_secs(ttl)
    }
}
