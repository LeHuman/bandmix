use std::io::{Read, Write};
use std::{fs, io};

use bincode::{enc::write::Writer, Decode, Encode};
use flate2::bufread::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

struct BinIO<'a> {
    encoder: &'a mut ZlibEncoder<Vec<u8>>,
}

impl BinIO<'_> {
    fn new(encoder: &mut ZlibEncoder<Vec<u8>>) -> BinIO {
        return BinIO { encoder: encoder };
    }

    fn decompress(compressed_bytes: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut decoder = ZlibDecoder::new(compressed_bytes);
        let mut buffer = Vec::new();
        decoder.read_to_end(&mut buffer)?;
        return Ok(buffer);
    }
}

impl Writer for BinIO<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        let result = self.encoder.write(bytes);
        if result.is_err() {
            return Err(bincode::error::EncodeError::Other(
                "Encoder returned an error",
            ));
        }
        Ok(())
    }
}

pub trait Cache {
    fn _load(compressed_bytes: &[u8]) -> Result<Self, bincode::error::DecodeError>
    where
        Self: Default + Encode + Decode,
    {
        let config = bincode::config::standard();
        let bytes = match BinIO::decompress(compressed_bytes) {
            Ok(d) => d,
            Err(_) => return Err(bincode::error::DecodeError::Other("Failed to decompress")),
        };
        let (cache, _len): (Self, usize) = bincode::decode_from_slice(&bytes[..], config)?;
        return Ok(cache);
    }

    fn _dump(&self) -> Result<Vec<u8>, bincode::error::EncodeError>
    where
        Self: Default + Encode + Decode,
    {
        let config = bincode::config::standard();
        let mut encoder: ZlibEncoder<Vec<u8>> = ZlibEncoder::new(Vec::new(), Compression::best());
        let io = BinIO::new(&mut encoder);
        bincode::encode_into_writer(self, io, config)?;
        let compressed_bytes = encoder.finish();

        if compressed_bytes.is_err() {
            return Err(bincode::error::EncodeError::Other(
                "Failed to dump final compressed binary",
            ));
        }

        return Ok(compressed_bytes.expect("Failed to load compressed bytes"));
    }

    fn save(&self, cache_file: &str) -> io::Result<()>
    where
        Self: Default + Encode + Decode,
    {
        let bin = match self._dump() {
            Ok(b) => b,
            Err(_) => {
                return Err(std::io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "bin dump failed",
                ))
            }
        };
        fs::write(cache_file, bin)
    }

    fn load(cache_file: &str) -> Self
    where
        Self: Default + Encode + Decode,
    {
        let mut file = match fs::File::open(cache_file) {
            Ok(file) => file,
            Err(_) => return Self::default(),
        };

        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            return Self::default();
        }

        match Cache::_load(&buf) {
            Ok(cache) => cache,
            Err(_) => Self::default(),
        }
    }
}

#[test]
fn test_cache_trait() {
    #[derive(Eq, PartialEq, Encode, Decode, Default)]
    struct TestCache {
        val: u32,
    }
    impl Cache for TestCache {}

    let a = TestCache { val: 25 };
    let mut _b = TestCache { val: 50 };
    let result = a.save("__testing_cache_rust.cache");
    assert!(result.is_ok(), "Failed to save cache");
    _b = TestCache::load("__testing_cache_rust.cache");
    let _ = fs::remove_file("__testing_cache_rust.cache");

    assert_eq!(_b.val, a.val, "Testing cache update matches");
}
