use sqlparser::ast::{BinaryOperator, Expr, Value};
use std::collections::HashMap;

#[derive(Debug)]
pub enum ParseResult {
    Boolean(bool),
    String(Option<String>),
}

pub trait Parser {
    fn evaluate(
        row: &HashMap<String, Option<String>>,
        expr: &Box<Expr>,
    ) -> Result<ParseResult, &'static str>;
}

pub struct Parse {}

struct EvaluateConjunction {}

struct EvaluateCondition {}

impl Parser for Parse {
    fn evaluate(
        row: &std::collections::HashMap<
            std::string::String,
            std::option::Option<std::string::String>,
        >,
        expr: &std::boxed::Box<sqlparser::ast::Expr>,
    ) -> std::result::Result<ParseResult, &'static str> {
        match &**expr {
            Expr::BinaryOp { op, .. } => match op {
                BinaryOperator::And | BinaryOperator::Or => {
                    Ok(EvaluateConjunction::evaluate(row, expr)?)
                }
                BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq => Ok(EvaluateCondition::evaluate(row, expr)?),

                _ => panic!(),
            },
            Expr::Value(val) => {
                let str = &val.clone().into_string();

                match str {
                    None => match &val.value {
                        Value::Boolean(bool) => Ok(ParseResult::Boolean(bool.clone())),
                        Value::Number(number, _) => Ok(ParseResult::String(Some(number.clone()))),
                        Value::SingleQuotedString(value)
                        | Value::TripleSingleQuotedString(value)
                        | Value::TripleDoubleQuotedString(value)
                        | Value::EscapedStringLiteral(value)
                        | Value::UnicodeStringLiteral(value)
                        | Value::SingleQuotedByteStringLiteral(value)
                        | Value::DoubleQuotedByteStringLiteral(value)
                        | Value::TripleSingleQuotedByteStringLiteral(value)
                        | Value::TripleDoubleQuotedByteStringLiteral(value)
                        | Value::SingleQuotedRawStringLiteral(value)
                        | Value::DoubleQuotedRawStringLiteral(value)
                        | Value::TripleSingleQuotedRawStringLiteral(value)
                        | Value::TripleDoubleQuotedRawStringLiteral(value)
                        | Value::NationalStringLiteral(value)
                        | Value::HexStringLiteral(value)
                        | Value::DoubleQuotedString(value) => {
                            Ok(ParseResult::String(Some(value.clone())))
                        }
                        Value::DollarQuotedString(value) => {
                            Ok(ParseResult::String(Some(value.value.clone())))
                        }
                        Value::Null => Ok(ParseResult::String(None)),
                        Value::Placeholder(_) => unreachable!(),
                    },
                    Some(str) => Ok(ParseResult::String(Some(str.clone()))),
                }
            }
            Expr::Identifier(identifier) => Ok(ParseResult::String(
                row.get(&identifier.value).unwrap().clone(),
            )),
            Expr::CompoundIdentifier(identifiers) => Ok(ParseResult::String(
                row.get(&identifiers.last().unwrap().value).unwrap().clone(),
            )),
            Expr::IsNull(expr) => match Parse::evaluate(row, expr)? {
                ParseResult::Boolean(_) => Ok(ParseResult::Boolean(false)),
                ParseResult::String(value) => Ok(ParseResult::Boolean(value.is_none())),
            },
            Expr::IsNotNull(expr) => match Parse::evaluate(row, expr)? {
                ParseResult::Boolean(_) => Ok(ParseResult::Boolean(false)),
                ParseResult::String(value) => Ok(ParseResult::Boolean(value.is_some())),
            },
            Expr::Nested(expr) => Parse::evaluate(row, expr),
            _ => Err("Unsupported expression"),
        }
    }
}

