use snowflake::SnowflakeIdGenerator;
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

const SNOWFLAKE_EPOCH: u64 = 1669205840566;

static GENERATOR: once_cell::sync::Lazy<Mutex<SnowflakeIdGenerator>> = once_cell::sync::Lazy::new(|| {
    let epoch = UNIX_EPOCH + Duration::from_millis(SNOWFLAKE_EPOCH);
    let node = fastrand::i32(..1024);
    Mutex::new(SnowflakeIdGenerator::with_epoch(0, node, epoch))
});

pub fn next() -> i64 {
    GENERATOR.lock().unwrap().generate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next() {
        for idx in 0..10000 {
            let id = next();
            assert!(id > 0, "id: {}, idx: {}", id, idx);
        }
    }
}
