use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum PhraseStatus {
    Incomplete,
    Complete,
    NotAPhrase,
}

pub trait PhraseContext {
    fn get_phrase_status(&self, s: &str) -> PhraseStatus;
}

pub struct SimplePhraseContext {
    part_map: HashMap<String, PhraseStatus>
}

impl SimplePhraseContext {
    pub fn new() -> Self {
        SimplePhraseContext { part_map: HashMap::new() }
    }

    pub fn add_phrase(&mut self, phrase: &str) -> Result<(), String> {
        self.part_map.insert(phrase.to_string(), PhraseStatus::Complete);
        Ok(())
    }
}

impl PhraseContext for SimplePhraseContext {
    fn get_phrase_status(&self, s: &str) -> PhraseStatus {
        match self.part_map.get(s) {
            None => PhraseStatus::NotAPhrase,
            Some(status) => *status
        }
    }
}