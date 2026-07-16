use std::hash::Hash;
use std::num::NonZeroU32;
use std::time::Duration;

use governor::DefaultKeyedRateLimiter;
use governor::clock::{Clock, QuantaClock};

#[allow(dead_code)]
pub struct Ratelimiter<T: Hash + Eq + Clone> {
    pub clock: QuantaClock,
    pub global: Vec<DefaultKeyedRateLimiter<T>>,
    pub per_bucket: indexmap::IndexMap<&'static str, Vec<DefaultKeyedRateLimiter<T>>>,
} 

#[derive(Debug)]
pub struct RlExceeded {
    pub dur: Duration,
    pub bucket: &'static str,
    pub req_bucket: &'static str
}

impl RlExceeded {
    pub fn to_error_string(&self) -> String {
        return format!(
            "Ratelimit hit for bucket '{}', req bucket '{}', wait time: {:?}",
            self.bucket,
            self.req_bucket,
            self.dur
        )
        .into();
    }
}

#[derive(Debug)]
pub struct RlExceededError(pub RlExceeded);

impl std::fmt::Display for RlExceededError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_error_string())
    }
}

impl std::error::Error for RlExceededError {}

impl<T: Hash + Eq + Clone> Ratelimiter<T> {
    pub fn create_quota(
        limit_per: NonZeroU32,
        limit_time: Duration,
    ) -> Result<governor::Quota, crate::Error> {
        let quota = governor::Quota::with_period(limit_time)
            .ok_or("Failed to create quota")?
            .allow_burst(limit_per);

        Ok(quota)
    }

    pub fn limit(limit_per: u32, limit_time: Duration) -> DefaultKeyedRateLimiter<T> {
        let quota =
            Self::create_quota(NonZeroU32::new(limit_per).unwrap(), limit_time).expect("Failed to create quota");
        let lim = DefaultKeyedRateLimiter::keyed(quota);
        lim
    }

    pub fn check(&self, bucket: &'static str, key: T) -> Result<(), RlExceeded> {
        for global_lim in self.global.iter() {
            match global_lim.check_key(&key) {
                Ok(()) => continue,
                Err(wait) => {
                    return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket: "global", req_bucket: bucket });
                }
            };
        }

        // Check per bucket ratelimits
        if let Some(per_bucket) = self.per_bucket.get(bucket) {
            for lim in per_bucket.iter() {
                match lim.check_key(&key) {
                    Ok(()) => continue,
                    Err(wait) => {
                        return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket, req_bucket: bucket });
                    }
                };
            }
        }

        Ok(())
    }

    /// Same as check, but only checks bucket
    pub fn sub_check(&self, bucket: &'static str, key: T) -> Result<(), RlExceeded> {
        // Check per bucket ratelimits
        if let Some(per_bucket) = self.per_bucket.get(bucket) {
            for lim in per_bucket.iter() {
                match lim.check_key(&key) {
                    Ok(()) => continue,
                    Err(wait) => {
                        return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket, req_bucket: bucket });
                    }
                };
            }
        }

        Ok(())
    }
}
