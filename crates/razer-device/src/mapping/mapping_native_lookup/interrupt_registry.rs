//! Native string-key interrupt registration order (9cf82, 60418, 60560).
//! Notification dispatch observes this chain, including retained bucket capacity.

struct Entry<V> {
    identity: String,
    hash: u64,
    value: V,
}

pub(super) struct Registry<V> {
    entries: Vec<Entry<V>>,
    buckets: usize,
}

impl<V> Default for Registry<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            buckets: 0,
        }
    }
}

impl<V> Registry<V> {
    pub fn insert(&mut self, identity: String, value: V) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.identity == identity)
        {
            // 68bf4/9b408 replace the registration without moving its node.
            entry.value = value;
            return;
        }
        if self.entries.len() + 1 > self.buckets {
            let mut buckets = if self.buckets == 0 { 2 } else { 2 * self.buckets + 1 };
            while (2..)
                .take_while(|divisor| *divisor <= buckets / *divisor)
                .any(|divisor| buckets % divisor == 0)
            {
                buckets += 1;
            }
            self.rehash(buckets);
        }
        // 5fbe8/5feac/5ff8a/60016 use the same verified hash kernel.
        let hash = crate::simple_audio::source_hash(identity.as_bytes());
        let bucket = hash % self.buckets as u64;
        let position = self
            .entries
            .iter()
            .position(|entry| entry.hash % self.buckets as u64 == bucket)
            .unwrap_or(0);
        self.entries.insert(
            position,
            Entry {
                identity,
                hash,
                value,
            },
        );
    }

    fn rehash(&mut self, buckets: usize) {
        self.buckets = buckets;
        let Some(first) = self.entries.first() else { return };
        let mut previous_bucket = first.hash % buckets as u64;
        let mut seen = std::collections::BTreeSet::from([previous_bucket]);
        for position in 1..self.entries.len() {
            let bucket = self.entries[position].hash % buckets as u64;
            if bucket == previous_bucket {
                continue;
            }
            if seen.insert(bucket) {
                previous_bucket = bucket;
            } else {
                let entry = self.entries.remove(position);
                let head = self
                    .entries
                    .iter()
                    .position(|entry| entry.hash % buckets as u64 == bucket)
                    .unwrap();
                self.entries.insert(head, entry);
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.entries
            .iter()
            .map(|entry| (&entry.identity, &entry.value))
    }

    /// B0486 frees nodes and clears bucket links, retaining the bucket count.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(registry: &Registry<u16>) -> Vec<u16> {
        registry.iter().map(|(_, value)| *value).collect()
    }

    #[test]
    fn native_string_hash_chain_keeps_collision_runs_and_replacement_position() {
        // 5feac's eight-byte formula, independently evaluated for these IDs.
        let hashes = [
            0x32f7e3085ff99154,
            0xff2126836b707705,
            0x3ea7d5398354aefd,
            0x29cafbeacd5e1efb,
            0x3d4f1d4ad52e63f5,
            0xeda8acf4d029bbcc,
            0x924eedc92fbd34ad,
        ];
        let mut registry = Registry::default();
        for (key, hash) in (30..=36).zip(hashes) {
            let identity = format!("t21s{key}f0");
            assert_eq!(crate::simple_audio::source_hash(identity.as_bytes()), hash);
            registry.insert(identity, key);
        }
        assert_eq!(registry.buckets, 11);
        assert_eq!(values(&registry), [36, 33, 35, 34, 32, 31, 30]);
        registry.insert("t21s34f0".into(), 99);
        assert_eq!(values(&registry), [36, 33, 35, 99, 32, 31, 30]);
    }

    #[test]
    fn registration_clear_retains_capacity_and_changes_later_chain_order() {
        let mut registry = Registry::default();
        for key in 30..=36 {
            registry.insert(format!("t21s{key}f0"), key);
        }
        registry.clear();
        for key in 30..=34 {
            registry.insert(format!("t21s{key}f0"), key);
        }
        assert_eq!(registry.buckets, 11);
        assert_eq!(values(&registry), [34, 33, 32, 31, 30]);
        let mut fresh = Registry::default();
        for key in 30..=34 {
            fresh.insert(format!("t21s{key}f0"), key);
        }
        assert_eq!(fresh.buckets, 5);
        assert_eq!(values(&fresh), [33, 34, 32, 31, 30]);
    }

    #[test]
    fn rehash_moves_later_collision_runs_to_the_existing_bucket_head() {
        let mut registry = Registry {
            entries: [1, 2, 6, 11, 7, 16]
                .into_iter()
                .map(|hash| Entry {
                    identity: hash.to_string(),
                    hash,
                    value: hash as u16,
                })
                .collect(),
            buckets: 2,
        };
        registry.rehash(5);
        assert_eq!(values(&registry), [16, 11, 6, 1, 2, 7]);
    }
}
