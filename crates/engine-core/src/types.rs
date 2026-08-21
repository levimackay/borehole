use serde::{Deserialize, Serialize};
use std::fmt;

/// A path relative to the repository root, always stored with forward
/// slashes regardless of host OS. Every engine crate that touches a path
/// crossing a crate boundary (SQLite keys, JSON output, IPC payloads) uses
/// this type instead of `PathBuf` so Windows/macOS/Linux never disagree on
/// what a path "is" once it leaves the filesystem layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPath(String);

impl RepoPath {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().replace('\\', "/"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for RepoPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for RepoPath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// How much to trust a claim the engine is making. Every relationship the
/// evidence layer surfaces (a caller, a co-change edge, a symbol-level
/// history entry) carries one of these, because the engine resolves most
/// relationships by name+import-scope heuristics rather than full semantic
/// type-checking. This field is what lets the UI say "these areas may be
/// affected" instead of "this will break" — see docs/ARCHITECTURE.md
/// "Confidence is not optional" section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Resolved via import-scoped, type-aware-enough matching with no
    /// competing candidates. E.g. a same-file call, or a call resolved
    /// through a single unambiguous import.
    High,
    /// Resolved by name within a plausible scope but with some ambiguity
    /// (e.g. multiple imported symbols share the name, or resolution
    /// crossed a re-export).
    Medium,
    /// Name-only match with no scope information (e.g. a dynamic call, a
    /// language feature the parser doesn't model, or a symbol-level git
    /// history entry derived from line-range heuristics rather than exact
    /// tracking).
    Low,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Confidence::High => write!(f, "high"),
            Confidence::Medium => write!(f, "medium"),
            Confidence::Low => write!(f, "low"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Struct,
    Enum,
    Trait,
    Module,
    Variable,
    Constant,
    TypeAlias,
    Macro,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Module => "module",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::TypeAlias => "type_alias",
            SymbolKind::Macro => "macro",
        };
        write!(f, "{s}")
    }
}

/// A location within a single file, using both line/column (for display)
/// and byte offsets (for exact re-slicing of source text and for
/// intersecting with git diff hunks in `engine-git`). Lines and columns are
/// 1-indexed to match editor and `git blame` conventions; byte offsets are
/// 0-indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}

impl Span {
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Opaque, stable-for-the-life-of-one-index identifier for a symbol.
/// Assigned by `engine-index` when a parsed symbol is persisted (it is the
/// SQLite row id); nothing upstream of indexing knows this value. Across
/// re-indexes the same source symbol may get a different `SymbolId` unless
/// `engine-index`'s identity-matching (file + qualified name + kind) finds
/// its predecessor, in which case the id is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolId(pub i64);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    /// Dotted/namespaced path where the language supports it, e.g.
    /// `auth::session::SessionService::renew`. Falls back to `name` when
    /// the language has no meaningful qualification (e.g. top-level JS
    /// functions in a script file).
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub language: crate::Language,
    pub file: RepoPath,
    pub span: Span,
    pub is_exported: bool,
    pub doc_comment: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Call,
    Import,
    Extends,
    Implements,
    TypeUsage,
    Instantiation,
}

impl fmt::Display for ReferenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ReferenceKind::Call => "call",
            ReferenceKind::Import => "import",
            ReferenceKind::Extends => "extends",
            ReferenceKind::Implements => "implements",
            ReferenceKind::TypeUsage => "type_usage",
            ReferenceKind::Instantiation => "instantiation",
        };
        write!(f, "{s}")
    }
}

/// A resolved (or attempted) use-site. `to_symbol` is `None` when the
/// reference could not be resolved to a known symbol at all (still worth
/// recording — it's evidence that *something* named `to_name` is used
/// here, useful for confidence reporting and for external/unresolved-import
/// cases).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub from_symbol: Option<SymbolId>,
    pub from_file: RepoPath,
    pub from_span: Span,
    pub to_symbol: Option<SymbolId>,
    pub to_name: String,
    pub kind: ReferenceKind,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskTier {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for RiskTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskTier::Low => write!(f, "low"),
            RiskTier::Medium => write!(f, "medium"),
            RiskTier::High => write!(f, "high"),
            RiskTier::Critical => write!(f, "critical"),
        }
    }
}

/// One cited piece of support for a claim the evidence engine makes.
/// Every field in `engine-evidence`'s report types that isn't raw structural
/// data (counts, ids) should be backed by at least one `EvidenceRef` so the
/// UI can render a "[View evidence]" link instead of an assertion. This is
/// the mechanism behind brief requirement "every important conclusion
/// should have evidence."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceRef {
    Reference { file: RepoPath, span: Span },
    Commit { sha: String, summary: String },
    TestFile { file: RepoPath },
    ConfigFile { file: RepoPath, key: Option<String> },
}
