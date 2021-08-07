mod app_settings;
use app_settings::AppSettings;

mod protos;
use protos::gtfs_realtime::{FeedMessage, VehiclePosition_VehicleStopStatus};

extern crate protobuf;
use protobuf::Message;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::thread;

mod trip;
use trip::{Trip, TripContainer, Waypoint};

mod trip_stop;
use trip_stop::{TripStop};

fn main() {
  // Set these
  let app_settings = AppSettings {
    get_new_data: false,
    get_new_data_for_this_many_minutes: 40,
    gtfs_urls: [
      ("South-East Queensland".to_owned(), "https://gtfsrt.api.translink.com.au/api/realtime/SEQ/VehiclePositions".to_owned()),
      ("Cairns".to_owned(), "https://gtfsrt.api.translink.com.au/api/realtime/CNS/VehiclePositions".to_owned())
    ]
    .iter().cloned().collect(),
    data_path: "./data".to_owned(),
    output_path: "./output".to_owned(),
    expected_start_time: Some(1628294400), // 1628294400 == 10am
    expected_end_time: Some(1628296200) // 1628296200 == 10:30
  };
  
  if app_settings.get_new_data {
    request_gtfs_data_and_save(&app_settings.gtfs_urls,
      &app_settings.data_path,
      app_settings.get_new_data_for_this_many_minutes);
      println!("Finished data gathering");
      return;
  }

  let data_path = format!("{}/{}", app_settings.data_path, "South-East Queensland");
  let (trip_container, trip_stops) = read_files_and_parse_gtfs_data(&data_path,
    app_settings.expected_start_time, app_settings.expected_end_time);

  match write_data_to_output(trip_container, trip_stops, &app_settings.output_path) {
    Err(why) => panic!("Error outputting results: {}", why),
    Ok(_) => println!("All done"),
  }
}

fn request_gtfs_data_and_save(gtfs_urls: &HashMap<String, String>, data_path: &str, number_of_minutes: u32) {
  let mut count = 0;
  loop {
    let start = Instant::now();
    for gtfs_url_key_val in gtfs_urls {
      let mut resp = reqwest::blocking::get(gtfs_url_key_val.1).expect("Unable to query endpoint");
      let mut buf: Vec<u8> = Vec::new();
      resp.copy_to(&mut buf).expect("Writing bytes failed");
      let sub_data_path = format!("{}/{}", data_path, gtfs_url_key_val.0);
      if Path::new(data_path).exists() {
        if !Path::new(&sub_data_path).exists() {
          fs::create_dir(&sub_data_path).expect("Unable to create directory");
        }
      } else {
        panic!("Output path does not exist : {}", data_path);
      }
      let filename = format!("{}/gtfs-{}.dat", 
        sub_data_path, 
        SystemTime::now().duration_since(UNIX_EPOCH).expect("Time not working").as_secs());
      fs::write(&filename, buf).expect("Unable to write file");
      println!("File written {}", filename);
    }
    count += 1;
    if count > (number_of_minutes * 2) {
      break;
    }
    if start.elapsed() < Duration::from_secs(29) {
      thread::sleep(Duration::from_secs(30) - start.elapsed());
    }
  };
}

