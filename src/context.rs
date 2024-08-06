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
        let parts = phrase.split("_").collect::<Vec<&str>>();

        for part in parts.iter().take(parts.len() - 1) {
            self.part_map.insert(part.to_string(), PhraseStatus::Incomplete);
        }

        match parts.last() {
            None => unreachable!(),
            Some(part) => self.part_map.insert(part.to_string(), PhraseStatus::Complete),
        };

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

#[cfg(test)]
mod tests {
    use crate::context::{PhraseContext, PhraseStatus, SimplePhraseContext};

    #[test]
    fn create() {
        SimplePhraseContext::new();
    }

    #[test]
    fn add_single_word_phrase() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("phrase");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("phrase"), PhraseStatus::Complete);
    }

    #[test]
    fn get_non_phrase() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("phrase");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("not"), PhraseStatus::NotAPhrase);
    }

    #[test]
    fn add_two_word_phrase() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("some_phrase");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("some"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("phrase"), PhraseStatus::Complete);
    }
}