use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag},
    character::complete::space0,
    combinator::{complete, map, value},
    multi::{many0, many1, separated_list, separated_nonempty_list},
    sequence::delimited,
    IResult,
};

fn token(input: &str) -> IResult<&str, String> {
    let unquoted_token = map(is_not(" ;\""), String::from);

    let quoted_token = escaped_transform(is_not(r#""\"#), '\\', |control_char: &str| {
        alt((
            value(r#"""#, tag(r#"""#)),
            value(r#"\"#, tag(r#"\"#)),
            value("\r", tag("r")),
            value("\n", tag("n")),
            value("\t", tag("t")),
            value("\x0b", tag("v")), // vertical tab
                                     // TODO: mimic utils::append_escapes: pass through escaped backticks, left other
                                     // chars unchanged. Write tests for that!
        ))(control_char)
    });
    let double_quote = tag("\"");
    let quoted_token = delimited(&double_quote, quoted_token, &double_quote);

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
}
