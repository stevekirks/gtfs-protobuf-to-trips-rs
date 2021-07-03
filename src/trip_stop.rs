use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct TripStop
{
    #[serde(rename(serialize = "stopId"))]
    pub stop_id: String,
    pub coordinates: [f32;2]
}