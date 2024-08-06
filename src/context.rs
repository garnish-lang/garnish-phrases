use std::collections::HashMap;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SimpleContextCodes {
    IncompleteVersionExists,
    CompleteVersionExists,
}

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

    pub fn phrase_count(&self) -> usize {
        self.part_map.len()
    }

    pub fn add_phrase(&mut self, phrase: &str) -> Result<(), SimpleContextCodes> {
        let parts = phrase.split("_").collect::<Vec<&str>>();

        if parts.len() == 0 {
            // unreachable?
            return Ok(());
        }

        let mut running_parts = vec![];

        for part in parts.iter().take(parts.len() - 1) {
            if part.is_empty() {
                continue;
            }
            running_parts.push(*part);
            let incomplete_phrase = running_parts.join("_");
            match self.part_map.get(&incomplete_phrase) {
                None => {
                    self.part_map.insert(incomplete_phrase, PhraseStatus::Incomplete);
                },
                Some(status) => if *status == PhraseStatus::Complete {
                    return Err(SimpleContextCodes::CompleteVersionExists)
                }
            }

        }

        match parts.last() {
            None => unreachable!(),
            Some(part) => {
                if part.is_empty() {
                    return Ok(());
                }
                running_parts.push(*part);
                let complete_phrase = running_parts.join("_");
                match self.part_map.get(&complete_phrase) {
                    None => {
                        self.part_map.insert(complete_phrase, PhraseStatus::Complete);
                    }
                    Some(status) => if *status == PhraseStatus::Incomplete {
                        return Err(SimpleContextCodes::IncompleteVersionExists);
                    }
                }
            }
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
    use crate::context::{PhraseContext, PhraseStatus, SimpleContextCodes, SimplePhraseContext};

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
        assert_eq!(context.get_phrase_status("some_phrase"), PhraseStatus::Complete);
    }

    #[test]
    fn add_three_word_phrase() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("some_great_phrase");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("some"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great_phrase"), PhraseStatus::Complete);
    }

    #[test]
    fn add_three_word_phrase_extra_underscores_no_effect() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("_some__great__phrase");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("some"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great_phrase"), PhraseStatus::Complete);
        assert_eq!(context.phrase_count(), 3);
    }

    #[test]
    fn add_three_word_phrase_extra_underscores_end_forces_last_incomplete() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("_some__great__phrase__");

        assert_eq!(result, Ok(()));
        assert_eq!(context.get_phrase_status("some"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great"), PhraseStatus::Incomplete);
        assert_eq!(context.get_phrase_status("some_great_phrase"), PhraseStatus::Incomplete);
        assert_eq!(context.phrase_count(), 3);
    }

    #[test]
    fn empty_phrase_ok() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("");

        assert_eq!(result, Ok(()));
        assert_eq!(context.phrase_count(), 0);
    }

    #[test]
    fn just_underscore_ok() {
        let mut context = SimplePhraseContext::new();
        let result = context.add_phrase("_");

        assert_eq!(result, Ok(()));
        assert_eq!(context.phrase_count(), 0);
    }

    #[test]
    fn error_adding_complete_phrase_when_already_incomplete() {
        let mut context = SimplePhraseContext::new();
        context.add_phrase("some_phrase").unwrap();

        let result = context.add_phrase("some");

        assert_eq!(result, Err(SimpleContextCodes::IncompleteVersionExists));
    }

    #[test]
    fn error_adding_incomplete_phrase_when_already_complete() {
        let mut context = SimplePhraseContext::new();
        context.add_phrase("some").unwrap();

        let result = context.add_phrase("some_phrase");

        assert_eq!(result, Err(SimpleContextCodes::CompleteVersionExists));
    }
}