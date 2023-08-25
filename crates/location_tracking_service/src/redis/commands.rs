use crate::common::types::*;
use crate::redis::keys::*;
use actix_web::web::Data;
use chrono::{DateTime, Utc};
use fred::types::{GeoPosition, GeoUnit, GeoValue, MultipleGeoValues, RedisValue, SortOrder};
use futures::Future;
use shared::utils::{logger::*, prometheus};
use shared::{redis::types::RedisConnectionPool, tools::error::AppError};
use std::sync::Arc;

pub async fn set_ride_details(
    data: Data<AppState>,
    merchant_id: &MerchantId,
    city: &CityName,
    driver_id: &DriverId,
    ride_id: RideId,
    ride_status: RideStatus,
) -> Result<(), AppError> {
    let value = RideDetails {
        ride_id,
        ride_status,
    };
    let value = serde_json::to_string(&value).unwrap();

    data.generic_redis
        .set_with_expiry(
            &on_ride_key(merchant_id, city, driver_id),
            value,
            data.redis_expiry,
        )
        .await
}

pub async fn set_driver_details(
    data: Data<AppState>,
    driver_id: DriverId,
    driver_mode: DriverMode,
) -> Result<(), AppError> {
    let value = DriverDetails {
        driver_id: driver_id.clone(),
        driver_mode,
    };
    let value = serde_json::to_string(&value).unwrap();

    data.generic_redis
        .set_with_expiry(&driver_details_key(&driver_id), value, data.redis_expiry)
        .await
}

pub async fn get_drivers_within_radius(
    data: Data<AppState>,
    merchant_id: &MerchantId,
    city: &CityName,
    vehicle: &VehicleType,
    bucket: &u64,
    location: Point,
    radius: f64,
) -> Result<Vec<DriverLocationPoint>, AppError> {
    let nearby_drivers = data
        .location_redis
        .geo_search(
            driver_loc_bucket_key(&merchant_id, &city, &vehicle, &bucket).as_str(),
            None,
            Some(GeoPosition::from((location.lon, location.lat))),
            Some((radius, GeoUnit::Kilometers)),
            None,
            Some(SortOrder::Asc),
            None,
            true,
            true,
            false,
        )
        .await?;

    let mut resp: Vec<DriverLocationPoint> = Vec::new();

    for driver in nearby_drivers {
        match driver.member {
            RedisValue::String(driver_id) => {
                let pos = driver
                    .position
                    .expect("GeoPosition not found for geo search");
                let lon = pos.longitude;
                let lat = pos.latitude;

                resp.push(DriverLocationPoint {
                    driver_id: driver_id.to_string(),
                    location: Point { lat: lat, lon: lon },
                });
            }
            _ => {
                info!("Invalid RedisValue variant");
            }
        }
    }

    return Ok(resp);
}

pub async fn push_drainer_driver_location(
    data: Data<AppState>,
    merchant_id: &MerchantId,
    city: &CityName,
    vehicle: &VehicleType,
    bucket: &u64,
    geo_entries: &Vec<(Latitude, Longitude, DriverId)>,
) -> Result<(), AppError> {
    let geo_values: Vec<GeoValue> = geo_entries
        .to_owned()
        .into_iter()
        .map(|(lat, lon, driver_id)| {
            prometheus::QUEUE_GUAGE.dec();
            GeoValue {
                coordinates: GeoPosition {
                    latitude: lat,
                    longitude: lon,
                },
                member: driver_id.into(),
            }
        })
        .collect();
    let multiple_geo_values: MultipleGeoValues = geo_values.into();

    let _ = data
        .location_redis
        .geo_add(
            &driver_loc_bucket_key(merchant_id, city, vehicle, bucket),
            multiple_geo_values,
            None,
            false,
        )
        .await?;

    return Ok(());
}

pub async fn get_and_set_driver_last_location_update_timestamp(
    data: Data<AppState>,
    driver_id: &DriverId,
) -> Result<DateTime<Utc>, AppError> {
    let last_location_update_ts = data
        .generic_redis
        .get_key(&driver_loc_ts_key(&driver_id))
        .await?;

    let _ = data
        .generic_redis
        .set_with_expiry(
            &driver_loc_ts_key(&driver_id),
            (Utc::now()).to_rfc3339(),
            data.last_location_timstamp_expiry.try_into().unwrap(),
        )
        .await?;

    if let Some(ts) = last_location_update_ts {
        if let Ok(x) = DateTime::parse_from_rfc3339(&ts) {
            return Ok(x.with_timezone(&Utc));
        }
    }

    return Err(AppError::InternalServerError);
}

pub async fn get_driver_ride_status(
    data: Data<AppState>,
    driver_id: &DriverId,
    merchant_id: &MerchantId,
    city: &CityName,
) -> Result<RideDetails, AppError> {
    let ride_details = data
        .generic_redis
        .get_key(&on_ride_key(&merchant_id, &city, &driver_id))
        .await?;

    let ride_details = match ride_details {
        Some(ride_details) => ride_details,
        None => return Err(AppError::InternalServerError),
    };

    let ride_details = serde_json::from_str::<RideDetails>(&ride_details)
        .map_err(|err| AppError::InternalError(err.to_string()))?;

    Ok(ride_details)
}

pub async fn push_on_ride_driver_location(
    data: Data<AppState>,
    driver_id: &DriverId,
    merchant_id: &MerchantId,
    city: &CityName,
    geo_entries: &Vec<(Latitude, Longitude, String)>,
) -> Result<(), AppError> {
    let geo_values: Vec<GeoValue> = geo_entries
        .to_owned()
        .into_iter()
        .map(|(lon, lat, timestamp)| {
            prometheus::QUEUE_GUAGE.dec();
            GeoValue {
                coordinates: GeoPosition {
                    longitude: lon,
                    latitude: lat,
                },
                member: timestamp.into(),
            }
        })
        .collect();
    let multiple_geo_values: MultipleGeoValues = geo_values.into();

    let _ = data
        .location_redis
        .geo_add(
            &on_ride_loc_key(&merchant_id, &city, &driver_id),
            multiple_geo_values,
            None,
            false,
        )
        .await?;

    return Ok(());
}

pub async fn get_on_ride_driver_location_count(
    data: Data<AppState>,
    driver_id: &DriverId,
    merchant_id: &MerchantId,
    city: &CityName,
) -> Result<u64, AppError> {
    let driver_location_count = data
        .location_redis
        .zcard(&on_ride_loc_key(&merchant_id, &city, &driver_id))
        .await?;

    Ok(driver_location_count)
}

pub async fn get_on_ride_driver_locations(
    data: Data<AppState>,
    driver_id: &DriverId,
    merchant_id: &MerchantId,
    city: &CityName,
) -> Result<Vec<Point>, AppError> {
    let members = data
        .location_redis
        .zrange(
            &on_ride_loc_key(&merchant_id, &city, &driver_id),
            0,
            -1,
            None,
            false,
            None,
            false,
        )
        .await?;

    let points = data
        .location_redis
        .geopos(&on_ride_loc_key(&merchant_id, &city, &driver_id), members)
        .await?;

    let _: () = data
        .location_redis
        .delete_key(&on_ride_loc_key(&merchant_id, &city, &driver_id))
        .await?;

    Ok(points
        .into_iter()
        .map(|point| {
            return Point {
                lat: point.lat,
                lon: point.lon,
            };
        })
        .collect::<Vec<Point>>())
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
        let resp = callback(args).await;
        let _ = redis.delete_key(key).await;
        resp?
    }

    Ok(())
}
