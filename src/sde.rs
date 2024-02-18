
use std::io;
use std::io::{Read};
/// SDE is the Eve Online Static Data Export
/// this module is intended to help to download a copy of the data to be used by subsequently loading
/// it into memory and performing pathfinding

pub struct SdeZipReader<T: io::Read> {
    reader: T
}

impl<T:Read> SdeZipReader<T> {
    pub fn new(reader : T) -> SdeZipReader<T> {
        SdeZipReader{reader}
    }
}

/// Read SDE yaml files incrementally and return a buffer of their contents
/// plus the filename.
impl<T: Read> Iterator for SdeZipReader<T> {
    type Item = (String, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Ok(Some(mut x)) =
            zip::read::read_zipfile_from_stream(&mut self.reader) {

            if x.is_dir() {
                continue;
            }

            let zip_file_name = x.name().to_string();
            if zip_file_name.starts_with("sde/fsd/universe/eve")
                && zip_file_name.ends_with(".staticdata")
            {
                assert_ne!(x.size(), 0);
                let mut buf = Vec::<u8>::with_capacity(x.size() as usize);
                x.read_to_end(&mut buf).ok()?;
                return Some((zip_file_name, buf));
            }
        }

        None
    }
}