fn read_files_and_parse_gtfs_data(data_path: &str, expected_start_time: Option<u64>, expected_end_time: Option<u64>) -> (TripContainer, Vec<TripStop>) {
  let start_of_file_parsing = Instant::now();

  let paths = fs::read_dir(data_path).expect("Unable to read directory");
  let mut trip_container = TripContainer::new();
  for path in paths {
    let file_name = path.expect("Unable to read path").path().into_os_string().into_string().expect("Unable to get name from path");
    
    if !file_name.ends_with(".dat") {
      println!("Ignoring file {}", file_name);
      continue;
    } else {
      println!("Reading file {}", file_name);
    }

    let file_trips = parse_gtfs_data(&file_name);
    println!("{} trips in file {}", file_trips.iter().count(), file_name);

    for file_trip in file_trips {
      if !trip_container.trips.iter().any(|i| i.vehicle_id == file_trip.vehicle_id) {
        trip_container.trips.push(Trip {
          nodes: Vec::new(),
          vehicle_id: file_trip.vehicle_id.clone(),
          start_time: 0,
          end_time: 0,
          waypoints: Vec::new(),
        });
      }

      let trip_idx = trip_container.trips.iter().position(|i| i.vehicle_id == file_trip.vehicle_id).expect("Unable to get index of trip");

      for file_trip_waypoint in file_trip.waypoints {
        if !trip_container.trips[trip_idx].waypoints.iter().any(|i| i.timestamp == file_trip_waypoint.timestamp) {
          trip_container.trips[trip_idx].waypoints.push(file_trip_waypoint);
        }
      }
    }
  }

  // Sort the waypoints (needs to happen before distance counts)
  for trip in trip_container.trips.iter_mut() {
    trip.waypoints.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
  }

  println!("Total trips before conditions: {}", trip_container.trips.len());

  // Trip Filters and Conditions

  // Retain only realistic trips. ie trips that dont have movement faster than 110km/hr
  let mut unrealistic_trip_ids = Vec::new();
  for trip in trip_container.trips.iter_mut() {
    trip.waypoints.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    if expected_start_time.is_some() {
      // Remove early times if we have an expected start time
      trip.waypoints.retain(|i| i.timestamp > expected_start_time.unwrap() 
        && (expected_end_time.is_none() || i.timestamp < expected_end_time.unwrap()));
    }
    let waypoint_count = trip.waypoints.len();
    for idx in 0..waypoint_count {
      if idx+1 == waypoint_count {
        break;
      }
      let seconds_between = trip.waypoints[idx+1].timestamp - trip.waypoints[idx].timestamp;
      let fraction60: f32 = 60f32 / seconds_between as f32;
      let distance = Trip::distance_travelled_in_meters_between(&trip.waypoints[idx], &trip.waypoints[idx+1]);
      const HUNDRED_TEN_KM_PER_HR_PER_MINUTE: f32 = 1833f32; // 1833 meters per minute
      if distance > HUNDRED_TEN_KM_PER_HR_PER_MINUTE // distance between any two segments should not be too big
      || (distance * fraction60) > HUNDRED_TEN_KM_PER_HR_PER_MINUTE { // the speed between two segments should not be faster than 110km/hr
        unrealistic_trip_ids.push(trip.vehicle_id.clone());
      }
    }
  }

  // Retain trips 
  // -   that are realistic
  // -   have a minimum number of waypoints
  // -   travel a minimum distance
  trip_container.trips.retain(|i| 
    !unrealistic_trip_ids.iter().any(|u| u == &i.vehicle_id)
    && i.waypoints.iter().count() > 10 
    && i.distance_travelled_in_meters() > 1000f32
    //&& i.waypoints.iter().any(|w| w.coordinates[1] > -26.989452) // Sunshine Coast only
    //&& i.waypoints.iter().any(|w| w.coordinates[1] < -26.989452 && w.coordinates[1] > -27.731124) // Brisbane
    //&& i.waypoints.iter().any(|w| w.coordinates[1] < -27.731124) // Gold Coast
  );
  //-26.989452 north of brisbane
  //-27.731124 south of brisbane

  // end of Trip Filters and Conditions

  if trip_container.trips.len() == 0 {
    panic!("No trips match all conditions!");
  }

  println!("Total trips after conditions applied: {}", trip_container.trips.len());
  
  // Set absolute start and end timestamps
  for trip in trip_container.trips.iter_mut() {
    trip.waypoints.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    trip.start_time = trip.waypoints[0].timestamp;
    trip.end_time = trip.waypoints[trip.waypoints.len()-1].timestamp;
    if trip_container.start_timestamp == 0 || trip.start_time < trip_container.start_timestamp {
      trip_container.start_timestamp = trip.start_time;
    }
    if trip_container.loop_length == 0 || trip.end_time > trip_container.loop_length {
      trip_container.loop_length = trip.end_time;
    }
  }
  // Set loop length relative to start time
  trip_container.loop_length = trip_container.loop_length - trip_container.start_timestamp;

  // Set trip stops
  let mut trip_stops: Vec<TripStop> = Vec::new();
  let mut trip_stop_and_counts: HashMap<String, i32> = HashMap::new();
  for trip in trip_container.trips.iter_mut() {
    // Set times relative to start time
    trip.start_time = trip.start_time - trip_container.start_timestamp;
    trip.end_time = trip.end_time - trip_container.start_timestamp;
    for waypoint in trip.waypoints.iter_mut() {
      waypoint.timestamp = waypoint.timestamp - trip_container.start_timestamp;
      if waypoint.stop_id.is_some() {
        let waypoint_stop_id = waypoint.stop_id.clone().unwrap();
        if !trip.nodes.iter().any(|i| i == waypoint_stop_id.as_str()) {
          trip.nodes.push(waypoint_stop_id.clone());
        }
        if !trip_stop_and_counts.contains_key(&waypoint_stop_id) {
          trip_stops.push(TripStop {
            stop_id: waypoint_stop_id.clone(),
            coordinates: [
              waypoint.coordinates[0],
              waypoint.coordinates[1]
            ]
          });
        }
        *trip_stop_and_counts.entry(waypoint_stop_id.clone()).or_insert(0) += 1;
      }
    }
  }

  if trip_stops.len() == 0 {
    panic!("There's no trip stops!");
  }

  trip_stops.sort_by(|a, b| trip_stop_and_counts[&b.stop_id].cmp(&trip_stop_and_counts[&a.stop_id]));
  println!("Most popular trip stop {} has count {}", trip_stops[0].stop_id, trip_stop_and_counts[&trip_stops[0].stop_id]);
  let number_of_trip_stops = trip_stops.iter().len();
  if number_of_trip_stops > 50 {
    trip_stops.drain(50..);
    println!("There are {} trip stops. Keeping most popular {}", number_of_trip_stops, trip_stops.iter().len());
  }

  println!("Parsing files tool {} milliseconds", start_of_file_parsing.elapsed().as_millis());

  (trip_container, trip_stops)
}