impl Parser for EvaluateConjunction {
    fn evaluate(
        row: &std::collections::HashMap<
            std::string::String,
            std::option::Option<std::string::String>,
        >,
        expr: &std::boxed::Box<sqlparser::ast::Expr>,
    ) -> Result<ParseResult, &'static str> {
        match &**expr {
            Expr::BinaryOp { left, op, right } => match op {
                BinaryOperator::And | BinaryOperator::Or => {
                    let parsed_left = Parse::evaluate(row, left)?;

                    match op {
                        BinaryOperator::And => {
                            if let ParseResult::Boolean(b) = parsed_left {
                                if !b {
                                    return Ok(ParseResult::Boolean(false));
                                }
                            }
                        }
                        BinaryOperator::Or => {
                            if let ParseResult::Boolean(b) = parsed_left {
                                if b {
                                    return Ok(ParseResult::Boolean(true));
                                }
                            }
                        }

                        _ => unreachable!(),
                    }

                    let parsed_right = Parse::evaluate(row, right)?;

                    match op {
                        BinaryOperator::And => {
                            if let ParseResult::Boolean(b) = parsed_right {
                                return Ok(ParseResult::Boolean(b));
                            };
                            panic!()
                        }
                        BinaryOperator::Or => {
                            if let ParseResult::Boolean(b) = parsed_right {
                                return Ok(ParseResult::Boolean(b));
                            };
                            panic!()
                        }
                        _ => unreachable!(),
                    }
                }
                _ => panic!(),
            },
            _ => todo!(),
        }
    }
}

impl Parser for EvaluateCondition {
    fn evaluate(
        row: &HashMap<String, Option<String>>,
        expr: &Box<Expr>,
    ) -> Result<ParseResult, &'static str> {
        if let Expr::BinaryOp { left, op, right } = &**expr {
            let parsed_left = Parse::evaluate(row, left)?;
            let parsed_right = Parse::evaluate(row, right)?;

            match op {
                BinaryOperator::Eq => {
                    return match (parsed_left, parsed_right) {
                        (ParseResult::Boolean(l), ParseResult::Boolean(r)) => {
                            Ok(ParseResult::Boolean(l == r))
                        }
                        (ParseResult::String(l), ParseResult::String(r)) => {
                            Ok(ParseResult::Boolean(l == r))
                        }
                        _ => Ok(ParseResult::Boolean(false)),
                    };
                }
                BinaryOperator::NotEq => {
                    return match (parsed_left, parsed_right) {
                        (ParseResult::Boolean(l), ParseResult::Boolean(r)) => {
                            Ok(ParseResult::Boolean(l != r))
                        }
                        (ParseResult::String(l), ParseResult::String(r)) => {
                            Ok(ParseResult::Boolean(l != r))
                        }
                        _ => Ok(ParseResult::Boolean(true)),
                    };
                }
                BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq => {
                    return match (parsed_left, parsed_right) {
                        (ParseResult::String(l), ParseResult::String(r)) => {
                            if l.is_none() || r.is_none() {
                                return Ok(ParseResult::Boolean(false));
                            }

                            match op {
                                BinaryOperator::Lt => {
                                    Ok(ParseResult::Boolean(l.unwrap() < r.unwrap()))
                                }
                                BinaryOperator::LtEq => {
                                    Ok(ParseResult::Boolean(l.unwrap() <= r.unwrap()))
                                }
                                BinaryOperator::Gt => {
                                    Ok(ParseResult::Boolean(l.unwrap() > r.unwrap()))
                                }
                                BinaryOperator::GtEq => {
                                    Ok(ParseResult::Boolean(l.unwrap() >= r.unwrap()))
                                }
                                _ => unreachable!(),
                            }
                        }

                        (_, _) => Ok(ParseResult::Boolean(false)),
                    }
                }
                _ => panic!(),
            }
        }

        unreachable!();
    }
}

#[cfg(test)]
mod tests {
    use crate::materialization::evaluator::{Parse, Parser};
    use log::debug;
    use sqlparser::ast::Statement;
    use sqlparser::dialect::MySqlDialect;
    use std::collections::HashMap;

    #[test]
    pub fn test_evaluator() {
        let mut row = HashMap::new();
        row.insert(String::from("a"), Some(String::from("1")));
        row.insert(String::from("x"), None);

        let parsed_sql = sqlparser::parser::Parser::parse_sql(
            &MySqlDialect {},
            "update account set a=\"b\" where (x is NULL AND a = \"SUCCESS\")",
        )
        .unwrap();

        if let Statement::Update { selection, .. } = parsed_sql.first().unwrap() {
            let expr = selection.clone().unwrap();
            debug!("{:?}", Parse::evaluate(&row, &Box::from(expr)));
        }
    }
}
