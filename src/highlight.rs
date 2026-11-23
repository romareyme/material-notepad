//! Syntax highlighting.
//!
//! A small lexer covers the languages Notepad Next exposes through Scintilla
//! lexers.  It is intentionally table driven: one scanner handles C-like,
//! scripting and configuration languages, while Markdown, markup, JSON, INI,
//! YAML and diff get dedicated line parsers.

use std::ops::Range;

use eframe::egui::Color32;

use crate::theme::SyntaxColors;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    PlainText,
    C,
    Cpp,
    CSharp,
    Java,
    JavaScript,
    TypeScript,
    Python,
    Rust,
    Go,
    Ruby,
    Lua,
    Perl,
    Php,
    Swift,
    Kotlin,
    Scala,
    Sql,
    Html,
    Xml,
    Css,
    Scss,
    Json,
    Yaml,
    Toml,
    Ini,
    Markdown,
    Shell,
    Batch,
    PowerShell,
    Makefile,
    CMake,
    Diff,
    Dockerfile,
    Properties,
    Tex,
    R,
    Matlab,
    Haskell,
    Erlang,
    Elixir,
    Clojure,
    Lisp,
    Fortran,
    Pascal,
    Ada,
    Asm,
    Verilog,
    Vhdl,
    Graphql,
    Protobuf,
    Vue,
    Svelte,
    Log,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Keyword,
    Type,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Preprocessor,
    Constant,
    Attribute,
    Tag,
    Heading,
    Link,
    Invalid,
}