fn parse_gtfs_data(file_name: &str) -> Vec<Trip> {
  let gtfs_bytes = fs::read(file_name).expect("Unable to read file");
  let msg = FeedMessage::parse_from_bytes(&gtfs_bytes).expect("Unable to parse protobuf data");
  let mut trips: Vec<Trip> = Vec::new();
  for f_entity in msg.entity {
    if f_entity.vehicle.is_none() {
      continue;
    }
    let vehicle = f_entity.vehicle.unwrap();
    let vehicle_timestamp = vehicle.get_timestamp();
    let stop_id = vehicle.get_stop_id().to_string();
    let vehicle_stop_status = vehicle.get_current_status();
    let vehicle_descriptor = vehicle.vehicle.unwrap();
    let vehicle_position = vehicle.position.unwrap();
    let id = vehicle_descriptor.get_id();

    let existing_trip_idx = trips.iter().position(|i| i.vehicle_id == id);
    if existing_trip_idx.is_none() {
      trips.push(Trip {
        nodes: Vec::new(),
        vehicle_id: id.to_string(),
        start_time: 0, // set later
        end_time: 0,
        waypoints: Vec::new(),
      });
    }
    let existing_trip_idx_u = trips.iter().position(|i| i.vehicle_id == id).unwrap();

    let mut waypoint = Waypoint {
      stop_id: None,
      coordinates: [
        (vehicle_position.get_longitude() * 1000000f32).round() / 1000000f32, // discard unnecessary accuracy to reduce filesize
        (vehicle_position.get_latitude() * 1000000f32).round() / 1000000f32,
      ],
      timestamp: vehicle_timestamp,
    };
    if matches!(vehicle_stop_status, VehiclePosition_VehicleStopStatus::STOPPED_AT) {
      waypoint.stop_id = Some(stop_id.clone());
    }

    if !trips[existing_trip_idx_u].waypoints.iter().any(|i| i.timestamp == vehicle_timestamp) {
      trips[existing_trip_idx_u].waypoints.push(waypoint);
      if matches!(vehicle_stop_status, VehiclePosition_VehicleStopStatus::STOPPED_AT) {
        trips[existing_trip_idx_u].nodes.push(stop_id.clone());
      }
    }
  }

  trips
}

fn get_geojson_from_trip_stops(trip_stops: Vec<TripStop>) -> (Vec<String>, String) {
  let mut trip_stop_list = Vec::new();
  let mut trip_stop_features = Vec::new();
  for trip_stop in &trip_stops {
    let trip_stop_feature =
      "{\n \"type\": \"Feature\",\"geometry\":{\"type\":\"Point\",\"coordinates\":[".to_owned()
        + trip_stop.coordinates[0].to_string().as_str()
        + ","
        + trip_stop.coordinates[1].to_string().as_str()
        + "]},\"properties\":{\"stopId\":\""
        + trip_stop.stop_id.as_str()
        + "\",\"name\":\""
        + trip_stop.stop_id.as_str()
        + "\"}}";
    trip_stop_features.push(trip_stop_feature);
    trip_stop_list.push(trip_stop.stop_id.clone());
  }
  let trip_stops_geojson = "{\n \"type\": \"FeatureCollection\",\n \"features\": [".to_owned()
    + trip_stop_features.join(",").as_str()
    + "]\n }";
  (trip_stop_list, trip_stops_geojson)
}

fn write_data_to_output(trip_container: TripContainer, trip_stops: Vec<TripStop>, output_path: &str) -> Result<(), io::Error> {
  let trip_container_json = serde_json::to_string(&trip_container).expect("Unable to serialise");
  let (trip_stops_list, trip_stops_geojson) = get_geojson_from_trip_stops(trip_stops);
  let trip_stops_json = serde_json::to_string(&trip_stops_list).expect("Unable to serialise");

  fs::write(output_path.to_owned() + "/trips.json", trip_container_json).expect("Unable to write file");
  fs::write(output_path.to_owned() + "/stops-list.json", trip_stops_json).expect("Unable to write file");
  fs::write(output_path.to_owned() + "/geojson-stops.json", trip_stops_geojson).expect("Unable to write file");

  println!("Output files written");

  Ok(())
}