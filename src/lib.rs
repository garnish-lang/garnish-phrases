mod context;

use garnish_lang_compiler::lex::{LexerToken, TokenType};
use garnish_lang_compiler::parse::{Definition, ParseNode, ParseResult, SecondaryDefinition};
use crate::context::{PhraseContext, PhraseStatus};

struct PhraseInfo {
    phrase_parts: Vec<String>,
    arguments: Vec<usize>,
}

impl PhraseInfo {
    pub fn new(part: String) -> Self {
        PhraseInfo { phrase_parts: vec![part], arguments: vec![] }
    }

    pub fn full_text(&self) -> String {
        self.phrase_parts.join("_")
    }

    pub fn full_text_with(&self, part: &str) -> String {
        format!("{}_{}", self.full_text(), part)
    }

    pub fn add_part(&mut self, part: String) {
        self.phrase_parts.push(part);
    }

    pub fn add_argument(&mut self, argument: usize) {
        self.arguments.push(argument);
    }
}

pub fn reduce_phrases<Context: PhraseContext>(
    parse_result: &ParseResult,
    context: &Context,
) -> Result<ParseResult, String> {
    let current_index = parse_result.get_root();
    let mut new_result = parse_result.clone();
    let mut phrases = vec![];

    // a single node can't be a parent
    // and only needs a single check
    if parse_result.get_nodes().len() == 1 {
        check_node_index_for_phrase(
            Some(current_index),
            &mut phrases,
            context,
            parse_result,
            &mut new_result,
            false,
        )?;

        return Ok(new_result);
    }

    let mut parent_stack = vec![];
    let mut process_stack = vec![current_index];

    while let Some(current_index) = process_stack.pop() {
        match parse_result.get_node(current_index) {
            None => Err(format!("Node at index {} not present", current_index))?,
            Some(node) => {
                match (node.get_left(), node.get_right()) {
                    (None, None) => continue, // not a parent, skip
                    (Some(left_index), Some(right_index)) => {
                        // process left then right will result in parent stack processing
                        // left before right
                        process_stack.push(left_index);
                        process_stack.push(right_index);
                    }
                    (Some(left_index), None) => {
                        process_stack.push(left_index);
                    }
                    (None, Some(right_index)) => {
                        process_stack.push(right_index);
                    }
                }

                parent_stack.push(current_index);
            }
        }
    }

    while let Some(current_index) = parent_stack.pop() {
        let node = parse_result.get_node(current_index)
            .ok_or(format!("Node at index {} not present", current_index))?;

        // check left then right for phrases
        check_node_index_for_phrase(
            node.get_left(),
            &mut phrases,
            context,
            parse_result,
            &mut new_result,
            true,
        )?;

        check_node_index_for_phrase(
            node.get_right(),
            &mut phrases,
            context,
            parse_result,
            &mut new_result,
            false,
        )?;
    }

    return Ok(new_result);
}

fn check_node_index_for_phrase<Context: PhraseContext>(
    node_index_opt: Option<usize>,
    phrases: &mut Vec<PhraseInfo>,
    context: &Context,
    original_result: &ParseResult,
    result: &mut ParseResult,
    is_left_of_parent: bool,
) -> Result<(), String> {
    match node_index_opt {
        None => Ok(()),
        Some(index) => match original_result.get_node(index) {
            None => Ok(()),
            Some(node) => check_node_for_phrase(
                node,
                index,
                phrases,
                context,
                result,
                is_left_of_parent,
            )
        }
    }
}

