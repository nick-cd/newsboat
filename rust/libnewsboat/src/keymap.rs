use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take},
    character::complete::{none_of, space0},
    combinator::{complete, map, recognize, value},
    multi::{many0, many1, separated_list, separated_nonempty_list},
    sequence::{delimited, tuple},
    IResult,
};

fn unquoted_token(input: &str) -> IResult<&str, String> {
    let parser = tuple((none_of("\";"), is_not(" ;")));
    let parser = map(recognize(parser), String::from);

    parser(input)
}

fn quoted_token(input: &str) -> IResult<&str, String> {
    let parser = escaped_transform(is_not(r#""\"#), '\\', |control_char: &str| {
        alt((
            value(r#"""#, tag(r#"""#)),
            value(r#"\"#, tag(r#"\"#)),
            value("\r", tag("r")),
            value("\n", tag("n")),
            value("\t", tag("t")),
            value("\x0b", tag("v")), // vertical tab
            take(1usize),            // all other escaped characters are passed through, unmodified
        ))(control_char)
    });

    let double_quote = tag("\"");
    let parser = delimited(&double_quote, parser, &double_quote);

    parser(input)
}

fn token(input: &str) -> IResult<&str, String> {
    let parser = alt((quoted_token, unquoted_token));
    parser(input)
}

fn operation_with_args(input: &str) -> IResult<&str, Vec<String>> {
    let parser = separated_nonempty_list(many1(tag(" ")), token);
    parser(input)
}

fn operation_sequence(input: &str) -> IResult<&str, Vec<Vec<String>>> {
    let semicolon = delimited(space0, tag(";"), space0);

    let parser = separated_list(many1(&semicolon), operation_with_args);
    let parser = delimited(many0(&semicolon), parser, many0(&semicolon));

    let parser = complete(parser);

    parser(input)
}

/// Split a semicolon-separated list of operations into a vector. Each operation is represented by
/// a non-empty sub-vector, where the first element is the name of the operation, and the rest of
/// the elements are operation's arguments.
///
/// This function assumes that the input string:
/// 1. doesn't contain a comment;
/// 2. doesn't contain backticks that need to be processed.
pub fn tokenize_operation_sequence(input: &str) -> Vec<Vec<String>> {
    match operation_sequence(input) {
        Ok((_leftovers, tokens)) => tokens,
        Err(error) => {
            // TODO: handle errors more gracefully, or more in like with tokenize_quoted
            panic!(format!("{} for input {:?}", error, input));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tokenize_operation_sequence;

    #[test]
    fn t_tokenize_operation_sequence_works_for_all_cpp_inputs() {
        assert_eq!(tokenize_operation_sequence(""), Vec::<Vec<String>>::new());
        assert_eq!(tokenize_operation_sequence("open"), vec![vec!["open"]]);
        assert_eq!(
            tokenize_operation_sequence("open-all-unread-in-browser-and-mark-read"),
            vec![vec!["open-all-unread-in-browser-and-mark-read"]]
        );
        assert_eq!(
            tokenize_operation_sequence("; ; ; ;"),
            Vec::<Vec<String>>::new()
        );
        assert_eq!(
            tokenize_operation_sequence("open ; next"),
            vec![vec!["open"], vec!["next"]]
        );
        assert_eq!(
            tokenize_operation_sequence("open ; next ; prev"),
            vec![vec!["open"], vec!["next"], vec!["prev"]]
        );
        assert_eq!(
            tokenize_operation_sequence("open ; next ; prev ; quit"),
            vec![vec!["open"], vec!["next"], vec!["prev"], vec!["quit"]]
        );
        assert_eq!(
            tokenize_operation_sequence(r#"set "arg 1""#),
            vec![vec!["set", "arg 1"]]
        );
        // TODO: how would the old parser react if the last token didn't close the double quotes?
        assert_eq!(
            tokenize_operation_sequence(r#"set "arg 1" ; set "arg 2" "arg 3""#),
            vec![vec!["set", "arg 1"], vec!["set", "arg 2", "arg 3"]]
        );
        assert_eq!(
            tokenize_operation_sequence(r#"set browser "firefox"; open-in-browser"#),
            vec![vec!["set", "browser", "firefox"], vec!["open-in-browser"]]
        );
        assert_eq!(
            tokenize_operation_sequence("set browser firefox; open-in-browser"),
            vec![vec!["set", "browser", "firefox"], vec!["open-in-browser"]]
        );
        assert_eq!(
            tokenize_operation_sequence("open-in-browser; quit"),
            vec![vec!["open-in-browser"], vec!["quit"]]
        );
        assert_eq!(
            tokenize_operation_sequence(r#"open; set browser "firefox --private-window"; quit"#),
            vec![
                vec!["open"],
                vec!["set", "browser", "firefox --private-window"],
                vec!["quit"]
            ]
        );
        assert_eq!(
            tokenize_operation_sequence(r#"open ;set browser "firefox --private-window" ;quit"#),
            vec![
                vec!["open"],
                vec!["set", "browser", "firefox --private-window"],
                vec!["quit"]
            ]
        );
        assert_eq!(
            tokenize_operation_sequence(r#"open;set browser "firefox --private-window";quit"#),
            vec![
                vec!["open"],
                vec!["set", "browser", "firefox --private-window"],
                vec!["quit"]
            ]
        );
        assert_eq!(
            tokenize_operation_sequence("; ;; ; open",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(";;; ;; ; open",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(";;; ;; ; open ;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(";;; ;; ; open ;; ;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(";;; ;; ; open ; ;;;;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(";;; open ; ;;;;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence("; open ;; ;; ;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence("open ; ;;; ;;",),
            vec![vec!["open"]]
        );
        assert_eq!(
            tokenize_operation_sequence(
                r#"set browser "sleep 3; do-something ; echo hi"; open-in-browser"#
            ),
            vec![
                vec!["set", "browser", "sleep 3; do-something ; echo hi"],
                vec!["open-in-browser"]
            ]
        );
    }

    #[test]
    fn t_tokenize_operation_sequence_ignores_escaped_sequences_outside_double_quotes() {
        assert_eq!(tokenize_operation_sequence(r#"\t"#), vec![vec![r#"\t"#]]);
        assert_eq!(tokenize_operation_sequence(r#"\r"#), vec![vec![r#"\r"#]]);
        assert_eq!(tokenize_operation_sequence(r#"\n"#), vec![vec![r#"\n"#]]);
        assert_eq!(tokenize_operation_sequence(r#"\v"#), vec![vec![r#"\v"#]]);
        assert_eq!(tokenize_operation_sequence(r#"\""#), vec![vec![r#"\""#]]);
        assert_eq!(tokenize_operation_sequence(r#"\\"#), vec![vec![r#"\\"#]]);
    }

    #[test]
    fn t_tokenize_operation_sequence_expands_escaped_sequences_inside_double_quotes() {
        assert_eq!(tokenize_operation_sequence(r#""\t""#), vec![vec!["\t"]]);
        assert_eq!(tokenize_operation_sequence(r#""\r""#), vec![vec!["\r"]]);
        assert_eq!(tokenize_operation_sequence(r#""\n""#), vec![vec!["\n"]]);
        assert_eq!(tokenize_operation_sequence(r#""\v""#), vec![vec!["\x0b"]]); // vertical tab
        assert_eq!(tokenize_operation_sequence(r#""\"""#), vec![vec!["\""]]);
        assert_eq!(tokenize_operation_sequence(r#""\\""#), vec![vec!["\\"]]);
    }

    #[test]
    fn t_tokenize_operation_sequence_passes_through_unsupported_escaped_chars_inside_double_quotes()
    {
        assert_eq!(tokenize_operation_sequence(r#""\1""#), vec![vec!["1"]]);
        assert_eq!(tokenize_operation_sequence(r#""\W""#), vec![vec!["W"]]);
        assert_eq!(tokenize_operation_sequence(r#""\b""#), vec![vec!["b"]]);
        assert_eq!(tokenize_operation_sequence(r#""\d""#), vec![vec!["d"]]);
        assert_eq!(tokenize_operation_sequence(r#""\x""#), vec![vec!["x"]]);
    }
}
