use std::sync::Arc;

use futures::Future;
use shared::{redis::interface::types::RedisConnectionPool, tools::error::AppError};

use super::types::*;

// Generic Redis
pub fn on_ride_key(merchant_id: &MerchantId, city: &CityName, driver_id: &DriverId) -> String {
    format!("ds:on_ride:{merchant_id}:{city}:{driver_id}")
}

// Generic Redis
pub fn driver_details_key(driver_id: &DriverId) -> String {
    format!("ds:driver_details:{driver_id}")
}

// Generic Redis
pub fn driver_loc_ts_key(driver_id: &DriverId) -> String {
    format!("dl:ts:{}", driver_id)
}

// Generic Redis
pub fn health_check_key() -> String {
    format!("health_check")
}

// Generic Redis
pub fn driver_processing_location_update_lock_key(driver_id: &DriverId, city: &CityName) -> String {
    format!("dl:processing:{driver_id}:{city}")
}

// Location Redis
pub fn on_ride_loc_key(merchant_id: &String, city: &CityName, driver_id: &DriverId) -> String {
    format!("dl:loc:{merchant_id}:{city}:{driver_id}")
}

// Location Redis
pub fn driver_loc_bucket_key(
    merchant_id: &MerchantId,
    city: &CityName,
    vehicle_type: &VehicleType,
    bucket: &u64,
) -> String {
    format!("dl:loc:{merchant_id}:{city}:{vehicle_type}:{bucket}")
}

pub async fn with_lock_redis<F, Args, Fut>(
    redis: Arc<RedisConnectionPool>,
    key: &str,
    expiry: i64,
    callback: F,
    args: Args,
) -> Result<(), AppError>
where
    F: Fn(Args) -> Fut,
    Args: Send + 'static,
    Fut: Future<Output = Result<(), AppError>>,
{
    let lock = redis.setnx_with_expiry(key, true, expiry).await;

    if let Ok(_) = lock {
        callback(args).await?;
        let _ = redis.delete_key(key).await;
    }

    Ok(())
}
