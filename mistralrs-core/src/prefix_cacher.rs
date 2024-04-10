use candle_core::{Device, Result};
use indexmap::IndexMap;

use crate::{models::LayerCaches, sequence::Sequence};

pub struct PrefixCacheManager {
    caches: IndexMap<Vec<u32>, LayerCaches>,
    cpu_caches: IndexMap<Vec<u32>, LayerCaches>,
    device: Device,
    pub n_on_device: usize,
}

impl PrefixCacheManager {
    pub fn new(device: Device, n_on_device: usize) -> Self {
        PrefixCacheManager {
            caches: IndexMap::new(),
            cpu_caches: IndexMap::new(),
            device,
            n_on_device,
        }
    }

    /// This always keeps the cache on the device. If later on, a new seq cannot be allocated due to memory shortage,
    /// some caches will be evicted.
    pub fn add_sequence(&mut self, seq: &mut Sequence) {
        self.caches
            .insert(seq.get_toks().to_vec(), seq.cache().clone());
    }

    /// Evict the caches to CPU. This will evict the first k seqs such that the number of sequences on device after the copy is
    /// the maximum allowed. Returns the number of evicted sequences.
    pub fn evict_to_cpu(&mut self) -> Result<usize> {
        // Intentionally evict the first ones first, as they are the oldest
        for (ids, cache) in self.caches.drain(0..self.caches.len() - self.n_on_device) {
            let mut new_cache = Vec::new();
            for layer in cache {
                if let Some((ref q, ref k)) = layer {
                    new_cache.push(Some((
                        q.to_device(&Device::Cpu)?,
                        k.to_device(&Device::Cpu)?,
                    )));
                } else {
                    new_cache.push(None);
                }
            }
            self.cpu_caches.insert(ids, new_cache);
        }
        Ok(self.caches.len() - self.n_on_device)
    }

    /// Search for a matching cache given some toks
    pub fn search_for_matching_cache(&self, toks: &[u32]) -> Result<Option<LayerCaches>> {
        if let Some(cache) = self.caches.get(toks) {
            Ok(Some(cache.clone()))
        } else if let Some(cache) = self.cpu_caches.get(toks) {
            let mut new_cache = Vec::new();
            for layer in cache {
                if let Some((ref q, ref k)) = layer {
                    new_cache.push(Some((
                        q.to_device(&Device::Cpu)?,
                        k.to_device(&Device::Cpu)?,
                    )));
                } else {
                    new_cache.push(None);
                }
            }
            Ok(Some(new_cache))
        } else {
            Ok(None)
        }
    }
}
