use super::NoUnusedVars;
use regex::Regex;
use serde_json::Value;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum VarsOption {
    /// All variables are checked for usage, including those in the global scope.
    #[default]
    All,
    /// Checks only that locally-declared variables are used but will allow
    /// global variables to be unused.
    Local,
}

impl TryFrom<&String> for VarsOption {
    type Error = String;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "all" => Ok(Self::All),
            "local" => Ok(Self::Local),
            _ => Err(format!("Expected 'all' or 'local', got {value}")),
        }
    }
}

impl TryFrom<&Value> for VarsOption {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => Self::try_from(s),
            _ => Err(format!("Expected a string, got {value}")),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum ArgsOption {
    /// Unused positional arguments that occur before the last used argument
    /// will not be checked, but all named arguments and all positional
    /// arguments after the last used argument will be checked.
    #[default]
    AfterUsed,
    /// All named arguments must be used
    All,
    /// Do not check arguments
    None,
}

impl TryFrom<&Value> for ArgsOption {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => match s.as_str() {
                "after-used" => Ok(Self::AfterUsed),
                "all" => Ok(Self::All),
                "none" => Ok(Self::None),
                _ => Err(format!("Expected 'after-used', 'all', or 'none', got '{s}")),
            },
            _ => Err(format!("Expected a string, got {value}")),
        }
    }
}

/// Parses a potential pattern into a [`Regex`] that accepts unicode characters.
fn parse_unicode_rule(value: Option<&Value>, name: &str) -> Option<Regex> {
    value
        .and_then(Value::as_str)
        .map(|pattern| regex::RegexBuilder::new(pattern).unicode(true).build())
        .transpose()
        .map_err(|err| panic!("Invalid '{name}' option for no-unused-vars: {err}"))
        .unwrap()
}
impl From<Value> for NoUnusedVars {
    fn from(value: Value) -> Self {
        let Some(config) = value.get(0) else { return Self::default() };
        match config {
            Value::String(vars) => {
                let vars: VarsOption = vars
                    .try_into()
                    .map_err(|err| format!("Invalid 'vars' option for no-unused-vars: {err:}"))
                    .unwrap();
                Self { vars, ..Default::default() }
            }
            Value::Object(config) => {
                let vars = config
                    .get("vars")
                    .map(|vars| {
                        let vars: VarsOption = vars
                            .try_into()
                            .map_err(|err| {
                                format!("Invalid 'vars' option for no-unused-vars: {err:}")
                            })
                            .unwrap();
                        vars
                    })
                    .unwrap_or_default();

                let vars_ignore_pattern: Option<Regex> =
                    parse_unicode_rule(config.get("varsIgnorePattern"), "varsIgnorePattern");

                let args: ArgsOption = config
                    .get("args")
                    .map(|args| {
                        let args: ArgsOption = args
                            .try_into()
                            .map_err(|err| {
                                format!("Invalid 'args' option for no-unused-vars: {err:}")
                            })
                            .unwrap();
                        args
                    })
                    .unwrap_or_default();

                let args_ignore_pattern: Option<Regex> =
                    parse_unicode_rule(config.get("argsIgnorePattern"), "argsIgnorePattern");

                let caught_errors: bool = config
                    .get("caughtErrors")
                    .map(|caught_errors| {
                        match caught_errors {
                            Value::String(s) => match s.as_str() {
                                "all" => true,
                                "none" => false,
                                _ => panic!("Invalid 'caughtErrors' option for no-unused-vars: Expected 'all' or 'none', got {s}"),
                            },
                            _ => panic!("Invalid 'caughtErrors' option for no-unused-vars: Expected a string, got {caught_errors}"),
                            }
                        }).unwrap_or_default();

                let caught_errors_ignore_pattern = parse_unicode_rule(
                    config.get("caughtErrorsIgnorePattern"),
                    "caughtErrorsIgnorePattern",
                );

                let destructured_array_ignore_pattern: Option<Regex> = parse_unicode_rule(
                    config.get("destructuredArrayIgnorePattern"),
                    "destructuredArrayIgnorePattern",
                );

                let ignore_rest_siblings: bool = config
                    .get("ignoreRestSiblings")
                    .map_or(Some(false), Value::as_bool)
                    .unwrap_or(false);

                Self {
                    vars,
                    vars_ignore_pattern,
                    args,
                    args_ignore_pattern,
                    caught_errors,
                    caught_errors_ignore_pattern,
                    destructured_array_ignore_pattern,
                    ignore_rest_siblings,
                }
            }
            Value::Null => Self::default(),
            _ => panic!(
                "Invalid 'vars' option for no-unused-vars: Expected a string or an object, got {config}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_options_default() {
        let rule = NoUnusedVars::default();
        assert_eq!(rule.vars, VarsOption::All);
        assert!(rule.vars_ignore_pattern.is_none());
        assert_eq!(rule.args, ArgsOption::AfterUsed);
        assert!(rule.args_ignore_pattern.is_none());
        assert!(!rule.caught_errors);
        assert!(rule.caught_errors_ignore_pattern.is_none());
        assert!(rule.destructured_array_ignore_pattern.is_none());
        assert!(!rule.ignore_rest_siblings);
    }

    #[test]
    fn test_options_from_string() {
        let rule: NoUnusedVars = json!(["all"]).into();
        assert_eq!(rule.vars, VarsOption::All);

        let rule: NoUnusedVars = json!(["local"]).into();
        assert_eq!(rule.vars, VarsOption::Local);
    }

    #[test]
    fn test_options_from_object() {
        let rule: NoUnusedVars = json!([
            {
                "vars": "local",
                "varsIgnorePattern": "^_",
                "args": "all",
                "argsIgnorePattern": "^_",
                "caughtErrors": "all",
                "caughtErrorsIgnorePattern": "^_",
                "destructuredArrayIgnorePattern": "^_",
                "ignoreRestSiblings": true
            }
        ])
        .into();

        assert_eq!(rule.vars, VarsOption::Local);
        assert_eq!(rule.vars_ignore_pattern.unwrap().as_str(), "^_");
        assert_eq!(rule.args, ArgsOption::All);
        assert_eq!(rule.args_ignore_pattern.unwrap().as_str(), "^_");
        assert!(rule.caught_errors);
        assert_eq!(rule.caught_errors_ignore_pattern.unwrap().as_str(), "^_");
        assert_eq!(rule.destructured_array_ignore_pattern.unwrap().as_str(), "^_");
        assert!(rule.ignore_rest_siblings);
    }

    #[test]
    fn test_options_from_null() {
        let opts = NoUnusedVars::from(json!(null));
        let default = NoUnusedVars::default();
        assert_eq!(opts.vars, default.vars);
        assert!(opts.vars_ignore_pattern.is_none());
        assert!(default.vars_ignore_pattern.is_none());

        assert_eq!(opts.args, default.args);
        assert!(opts.args_ignore_pattern.is_none());
        assert!(default.args_ignore_pattern.is_none());

        assert_eq!(opts.caught_errors, default.caught_errors);
        assert!(opts.caught_errors_ignore_pattern.is_none());
        assert!(default.caught_errors_ignore_pattern.is_none());

        assert_eq!(opts.ignore_rest_siblings, default.ignore_rest_siblings);
    }

    #[test]
    fn test_parse_unicode_regex() {
        let pat = json!("^_");
        parse_unicode_rule(Some(&pat), "varsIgnorePattern")
            .expect("json strings should get parsed into a regex");
    }
}
