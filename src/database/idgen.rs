use snowflake::SnowflakeIdGenerator;
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

const SNOWFLAKE_EPOCH: u64 = 1669205840566;

static GENERATOR: once_cell::sync::OnceCell<Mutex<SnowflakeIdGenerator>> = once_cell::sync::OnceCell::new();

fn new() -> Mutex<SnowflakeIdGenerator> {
    let epoch = UNIX_EPOCH + Duration::from_millis(SNOWFLAKE_EPOCH);
    let machine_id = fastrand::i32(0..32);
    let node_id = fastrand::i32(0..32);
    Mutex::new(SnowflakeIdGenerator::with_epoch(machine_id, node_id, epoch))
}

pub fn next() -> i64 {
    GENERATOR.get_or_init(new).lock().unwrap().generate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next() {
        for idx in 0..10000 {
            let id = next();
            assert!(
                id > 0,
                "id: {}, idx: {}, gen: {:?}",
                id,
                idx,
                GENERATOR.get().unwrap().lock()
            );
        }
    }
}
