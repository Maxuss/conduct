pub mod ast;
pub mod bin;
pub mod err;
pub mod parser;
pub mod tk;
pub mod validate;

pub use ahash::*;
pub use logos::*;

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{Read, Write},
        path::PathBuf,
    };

    use ariadne::Fmt;
    use logos::Logos;

    use crate::{
        bin::{from_binary, to_binary},
        check,
        err::{CodeArea, CodeSource, ConductCache, ErrorReport, FancyColorGenerator, Res},
        parser::Parser,
        tk::Token,
        validate::Validator,
    };

    #[allow(unused_macros)]
    macro_rules! printcheck {
        ($expr:expr) => {
            println!("{:#?}", check!($expr))
        };
    }

    #[test]
    fn basic_tokenization() {
        let text = r#"
        /*
        Line 1 comment
        Line 2 comment
        */
        import std.io

        let var = 0xFFAAFF
        var = 0b010101
        var = 0o143047
        var = 1234567890123456
        var = "Hello, World!"

        native fun callable(name) {
            println("Hello ${name}")
        }
        "#;
        let lexer = Token::lexer(text.trim());
        for tk in lexer {
            println!("{tk:?}")
        }
    }

    #[test]
    fn binary_ops() {
        let mut parser = Parser::new_inline("1234 + 16 ** 12 / 13 == 24");
        let expr = parser.parse_expression();
        assert!(expr.is_ok())
    }

    #[test]
    fn paths() {
        let mut parser = Parser::new_inline("variable.property[index](arg1, arg2, 0xBAD,)");
        let expr = parser.parse_expression();
        assert!(expr.is_ok())
    }

    #[test]
    fn unary_operator() {
        let mut parser = Parser::new_inline("!true & -3 ** 4");
        let expr = parser.parse_expression();
        assert!(expr.is_ok())
    }

    #[test]
    fn error_reports() {
        println!();
        let area1 = CodeArea {
            src: CodeSource::File("src/parser.rs".into()),
            span: (276, 360),
        };
        let area2 = CodeArea {
            src: CodeSource::Inline("let b = false\nlet a = different".to_owned()),
            span: (14, 31),
        };
        let area_current = CodeArea {
            src: CodeSource::Inline("import 'tech/test.cd'".to_owned()),
            span: (7, 21),
        };

        let mut colors = FancyColorGenerator::default();

        let report = ErrorReport {
            code: "E99",
            call_stack: vec![area1.clone(), area2.clone()],
            current_module: "tests".to_owned(),
            position: area_current.clone(),
            message: "Syntax error".to_string(),
            labels: vec![
                (
                    area1,
                    format!(
                        "Something went {} wrong here",
                        "very".fg(colors.next_color())
                    ),
                ),
                (
                    area2,
                    format!(
                        "And then this thing {} too!",
                        "failed miserably".fg(colors.next_color())
                    ),
                ),
                (
                    area_current,
                    format!(
                        "This is literally {}. The Gorbino's Quest of {}",
                        "Gorbino's Quest".fg(colors.next_color()),
                        "Errors".fg(colors.next_color())
                    ),
                ),
            ],
        };
        assert!(report.report().print(ConductCache::default()).is_ok());
    }

    #[test]
    fn errors() {
        let mut parser = Parser::new_inline("val(a");
        let expr = parser.parse_expression();
        assert!(expr.is_err())
    }

    #[test]
    fn stmt_import() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
import std.ffi
import '../include/headers.cdh'
import '../lib/frog.cdl'
        "#
            .trim(),
        );
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        Ok(())
    }

    #[test]
    fn stmt_return() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
return
return abc
xreturn 123
        "#
            .trim(),
        );
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        Ok(())
    }

    #[test]
    fn stmt_let() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
let a
let b = 1 + d()
let c = nil
        "#
            .trim(),
        );
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_const() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
const a = 0xFFAAFF;
const b = 1 + d()
const c = nil
        "#
            .trim(),
        );
        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_native_const() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
native const a;
native const b
native const internal$constant
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_native_fun() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
native fn pow(a, b)
native fn eval(code)
native fn noargs()
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_native_let() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
native let a;
        "#
            .trim(),
        );

        assert!(parser.parse_statement().is_err());

        Ok(())
    }

    #[test]
    fn stmt_fun() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#" 
fn main(args) {
    let a = 123
    let b = 456
}

fn empty() {

}

fn semicolon() {

};
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_if() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#" 
if true {
    // empty
}

if !false {
    let a = b
} else {
    // do other stuff
}

if false {
    let a = b
} else if true {
    let a = c
} else if nil {
    let a = d
} else {
    let a = nil
}
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_assign() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#" 
// let a

a = false
a += 1
a -= "Hello, World!"
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn stmt_expr() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#" 
// import sys
// import std.io

