use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct TripContainer
{
    #[serde(rename(serialize = "startTimestamp"))]
    pub start_timestamp: u64,
    #[serde(rename(serialize = "loopLength"))]
    pub loop_length: u64,
    #[serde(rename(serialize = "timeMultiplier"))]
    pub time_multiplier: f32,
    pub trips: Vec<Trip>
}

impl TripContainer {
    pub fn new() -> Self {
        Self {
            start_timestamp: 0,
            loop_length: 0,
            time_multiplier: 1f32,
            trips: Vec::new()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Trip
{
    pub nodes: Vec<String>,
    #[serde(rename(serialize = "vehicleId"))]
    pub vehicle_id: String,
    #[serde(rename(serialize = "startTime"))]
    pub start_time: u64,
    #[serde(rename(serialize = "endTime"))]
    pub end_time: u64,
    #[serde(rename(serialize = "segments"))] // TODO: rename
    pub waypoints: Vec<Waypoint>
}

impl Trip {
    pub fn distance_travelled_in_meters(&self) -> f32 {
        let mut distance_travelled = 0f32;
        for idx in 0..self.waypoints.len() {
            if idx+1 == self.waypoints.len() {
                break;
            }
            let d = Trip::distance_travelled_in_meters_between(&self.waypoints[idx], &self.waypoints[idx+1]);
            distance_travelled += d;
        }
        distance_travelled
    }

    pub fn distance_travelled_in_meters_between(wp1: &Waypoint, wp2: &Waypoint) -> f32 {
        let lon1 = wp1.coordinates[0];
        let lon2 = wp2.coordinates[0];
        let lat1 = wp1.coordinates[1];
        let lat2 = wp2.coordinates[1];

        const R: f32 = 6371e3; // metres
        let rlat1 = lat1 * std::f32::consts::PI / 180f32; // φ1 in radians 
        let rlat2 = lat2 * std::f32::consts::PI / 180f32; // φ2
        let drlat = (lat2-lat1) * std::f32::consts::PI / 180f32; // Δφ
        let dlon = (lon2-lon1) * std::f32::consts::PI / 180f32; // Δλ

        let a = (drlat/2f32).sin() * (drlat/2f32).sin() +
                (rlat1).cos() * (rlat2).cos() *
                (dlon/2f32).sin() * (dlon/2f32).sin();
        let c = 2f32 * ((a).sqrt()).atan2((1f32-a).sqrt());

        let d = R * c; // in metres
        d
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Waypoint
{
    #[serde(skip_serializing)]
    pub stop_id: Option<String>,
    // [longitude,latitude]
    pub coordinates: [f32;2],
    pub timestamp: u64
}