fn check_node_for_phrase<Context: PhraseContext>(
    node: &ParseNode,
    node_index: usize,
    phrases: &mut Vec<PhraseInfo>,
    context: &Context,
    result: &mut ParseResult,
    is_left_of_parent: bool,
) -> Result<(), String> {
    let arg_index = match node.get_definition() {
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
                            None
                        }
                        PhraseStatus::Complete => {
                            // single word phrase, resolve immediately
                            resolve_single_word_phrase(
                                node,
                                node_index,
                                result,
                            )?
                        }
                        PhraseStatus::NotAPhrase => Some(node_index) // continue no changes
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
                                    None
                                }
                                PhraseStatus::Complete => {
                                    resolve_single_word_phrase(
                                        node,
                                        node_index,
                                        result,
                                    )?
                                }
                                PhraseStatus::NotAPhrase => {
                                    Some(node_index)
                                } // continue no changes
                            }
                        }
                        PhraseStatus::Incomplete => {
                            // continuation
                            info.add_part(phrase_text);
                            None
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
                                        node.get_lex_token().get_column(),
                                    );
                                    node.set_lex_token(new_token);
                                }
                            }

                            let arg = match info.arguments.len() {
                                0 => {
                                    let new_index = result.get_nodes().len();
                                    match node.get_parent().and_then(|p| result.get_node_mut(p)) {
                                        None => Err(format!("Node at {:?} not found", node.get_parent()))?,
                                        Some(parent) => {
                                            parent.set_right(Some(new_index));

                                            result.add_node(ParseNode::new(
                                                Definition::EmptyApply,
                                                SecondaryDefinition::UnarySuffix,
                                                node.get_parent(),
                                                Some(node_index),
                                                None,
                                                node.get_lex_token().clone(), // clone so debugging points to identifier
                                            ));

                                            match result.get_node_mut(node_index) {
                                                None => Err(format!("Node at {} not found", node_index))?,
                                                Some(node) => {
                                                    node.set_parent(Some(new_index));
                                                }
                                            }
                                        }
                                    }

                                    Some(new_index)
                                }
                                1 => {
                                    match node.get_parent().and_then(|p| result.get_node_mut(p)) {
                                        None => Err(format!("Node at {:?} not found", node.get_parent()))?,
                                        Some(parent) => {
                                            // Using ApplyTo instead of Apply so no swapping needs to be done
                                            parent.set_definition(Definition::ApplyTo);

                                            // for single argument just replace current left side to point to argument
                                            let new_left = info.arguments.get(0).cloned();
                                            parent.set_left(new_left);

                                            // update argument to correct parent
                                            match new_left.and_then(|i| result.get_node_mut(i)) {
                                                None => Err(format!("Node at {:?} not found", new_left))?,
                                                Some(left_node) => {
                                                    left_node.set_parent(node.get_parent())
                                                }
                                            }
                                        }
                                    }
                                    node.get_parent()
                                }
                                _n => {
                                    let mut next_parent = match node.get_parent().and_then(|p| result.get_node_mut(p)) {
                                        None => Err(format!("Node at {:?} not found", node.get_parent()))?,
                                        Some(parent) => {
                                            // Using ApplyTo instead of Apply so no swapping needs to be done
                                            parent.set_definition(Definition::ApplyTo);

                                            parent.get_left()
                                        }
                                    };

                                    // descend list attaching arguments in reverse order
                                    // last two arguments will have same parent as left and right
                                    // end at 1 so the 0th item can always be put on last list's left
                                    for i in (1..info.arguments.len()).rev() {
                                        let arg_index = *info.arguments.get(i).unwrap();

                                        // update argument's parent
                                        match result.get_node_mut(arg_index) {
                                            None => Err(format!("Node at {} not found", arg_index))?,
                                            Some(right) => {
                                                right.set_parent(next_parent);
                                            }
                                        }

                                        // update parent's right to point to argument
                                        // and set next parent to left
                                        let left = match next_parent.and_then(|i| result.get_node_mut(i)) {
                                            None => Err(format!("Node at {:?} not found", next_parent))?,
                                            Some(parent) => {
                                                parent.set_right(Some(arg_index));
                                                let left = parent.get_left();

                                                // if on second to last arg
                                                // grab last arg and update it and parent
                                                if i == 1 {
                                                    let arg_index = *info.arguments.get(0).unwrap();
                                                    parent.set_left(Some(arg_index));

                                                    match result.get_node_mut(arg_index) {
                                                        None => Err(format!("Node at {:?} not found", arg_index))?,
                                                        Some(left) => {
                                                            left.set_parent(next_parent);
                                                            break;
                                                        }
                                                    }
                                                }

                                                left
                                            }
                                        };

                                        next_parent = left;
                                    }

                                    node.get_parent()
                                }
                            };

                            phrases.pop();

                            arg
                        }
                    }
                }
            }
        }
        // List to left of parent should not be included in arg lists
        Definition::List if is_left_of_parent => None,
        _ => Some(node_index)
    };

    match arg_index {
        None => (),
        // add to argument list if there's an existing phrase
        Some(index) => match phrases.last_mut() {
            None => (),
            Some(info) => {
                info.add_argument(index);
            }
        }
    }

    Ok(())
}

