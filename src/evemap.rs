use eyre::{eyre, WrapErr};
use rayon::prelude::*;
use sde::SdeZipReader;
use std::collections::HashMap;
use std::fmt::Display;
use std::io;

use crate::sde;

/// NUM_IN_PLACE_JUMPS is used by the Neighbours type which has enum variants for an in place array
/// as well as a dynamically grown vector. The in place jump array serves for many systems at a count of <=3
/// and avoids an extra indirection to a separate heap allocated vec.
/// Note: it's _probably_ entirely irrelevant as an optimization and should probably be removed
const NUM_IN_PLACE_JUMPS: usize = 3;

/// Map is a wrapper around a number of structures that allow you to work with an Eve Map
pub struct Map {
    /// systems is a packed vector of solarsystems including the minimal SolarSystemMapItem
    /// this is basically only the SolarSystemId, and the list of neighbours
    /// it is intended to be indexed by a SolarSystemIndex
    systems: Vec<SolarSystemMapItem>,
    /// extended_systems is a pair to systems, but includes more information not strictly required
    /// to expand and explore neighbours
    extended_systems: Vec<SolarSystemEx>,
    /// lookup for a system name to an ID. Because it's not strictly limited to Systems, could yield an ID
    /// for something that would not be contained in system_id_to_index
    name_to_id: HashMap<String, u64>,
    /// lookup to convert a SolarSystemId to a SolarSystemIndex for direct lookups in the vec
    system_id_to_index: HashMap<SolarSystemId, SolarSystemIndex>,
}

impl<'a> IntoIterator for &'a Map {
    type Item = &'a SolarSystemMapItem;
    type IntoIter = std::slice::Iter<'a, SolarSystemMapItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.systems.iter()
    }
}

/// SolarSystemIndex is a newtype wrapper of the offset of a SolarSystem in the solarsystems vector
/// It is intended to only be ever created with the invariant that the lookup id is valid for the
/// systems and extended_systems vecs, allowing unchecked lookups.
///
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub struct SolarSystemIndex(u16);

impl From<SolarSystemIndex> for usize {
    #[inline]
    fn from(value: SolarSystemIndex) -> Self {
        value.0 as usize
    }
}

/// SolarSystemId is a newtype wrapper of the u64 solarsystem ID from Eve
/// it is not primarily used for lookups of systems at runtime, as it's not compact and 0 based
/// there are also only ~5000 systems in Eve, which can be represented with a much smaller u16
///
/// As such assume that SolarSystemId is an artifact of reading, writing and communicating with users,
/// NOT a node identifier as used by A*
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Hash)]
pub struct SolarSystemId(u64);

#[derive(Debug, Clone)]
pub struct StargateData {
    pub stargate_id: u64,
    pub solar_system_id: SolarSystemId,
    pub destination_stargate_id: u64,
}

/// Neighbours is a structure that's either an in-place array, or a
/// Vec (with associated indirection). It's a fun experiment.
#[derive(Debug)]
pub enum Neighbours {
    /// A fixed size array that will be allocated in place
    InPlace([Option<SolarSystemIndex>; NUM_IN_PLACE_JUMPS]),
    /// A separately allocated heap vec
    Vec(Vec<SolarSystemIndex>),
}

#[derive(Debug)]
pub struct SolarSystemMapItem {
    pub solar_system_id: SolarSystemId,
    pub neighbours: once_cell::unsync::OnceCell<Neighbours>,
}

/// SolarSystemEx is a larger object with more information in it than SolarSystemMapItem
///
pub struct SolarSystemEx {
    /// solar system name in Eve Online, eg. Yulai
    pub name: String,
    /// typed 64bit solarsystem id
    pub solar_system_id: SolarSystemId,
    pub constellation_id: u64,
    pub region_id: u64,
}

