extern crate alloc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;
use core::fmt;
use hashbrown::HashMap;
use hashbrown::HashSet;
use utf8_ranges::{Utf8Range, Utf8Sequences};

use crate::automaton::Automaton;

const DEFAULT_STATE_LIMIT: usize = 10_000;

#[derive(Debug)]
pub enum LevenshteinError {
    TooManyStates(usize),
}

impl fmt::Display for LevenshteinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LevenshteinError::TooManyStates(size_limit) => write!(
                f,
                "Levenshtein automaton exceeds size limit of \
                           {size_limit} states"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LevenshteinError {}

#[cfg(not(feature = "std"))]
impl core::error::Error for LevenshteinError {}

pub struct Levenshtein {
    prog: DynamicLevenshtein,
    dfa: Dfa,
}

impl Levenshtein {
    pub fn new(
        query: &str,
        distance: u32,
    ) -> Result<Levenshtein, LevenshteinError> {
        let lev = DynamicLevenshtein {
            query: query.into(),
            dist: distance as usize,
        };
        let dfa = DfaBuilder::new(lev.clone()).build()?;
        Ok(Levenshtein { prog: lev, dfa })
    }

    pub fn new_with_limit(
        query: &str,
        distance: u32,
        state_limit: usize,
    ) -> Result<Levenshtein, LevenshteinError> {
        let lev = DynamicLevenshtein {
            query: query.into(),
            dist: distance as usize,
        };
        let dfa =
            DfaBuilder::new(lev.clone()).build_with_limit(state_limit)?;
        Ok(Levenshtein { prog: lev, dfa })
    }
}

impl fmt::Debug for Levenshtein {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Levenshtein(query: {:?}, distance: {:?})",
            self.prog.query, self.prog.dist
        )
    }
}

#[derive(Clone)]
struct DynamicLevenshtein {
    query: String,
    dist: usize,
}

impl DynamicLevenshtein {
    fn start(&self) -> Vec<usize> {
        (0..self.query.chars().count() + 1).collect()
    }

    fn is_match(&self, state: &[usize]) -> bool {
        state.last().map_or(false, |&n| n <= self.dist)
    }

    fn can_match(&self, state: &[usize]) -> bool {
        state.iter().min().map_or(false, |&n| n <= self.dist)
    }

    fn accept(&self, state: &[usize], chr: Option<char>) -> Vec<usize> {
        let mut next = vec![state[0] + 1];
        for (i, c) in self.query.chars().enumerate() {
            let cost = usize::from(Some(c) != chr);
            let v = cmp::min(
                cmp::min(next[i] + 1, state[i + 1] + 1),
                state[i] + cost,
            );
            next.push(cmp::min(v, self.dist + 1));
        }
        next
    }
}

impl Automaton for Levenshtein {
    type State = Option<usize>;

    #[inline]
    fn start(&self) -> Option<usize> {
        Some(0)
    }

    #[inline]
    fn is_match(&self, state: &Option<usize>) -> bool {
        state.map_or(false, |state| self.dfa.states[state].is_match)
    }

    #[inline]
    fn can_match(&self, state: &Option<usize>) -> bool {
        state.is_some()
    }

    #[inline]
    fn accept(&self, state: &Option<usize>, byte: u8) -> Option<usize> {
        state.and_then(|state| self.dfa.states[state].next[byte as usize])
    }
}

#[derive(Debug)]
struct Dfa {
    states: Vec<State>,
}

struct State {
    next: [Option<usize>; 256],
    is_match: bool,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "State {{")?;
        writeln!(f, "  is_match: {:?}", self.is_match)?;
        for i in 0..256 {
            if let Some(si) = self.next[i] {
                writeln!(f, "  {i:?}: {si:?}")?;
            }
        }
        write!(f, "}}")
    }
}

struct DfaBuilder {
    dfa: Dfa,
    lev: DynamicLevenshtein,
    cache: HashMap<Vec<usize>, usize>,
}

impl DfaBuilder {
    fn new(lev: DynamicLevenshtein) -> DfaBuilder {
        DfaBuilder {
            dfa: Dfa { states: Vec::with_capacity(16) },
            lev,
            cache: HashMap::with_capacity(1024),
        }
    }