impl TokenKind {
    pub fn color(self, colors: &SyntaxColors) -> Color32 {
        match self {
            TokenKind::Keyword => colors.keyword,
            TokenKind::Type => colors.type_name,
            TokenKind::Function => colors.function,
            TokenKind::String => colors.string,
            TokenKind::Number => colors.number,
            TokenKind::Comment => colors.comment,
            TokenKind::Operator => colors.operator,
            TokenKind::Punctuation => colors.punctuation,
            TokenKind::Preprocessor => colors.preprocessor,
            TokenKind::Constant => colors.constant,
            TokenKind::Attribute => colors.attribute,
            TokenKind::Tag => colors.tag,
            TokenKind::Heading => colors.heading,
            TokenKind::Link => colors.link,
            TokenKind::Invalid => colors.invalid,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Span {
    pub range: Range<usize>,
    pub kind: TokenKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Special {
    None,
    Markdown,
    Markup,
    Json,
    IniLike,
    Yaml,
    Diff,
}

struct Spec {
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    constants: &'static [&'static str],
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    triple_quotes: bool,
    case_insensitive: bool,
    preprocessor: bool,
    upper_is_type: bool,
    special: Special,
}

const NO_KEYWORDS: &[&str] = &[];

impl Language {
    pub const ALL: [Language; 54] = [
        Language::PlainText,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Java,
        Language::JavaScript,
        Language::TypeScript,
        Language::Python,
        Language::Rust,
        Language::Go,
        Language::Ruby,
        Language::Lua,
        Language::Perl,
        Language::Php,
        Language::Swift,
        Language::Kotlin,
        Language::Scala,
        Language::Sql,
        Language::Html,
        Language::Xml,
        Language::Css,
        Language::Scss,
        Language::Json,
        Language::Yaml,
        Language::Toml,
        Language::Ini,
        Language::Markdown,
        Language::Shell,
        Language::Batch,
        Language::PowerShell,
        Language::Makefile,
        Language::CMake,
        Language::Diff,
        Language::Dockerfile,
        Language::Properties,
        Language::Tex,
        Language::R,
        Language::Matlab,
        Language::Haskell,
        Language::Erlang,
        Language::Elixir,
        Language::Clojure,
        Language::Lisp,
        Language::Fortran,
        Language::Pascal,
        Language::Ada,
        Language::Asm,
        Language::Verilog,
        Language::Vhdl,
        Language::Graphql,
        Language::Protobuf,
        Language::Vue,
        Language::Svelte,
        Language::Log,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Language::PlainText => "Plain Text",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::CSharp => "C#",
            Language::Java => "Java",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::Rust => "Rust",
            Language::Go => "Go",
            Language::Ruby => "Ruby",
            Language::Lua => "Lua",
            Language::Perl => "Perl",
            Language::Php => "PHP",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
            Language::Scala => "Scala",
            Language::Sql => "SQL",
            Language::Html => "HTML",
            Language::Xml => "XML",
            Language::Css => "CSS",
            Language::Scss => "SCSS",
            Language::Json => "JSON",
            Language::Yaml => "YAML",
            Language::Toml => "TOML",
            Language::Ini => "INI",
            Language::Markdown => "Markdown",
            Language::Shell => "Shell",
            Language::Batch => "Batch",
            Language::PowerShell => "PowerShell",
            Language::Makefile => "Makefile",
            Language::CMake => "CMake",
            Language::Diff => "Diff",
            Language::Dockerfile => "Dockerfile",
            Language::Properties => "Properties",
            Language::Tex => "TeX",
            Language::R => "R",
            Language::Matlab => "MATLAB",
            Language::Haskell => "Haskell",
            Language::Erlang => "Erlang",
            Language::Elixir => "Elixir",
            Language::Clojure => "Clojure",
            Language::Lisp => "Lisp",
            Language::Fortran => "Fortran",
            Language::Pascal => "Pascal",
            Language::Ada => "Ada",
            Language::Asm => "Assembly",
            Language::Verilog => "Verilog",
            Language::Vhdl => "VHDL",
            Language::Graphql => "GraphQL",
            Language::Protobuf => "Protocol Buffers",
            Language::Vue => "Vue",
            Language::Svelte => "Svelte",
            Language::Log => "Log",
        }
    }

    /// Detect the language from a file name, extension and, as a fallback, a
    /// shebang line.
    pub fn detect(extension: Option<&str>, text: &str, filename: Option<&str>) -> Language {
        if let Some(name) = filename {
            let lower = name.to_ascii_lowercase();
            match lower.as_str() {
                "makefile" | "gnumakefile" => return Language::Makefile,
                "cmakelists.txt" => return Language::CMake,
                "dockerfile" => return Language::Dockerfile,
                ".gitignore" | ".gitattributes" | ".editorconfig" => return Language::Properties,
                _ => {}
            }
        }
        if let Some(ext) = extension {
            let ext = ext.to_ascii_lowercase();
            let lang = match ext.as_str() {
                "c" | "h" => Language::C,
                "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" | "ino" => Language::Cpp,
                "cs" => Language::CSharp,
                "java" => Language::Java,
                "js" | "mjs" | "cjs" | "jsx" => Language::JavaScript,
                "ts" | "tsx" | "mts" => Language::TypeScript,
                "py" | "pyw" | "pyi" => Language::Python,
                "rs" => Language::Rust,
                "go" => Language::Go,
                "rb" | "rake" | "gemspec" => Language::Ruby,
                "lua" => Language::Lua,
                "pl" | "pm" => Language::Perl,
                "php" => Language::Php,
                "swift" => Language::Swift,
                "kt" | "kts" => Language::Kotlin,
                "scala" | "sc" => Language::Scala,
                "sql" => Language::Sql,
                "html" | "htm" | "xhtml" => Language::Html,
                "xml" | "xsd" | "xsl" | "svg" | "plist" => Language::Xml,
                "css" => Language::Css,
                "scss" | "sass" => Language::Scss,
                "json" | "jsonc" | "ipynb" => Language::Json,
                "yaml" | "yml" => Language::Yaml,
                "toml" => Language::Toml,
                "ini" | "cfg" | "conf" | "cnf" => Language::Ini,
                "md" | "markdown" | "mdown" => Language::Markdown,
                "sh" | "bash" | "zsh" | "fish" | "ksh" => Language::Shell,
                "bat" | "cmd" => Language::Batch,
                "ps1" | "psm1" | "psd1" => Language::PowerShell,
                "mk" => Language::Makefile,
                "cmake" => Language::CMake,
                "diff" | "patch" => Language::Diff,
                "dockerfile" | "containerfile" => Language::Dockerfile,
                "properties" | "env" | "gitignore" => Language::Properties,
                "tex" | "sty" | "cls" => Language::Tex,
                "r" => Language::R,
                "m" => Language::Matlab,
                "hs" | "lhs" => Language::Haskell,
                "erl" | "hrl" => Language::Erlang,
                "ex" | "exs" => Language::Elixir,
                "clj" | "cljs" | "cljc" => Language::Clojure,
                "lisp" | "el" | "scm" | "ss" => Language::Lisp,
                "f" | "f90" | "f95" | "f03" => Language::Fortran,
                "pas" | "pp" | "dpr" => Language::Pascal,
                "adb" | "ads" => Language::Ada,
                "asm" | "s" => Language::Asm,
                "v" | "vh" | "sv" => Language::Verilog,
                "vhd" | "vhdl" => Language::Vhdl,
                "graphql" | "gql" => Language::Graphql,
                "proto" => Language::Protobuf,
                "vue" => Language::Vue,
                "svelte" => Language::Svelte,
                "log" => Language::Log,
                _ => Language::PlainText,
            };
            if lang != Language::PlainText {
                return lang;
            }
        }
        let first = text.lines().next().unwrap_or("");
        if let Some(rest) = first.strip_prefix("#!") {
            let rest = rest.to_ascii_lowercase();
            if rest.contains("python") {
                return Language::Python;
            }
            if rest.contains("bash") || rest.contains("sh") || rest.contains("zsh") {
                return Language::Shell;
            }
            if rest.contains("perl") {
                return Language::Perl;
            }
            if rest.contains("node") {
                return Language::JavaScript;
            }
            if rest.contains("ruby") {
                return Language::Ruby;
            }
            if rest.contains("lua") {
                return Language::Lua;
            }
        }
        Language::PlainText
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Language::C => &["c", "h"],
            Language::Cpp => &["cpp", "cxx", "cc", "hpp", "hxx", "ino"],
            Language::CSharp => &["cs"],
            Language::Java => &["java"],
            Language::JavaScript => &["js", "mjs", "cjs", "jsx"],
            Language::TypeScript => &["ts", "tsx"],
            Language::Python => &["py", "pyw", "pyi"],
            Language::Rust => &["rs"],
            Language::Go => &["go"],
            Language::Ruby => &["rb"],
            Language::Lua => &["lua"],
            Language::Perl => &["pl", "pm"],
            Language::Php => &["php"],
            Language::Swift => &["swift"],
            Language::Kotlin => &["kt", "kts"],
            Language::Scala => &["scala"],
            Language::Sql => &["sql"],
            Language::Html => &["html", "htm"],
            Language::Xml => &["xml", "svg", "xsd", "xsl"],
            Language::Css => &["css"],
            Language::Scss => &["scss", "sass"],
            Language::Json => &["json", "jsonc"],
            Language::Yaml => &["yaml", "yml"],
            Language::Toml => &["toml"],
            Language::Ini => &["ini", "cfg", "conf"],
            Language::Markdown => &["md", "markdown"],
            Language::Shell => &["sh", "bash", "zsh"],
            Language::Batch => &["bat", "cmd"],
            Language::PowerShell => &["ps1", "psm1"],
            Language::Makefile => &["mk"],
            Language::CMake => &["cmake"],
            Language::Diff => &["diff", "patch"],
            Language::Dockerfile => &["dockerfile"],
            Language::Properties => &["properties", "env"],
            Language::Tex => &["tex", "sty"],
            Language::R => &["r"],
            Language::Matlab => &["m"],
            Language::Haskell => &["hs"],
            Language::Erlang => &["erl", "hrl"],
            Language::Elixir => &["ex", "exs"],
            Language::Clojure => &["clj", "cljs"],
            Language::Lisp => &["lisp", "el", "scm"],
            Language::Fortran => &["f", "f90", "f95"],
            Language::Pascal => &["pas", "pp"],
            Language::Ada => &["adb", "ads"],
            Language::Asm => &["asm", "s"],
            Language::Verilog => &["v", "vh", "sv"],
            Language::Vhdl => &["vhd", "vhdl"],
            Language::Graphql => &["graphql", "gql"],
            Language::Protobuf => &["proto"],
            Language::Vue => &["vue"],
            Language::Svelte => &["svelte"],
            Language::Log => &["log"],
            _ => &[],
        }
    }

    fn spec(self) -> Spec {
        let plain = Spec {
            keywords: NO_KEYWORDS,
            types: NO_KEYWORDS,
            constants: NO_KEYWORDS,
            line_comments: &[],
            block_comment: None,
            quotes: &[],
            triple_quotes: false,
            case_insensitive: false,
            preprocessor: false,
            upper_is_type: false,
            special: Special::None,
        };
        match self {
            Language::PlainText | Language::Log => plain,
            Language::C => Spec {
                keywords: C_KEYWORDS,
                types: C_TYPES,
                constants: &["NULL", "true", "false"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                preprocessor: true,
                ..plain
            },
            Language::Cpp => Spec {
                keywords: CPP_KEYWORDS,
                types: C_TYPES,
                constants: &["NULL", "nullptr", "true", "false"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                preprocessor: true,
                upper_is_type: true,
                ..plain
            },
            Language::CSharp => Spec {
                keywords: CSHARP_KEYWORDS,
                types: CSHARP_TYPES,
                constants: &["true", "false", "null"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                upper_is_type: true,
                ..plain
            },
            Language::Java => Spec {
                keywords: JAVA_KEYWORDS,
                types: JAVA_TYPES,
                constants: &["true", "false", "null"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                upper_is_type: true,
                ..plain
            },
            Language::JavaScript => Spec {
                keywords: JS_KEYWORDS,
                types: &[],
                constants: &["true", "false", "null", "undefined", "NaN", "Infinity"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\'', '`'],
                ..plain
            },
            Language::TypeScript => Spec {
                keywords: TS_KEYWORDS,
                types: TS_TYPES,
                constants: &["true", "false", "null", "undefined"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\'', '`'],
                upper_is_type: true,
                ..plain
            },
            Language::Python => Spec {
                keywords: PYTHON_KEYWORDS,
                types: PYTHON_TYPES,
                constants: &["True", "False", "None", "self", "cls"],
                line_comments: &["#"],
                quotes: &['"', '\''],
                triple_quotes: true,
                ..plain
            },
            Language::Rust => Spec {
                keywords: RUST_KEYWORDS,
                types: RUST_TYPES,
                constants: &["true", "false"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"'],
                upper_is_type: true,
                ..plain
            },
            Language::Go => Spec {
                keywords: GO_KEYWORDS,
                types: GO_TYPES,
                constants: &["true", "false", "nil", "iota"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\'', '`'],
                upper_is_type: true,
                ..plain
            },
            Language::Ruby => Spec {
                keywords: RUBY_KEYWORDS,
                types: &[],
                constants: &["true", "false", "nil", "self"],
                line_comments: &["#"],
                quotes: &['"', '\''],
                upper_is_type: true,
                ..plain
            },
            Language::Lua => Spec {
                keywords: LUA_KEYWORDS,
                types: &[],
                constants: &["true", "false", "nil"],
                line_comments: &["--"],
                block_comment: Some(("--[[", "]]")),
                quotes: &['"', '\''],
                ..plain
            },
            Language::Perl => Spec {
                keywords: PERL_KEYWORDS,
                types: &[],
                constants: &["undef"],
                line_comments: &["#"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Php => Spec {
                keywords: PHP_KEYWORDS,
                types: &[],
                constants: &["true", "false", "null"],
                line_comments: &["//", "#"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                ..plain
            },
            Language::Swift => Spec {
                keywords: SWIFT_KEYWORDS,
                types: SWIFT_TYPES,
                constants: &["true", "false", "nil"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"'],
                upper_is_type: true,
                ..plain
            },
            Language::Kotlin => Spec {
                keywords: KOTLIN_KEYWORDS,
                types: KOTLIN_TYPES,
                constants: &["true", "false", "null"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                triple_quotes: true,
                upper_is_type: true,
                ..plain
            },
            Language::Scala => Spec {
                keywords: SCALA_KEYWORDS,
                types: &[],
                constants: &["true", "false", "null", "None", "Nil"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                upper_is_type: true,
                ..plain
            },
            Language::Sql => Spec {
                keywords: SQL_KEYWORDS,
                types: SQL_TYPES,
                constants: &["true", "false", "null"],
                line_comments: &["--"],
                block_comment: Some(("/*", "*/")),
                quotes: &['\'', '"'],
                case_insensitive: true,
                ..plain
            },
            Language::Html | Language::Xml | Language::Vue | Language::Svelte => Spec {
                special: Special::Markup,
                line_comments: &[],
                block_comment: Some(("<!--", "-->")),
                quotes: &['"', '\''],
                ..plain
            },
            Language::Css | Language::Scss => Spec {
                keywords: CSS_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &[],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                ..plain
            },
            Language::Json => Spec {
                special: Special::Json,
                quotes: &['"'],
                ..plain
            },
            Language::Yaml => Spec {
                special: Special::Yaml,
                line_comments: &["#"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Toml | Language::Ini | Language::Properties => Spec {
                special: Special::IniLike,
                line_comments: &["#", ";", "!"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Markdown => Spec {
                special: Special::Markdown,
                ..plain
            },
            Language::Shell => Spec {
                keywords: SHELL_KEYWORDS,
                types: &[],
                constants: &["true", "false"],
                line_comments: &["#"],
                quotes: &['"', '\'', '`'],
                ..plain
            },
            Language::Batch => Spec {
                keywords: BATCH_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &["REM", "rem", "::"],
                quotes: &['"'],
                case_insensitive: true,
                ..plain
            },
            Language::PowerShell => Spec {
                keywords: POWERSHELL_KEYWORDS,
                types: &[],
                constants: &["$true", "$false", "$null"],
                line_comments: &["#"],
                block_comment: Some(("<#", "#>")),
                quotes: &['"', '\''],
                case_insensitive: true,
                ..plain
            },
            Language::Makefile => Spec {
                keywords: MAKEFILE_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &["#"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::CMake => Spec {
                keywords: CMAKE_KEYWORDS,
                types: &[],
                constants: &["TRUE", "FALSE", "ON", "OFF"],
                line_comments: &["#"],
                quotes: &['"'],
                case_insensitive: true,
                ..plain
            },
            Language::Diff => Spec {
                special: Special::Diff,
                ..plain
            },
            Language::Dockerfile => Spec {
                keywords: DOCKER_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &["#"],
                quotes: &['"', '\''],
                case_insensitive: true,
                ..plain
            },
            Language::Tex => Spec {
                keywords: TEX_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &["%"],
                block_comment: None,
                quotes: &[],
                ..plain
            },
            Language::R => Spec {
                keywords: R_KEYWORDS,
                types: &[],
                constants: &["TRUE", "FALSE", "NULL", "NA", "NaN", "Inf"],
                line_comments: &["#"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Matlab => Spec {
                keywords: MATLAB_KEYWORDS,
                types: &[],
                constants: &["true", "false", "NaN", "Inf"],
                line_comments: &["%"],
                block_comment: Some(("%{", "%}")),
                quotes: &['"', '\''],
                ..plain
            },
            Language::Haskell => Spec {
                keywords: HASKELL_KEYWORDS,
                types: &[],
                constants: &["True", "False", "Nothing", "Just"],
                line_comments: &["--"],
                block_comment: Some(("{-", "-}")),
                quotes: &['"'],
                upper_is_type: true,
                ..plain
            },
            Language::Erlang => Spec {
                keywords: ERLANG_KEYWORDS,
                types: &[],
                constants: &["true", "false", "undefined"],
                line_comments: &["%"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Elixir => Spec {
                keywords: ELIXIR_KEYWORDS,
                types: &[],
                constants: &["true", "false", "nil"],
                line_comments: &["#"],
                quotes: &['"', '\''],
                triple_quotes: true,
                ..plain
            },
            Language::Clojure | Language::Lisp => Spec {
                keywords: LISP_KEYWORDS,
                types: &[],
                constants: &["nil", "true", "false", "t"],
                line_comments: &[";"],
                quotes: &['"'],
                ..plain
            },
            Language::Fortran => Spec {
                keywords: FORTRAN_KEYWORDS,
                types: &[],
                constants: &[".true.", ".false."],
                line_comments: &["!"],
                quotes: &['"', '\''],
                case_insensitive: true,
                ..plain
            },
            Language::Pascal => Spec {
                keywords: PASCAL_KEYWORDS,
                types: &[],
                constants: &["true", "false", "nil"],
                line_comments: &["//"],
                block_comment: Some(("(*", "*)")),
                quotes: &['\''],
                case_insensitive: true,
                ..plain
            },
            Language::Ada => Spec {
                keywords: ADA_KEYWORDS,
                types: &[],
                constants: &["true", "false"],
                line_comments: &["--"],
                quotes: &['"'],
                case_insensitive: true,
                ..plain
            },
            Language::Asm => Spec {
                keywords: ASM_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &[";", "#"],
                quotes: &['"', '\''],
                ..plain
            },
            Language::Verilog => Spec {
                keywords: VERILOG_KEYWORDS,
                types: &[],
                constants: &[],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"'],
                ..plain
            },
            Language::Vhdl => Spec {
                keywords: VHDL_KEYWORDS,
                types: &[],
                constants: &["true", "false"],
                line_comments: &["--"],
                quotes: &['"', '\''],
                case_insensitive: true,
                ..plain
            },
            Language::Graphql => Spec {
                keywords: GRAPHQL_KEYWORDS,
                types: &[],
                constants: &["true", "false", "null"],
                line_comments: &["#"],
                quotes: &['"'],
                ..plain
            },
            Language::Protobuf => Spec {
                keywords: PROTOBUF_KEYWORDS,
                types: PROTOBUF_TYPES,
                constants: &["true", "false"],
                line_comments: &["//"],
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                ..plain
            },
        }
    }
}

/// Highlight a whole document. Spans are sorted, non-overlapping and only
/// cover tokens (gaps are rendered with the default text colour).
pub fn highlight(text: &str, language: Language, colors: &SyntaxColors) -> Vec<Span> {
    let _ = colors;
    let spec = language.spec();
    let mut spans = Vec::new();
    match spec.special {
        Special::Markdown => markdown(text, &mut spans),
        Special::Markup => markup(text, &mut spans),
        Special::Json => json(text, &mut spans),
        Special::IniLike => ini_like(text, &mut spans),
        Special::Yaml => yaml(text, &mut spans),
        Special::Diff => diff(text, &mut spans),
        Special::None => {
            if language == Language::Css || language == Language::Scss {
                css(text, &spec, &mut spans);
            } else {
                generic(text, &spec, &mut spans);
            }
        }
    }
    normalize(&mut spans, text.len());
    spans
}

fn normalize(spans: &mut Vec<Span>, len: usize) {
    spans.retain(|s| s.range.start < s.range.end && s.range.end <= len);
    spans.sort_by_key(|s| (s.range.start, s.range.end));
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans.drain(..) {
        if let Some(last) = merged.last_mut() {
            if last.kind == span.kind && last.range.end >= span.range.start {
                last.range.end = last.range.end.max(span.range.end);
                continue;
            }
        }
        merged.push(span);
    }
    *spans = merged;
}

fn generic(text: &str, spec: &Spec, spans: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut line_start = true;

    while i < len {
        let b = bytes[i];
        if b == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if b == b' ' || b == b'\t' || b == b'\r' {
            i += 1;
            continue;
        }
        // Preprocessor directives.
        if spec.preprocessor && b == b'#' && line_start {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            spans.push(Span {
                range: start..i,
                kind: TokenKind::Preprocessor,
            });
            continue;
        }
        line_start = false;

        // Line comments.
        let mut matched = false;
        for marker in spec.line_comments {
            if text[i..].starts_with(marker) {
                let start = i;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                spans.push(Span {
                    range: start..i,
                    kind: TokenKind::Comment,
                });
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        // Block comments.
        if let Some((open, close)) = spec.block_comment {
            if text[i..].starts_with(open) {
                let start = i;
                i += open.len();
                if let Some(pos) = text[i..].find(close) {
                    i += pos + close.len();
                } else {
                    i = len;
                }
                spans.push(Span {
                    range: start..i,
                    kind: TokenKind::Comment,
                });
                continue;
            }
        }

        // Triple quoted strings (Python, Kotlin, Elixir).
        if spec.triple_quotes {
            let mut found = None;
            for q in spec.quotes {
                if text[i..].starts_with(&q.to_string().repeat(3)) {
                    found = Some(*q);
                    break;
                }
            }
            if let Some(q) = found {
                let start = i;
                let delimiter = q.to_string().repeat(3);
                i += delimiter.len();
                if let Some(pos) = text[i..].find(&delimiter) {
                    i += pos + delimiter.len();
                } else {
                    i = len;
                }
                spans.push(Span {
                    range: start..i,
                    kind: TokenKind::String,
                });
                continue;
            }
        }

        // Strings.
        let ch = text[i..].chars().next().unwrap_or(' ');
        if spec.quotes.contains(&ch) {
            let start = i;
            i += ch.len_utf8();
            while i < len {
                let c = text[i..].chars().next().unwrap_or(' ');
                if c == '\\' {
                    i += 1;
                    if i < len {
                        i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    }
                    continue;
                }
                if c == ch {
                    i += c.len_utf8();
                    break;
                }
                if c == '\n' {
                    break;
                }
                i += c.len_utf8();
            }
            spans.push(Span {
                range: start..i,
                kind: TokenKind::String,
            });
            continue;
        }

        // Numbers.
        if b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < len {
                let c = bytes[i];
                if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' {
                    i += 1;
                } else if (c == b'+' || c == b'-')
                    && i > start
                    && matches!(bytes[i - 1], b'e' | b'E')
                {
                    i += 1;
                } else {
                    break;
                }
            }
            spans.push(Span {
                range: start..i,
                kind: TokenKind::Number,
            });
            continue;
        }

        // Attributes / decorators.
        if b == b'@' || (b == b'#' && matches!(text.as_bytes().get(i + 1), Some(&b'['))) {
            let start = i;
            i += 1;
            while i < len {
                let c = bytes[i];
                if c.is_ascii_alphanumeric() || c == b'_' || c == b':' {
                    i += 1;
                } else {
                    break;
                }
            }
            spans.push(Span {
                range: start..i,
                kind: TokenKind::Attribute,
            });
            continue;
        }

        // Identifiers.
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let start = i;
            while i < len {
                let c = text[i..].chars().next().unwrap_or(' ');
                if c.is_alphanumeric() || c == '_' || c == '$' {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
            let word = &text[start..i];
            let kind = if spec
                .keywords
                .iter()
                .any(|k| eq_word(k, word, spec.case_insensitive))
            {
                Some(TokenKind::Keyword)
            } else if spec
                .types
                .iter()
                .any(|k| eq_word(k, word, spec.case_insensitive))
            {
                Some(TokenKind::Type)
            } else if spec
                .constants
                .iter()
                .any(|k| eq_word(k, word, spec.case_insensitive))
            {
                Some(TokenKind::Constant)
            } else if next_non_space_is(bytes, i, b'(') {
                Some(TokenKind::Function)
            } else if spec.upper_is_type
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                && word
                    .chars()
                    .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
                && word.len() > 2
            {
                Some(TokenKind::Constant)
            } else if spec.upper_is_type
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
            {
                Some(TokenKind::Type)
            } else {
                None
            };
            if let Some(kind) = kind {
                spans.push(Span {
                    range: start..i,
                    kind,
                });
            }
            continue;
        }

        // Operators and punctuation.
        let ch_len = ch.len_utf8();
        let kind = match b {
            b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';' | b':' | b'.' => {
                TokenKind::Punctuation
            }
            b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'!' | b'&' | b'|' | b'^'
            | b'~' | b'?' => TokenKind::Operator,
            _ => {
                i += ch_len;
                continue;
            }
        };
        // Multi-character operators stay a single span where they are common.
        let start = i;
        i += ch_len;
        spans.push(Span {
            range: start..i,
            kind,
        });
    }
}

fn eq_word(keyword: &str, word: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        keyword.eq_ignore_ascii_case(word)
    } else {
        keyword == word
    }
}

fn next_non_space_is(bytes: &[u8], mut i: usize, needle: u8) -> bool {
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    bytes.get(i) == Some(&needle)
}

fn push(spans: &mut Vec<Span>, start: usize, end: usize, kind: TokenKind) {
    if end > start {
        spans.push(Span {
            range: start..end,
            kind,
        });
    }
}

fn markdown(text: &str, spans: &mut Vec<Span>) {
    let mut offset = 0usize;
    let mut in_fence = false;
    let mut fence = String::new();
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches('\n');
        let trimmed = content.trim_start();
        let indent = content.len() - trimmed.len();
        if in_fence {
            push(spans, offset, offset + content.len(), TokenKind::String);
            if trimmed.starts_with(&fence) {
                in_fence = false;
            }
            offset += line.len();
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker: String = trimmed
                .chars()
                .take_while(|c| *c == '`' || *c == '~')
                .collect();
            in_fence = true;
            fence = marker;
            push(spans, offset, offset + content.len(), TokenKind::Tag);
            offset += line.len();
            continue;
        }
        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if hashes <= 6 && trimmed.chars().nth(hashes) == Some(' ') {
                push(spans, offset, offset + content.len(), TokenKind::Heading);
                offset += line.len();
                continue;
            }
        }
        if trimmed.starts_with('>') {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
            offset += line.len();
            continue;
        }
        if trimmed.starts_with("---") || trimmed.starts_with("***") {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
            offset += line.len();
            continue;
        }
        // List markers.
        let marker_len = if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
        {
            2
        } else {
            let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && trimmed[digits..].starts_with(". ") {
                digits + 2
            } else {
                0
            }
        };
        if marker_len > 0 {
            push(
                spans,
                offset + indent,
                offset + indent + marker_len,
                TokenKind::Punctuation,
            );
        }
        inline_markdown(content, offset, spans);
        offset += line.len();
    }
}

fn inline_markdown(content: &str, offset: usize, spans: &mut Vec<Span>) {
    let bytes = content.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if content[i..].starts_with("**") || content[i..].starts_with("__") {
            if let Some(end) = content[i + 2..].find(&content[i..i + 2]) {
                push(spans, offset + i, offset + i + 2 + end + 2, TokenKind::Tag);
                i += 2 + end + 2;
                continue;
            }
        }
        if content[i..].starts_with('`') {
            if let Some(end) = content[i + 1..].find('`') {
                push(
                    spans,
                    offset + i,
                    offset + i + 1 + end + 1,
                    TokenKind::String,
                );
                i += 1 + end + 1;
                continue;
            }
        }
        if content[i..].starts_with('[') {
            if let Some(close) = content[i..].find(']') {
                let after = i + close + 1;
                if content[after..].starts_with('(') {
                    if let Some(paren) = content[after..].find(')') {
                        push(
                            spans,
                            offset + after + 1,
                            offset + after + paren,
                            TokenKind::Link,
                        );
                        i = after + paren + 1;
                        continue;
                    }
                }
            }
        }
        if content[i..].starts_with('<') {
            if let Some(end) = content[i..].find('>') {
                push(spans, offset + i, offset + i + end + 1, TokenKind::Tag);
                i += end + 1;
                continue;
            }
        }
        i += content[i..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
    }
}

fn markup(text: &str, spans: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    while i < len {
        if text[i..].starts_with("<!--") {
            let start = i;
            i = text[i..].find("-->").map(|p| i + p + 3).unwrap_or(len);
            push(spans, start, i, TokenKind::Comment);
            continue;
        }
        if text[i..].starts_with("<![CDATA[") {
            let start = i;
            i = text[i..].find("]]>").map(|p| i + p + 3).unwrap_or(len);
            push(spans, start, i, TokenKind::String);
            continue;
        }
        if text[i..].starts_with("<!DOCTYPE") || text[i..].starts_with("<!doctype") {
            let start = i;
            i = text[i..].find('>').map(|p| i + p + 1).unwrap_or(len);
            push(spans, start, i, TokenKind::Preprocessor);
            continue;
        }
        if bytes[i] == b'<' {
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'/' {
                i += 1;
            }
            let name_start = i;
            while i < len {
                let c = bytes[i];
                if c.is_ascii_alphanumeric() || c == b'-' || c == b':' || c == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            push(spans, start, i, TokenKind::Tag);
            let _ = name_start;
            // Attributes until '>'.
            while i < len && bytes[i] != b'>' {
                if bytes[i] == b'"' || bytes[i] == b'\'' {
                    let quote = bytes[i];
                    let vstart = i;
                    i += 1;
                    while i < len && bytes[i] != quote {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    push(spans, vstart, i, TokenKind::String);
                    continue;
                }
                if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' || bytes[i] == b':' {
                    let astart = i;
                    while i < len
                        && (bytes[i].is_ascii_alphanumeric()
                            || bytes[i] == b'-'
                            || bytes[i] == b'_'
                            || bytes[i] == b':')
                    {
                        i += 1;
                    }
                    push(spans, astart, i, TokenKind::Attribute);
                    continue;
                }
                i += 1;
            }
            if i < len {
                i += 1;
                push(spans, i - 1, i, TokenKind::Tag);
            }
            continue;
        }
        if bytes[i] == b'&' {
            if let Some(end) = text[i..].find(';').filter(|e| *e < 12) {
                push(spans, i, i + end + 1, TokenKind::Constant);
                i += end + 1;
                continue;
            }
        }
        i += 1;
    }
}

fn json(text: &str, spans: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    while i < len {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let mut j = i;
                while j < len && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                let kind = if bytes.get(j) == Some(&b':') {
                    TokenKind::Attribute
                } else {
                    TokenKind::String
                };
                push(spans, start, i, kind);
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                i += 1;
                while i < len
                    && (bytes[i].is_ascii_digit()
                        || bytes[i] == b'.'
                        || bytes[i] == b'e'
                        || bytes[i] == b'E'
                        || bytes[i] == b'+'
                        || bytes[i] == b'-')
                {
                    i += 1;
                }
                push(spans, start, i, TokenKind::Number);
            }
            b't' | b'f' | b'n' => {
                let start = i;
                while i < len && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                push(spans, start, i, TokenKind::Constant);
            }
            b'/' if text[i..].starts_with("//") => {
                let start = i;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                push(spans, start, i, TokenKind::Comment);
            }
            b'/' if text[i..].starts_with("/*") => {
                let start = i;
                i = text[i..].find("*/").map(|p| i + p + 2).unwrap_or(len);
                push(spans, start, i, TokenKind::Comment);
            }
            _ => i += 1,
        }
    }
}

fn ini_like(text: &str, spans: &mut Vec<Span>) {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let trimmed = content.trim_start();
        let indent = content.len() - trimmed.len();
        if trimmed.starts_with('[') && trimmed.contains(']') {
            push(spans, offset, offset + content.len(), TokenKind::Heading);
        } else if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with('!') {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
        } else if let Some(eq) = trimmed.find(['=', ':']) {
            let key = &trimmed[..eq];
            if !key.trim().is_empty() {
                push(
                    spans,
                    offset + indent,
                    offset + indent + eq,
                    TokenKind::Attribute,
                );
            }
            let value = &trimmed[eq + 1..];
            let vstart = offset + indent + eq + 1;
            if let Some(q) = value.find(['"', '\'']) {
                push(spans, vstart + q, offset + content.len(), TokenKind::String);
            } else if value.trim() == "true"
                || value.trim() == "false"
                || value.trim().parse::<f64>().is_ok()
            {
                push(spans, vstart, offset + content.len(), TokenKind::Number);
            }
        }
        offset += line.len();
    }
}

fn yaml(text: &str, spans: &mut Vec<Span>) {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let trimmed = content.trim_start();
        let indent = content.len() - trimmed.len();
        if trimmed.starts_with('#') {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
        } else if trimmed.starts_with("---") || trimmed.starts_with("...") {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
        } else {
            if let Some(colon) = trimmed.find(':') {
                let key = &trimmed[..colon];
                if !key.trim().is_empty() && !key.contains('#') {
                    push(
                        spans,
                        offset + indent,
                        offset + indent + colon,
                        TokenKind::Attribute,
                    );
                }
                let value = trimmed[colon + 1..].trim();
                if value.starts_with('"') || value.starts_with('\'') {
                    let vstart = content.len() - value.len();
                    push(
                        spans,
                        offset + vstart,
                        offset + content.len(),
                        TokenKind::String,
                    );
                } else if value == "true"
                    || value == "false"
                    || value == "null"
                    || value.parse::<f64>().is_ok()
                {
                    let vstart = content.len() - value.len();
                    push(
                        spans,
                        offset + vstart,
                        offset + content.len(),
                        TokenKind::Constant,
                    );
                }
            } else if trimmed.starts_with("- ") {
                push(
                    spans,
                    offset + indent,
                    offset + indent + 1,
                    TokenKind::Punctuation,
                );
            }
        }
        offset += line.len();
    }
}

fn diff(text: &str, spans: &mut Vec<Span>) {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content.starts_with('+') {
            push(spans, offset, offset + content.len(), TokenKind::String);
        } else if content.starts_with('-') {
            push(spans, offset, offset + content.len(), TokenKind::Invalid);
        } else if content.starts_with("@@")
            || content.starts_with("diff ")
            || content.starts_with("index ")
        {
            push(spans, offset, offset + content.len(), TokenKind::Comment);
        }
        offset += line.len();
    }
}

fn css(text: &str, spec: &Spec, spans: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut in_selector = true;
    while i < len {
        if text[i..].starts_with("/*") {
            let start = i;
            i = text[i..].find("*/").map(|p| i + p + 2).unwrap_or(len);
            push(spans, start, i, TokenKind::Comment);
            continue;
        }
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            push(spans, start, i, TokenKind::String);
            continue;
        }
        if bytes[i] == b'{' {
            in_selector = false;
            push(spans, i, i + 1, TokenKind::Punctuation);
            i += 1;
            continue;
        }
        if bytes[i] == b'}' {
            in_selector = true;
            push(spans, i, i + 1, TokenKind::Punctuation);
            i += 1;
            continue;
        }
        if bytes[i] == b'#' && in_selector {
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
            {
                i += 1;
            }
            push(spans, start, i, TokenKind::Constant);
            continue;
        }
        if bytes[i] == b'@' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            push(spans, start, i, TokenKind::Preprocessor);
            continue;
        }
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'-' || bytes[i] == b'_' {
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'-'
                    || bytes[i] == b'_'
                    || (in_selector && (bytes[i] == b'.' || bytes[i] == b':')))
            {
                i += 1;
            }
            let word = &text[start..i];
            let kind = if in_selector {
                Some(TokenKind::Tag)
            } else if next_non_space_is(bytes, i, b':') {
                Some(TokenKind::Attribute)
            } else if spec.keywords.iter().any(|k| k.eq_ignore_ascii_case(word)) {
                Some(TokenKind::Keyword)
            } else {
                None
            };
            if let Some(kind) = kind {
                push(spans, start, i, kind);
            }
            continue;
        }
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'%')
            {
                i += 1;
            }
            push(spans, start, i, TokenKind::Number);
            continue;
        }
        i += 1;
    }
}

macro_rules! words {
    ($($name:ident => [$($word:expr),* $(,)?]),* $(,)?) => {
        $(const $name: &[&str] = &[$($word),*];)*
    };
}

words! {
    C_KEYWORDS => ["auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while", "_Bool", "_Complex", "_Imaginary"],
    C_TYPES => ["char", "double", "float", "int", "long", "short", "signed", "unsigned", "void", "size_t", "ssize_t", "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "bool", "FILE"],
    CPP_KEYWORDS => ["alignas", "alignof", "and", "and_eq", "asm", "auto", "bitand", "bitor", "break", "case", "catch", "class", "compl", "concept", "const", "consteval", "constexpr", "constinit", "const_cast", "continue", "co_await", "co_return", "co_yield", "decltype", "default", "delete", "do", "dynamic_cast", "else", "enum", "explicit", "export", "extern", "for", "friend", "goto", "if", "inline", "mutable", "namespace", "new", "noexcept", "not", "not_eq", "operator", "or", "or_eq", "private", "protected", "public", "reinterpret_cast", "requires", "return", "sizeof", "static", "static_assert", "static_cast", "struct", "switch", "template", "this", "thread_local", "throw", "try", "typedef", "typeid", "typename", "union", "using", "virtual", "volatile", "while", "xor", "xor_eq"],
    CSHARP_KEYWORDS => ["abstract", "as", "async", "await", "base", "break", "case", "catch", "checked", "class", "const", "continue", "default", "delegate", "do", "else", "enum", "event", "explicit", "extern", "finally", "fixed", "for", "foreach", "goto", "if", "implicit", "in", "interface", "internal", "is", "lock", "namespace", "new", "operator", "out", "override", "params", "private", "protected", "public", "readonly", "record", "ref", "return", "sealed", "sizeof", "stackalloc", "static", "struct", "switch", "this", "throw", "try", "typeof", "unchecked", "unsafe", "using", "var", "virtual", "volatile", "while", "yield"],
    CSHARP_TYPES => ["bool", "byte", "char", "decimal", "double", "float", "int", "long", "object", "sbyte", "short", "string", "uint", "ulong", "ushort", "void"],
    JAVA_KEYWORDS => ["abstract", "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "final", "finally", "for", "goto", "if", "implements", "import", "instanceof", "interface", "native", "new", "package", "private", "protected", "public", "record", "return", "sealed", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try", "var", "volatile", "while", "yield"],
    JAVA_TYPES => ["boolean", "byte", "char", "double", "float", "int", "long", "short", "void", "String", "Object", "Integer", "Long", "Double", "Float", "Boolean", "List", "Map", "Set"],
    JS_KEYWORDS => ["async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else", "export", "extends", "finally", "for", "function", "if", "import", "in", "instanceof", "let", "new", "of", "return", "static", "super", "switch", "this", "throw", "try", "typeof", "var", "void", "while", "with", "yield"],
    TS_KEYWORDS => ["abstract", "as", "asserts", "async", "await", "break", "case", "catch", "class", "const", "continue", "declare", "default", "delete", "do", "else", "enum", "export", "extends", "finally", "for", "from", "function", "get", "if", "implements", "import", "in", "infer", "instanceof", "interface", "is", "keyof", "let", "namespace", "new", "of", "private", "protected", "public", "readonly", "return", "set", "static", "super", "switch", "this", "throw", "try", "type", "typeof", "var", "void", "while", "yield"],
    TS_TYPES => ["any", "boolean", "never", "number", "object", "string", "symbol", "unknown", "void", "bigint"],
    PYTHON_KEYWORDS => ["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match", "case", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield"],
    PYTHON_TYPES => ["int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes", "object", "type"],
    RUST_KEYWORDS => ["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while", "yield"],
    RUST_TYPES => ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "HashMap", "HashSet", "BTreeMap", "Some", "None", "Ok", "Err"],
    GO_KEYWORDS => ["break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type", "var"],
    GO_TYPES => ["bool", "byte", "complex64", "complex128", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "any"],
    RUBY_KEYWORDS => ["alias", "and", "begin", "break", "case", "class", "def", "defined", "do", "else", "elsif", "end", "ensure", "for", "if", "in", "module", "next", "not", "or", "redo", "rescue", "retry", "return", "then", "undef", "unless", "until", "when", "while", "yield"],
    LUA_KEYWORDS => ["and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local", "not", "or", "repeat", "return", "then", "until", "while"],
    PERL_KEYWORDS => ["my", "our", "sub", "if", "elsif", "else", "unless", "while", "for", "foreach", "last", "next", "redo", "return", "use", "require", "package", "print", "say", "die", "eval", "local", "state"],
    PHP_KEYWORDS => ["abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone", "const", "continue", "declare", "default", "do", "echo", "else", "elseif", "empty", "eval", "exit", "extends", "final", "finally", "fn", "for", "foreach", "function", "global", "goto", "if", "implements", "include", "include_once", "instanceof", "insteadof", "interface", "isset", "list", "match", "namespace", "new", "or", "print", "private", "protected", "public", "readonly", "require", "require_once", "return", "static", "switch", "throw", "trait", "try", "unset", "use", "var", "while", "xor", "yield"],
    SWIFT_KEYWORDS => ["associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func", "import", "init", "inout", "internal", "let", "open", "operator", "private", "protocol", "public", "rethrows", "static", "struct", "subscript", "typealias", "var", "break", "case", "continue", "default", "defer", "do", "else", "fallthrough", "for", "guard", "if", "in", "repeat", "return", "switch", "where", "while", "as", "catch", "false", "is", "nil", "super", "self", "Self", "throw", "throws", "true", "try", "async", "await"],
    SWIFT_TYPES => ["Int", "Double", "Float", "Bool", "String", "Character", "Array", "Dictionary", "Set", "Optional", "Any", "AnyObject", "Void"],
    KOTLIN_KEYWORDS => ["as", "break", "class", "continue", "do", "else", "false", "for", "fun", "if", "in", "interface", "is", "null", "object", "package", "return", "super", "this", "throw", "true", "try", "typealias", "typeof", "val", "var", "when", "while", "by", "catch", "constructor", "delegate", "dynamic", "field", "file", "finally", "get", "import", "init", "param", "property", "receiver", "set", "setparam", "where", "actual", "abstract", "annotation", "companion", "const", "crossinline", "data", "enum", "expect", "external", "final", "infix", "inline", "inner", "internal", "lateinit", "noinline", "open", "operator", "out", "override", "private", "protected", "public", "reified", "sealed", "suspend", "tailrec", "vararg"],
    KOTLIN_TYPES => ["Int", "Long", "Short", "Byte", "Double", "Float", "Boolean", "Char", "String", "Unit", "Any", "Nothing", "List", "Map", "Set"],
    SCALA_KEYWORDS => ["abstract", "case", "catch", "class", "def", "do", "else", "extends", "false", "final", "finally", "for", "forSome", "if", "implicit", "import", "lazy", "match", "new", "null", "object", "override", "package", "private", "protected", "return", "sealed", "super", "this", "throw", "trait", "try", "true", "type", "val", "var", "while", "with", "yield"],
    SQL_KEYWORDS => ["select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create", "table", "alter", "drop", "index", "view", "join", "inner", "left", "right", "full", "outer", "on", "group", "by", "order", "having", "union", "all", "and", "or", "not", "null", "is", "like", "between", "exists", "case", "when", "then", "else", "end", "distinct", "limit", "offset", "as", "primary", "key", "foreign", "references", "default", "unique", "with", "returning"],
    SQL_TYPES => ["int", "integer", "bigint", "smallint", "decimal", "numeric", "float", "real", "double", "char", "varchar", "text", "date", "time", "timestamp", "boolean", "blob", "uuid", "json", "jsonb", "serial", "bigserial"],
    CSS_KEYWORDS => ["important", "inherit", "initial", "unset", "none", "auto", "block", "inline", "flex", "grid", "absolute", "relative", "fixed", "sticky", "bold", "italic", "center"],
    SHELL_KEYWORDS => ["if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "function", "in", "select", "time", "echo", "cd", "exit", "export", "local", "read", "set", "unset", "shift", "source", "alias", "return"],
    BATCH_KEYWORDS => ["echo", "set", "if", "exist", "not", "errorlevel", "goto", "call", "for", "in", "do", "del", "copy", "move", "ren", "mkdir", "rmdir", "cd", "pause", "exit", "rem", "start", "setlocal", "endlocal"],
    POWERSHELL_KEYWORDS => ["if", "elseif", "else", "switch", "foreach", "for", "while", "do", "until", "function", "filter", "param", "begin", "process", "end", "return", "throw", "try", "catch", "finally", "trap", "break", "continue", "class", "enum", "using", "namespace"],
    MAKEFILE_KEYWORDS => ["ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "include", "define", "endef", "export", "unexport", "override", "vpath"],
    CMAKE_KEYWORDS => ["add_executable", "add_library", "add_subdirectory", "cmake_minimum_required", "find_package", "include_directories", "install", "message", "project", "set", "target_include_directories", "target_link_libraries", "target_compile_definitions", "target_compile_options", "option", "if", "else", "elseif", "endif", "foreach", "endforeach", "function", "endfunction", "macro", "endmacro", "list", "string", "file"],
    DOCKER_KEYWORDS => ["from", "run", "cmd", "label", "maintainer", "expose", "env", "add", "copy", "entrypoint", "volume", "user", "workdir", "arg", "onbuild", "stopsignal", "healthcheck", "shell", "as"],
    TEX_KEYWORDS => ["begin", "end", "documentclass", "usepackage", "section", "subsection", "chapter", "paragraph", "textbf", "textit", "emph", "label", "ref", "cite", "item", "itemize", "enumerate", "frac", "sum", "int", "alpha", "beta"],
    R_KEYWORDS => ["if", "else", "repeat", "while", "function", "for", "in", "next", "break", "library", "require", "return", "switch"],
    MATLAB_KEYWORDS => ["function", "end", "if", "else", "elseif", "for", "while", "switch", "case", "otherwise", "try", "catch", "return", "break", "continue", "classdef", "properties", "methods", "events", "global", "persistent", "parfor", "spmd"],
    HASKELL_KEYWORDS => ["case", "class", "data", "default", "deriving", "do", "else", "if", "import", "in", "infix", "infixl", "infixr", "instance", "let", "module", "newtype", "of", "then", "type", "where", "forall"],
    ERLANG_KEYWORDS => ["after", "begin", "case", "catch", "cond", "end", "fun", "if", "let", "of", "query", "receive", "try", "when", "andalso", "orelse", "not", "and", "or", "xor"],
    ELIXIR_KEYWORDS => ["do", "end", "fn", "def", "defmodule", "defp", "defmacro", "defstruct", "if", "unless", "case", "cond", "for", "with", "try", "rescue", "catch", "after", "raise", "import", "alias", "use", "require", "when", "and", "or", "not", "in"],
    LISP_KEYWORDS => ["def", "defn", "defmacro", "defmethod", "defmulti", "fn", "let", "letfn", "if", "if-let", "if-not", "do", "loop", "recur", "when", "when-not", "when-let", "cond", "case", "and", "or", "not", "ns", "define", "lambda", "let*", "letrec", "begin", "set!", "display", "newline", "car", "cdr", "cons", "list", "quote"],
    FORTRAN_KEYWORDS => ["program", "module", "contains", "subroutine", "function", "end", "if", "then", "else", "do", "while", "integer", "real", "complex", "character", "logical", "implicit", "none", "call", "return", "use", "allocate", "deallocate"],
    PASCAL_KEYWORDS => ["program", "begin", "end", "var", "const", "type", "procedure", "function", "if", "then", "else", "while", "do", "for", "to", "downto", "repeat", "until", "case", "of", "record", "array", "uses", "unit", "interface", "implementation"],
    ADA_KEYWORDS => ["procedure", "function", "is", "begin", "end", "if", "then", "else", "elsif", "loop", "while", "for", "in", "out", "declare", "package", "with", "use", "type", "record", "return", "exception", "raise", "case", "when"],
    ASM_KEYWORDS => ["mov", "push", "pop", "call", "ret", "jmp", "je", "jne", "jz", "jnz", "cmp", "add", "sub", "mul", "div", "inc", "dec", "and", "or", "xor", "not", "shl", "shr", "lea", "nop", "int", "syscall", "section", "global", "db", "dw", "dd", "dq"],
    VERILOG_KEYWORDS => ["module", "endmodule", "input", "output", "inout", "wire", "reg", "assign", "always", "initial", "begin", "end", "if", "else", "case", "endcase", "for", "while", "repeat", "function", "endfunction", "task", "endtask", "posedge", "negedge", "parameter", "localparam"],
    VHDL_KEYWORDS => ["entity", "architecture", "begin", "end", "process", "if", "then", "else", "elsif", "case", "when", "for", "loop", "while", "signal", "variable", "constant", "type", "in", "out", "library", "use", "port", "map", "generic"],
    GRAPHQL_KEYWORDS => ["query", "mutation", "subscription", "fragment", "on", "type", "interface", "enum", "input", "scalar", "schema", "extend", "directive", "implements"],
    PROTOBUF_KEYWORDS => ["syntax", "package", "import", "option", "message", "enum", "service", "rpc", "returns", "repeated", "optional", "required", "reserved", "oneof", "map", "stream"],
    PROTOBUF_TYPES => ["double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes"],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str, lang: Language) -> Vec<(String, TokenKind)> {
        let colors = crate::theme::Theme::new(
            true,
            eframe::egui::Color32::from_rgb(103, 80, 164),
            1.0,
            14.0,
        )
        .syntax();
        highlight(text, lang, &colors)
            .into_iter()
            .map(|s| (text[s.range].to_owned(), s.kind))
            .collect()
    }

    #[test]
    fn detects_by_extension_and_name() {
        assert_eq!(Language::detect(Some("rs"), "", None), Language::Rust);
        assert_eq!(
            Language::detect(None, "", Some("Makefile")),
            Language::Makefile
        );
        assert_eq!(
            Language::detect(None, "#!/usr/bin/env python3\n", None),
            Language::Python
        );
        assert_eq!(Language::detect(Some("xyz"), "", None), Language::PlainText);
    }

    #[test]
    fn rust_keywords_and_functions() {
        let tokens = kinds("fn main() { let x = 1; }", Language::Rust);
        assert!(tokens.contains(&("fn".to_owned(), TokenKind::Keyword)));
        assert!(tokens.contains(&("main".to_owned(), TokenKind::Function)));
        assert!(tokens.contains(&("1".to_owned(), TokenKind::Number)));
    }

    #[test]
    fn strings_and_comments() {
        let tokens = kinds("let s = \"hi // not comment\"; // real", Language::Rust);
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "\"hi // not comment\"" && *k == TokenKind::String));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "// real" && *k == TokenKind::Comment));
    }

    #[test]
    fn spans_are_sorted_and_inside_text() {
        let text = "fn main() { println!(\"h\\u{e9}llo\"); } // \\u{43c}\\u{438}\\u{440}\\n";
        let colors = crate::theme::Theme::new(
            false,
            eframe::egui::Color32::from_rgb(0, 99, 155),
            1.0,
            14.0,
        )
        .syntax();
        let spans = highlight(text, Language::Rust, &colors);
        let mut last = 0;
        for span in &spans {
            assert!(span.range.start >= last);
            assert!(span.range.end <= text.len());
            assert!(text.is_char_boundary(span.range.start));
            assert!(text.is_char_boundary(span.range.end));
            last = span.range.end;
        }
    }

    #[test]
    fn markdown_headings_and_code() {
        let tokens = kinds("# Title\n\n```rs\nfn x() {}\n```\n", Language::Markdown);
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "# Title" && *k == TokenKind::Heading));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "fn x() {}" && *k == TokenKind::String));
    }

    #[test]
    fn json_keys_are_attributes() {
        let tokens = kinds("{\"name\": \"value\", \"n\": 12}", Language::Json);
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "\"name\"" && *k == TokenKind::Attribute));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "\"value\"" && *k == TokenKind::String));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "12" && *k == TokenKind::Number));
    }

    #[test]
    fn html_tags_attributes() {
        let tokens = kinds("<a href=\"x\">link</a>", Language::Html);
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "<a" && *k == TokenKind::Tag));
        assert!(tokens
            .iter()
            .any(|(t, k)| t == "href" && *k == TokenKind::Attribute));
    }

    #[test]
    fn preview_fixture_is_highlighted() {
        let text = "use std::io;\n\nfn main() {\n    println!(\"Hi\");\n}\n";
        let tokens = kinds(text, Language::Rust);
        assert!(tokens.len() > 5);
    }
}