impl Map {
    // TODO: Split this up into functions
    /// this function takes a ZIP containing an SDE and converts it into a Map
    /// this requires creating a number of internal lookups (eg. what system is a given gateID in?)
    /// in order to build our map, indexes and neighbours
    pub fn new<T: io::Read + Send>(reader: &mut SdeZipReader<T>) -> Result<Map, eyre::Error> {
        let mut stargates_by_system =
            Vec::<(SolarSystemId, Vec<StargateData>)>::with_capacity(6000);
        let mut stellar_items = Vec::<(u64, String, MapType)>::with_capacity(6000);

        // Read all the stellar items from the SDE (Region/Constellation/System)
        // pipe in parallel to parsing function (using rayon) and collect the result
        let p = reader
            .par_bridge() // rayon parallel iterator bridge
            .map(|(filename, file_content)| parse(filename.as_str(), file_content.as_slice()))
            .collect::<Result<Vec<_>, _>>()?;

        // flatten() unwraps the Option<>s in the reader
        for (new_stellar_item, maybe_stargates) in p.iter().flatten() {
            stellar_items.push(new_stellar_item.to_owned());
            if let Some(stargates) = maybe_stargates {
                stargates_by_system.push((*stargates).clone());
            }
        }

        // stellar items and stargates_by_system are all we care about now
        drop(p);

        // Used for name lookups, since the hierarchy is based on filename
        // and the filenames have the name of the region/constellation/system
        // NB: Strictly speaking the names should come from a translation table
        // for multiple languages, but the disk structure represents the names
        let stellar_item_name_to_id: HashMap<String, u64> = stellar_items
            .iter()
            .map(|(id, name, _)| (name.clone(), *id))
            .collect();

        let system_count: usize = stellar_items
            .iter()
            .filter(|(_, _, t)| t.is_solarsystem())
            .count();

        let mut solarsystems: Vec<SolarSystemMapItem> = Vec::with_capacity(system_count);
        let mut solarsystems_ex: Vec<SolarSystemEx> = Vec::with_capacity(system_count);

        for x in &stellar_items {
            if let (
                id,
                name,
                MapType::SolarSystem {
                    region,
                    constellation,
                },
            ) = x
            {
                solarsystems.push(SolarSystemMapItem {
                    solar_system_id: SolarSystemId(*id),
                    neighbours: Default::default(),
                });

                solarsystems_ex.push(SolarSystemEx {
                    name: name.clone(),
                    solar_system_id: SolarSystemId(*id),
                    constellation_id: *stellar_item_name_to_id
                        .get(constellation.as_str())
                        .ok_or_else(|| {
                            eyre!(
                                "constellation {} not found for system {}",
                                constellation,
                                name
                            )
                        })?,
                    region_id: *stellar_item_name_to_id
                        .get(region.as_str())
                        .ok_or_else(|| eyre!("region {} not found for system {}", region, name))?,
                });
            }
        }

        // since solarsystem ID is unique, don't need any stable sorting
        solarsystems.sort_unstable_by_key(|x| x.solar_system_id.0);
        solarsystems_ex.sort_unstable_by_key(|x| x.solar_system_id.0);

        // build a lookup of the offset of a SolarSystemID
        let solarsystem_lookup: HashMap<SolarSystemId, SolarSystemIndex> = solarsystems
            .iter()
            .enumerate()
            .map(|(i, ss)| Ok((ss.solar_system_id, SolarSystemIndex(i.try_into()?))))
            .collect::<eyre::Result<_>>()?;

        //
        let stargate_id_to_system_id: HashMap<u64, SolarSystemIndex> = stargates_by_system
            .iter()
            .flat_map(|(ssid, gates)| {
                let ss_idx = solarsystem_lookup[ssid];
                gates.iter().map(move |g| (g.stargate_id, ss_idx))
            })
            .collect();

        // Finally, build and set the neighours for each solarsystem
        for (ssid, stargates) in &stargates_by_system {
            let ss_idx = solarsystem_lookup[ssid];

            let neighbours = stargates
                .iter()
                .map(|g| stargate_id_to_system_id[&g.destination_stargate_id])
                .collect::<Neighbours>();

            if let Some(ss) = solarsystems.get_mut(ss_idx.0 as usize) {
                ss.neighbours.set(neighbours).or_else(|_| Err(eyre!("unable to set neighbours on {ssid}")))?;
            }
        }

        Ok(Map {
            systems: solarsystems,
            extended_systems: solarsystems_ex,
            name_to_id: stellar_item_name_to_id,
            system_id_to_index: solarsystem_lookup,
        })
    }
}

impl Map {
    #[inline]
    pub fn get_solarsystem_id_by_name(&self, name: &str) -> Option<SolarSystemId> {
        self.name_to_id.get(name).map(|i| SolarSystemId(*i))
    }

    #[inline]
    pub fn get_solarsystem_idx(&self, i: &SolarSystemId) -> SolarSystemIndex {
        *self.system_id_to_index.get(i).unwrap()
    }

    #[inline]
    pub fn get_system(&self, i: &SolarSystemIndex) -> &SolarSystemMapItem {
        let a = &self.systems;
        unsafe{ a.get_unchecked(usize::from(*i)) }
    }

    #[inline]
    pub fn get_neighbours(&self, i: &SolarSystemIndex) -> Box<dyn Iterator<Item=SolarSystemIndex> + '_> {
        self.get_system(i).get_neighbours()
    }

    #[inline]
    pub fn get_extended_solarsystem_info(&self, system_index: &SolarSystemIndex) -> &SolarSystemEx {
        let a = &self.extended_systems;
        unsafe{ a.get_unchecked(usize::from(*system_index)) }
    }
}

type SolarSystemStargates = Option<(SolarSystemId, Vec<StargateData>)>;
type IdNameType = (u64, String, MapType);

#[derive(serde::Serialize, serde::Deserialize)]
struct Gate {
    destination: u64,
}