    fn build_with_limit(
        mut self,
        state_limit: usize,
    ) -> Result<Dfa, LevenshteinError> {
        let mut stack = vec![self.lev.start()];
        let mut seen = HashSet::new();
        let query = self.lev.query.clone();
        while let Some(lev_state) = stack.pop() {
            let dfa_si = self.cached_state(&lev_state).unwrap();
            let mismatch = self.add_mismatch_utf8_states(dfa_si, &lev_state);
            if let Some((next_si, lev_next)) = mismatch {
                if !seen.contains(&next_si) {
                    seen.insert(next_si);
                    stack.push(lev_next);
                }
            }
            for (i, c) in query.chars().enumerate() {
                if lev_state[i] > self.lev.dist {
                    continue;
                }
                let lev_next = self.lev.accept(&lev_state, Some(c));
                let next_si = self.cached_state(&lev_next);
                if let Some(next_si) = next_si {
                    self.add_utf8_sequences(true, dfa_si, next_si, c, c);
                    if !seen.contains(&next_si) {
                        seen.insert(next_si);
                        stack.push(lev_next);
                    }
                }
            }
            if self.dfa.states.len() > state_limit {
                return Err(LevenshteinError::TooManyStates(state_limit));
            }
        }
        Ok(self.dfa)
    }

    fn build(self) -> Result<Dfa, LevenshteinError> {
        self.build_with_limit(DEFAULT_STATE_LIMIT)
    }

    fn cached_state(&mut self, lev_state: &[usize]) -> Option<usize> {
        self.cached(lev_state).map(|(si, _)| si)
    }

    fn cached(&mut self, lev_state: &[usize]) -> Option<(usize, bool)> {
        if !self.lev.can_match(lev_state) {
            return None;
        }
        Some(match self.cache.entry(lev_state.to_vec()) {
            hashbrown::hash_map::Entry::Occupied(v) => (*v.get(), true),
            hashbrown::hash_map::Entry::Vacant(v) => {
                let is_match = self.lev.is_match(lev_state);
                self.dfa.states.push(State { next: [None; 256], is_match });
                (*v.insert(self.dfa.states.len() - 1), false)
            }
        })
    }

    fn add_mismatch_utf8_states(
        &mut self,
        from_si: usize,
        lev_state: &[usize],
    ) -> Option<(usize, Vec<usize>)> {
        let mismatch_state = self.lev.accept(lev_state, None);
        let to_si = match self.cached(&mismatch_state) {
            None => return None,
            Some((si, _)) => si,
        };
        self.add_utf8_sequences(false, from_si, to_si, '\u{0}', '\u{10FFFF}');
        Some((to_si, mismatch_state))
    }

    fn add_utf8_sequences(
        &mut self,
        overwrite: bool,
        from_si: usize,
        to_si: usize,
        from_chr: char,
        to_chr: char,
    ) {
        for seq in Utf8Sequences::new(from_chr, to_chr) {
            let mut fsi = from_si;
            for range in &seq.as_slice()[0..seq.len() - 1] {
                // For match paths (`overwrite=true`) on a single-byte
                // range (the typical "this byte of char `c`" case), we
                // must **preserve** any prior transitions on the shared
                // UTF-8 prefix — otherwise mismatch fallbacks and
                // sibling match paths get clobbered, silently rejecting
                // valid input. Concretely, CJK chars all share leading
                // bytes (E4..E9): without preservation, query "刘德"
                // first installs E5→I_刘, then E5→I_德 overwriting I_刘
                // entirely, so the DFA can't even match "刘德" itself
                // at distance ≥ 1.
                //
                // Solution: *fork* the existing intermediate by cloning
                // its full 256-byte transition table into a fresh state
                // that subsequent overwrites in this chain can modify
                // without disturbing the original (still reachable via
                // other paths, or about to be replaced atomically).
                let tsi = if overwrite && range.start == range.end {
                    let b = range.start as usize;
                    match self.dfa.states[fsi].next[b] {
                        Some(existing) => {
                            let cloned_next =
                                self.dfa.states[existing].next;
                            let cloned_is_match =
                                self.dfa.states[existing].is_match;
                            self.dfa.states.push(State {
                                next: cloned_next,
                                is_match: cloned_is_match,
                            });
                            let new_si = self.dfa.states.len() - 1;
                            self.dfa.states[fsi].next[b] = Some(new_si);
                            new_si
                        }
                        None => {
                            let new_si = self.new_state(false);
                            self.dfa.states[fsi].next[b] = Some(new_si);
                            new_si
                        }
                    }
                } else {
                    let new_si = self.new_state(false);
                    self.add_utf8_range(overwrite, fsi, new_si, range);
                    new_si
                };
                fsi = tsi;
            }
            self.add_utf8_range(
                overwrite,
                fsi,
                to_si,
                &seq.as_slice()[seq.len() - 1],
            );
        }
    }

    fn add_utf8_range(
        &mut self,
        overwrite: bool,
        from_si: usize,
        to_si: usize,
        range: &Utf8Range,
    ) {
        for b in range.start..range.end + 1 {
            if overwrite || self.dfa.states[from_si].next[b as usize].is_none()
            {
                self.dfa.states[from_si].next[b as usize] = Some(to_si);
            }
        }
    }

    fn new_state(&mut self, is_match: bool) -> usize {
        let si = self.dfa.states.len();
        self.dfa.states.push(State { next: [None; 256], is_match });
        si
    }
}
