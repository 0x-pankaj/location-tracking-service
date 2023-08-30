use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::common::types::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NearbyDriversRequest {
    pub lat: Latitude,
    pub lon: Longitude,
    pub vehicle_type: Option<VehicleType>,
    pub radius: Radius,
    pub merchant_id: MerchantId,
}

pub type NearbyDriverResponse = Vec<DriverLocation>;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetDriversLocationRequest {
    pub driver_ids: Vec<DriverId>,
}

pub type GetDriversLocationResponse = Vec<DriverLocation>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriverLocation {
    pub driver_id: DriverId,
    pub lat: Latitude,
    pub lon: Longitude,
    pub coordinates_calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub merchant_id: MerchantId,
}