/// This is the union of all the fields that we're interested in from all the different universe yaml files
#[derive(serde::Serialize, serde::Deserialize)]
struct UnionSystemData {
    #[serde(rename = "solarSystemID")]
    solar_system_id: Option<u64>,

    #[serde(rename = "constellationID")]
    constellation_id: Option<u64>,

    #[serde(rename = "regionID")]
    region_id: Option<u64>,

    stargates: Option<HashMap<u64, Gate>>
}

/// parse parses all types of Eve Map YAML files, including Regions, Constellations and Systems
/// the path hierarchy itself defines membership, and we use the path names (english) as the names for each
/// In the case of Regions we're only interested in the name and ID
/// In the case of Constellations, we're interested in the name, ID and parent region
/// In the case of systems, we're interested in the name, ID, parents (region and constellation) and the stargates
/// The stargate data will define a destination gate, which we'll have to use to reconstruct the system->system jumps later
pub fn parse(
    name: &str,
    data: &[u8],
) -> eyre::Result<
    Option<(IdNameType, SolarSystemStargates)>,
> {
    use MapType::*;
    assert_ne!(data.len(), 0);
    let yaml_value: UnionSystemData = serde_yaml::from_slice(data)
        .wrap_err_with(|| format!("Failed to load yaml file {}", name))?;

    let path: Vec<&str> = name.rsplitn(5, "/").collect();

    // The attribute representing the ID of the different types is different in each case
    let t = match path[0] {
        "solarsystem.staticdata" => StelarItemType::SolarSystem,
        "constellation.staticdata" => StelarItemType::Constellation,
        "region.staticdata" => StelarItemType::Region,
        _ => return Ok(None),
    };

    let id = match t {
        StelarItemType::SolarSystem => yaml_value.solar_system_id.ok_or(eyre!("file did not contain \"solarSystemID\" field"))?,
        StelarItemType::Constellation => yaml_value.constellation_id.ok_or(eyre!("file did not contain \"constellationID\" field"))?,
        StelarItemType::Region => yaml_value.region_id.ok_or(eyre!("file did not contain \"regionID\" field"))?,
    };

    // make a quick lambda to simplify this below
    let path_item = |p: &Vec<&str>, i: usize| p
        .get(i)
        .ok_or(eyre!("could not get path item: {i}"))
        .map(|x| x.to_string());

    let stellar_item : IdNameType = (
        id,
        path_item(&path, 1)?,
        match t {
            StelarItemType::SolarSystem => SolarSystem {
                constellation: path_item(&path, 2).context("unable to get constellation parent of SolarSystem")?,
                region: path_item(&path, 3).context("unable to region parent of SolarSystem")?,
            },
            StelarItemType::Constellation => Constellation {
                region: path_item(&path, 2).context("unable to get region parent of constellation")?,
            },
            StelarItemType::Region => Region,
        },
    );

    if t != StelarItemType::SolarSystem {
        return Ok(Some((stellar_item, None)));
    }

    let ssid = SolarSystemId(id);

    let mut stargates = Vec::new();
    if let Some(gates) = yaml_value.stargates {
        for (stargate_id, gate) in gates {
            stargates.push(StargateData {
                stargate_id,
                solar_system_id: ssid,
                destination_stargate_id: gate.destination,
            })
        }
    }

    Ok(Some((stellar_item, Some((ssid, stargates)))))
}

impl Display for SolarSystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl SolarSystemMapItem {
    pub fn get_neighbours(&self) -> Box<dyn Iterator<Item = SolarSystemIndex> + '_> {
        match self.neighbours.get() {
            Some(Neighbours::Vec(v)) => Box::new(v.iter().copied()),
            Some(Neighbours::InPlace(a)) => Box::new(a.iter().filter_map(|n| *n)),
            None => Box::new(std::iter::empty()),
        }
    }
}

impl FromIterator<SolarSystemIndex> for Neighbours {
    fn from_iter<T: IntoIterator<Item = SolarSystemIndex>>(iter: T) -> Self {
        let values = iter.into_iter().collect::<Vec<_>>();
        if values.len() > NUM_IN_PLACE_JUMPS {
            return Neighbours::Vec(values);
        }

        let mut na: [Option<SolarSystemIndex>; NUM_IN_PLACE_JUMPS] = Default::default();
        for (i, v) in values.iter().enumerate() {
            na[i] = Some(*v);
        }
        Neighbours::InPlace(na)
    }
}

#[derive(PartialEq)]
pub enum StelarItemType {
    SolarSystem,
    Constellation,
    Region,
}

#[derive(Debug, Clone)]
pub enum MapType {
    Region,
    Constellation {
        region: String,
    },
    SolarSystem {
        region: String,
        constellation: String,
    },
}

impl MapType {
    pub fn is_solarsystem(&self) -> bool {
        matches!(self, MapType::SolarSystem {..})
    }
}