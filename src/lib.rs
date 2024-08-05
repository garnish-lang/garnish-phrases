use std::collections::HashMap;
use garnish_lang_compiler::parse::{Definition, ParseNode, ParseResult, SecondaryDefinition};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum PhraseStatus {
    Incomplete,
    Complete,
    NotAPhrase,
}

pub trait PhraseContext {
    fn get_phrase_status(&self, s: &str) -> PhraseStatus;
}

struct PhraseInfo {
    phrase_parts: Vec<String>,
}

impl PhraseInfo {
    pub fn new(part: String) -> Self {
        PhraseInfo { phrase_parts: vec![part] }
    }

    pub fn full_text(&self) -> String {
        self.phrase_parts.join("_")
    }

    pub fn full_text_with(&self, part: &str) -> String {
        format!("{}_{}", self.full_text(), part)
    }

    pub fn add_part(&mut self, part: String) {
        self.phrase_parts.push(part)
    }
}

pub fn reduce_phrases<Context: PhraseContext>(
    parse_result: &ParseResult,
    context: &Context,
) -> Result<ParseResult, String> {
    let mut current_index = parse_result.get_root();
    let mut next_node = parse_result.get_node(current_index);

    let mut new_result = parse_result.clone();

    // phrases read left to right regardless of precedence
    // and only work with identifiers alone or that are children of space lists
    // so don't need to check right of left most since list shouldn't be able to only have a right node
    while let Some(next) = next_node
        .ok_or(format!("Node {} not found", current_index))?
        .get_left() {
        current_index = next;
        next_node = parse_result.get_node(next)
    }

    // walk up nodes checking for phrases

    let mut phrases = vec![];
    while let Some(current_node) = next_node {
        match current_node.get_definition() {
            Definition::Identifier => {
                // check all identifier's for being a phrase part

                // if there is an existing phrase in progress
                // check if current identifier can be a part of that phrase
                let phrase_text = current_node.get_lex_token().get_text().clone();
                match phrases.last_mut() {
                    None => {
                        match context.get_phrase_status(&phrase_text) {
                            PhraseStatus::Incomplete => {
                                phrases.push(PhraseInfo::new(phrase_text));
                            }
                            PhraseStatus::Complete => {
                                // single word phrase
                                // and add a new empty apply node
                                let new_index = new_result.get_nodes().len();
                                new_result.add_node(ParseNode::new(
                                    Definition::EmptyApply,
                                    SecondaryDefinition::UnarySuffix,
                                    current_node.get_parent(),
                                    Some(current_index),
                                    None,
                                    current_node.get_lex_token().clone(), // clone so debugging points to identifier
                                ));

                                if new_result.get_root() == current_index {
                                    new_result.set_root(new_index);
                                }

                                match new_result.get_node_mut(current_index) {
                                    None => Err(format!("Node at {} not found", current_index))?,
                                    Some(node) => {
                                        node.set_parent(Some(new_index));
                                    }
                                }
                            }
                            PhraseStatus::NotAPhrase => {} // continue no changes
                        }
                    }
                    Some(info) => {
                        let new_phrase_text = info.full_text_with(&phrase_text);
                        match context.get_phrase_status(&new_phrase_text) {
                            PhraseStatus::NotAPhrase => {
                                // check if current text can be a phrase on its own
                                match context.get_phrase_status(&phrase_text) {
                                    PhraseStatus::Incomplete => {
                                        phrases.push(PhraseInfo::new(phrase_text));
                                    }
                                    PhraseStatus::Complete => {
                                        phrases.push(PhraseInfo::new(phrase_text));
                                        todo!()
                                    }
                                    PhraseStatus::NotAPhrase => {} // continue no changes
                                }
                            }
                            PhraseStatus::Incomplete => {
                                info.add_part(phrase_text);
                            }
                            PhraseStatus::Complete => {
                                todo!()
                            }
                        }
                    }
                }
            }
            Definition::List => {} // skip
            _ => {}
        }

        match current_node.get_parent() {
            None => {
                next_node = None;
            }
            Some(parent) => {
                current_index = parent;
                next_node = parse_result.get_node(parent);
            }
        }
    }

    return Ok(new_result);
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

#[cfg(test)]
mod tests {
    use garnish_lang_compiler::lex::lex;
    use garnish_lang_compiler::parse::{Definition, parse};
    use crate::{reduce_phrases, SimplePhraseContext};

    #[test]
    fn simple_phrase() {
        let input = "perform task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let context = SimplePhraseContext::new();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(1).unwrap();

        assert_eq!(phrased_tokens.get_root(), 1);
        assert_eq!(apply_token.get_definition(), Definition::EmptyApply);
        assert_eq!(apply_token.get_left(), Some(0));
        assert_eq!(apply_token.get_right(), None);
        assert_eq!(apply_token.get_parent(), None);
        assert_eq!(apply_token.get_lex_token().get_text(), "~~");

        let identifier_token = phrased_tokens.get_node(0).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(1));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");
    }

    #[test]
    fn single_word() {
        let input = "task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(1).unwrap();

        assert_eq!(phrased_tokens.get_root(), 1);

        assert_eq!(apply_token.get_definition(), Definition::EmptyApply);
        assert_eq!(apply_token.get_left(), Some(0));
        assert_eq!(apply_token.get_right(), None);
        assert_eq!(apply_token.get_parent(), None);
        assert_eq!(apply_token.get_lex_token().get_text(), "task");

        let identifier_token = phrased_tokens.get_node(0).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(1));
        assert_eq!(identifier_token.get_lex_token().get_text(), "task");
    }
}
