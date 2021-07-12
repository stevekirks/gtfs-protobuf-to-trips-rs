pub struct AppSettings {
    /// True to get new data
    pub get_new_data: bool,
    /// How many minutes of data do you want to get
    /// Only applicable if get_new_data is true
    pub get_new_data_for_this_many_minutes: u32,
    /// The URL to get new GTFS realtime data
    /// Only applicable if get_new_data is true
    pub gtfs_url: String,
    /// Local path to save the raw GTFS realtime data
    pub data_path: String,
    /// Local path to output the transformed trip data
    pub output_path: String,
    /// Set expected start time to exclude any early times
    pub expected_start_time: Option<u64>,
    /// Set expected start time to exclude any late times
    pub expected_end_time: Option<u64>
}