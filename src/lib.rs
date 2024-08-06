mod context;

use garnish_lang_compiler::lex::{LexerToken, TokenType};
use garnish_lang_compiler::parse::{Definition, ParseNode, ParseResult, SecondaryDefinition};
use crate::context::{PhraseContext, PhraseStatus};

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
        check_node_for_phrase(
            current_node,
            current_index,
            &mut phrases,
            context,
            parse_result,
            &mut new_result
        )?;

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

fn check_node_for_phrase<Context: PhraseContext>(
    node: &ParseNode,
    node_index: usize,
    phrases: &mut Vec<PhraseInfo>,
    context: &Context,
    original_result: &ParseResult,
    result: &mut ParseResult
) -> Result<(), String> {
    match node.get_definition() {
        Definition::Identifier => {
            // check all identifier's for being a phrase part

            // if there is an existing phrase in progress
            // check if current identifier can be a part of that phrase
            let phrase_text = node.get_lex_token().get_text().clone();
            match phrases.last_mut() {
                None => {
                    // no existing phrase
                    match context.get_phrase_status(&phrase_text) {
                        PhraseStatus::Incomplete => {
                            // start new phrase
                            phrases.push(PhraseInfo::new(phrase_text));
                        }
                        PhraseStatus::Complete => {
                            // single word phrase, resolve immediately
                            // and add a new empty apply node
                            let new_index = result.get_nodes().len();
                            result.add_node(ParseNode::new(
                                Definition::EmptyApply,
                                SecondaryDefinition::UnarySuffix,
                                node.get_parent(),
                                Some(node_index),
                                None,
                                node.get_lex_token().clone(), // clone so debugging points to identifier
                            ));

                            if result.get_root() == node_index {
                                result.set_root(new_index);
                            }

                            match result.get_node_mut(node_index) {
                                None => Err(format!("Node at {} not found", node_index))?,
                                Some(node) => {
                                    node.set_parent(Some(new_index));
                                }
                            }
                        }
                        PhraseStatus::NotAPhrase => {} // continue no changes
                    }
                }
                Some(info) => {
                    // existing phrase, first check if current is continuation
                    let new_phrase_text = info.full_text_with(&phrase_text);
                    match context.get_phrase_status(&new_phrase_text) {
                        PhraseStatus::NotAPhrase => {
                            // not a continuation
                            // check if current text can be a phrase on its own
                            match context.get_phrase_status(&phrase_text) {
                                PhraseStatus::Incomplete => {
                                    phrases.push(PhraseInfo::new(phrase_text));
                                }
                                PhraseStatus::Complete => {
                                    todo!()
                                }
                                PhraseStatus::NotAPhrase => {} // continue no changes
                            }
                        }
                        PhraseStatus::Incomplete => {
                            // continuation
                            info.add_part(phrase_text);
                        }
                        PhraseStatus::Complete => {
                            // end of multi-word phrase, resolve now

                            // update current node token to be full phrase
                            match result.get_node_mut(node_index) {
                                None => Err(format!("Node at {} not found", node_index))?,
                                Some(node) => {
                                    let new_token = LexerToken::new(
                                        new_phrase_text,
                                        TokenType::Identifier,
                                        node.get_lex_token().get_line(),
                                        node.get_lex_token().get_column()
                                    );
                                    node.set_lex_token(new_token);
                                }
                            }

                            // all multi-word phrases should have a parent
                            // update parent to be empty apply
                            match node.get_parent().and_then(|p| result.get_node_mut(p)) {
                                None => Err(format!("Node at {} not found", node_index))?,
                                Some(parent) => {
                                    parent.set_definition(Definition::EmptyApply);

                                    // empty expects left to be populated
                                    // but current node should be the right one
                                    // because if it were the left it would've resolved as a single word phrase
                                    parent.set_left(Some(node_index));
                                    parent.set_right(None);
                                }
                            }
                        }
                    }
                }
            }
        }
        Definition::List => {
            match node.get_right() {
                None => (),
                Some(right_index) => match original_result.get_node(right_index) {
                    None => {}
                    Some(right_node) => {
                        check_node_for_phrase(
                            right_node,
                            right_index,
                            phrases,
                            context,
                            original_result,
                            result
                        )?
                    }
                }
            }
        }
        _ => {}
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use garnish_lang_compiler::lex::lex;
    use garnish_lang_compiler::parse::{Definition, parse};
    use crate::reduce_phrases;
    use crate::context::SimplePhraseContext;

    #[test]
    fn simple_phrase() {
        let input = "perform task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(1).unwrap();

        assert_eq!(phrased_tokens.get_root(), 1);
        assert_eq!(apply_token.get_definition(), Definition::EmptyApply);
        assert_eq!(apply_token.get_left(), Some(2));
        assert_eq!(apply_token.get_right(), None);
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(2).unwrap();
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
