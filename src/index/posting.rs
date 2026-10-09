//! Adaptive RAM-only posting lists. IDs have no persistent meaning.
#[derive(Clone)]
pub(crate) enum Posting {
    Tiny { ids: [u32; 4], len: u8 },
    Sparse(Vec<u32>),
    Dense { words: Vec<u64>, len: usize },
}
impl Default for Posting {
    fn default() -> Self {
        Self::Tiny {
            ids: [0; 4],
            len: 0,
        }
    }
}
impl Posting {
    pub fn len(&self) -> usize {
        match self {
            Self::Tiny { len, .. } => *len as usize,
            Self::Sparse(ids) => ids.len(),
            Self::Dense { len, .. } => *len,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn contains(&self, id: &u32) -> bool {
        match self {
            Self::Tiny { ids, len } => ids[..*len as usize].binary_search(id).is_ok(),
            Self::Sparse(ids) => ids.binary_search(id).is_ok(),
            Self::Dense { words, .. } => words
                .get(*id as usize / 64)
                .is_some_and(|word| word & (1 << (*id % 64)) != 0),
        }
    }
    pub fn insert(&mut self, id: u32) {
        match self {
            Self::Tiny { ids, len } => {
                let n = *len as usize;
                let Err(pos) = ids[..n].binary_search(&id) else {
                    return;
                };
                if n < 4 {
                    ids.copy_within(pos..n, pos + 1);
                    ids[pos] = id;
                    *len += 1;
                    return;
                }
                let mut values = Vec::with_capacity(8);
                values.extend_from_slice(ids);
                values.insert(pos, id);
                *self = Self::Sparse(values);
            }
            Self::Sparse(ids) => {
                let Err(pos) = ids.binary_search(&id) else {
                    return;
                };
                ids.insert(pos, id);
            }
            Self::Dense { words, len } => {
                let required = id as usize / 64 + 1;
                // A distant ID must not force an enormous, mostly empty bitmap.
                if required > words.len() && required as u128 * 8 > (*len as u128 + 1) * 4 + 64 {
                    let mut ids: Vec<_> = self.iter().collect();
                    let pos = ids.binary_search(&id).unwrap_err();
                    ids.insert(pos, id);
                    *self = Self::Sparse(ids);
                    return;
                }
                words.resize(words.len().max(required), 0);
                let word = &mut words[id as usize / 64];
                let bit = 1 << (id % 64);
                if *word & bit == 0 {
                    *word |= bit;
                    *len += 1;
                }
                return;
            }
        }
        if let Self::Sparse(ids) = self {
            let required = *ids.last().unwrap() as usize / 64 + 1;
            if ids.len() >= 64 && required as u128 * 8 <= ids.len() as u128 * 2 {
                let mut words = vec![0; required];
                for id in ids.iter() {
                    words[*id as usize / 64] |= 1 << (*id % 64);
                }
                *self = Self::Dense {
                    words,
                    len: ids.len(),
                };
            }
        }
    }
    pub fn remove(&mut self, id: &u32) {
        match self {
            Self::Tiny { ids, len } => {
                if let Ok(pos) = ids[..*len as usize].binary_search(id) {
                    ids.copy_within(pos + 1..*len as usize, pos);
                    *len -= 1;
                }
            }
            Self::Sparse(ids) => {
                if let Ok(pos) = ids.binary_search(id) {
                    ids.remove(pos);
                }
                if ids.len() <= 4 {
                    let mut tiny = [0; 4];
                    tiny[..ids.len()].copy_from_slice(ids);
                    *self = Self::Tiny {
                        ids: tiny,
                        len: ids.len() as u8,
                    };
                }
            }
            Self::Dense { words, len } => {
                if let Some(word) = words.get_mut(*id as usize / 64) {
                    let bit = 1 << (*id % 64);
                    if *word & bit != 0 {
                        *word &= !bit;
                        *len -= 1;
                    }
                }
                if *len < 32 {
                    let ids: Vec<_> = self.iter().collect();
                    *self = Self::Sparse(ids);
                }
            }
        }
    }
    pub fn iter(&self) -> Iter<'_> {
        match self {
            Self::Tiny { ids, len } => Iter::Slice(ids[..*len as usize].iter()),
            Self::Sparse(ids) => Iter::Slice(ids.iter()),
            Self::Dense { words, .. } => Iter::Bits {
                words,
                next_word: 0,
                bits: 0,
                base: 0,
            },
        }
    }
}
pub(crate) enum Iter<'a> {
    Slice(std::slice::Iter<'a, u32>),
    Bits {
        words: &'a [u64],
        next_word: usize,
        bits: u64,
        base: u32,
    },
}
impl Iterator for Iter<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Slice(ids) => ids.next().copied(),
            Self::Bits {
                words,
                next_word,
                bits,
                base,
            } => loop {
                if *bits != 0 {
                    let bit = bits.trailing_zeros();
                    *bits &= *bits - 1;
                    return Some(*base + bit);
                }
                *bits = *words.get(*next_word)?;
                *base = (*next_word * 64) as u32;
                *next_word += 1;
            },
        }
    }
}
impl Extend<u32> for Posting {
    fn extend<T: IntoIterator<Item = u32>>(&mut self, ids: T) {
        for id in ids {
            self.insert(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn representations_and_distant_ids_match_ordered_set() {
        let mut posting = Posting::default();
        let mut expected = BTreeSet::new();
        for id in (0..600).rev().chain([0, 63, 64, 127, u32::MAX]) {
            posting.insert(id);
            expected.insert(id);
            assert_eq!(
                posting.iter().collect::<Vec<_>>(),
                expected.iter().copied().collect::<Vec<_>>()
            );
            assert_eq!(posting.len(), expected.len());
            assert!(posting.contains(&id));
        }
        assert!(matches!(posting, Posting::Sparse(_)));
        for id in expected.clone() {
            posting.remove(&id);
            expected.remove(&id);
            assert!(!posting.contains(&id));
            assert_eq!(
                posting.iter().collect::<Vec<_>>(),
                expected.iter().copied().collect::<Vec<_>>()
            );
        }
        assert!(posting.is_empty());
    }
    #[test]
    fn randomized_reuse_union_and_deletion_match_set() {
        let mut posting = Posting::default();
        let mut expected = BTreeSet::new();
        let mut seed = 7u64;
        for turn in 0..20_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = (seed >> 32) as u32 % 2048;
            if turn % 3 == 0 {
                posting.remove(&id);
                expected.remove(&id);
            } else {
                posting.insert(id);
                expected.insert(id);
            }
            if turn % 97 == 0 {
                assert_eq!(posting.len(), expected.len());
                assert_eq!(
                    posting.iter().collect::<Vec<_>>(),
                    expected.iter().copied().collect::<Vec<_>>()
                );
            }
        }
        assert!(matches!(posting, Posting::Dense { .. }));
        let copy = posting.clone();
        posting.extend(copy.iter());
        assert_eq!(posting.len(), expected.len());
        for id in expected {
            posting.remove(&id);
        }
        assert!(posting.is_empty());
    }
}
