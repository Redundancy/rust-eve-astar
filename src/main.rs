//! AStar implementation
//!
//! This is a fairly basic AStar implementation.
//! We open or download the Eve Online Static Data Export to get a set of map data on which to operate.
//! (I strongly recommend downloading the SDE from https://developers.eveonline.com/resource/resources)
//! Loading this data creates a Map structure, which is primarily a Vec<_> of SolarSystems using SolarSystemIndex
//! as a direct lookup newtype to index jumps to other systems.
//!
//! The A* implementation is mostly templated at its core, and relies on implementations of an openlist
//! and a closedlist to work. Very minimal and simple ones are included.
//! It's generic over integer/NotNan<f32>/NotNan<f64>
extern crate core;

mod astar;
mod evemap;
mod sde;
pub(crate) mod simpleclosed;
pub(crate) mod simpleopen;

use std::io;

/// https://developers.eveonline.com/resource/resources
const EVE_SDE_ZIP_URL: &str =
    "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/sde.zip";

use crate::astar::ClosedListState::StartingPoint;
use crate::astar::{ClosedList, OpenList};
use crate::evemap::SolarSystemIndex;
use clap::Parser;
use eyre::eyre;

/// Download the Eve Online SDE (Static Data Export) and run A* on the Eve Map Data, after loading
/// it.
#[derive(clap::Parser, Debug)]
struct Args {
    /// Path for a local file version of the SDE to avoid downloading it every time
    /// Since the download is a 100MB file, this can add up (and slow you down) if you're running everything over and over
    #[arg(short, long)]
    sde_path: Option<String>,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    let now = std::time::Instant::now();
    let reader: Box<dyn io::Read + Send> = match args.sde_path {
        None => {
            #[cfg(feature = "download")]
            {
                println!("Loading SDE from source NB: This is a 100MB download each time.\nDownload from here: {EVE_SDE_ZIP_URL}");
                Box::new(reqwest::blocking::get(crate::EVE_SDE_ZIP_URL).context("failed to download SDE")?)
            }
            #[cfg(not(feature = "download"))]
            return Err(eyre!("Cannot download SDE without \"download\" feature enabled. Download manually from here: {EVE_SDE_ZIP_URL}"));
        }
        Some(filepath) => {
            println!("Loading SDE from disk.");
            let f = std::fs::File::open(&filepath).expect("Error: file not found");
            Box::new(std::io::BufReader::new(f))
        }
    };

    let mut r = sde::SdeZipReader::new(reader);
    let map = evemap::Map::new(&mut r)?;

    let yulai_ssid = map.get_solarsystem_id_by_name("Yulai").unwrap();
    let yulai_idx = map.get_solarsystem_idx(&yulai_ssid);

    println!("map loaded: {:.2} seconds", now.elapsed().as_secs_f32());
    let pathfinder_start = std::time::Instant::now();

    let mut open = simpleopen::SimpleOpenList::new();
    // TODO: Capacity based on EveMap max-index
    let mut closed = simpleclosed::SimpleClosed::new(9000);

    // NOTE: can be a NotNan<f32>
    let one_jump = 1;
    // Also works (to satisfy Ord):
    // ordered_float::NotNan::new(1.0f32)?;

    let amarr_ssid = map.get_solarsystem_id_by_name("Amarr").unwrap();
    let amarr_idx = map.get_solarsystem_idx(&amarr_ssid);

    // Start things off
    let one_jump_estimate = 1;
    open.push_open(astar::OpenItem {
        heuristic: one_jump_estimate,
        node: amarr_idx,
    });
    closed[amarr_idx] = StartingPoint(0);


    let result = astar::astar(
        &mut open,
        &mut closed,
        |n| n == &yulai_idx,
        |_| one_jump_estimate,
        |n| map.get_neighbours(n).map(|p| (one_jump, p)).collect(),
    );

    let ns_time = pathfinder_start.elapsed().as_nanos();
    println!("pathfind: {} ns ({} ms)", ns_time, ns_time/1000000);
    if let Ok(p) = result {
        for (i, id) in closed.unwind(p).iter().enumerate() {
            let info = map.get_extended_solarsystem_info(&id);
            println!("{} {} - {}", i + 1, info.name, info.solar_system_id);
        }
    }

    Ok(())
}