fn resolve_single_word_phrase(
    node: &ParseNode,
    node_index: usize,
    result: &mut ParseResult,
) -> Result<Option<usize>, String> {
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

    Ok(Some(new_index))
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

    #[test]
    fn simple_phrase_with_argument() {
        let input = "perform 5 task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(3).unwrap();

        assert_eq!(phrased_tokens.get_root(), 3);
        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(2));
        assert_eq!(apply_token.get_right(), Some(4));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(2).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "5");

        let identifier_token = phrased_tokens.get_node(4).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");
    }

    #[test]
    fn simple_phrase_with_two_arguments() {
        let input = "perform 5 10 task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(5).unwrap();

        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(3));
        assert_eq!(apply_token.get_right(), Some(6));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(6).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");

        let identifier_token = phrased_tokens.get_node(3).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::List);
        assert_eq!(identifier_token.get_left(), Some(2));
        assert_eq!(identifier_token.get_right(), Some(4));
        assert_eq!(identifier_token.get_parent(), Some(5));

        let identifier_token = phrased_tokens.get_node(2).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "5");

        let identifier_token = phrased_tokens.get_node(4).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "10");
    }

    #[test]
    fn simple_phrase_with_three_arguments() {
        let input = "perform 5 10 15 task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(7).unwrap();

        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(5));
        assert_eq!(apply_token.get_right(), Some(8));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(8).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(7));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");

        let identifier_token = phrased_tokens.get_node(5).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::List);
        assert_eq!(identifier_token.get_left(), Some(3));
        assert_eq!(identifier_token.get_right(), Some(6));
        assert_eq!(identifier_token.get_parent(), Some(7));

        let identifier_token = phrased_tokens.get_node(6).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "15");

        let identifier_token = phrased_tokens.get_node(3).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::List);
        assert_eq!(identifier_token.get_left(), Some(2));
        assert_eq!(identifier_token.get_right(), Some(4));
        assert_eq!(identifier_token.get_parent(), Some(5));

        let identifier_token = phrased_tokens.get_node(2).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "5");

        let identifier_token = phrased_tokens.get_node(4).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "10");
    }

    #[test]
    fn three_word_two_arg_phrase() {
        let input = "perform 5 special 10 task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_special_task").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(7).unwrap();

        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(5));
        assert_eq!(apply_token.get_right(), Some(8));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(8).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(7));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_special_task");

        let identifier_token = phrased_tokens.get_node(5).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::List);
        assert_eq!(identifier_token.get_left(), Some(2));
        assert_eq!(identifier_token.get_right(), Some(6));
        assert_eq!(identifier_token.get_parent(), Some(7));

        let identifier_token = phrased_tokens.get_node(2).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "5");

        let identifier_token = phrased_tokens.get_node(6).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Number);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "10");
    }

    #[test]
    fn nested_phrase() {
        let input = "perform special task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();
        context.add_phrase("special").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(3).unwrap();

        assert_eq!(phrased_tokens.get_nodes().len(), 6);

        assert_eq!(phrased_tokens.get_root(), 3);
        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(5));
        assert_eq!(apply_token.get_right(), Some(4));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(2).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "special");

        let identifier_token = phrased_tokens.get_node(4).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");

        let identifier_token = phrased_tokens.get_node(5).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::EmptyApply);
        assert_eq!(identifier_token.get_left(), Some(2));
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(3));
        assert_eq!(identifier_token.get_lex_token().get_text(), "special");
    }

    #[test]
    fn nested_two_word_phrase() {
        let input = "perform super special task";

        let tokens = lex(input).unwrap();
        let parsed = parse(&tokens).unwrap();

        let mut context = SimplePhraseContext::new();
        context.add_phrase("perform_task").unwrap();
        context.add_phrase("super_special").unwrap();

        let phrased_tokens = reduce_phrases(&parsed, &context).unwrap();

        let apply_token = phrased_tokens.get_node(5).unwrap();

        assert_eq!(phrased_tokens.get_nodes().len(), 8);

        assert_eq!(phrased_tokens.get_root(), 5);
        assert_eq!(apply_token.get_definition(), Definition::ApplyTo);
        assert_eq!(apply_token.get_left(), Some(7));
        assert_eq!(apply_token.get_right(), Some(6));
        assert_eq!(apply_token.get_parent(), None);

        let identifier_token = phrased_tokens.get_node(6).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));
        assert_eq!(identifier_token.get_lex_token().get_text(), "perform_task");

        let identifier_token = phrased_tokens.get_node(7).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::EmptyApply);
        assert_eq!(identifier_token.get_left(), Some(4));
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(5));

        let identifier_token = phrased_tokens.get_node(4).unwrap();
        assert_eq!(identifier_token.get_definition(), Definition::Identifier);
        assert_eq!(identifier_token.get_left(), None);
        assert_eq!(identifier_token.get_right(), None);
        assert_eq!(identifier_token.get_parent(), Some(7));
        assert_eq!(identifier_token.get_lex_token().get_text(), "super_special");
    }
}