println('Hello, ${env[0]}')
12
file.create(args[0])
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn literal_array() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#" 
[]
[[1, 2, 3], [1, 2, 3], [1, 2, 3]]
[
    'a',
    'b',
    'c'
]
        "#
            .trim(),
        );

        check!(parser.parse_value());
        check!(parser.parse_value());
        check!(parser.parse_value());

        Ok(())
    }

    #[test]
    fn literal_compound() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
{
    int: 123,
    'string': "Hello, World!",
    array: [1, 2, 3]
}
{
    nested_object: {
        abc: 123
    }
}
{}
        "#
            .trim(),
        );

        check!(parser.parse_value());
        check!(parser.parse_value());
        check!(parser.parse_value());

        Ok(())
    }

    #[test]
    fn arrow_function() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
let noargs = () => {
    let a = 1;
}

let args = (arg1, arg2) => {
    import std.io
    println(arg1 + arg2)
}
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn ternaries() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
true ? a : b
nested ? a : true ? false : b
nil ? nil : nil
        "#
            .trim(),
        );

        check!(parser.parse_expression());
        check!(parser.parse_expression());
        check!(parser.parse_expression());

        Ok(())
    }

    #[test]
    fn type_definitions() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
type {
    hello: num,
    world: str
}

type {
    'hello': str,
}

type { }
        "#
            .trim(),
        );

        check!(parser.parse_expression());
        check!(parser.parse_expression());
        check!(parser.parse_expression());

        Ok(())
    }

    #[test]
    fn file_parsing() -> Res<()> {
        let path: PathBuf = "../tests/test.cd".into();
        let mut buf = String::new();
        let mut file = File::open(&path).unwrap();
        file.read_to_string(&mut buf).unwrap();
        let lexer = Token::lexer(&buf);
        let mut parser = Parser::new(CodeSource::File(path), lexer);
        let parsed = check!(parser.parse());

        let out = to_binary(parsed).unwrap();
        let out_path: PathBuf = "./target/file.cdt".into();
        File::create(out_path).unwrap().write_all(&out).unwrap();
        Ok(())
    }

    #[test]
    fn binary_parsing() {
        let path: PathBuf = "target/file.cdt".into();
        let mut out: Vec<u8> = Vec::new();
        File::open(path).unwrap().read_to_end(&mut out).unwrap();
        let stmts = from_binary(&out).unwrap();
        print!("{stmts:#?}")
    }

    #[test]
    fn stmt_module() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
module a
module 'hello!'
module test
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn string_escaping() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
"U\u1F60U"
"\ttab test\ta"
"carriage return\r\n"
        "#
            .trim(),
        );

        check!(parser.parse_value());
        check!(parser.parse_value());
        check!(parser.parse_value());

        Ok(())
    }

    #[test]
    fn for_statement() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
for i in 0..10 {
    for nested in 0..i {
        println("hello!")
    }
}

for ele in nil {
    println(ele)
}

for 'char' in 'chars' {

}
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn while_statement() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
while true {
    while false {
        break;
    }
    break
}

while nil {
    continue
}

while true ? true : false {

}
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn throw_statement() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
throw Error('Hello, World!')

// these next two are equal
throw nil
throw {}
        "#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn try_catch_statement() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
try {
    // all fine
    let a = false
}

try {
    let nested = true
} catch Infallible as _ {
    try { println("this is unreachable!"); } catch IoError as io { print("uh oh"); }
}

try {
    let a = false
    throw nil
} catch std.io.IoError as io {
    // catches a specific error
    println("An IO error has occurred!")
} catch * as error {
    // catches all other non-nil errors
    println("Error of type " + typeof(error) + " has occurred!")
} catch? {
    // catches all nil-throws
    println("A nil throw occurred!")
}
"#
            .trim(),
        );

        check!(parser.parse_statement());
        check!(parser.parse_statement());
        check!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn export_statement() -> Res<()> {
        let mut parser = Parser::new_inline(
            r#"
export std.io
export core
export __self__
"#
            .trim(),
        );

        printcheck!(parser.parse_statement());
        printcheck!(parser.parse_statement());
        printcheck!(parser.parse_statement());

        Ok(())
    }

    #[test]
    fn validation() -> Res<()> {
        let parser = Parser::new_inline(
            r#"
module main

import std.io
import core

const abc = false // defining a constant here

// some more code here
fn hello(name) {
    println("Hello, " + undefined)
}

let nil_check = nil!!

// attempting to reassign constant here
abc = true

if false {
    unreachable()
}
"#
            .trim(),
        );
        let validator = Validator::from(&parser);
        check!(parser.then_pipe(validator).finish_pipeline());
        // println!("{result:#?}");
        Ok(())
    }
}
