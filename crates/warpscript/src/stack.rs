// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The WarpScript stack and interpreter (port of `MemoryWarpScriptStack`).
//!
//! Scripts are executed **line by line**: each line is tokenised and every statement is applied
//! immediately, unless a macro is being defined (`<% … %>`), in which case statements are
//! compiled into the macro. The parsing state (block comments, multiline strings, open macros)
//! therefore lives on the stack and spans lines, exactly like in Warp 10.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;

use crate::datastore::Datastore;
use crate::error::stack::StackError;
use crate::error::{Error, Result};
use crate::format;
use crate::functions::{self, Entry};
use crate::macros::{Callable, Macro, StackFunction, Statement};
use crate::value::{Context, Value};

/// Default maximum recursion level (`DEFAULT_MAX_RECURSION_LEVEL`).
pub const DEFAULT_MAX_RECURSION_LEVEL: u64 = 16;
/// Default maximum number of operations (`DEFAULT_MAX_OPS`).
pub const DEFAULT_MAX_OPS: u64 = 1000;
/// Default maximum stack depth (`DEFAULT_MAX_DEPTH`).
pub const DEFAULT_MAX_DEPTH: usize = 1000;
/// Default maximum number of symbols (`DEFAULT_MAX_SYMBOLS`).
pub const DEFAULT_MAX_SYMBOLS: usize = 64;
/// Default maximum number of buckets (`DEFAULT_MAX_BUCKETS`).
pub const DEFAULT_MAX_BUCKETS: i64 = 1_000_000;
/// Default maximum number of geo cells (`DEFAULT_MAX_GEOCELLS`).
pub const DEFAULT_MAX_GEOCELLS: i64 = 10_000;
/// Default maximum loop duration in ms (`DEFAULT_MAX_LOOP_DURATION`).
pub const DEFAULT_MAX_LOOP_DURATION: i64 = 5000;
/// Default maximum number of pixels (`DEFAULT_MAX_PIXELS`).
pub const DEFAULT_MAX_PIXELS: i64 = 1_000_000;
/// Default maximum JSON size (`DEFAULT_MAX_JSON`, 20 MB).
pub const DEFAULT_MAX_JSON: i64 = 20 * 1024 * 1024;
/// Default fetch limit (`DEFAULT_FETCH_LIMIT`).
pub const DEFAULT_FETCH_LIMIT: i64 = 100_000;
/// Default GTS limit (`DEFAULT_GTS_LIMIT`).
pub const DEFAULT_GTS_LIMIT: i64 = 100_000;
/// Default number of registers (`DEFAULT_REGISTERS`).
pub const DEFAULT_REGISTERS: usize = 256;
/// Name of the top level section (`TOP_LEVEL_SECTION`).
pub const TOP_LEVEL_SECTION: &str = "[TOP]";
/// Message of the error raised by the interpreter when a `TIMEBOX` deadline has passed.
pub const DEADLINE_MESSAGE: &str = "Execution time limit reached.";

/// A decoded secure script: the key it was secured with and its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecureScript {
    /// The secure key given to `SECURE`.
    pub key: String,
    /// The WarpScript source.
    pub script: String,
}

/// Encrypts and decrypts secure scripts (`SECURE` / `UNSECURE` / `EVALSECURE`). The runtime has
/// no built-in codec: without one those functions fail with
/// `Missing secure script encryption key.`, exactly like a Warp 10 instance without an AES key.
pub trait SecureCodec {
    /// Encode `script` so that only [`SecureCodec::unsecure`] can read it back.
    fn secure(&self, key: &str, script: &str) -> Result<String>;
    /// Decode a string produced by [`SecureCodec::secure`].
    fn unsecure(&self, encoded: &str) -> Result<SecureScript>;
}

/// Resolves the tokens given to `CAPADD` into their attributes (`Tokens.extractReadToken` /
/// `extractWriteToken`). The runtime has no token store of its own: without a resolver every
/// token is invalid.
pub trait TokenResolver {
    /// The attributes of `token`, or `None` when it is not a valid read or write token.
    fn token_attributes(&self, token: &str) -> Option<IndexMap<String, String>>;
}

/// Prefix of the token attributes that carry capabilities (`CAPABILITIES_PREFIX`).
pub const CAPABILITIES_PREFIX: &str = ".cap:";

/// The Warp 10 revision this runtime reproduces (`Revision.REVISION`), reported by `REV` and
/// compared by `MINREV` / `MAXREV`.
pub const REVISION: &str = "3.5.0";

/// Where the interactive functions (`PEEK`, `PSTACK`…) print (`ATTRIBUTE_INTERACTIVE_WRITER`).
pub type InteractiveWriter = Rc<RefCell<dyn std::io::Write>>;

pub(crate) const MACRO_START: &str = "<%";
const MACRO_END: &str = "%>";
pub(crate) const COMMENT_START: &str = "/*";
const COMMENT_END: &str = "*/";
pub(crate) const MULTILINE_START: &str = "<'";
const MULTILINE_END: &str = "'>";
pub(crate) const SECURE_SCRIPT_START: &str = "<S";
const SECURE_SCRIPT_END: &str = "S>";

/// Execution limits of a stack (`ATTRIBUTE_MAX_*`). Soft limits can be raised up to the hard
/// limits; hard limits are set by the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Maximum stack depth.
    pub max_depth: usize,
    /// Maximum number of operations.
    pub max_ops: u64,
    /// Maximum recursion level.
    pub max_recursion: u64,
    /// Maximum number of symbols.
    pub max_symbols: usize,
    /// Maximum number of buckets.
    pub max_buckets: i64,
    /// Maximum number of geo cells.
    pub max_geocells: i64,
    /// Maximum number of pixels.
    pub max_pixels: i64,
    /// Maximum JSON size.
    pub max_json: i64,
    /// Maximum loop duration (ms).
    pub max_loop_duration: i64,
    /// Maximum number of datapoints fetched.
    pub fetch_limit: i64,
    /// Maximum number of GTS fetched.
    pub gts_limit: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_depth: DEFAULT_MAX_DEPTH,
            max_ops: DEFAULT_MAX_OPS,
            max_recursion: DEFAULT_MAX_RECURSION_LEVEL,
            max_symbols: DEFAULT_MAX_SYMBOLS,
            max_buckets: DEFAULT_MAX_BUCKETS,
            max_geocells: DEFAULT_MAX_GEOCELLS,
            max_pixels: DEFAULT_MAX_PIXELS,
            max_json: DEFAULT_MAX_JSON,
            max_loop_duration: DEFAULT_MAX_LOOP_DURATION,
            fetch_limit: DEFAULT_FETCH_LIMIT,
            gts_limit: DEFAULT_GTS_LIMIT,
        }
    }
}

impl Limits {
    /// Limits that never trigger (`MemoryWarpScriptStack.maxLimits`).
    pub fn unlimited() -> Self {
        Limits {
            max_depth: usize::MAX / 2,
            max_ops: u64::MAX - 1,
            max_recursion: u64::MAX - 1,
            max_symbols: usize::MAX / 2,
            max_buckets: i64::MAX - 1,
            max_geocells: i64::from(i32::MAX - 1),
            max_pixels: i64::MAX - 1,
            max_json: i64::MAX,
            max_loop_duration: i64::MAX,
            fetch_limit: i64::MAX - 1,
            gts_limit: i64::MAX - 1,
        }
    }
}

/// The WarpScript execution stack.
pub struct Stack {
    elements: Vec<Value>,
    /// Number of hidden elements at the bottom of `elements` (`HIDE` / `SHOW`).
    offset: usize,
    symbols: IndexMap<String, Value>,
    registers: Vec<Value>,
    defined: IndexMap<String, Option<Rc<Macro>>>,
    attributes: IndexMap<String, Value>,
    /// Current (soft) limits.
    pub limits: Limits,
    /// Hard limits: soft limits cannot be raised above these.
    pub hard_limits: Limits,
    current_ops: u64,
    rec_level: u64,
    section_name: String,
    macro_name: Option<String>,
    in_secure_macro: bool,
    allow_redefined: bool,
    /// Macros being defined, innermost last.
    macros: Vec<Vec<Statement>>,
    forced_macro: usize,
    in_comment: bool,
    in_multiline: bool,
    multiline: String,
    secure_script: Option<String>,
    return_depth: i64,
    last_error: Option<Error>,
    line_number: Option<usize>,
    deadline: Option<Instant>,
    secure_codec: Option<Rc<dyn SecureCodec>>,
    /// The datastore the datastore functions (`FETCH`, `FIND`…) work on, when one is connected.
    datastore: Option<Arc<dyn Datastore>>,
    config: IndexMap<String, String>,
    /// Capabilities granted to the script (`Capabilities`), set by the host from the token.
    capabilities: IndexMap<String, String>,
    /// Capabilities a `GUARD`ed macro exports to its caller (`EXPORTED_CAPABILITIES_ATTR`);
    /// `None` outside `GUARD`. `None` entries stand for Java's `null` name.
    exported_capabilities: Option<Vec<Option<String>>>,
    token_resolver: Option<Rc<dyn TokenResolver>>,
    interactive_writer: Option<InteractiveWriter>,
    /// `IMPORT` rules, longest alias first (`ATTRIBUTE_IMPORT_RULES`).
    import_rules: Vec<(String, String)>,
}

impl Default for Stack {
    fn default() -> Self {
        Stack::new()
    }
}

impl Stack {
    /// A stack with the default Warp 10 limits.
    pub fn new() -> Self {
        Stack::with_limits(Limits::default())
    }

    /// A stack whose soft and hard limits are both `limits`.
    pub fn with_limits(limits: Limits) -> Self {
        Stack {
            elements: Vec::with_capacity(32),
            offset: 0,
            symbols: IndexMap::new(),
            registers: vec![Value::Null; DEFAULT_REGISTERS],
            defined: IndexMap::new(),
            attributes: IndexMap::new(),
            hard_limits: limits.clone(),
            limits,
            current_ops: 0,
            rec_level: 0,
            section_name: TOP_LEVEL_SECTION.to_owned(),
            macro_name: None,
            in_secure_macro: false,
            allow_redefined: true,
            macros: Vec::new(),
            forced_macro: 0,
            in_comment: false,
            in_multiline: false,
            multiline: String::new(),
            secure_script: None,
            return_depth: 0,
            last_error: None,
            line_number: None,
            deadline: None,
            secure_codec: None,
            datastore: None,
            capabilities: IndexMap::new(),
            exported_capabilities: None,
            token_resolver: None,
            interactive_writer: None,
            import_rules: Vec::new(),
            config: IndexMap::new(),
        }
    }

    // ------------------------------------------------------------------------------------
    // Element access
    // ------------------------------------------------------------------------------------

    /// Number of visible elements.
    pub fn depth(&self) -> usize {
        self.elements.len() - self.offset
    }

    /// Number of elements including the hidden ones (what the depth limit applies to).
    pub fn total_depth(&self) -> usize {
        self.elements.len()
    }

    /// Push a value. The depth limit is enforced by [`Stack::check_depth`], which the
    /// interpreter calls after every statement.
    pub fn push(&mut self, value: impl Into<Value>) {
        self.elements.push(value.into());
    }

    /// Fail when the stack holds more elements than the depth limit allows.
    pub fn check_depth(&self) -> Result<()> {
        if self.elements.len() > self.limits.max_depth {
            return Err(StackError::DepthLimit {
                max: self.limits.max_depth,
            }
            .into());
        }
        Ok(())
    }

    /// Pop the top of the stack.
    pub fn pop(&mut self) -> Result<Value> {
        if self.depth() == 0 {
            return Err(Error::empty_stack());
        }
        self.elements.pop().ok_or_else(Error::empty_stack)
    }

    /// Remove and return the `n` top values (fewer when the stack is shallower), bottom-most
    /// first. It cannot fail: it is for the callers that have just measured the depth.
    pub fn take_top(&mut self, n: usize) -> Vec<Value> {
        let at = self.elements.len().saturating_sub(n);
        self.elements.split_off(at)
    }

    /// Remove the `n` top values (fewer when the stack is shallower), like [`Stack::take_top`].
    pub fn discard_top(&mut self, n: usize) {
        let at = self.elements.len().saturating_sub(n);
        self.elements.truncate(at);
    }

    /// Pop `n` values, bottom-most first (`popn`).
    pub fn popn(&mut self, n: usize) -> Result<Vec<Value>> {
        if self.depth() < n {
            return Err(StackError::IndexOutOfBound.into());
        }
        let at = self.elements.len() - n;
        Ok(self.elements.split_off(at))
    }

    /// Drop the top `n` values (`dropn`).
    pub fn dropn(&mut self, n: usize) -> Result<()> {
        if self.depth() < n {
            return Err(StackError::IndexOutOfBound.into());
        }
        let at = self.elements.len() - n;
        self.elements.truncate(at);
        Ok(())
    }

    /// Keep at most `depth` visible elements (`RESET`).
    pub fn reset(&mut self, depth: usize) {
        if self.depth() > depth {
            self.elements.truncate(self.offset + depth);
        }
    }

    /// A reference to the top of the stack.
    pub fn peek(&self) -> Result<&Value> {
        if self.depth() == 0 {
            return Err(Error::empty_stack());
        }
        Ok(&self.elements[self.elements.len() - 1])
    }

    /// A reference to the element `level` positions below the top (0 is the top).
    pub fn get(&self, level: usize) -> Result<&Value> {
        if level >= self.depth() {
            return Err(StackError::InvalidLevel.into());
        }
        Ok(&self.elements[self.elements.len() - 1 - level])
    }

    /// Mutable access to the visible elements, bottom first.
    pub fn elements_mut(&mut self) -> &mut [Value] {
        let offset = self.offset;
        &mut self.elements[offset..]
    }

    /// The visible elements, bottom first.
    pub fn elements(&self) -> &[Value] {
        &self.elements[self.offset..]
    }

    /// Remove every visible element.
    pub fn clear(&mut self) {
        self.elements.truncate(self.offset);
    }

    /// Hide all visible elements, returning how many were hidden (`HIDE`).
    pub fn hide_all(&mut self) -> usize {
        let count = self.depth();
        self.offset += count;
        count
    }

    /// Hide `count` elements (negative: keep `-count` visible), returning how many were hidden.
    pub fn hide(&mut self, count: i64) -> usize {
        let depth = self.depth() as i64;
        let count = if count == 0 {
            0
        } else if count > depth {
            depth
        } else if count < 0 {
            (count + depth).max(0)
        } else {
            count
        };
        self.offset += count as usize;
        count as usize
    }

    /// Show every hidden element (`SHOW`).
    pub fn show_all(&mut self) {
        self.offset = 0;
    }

    /// Show `count` hidden elements (negative: make `-count` elements visible in total).
    pub fn show(&mut self, count: i64) {
        let offset = self.offset as i64;
        let depth = self.depth() as i64;
        let count = if count == 0 {
            0
        } else if count > offset {
            offset
        } else if count < 0 {
            let c = count + depth;
            if c > 0 {
                0
            } else if -c > offset {
                offset
            } else {
                -c
            }
        } else {
            count
        };
        self.offset -= count as usize;
    }

    // ------------------------------------------------------------------------------------
    // Symbols, registers, definitions, attributes
    // ------------------------------------------------------------------------------------

    /// Value bound to `symbol`, if any.
    pub fn load(&self, symbol: &str) -> Option<&Value> {
        self.symbols.get(symbol)
    }

    /// `true` when `symbol` is bound (even to `null`).
    pub fn has_symbol(&self, symbol: &str) -> bool {
        self.symbols.contains_key(symbol)
    }

    /// Bind `symbol` to `value` (fails when the symbol table is full).
    pub fn store(&mut self, symbol: &str, value: Value) -> Result<()> {
        if !self.symbols.contains_key(symbol) && self.symbols.len() >= self.limits.max_symbols {
            return Err(StackError::SymbolLimit {
                max: self.limits.max_symbols,
            }
            .into());
        }
        self.symbols.insert(symbol.to_owned(), value);
        Ok(())
    }

    /// Remove the binding of `symbol`.
    pub fn forget(&mut self, symbol: &str) {
        self.symbols.shift_remove(symbol);
    }

    /// Remove every binding.
    pub fn forget_all(&mut self) {
        self.symbols.clear();
    }

    /// The symbol table.
    pub fn symbols(&self) -> &IndexMap<String, Value> {
        &self.symbols
    }

    /// Mutable access to the symbol table.
    pub fn symbols_mut(&mut self) -> &mut IndexMap<String, Value> {
        &mut self.symbols
    }

    /// Value of register `index`.
    pub fn load_register(&self, index: i64) -> Result<&Value> {
        let slot = usize::try_from(index).map_err(|_| self.invalid_register())?;
        self.registers
            .get(slot)
            .ok_or_else(|| self.invalid_register())
    }

    /// Set register `index`.
    pub fn store_register(&mut self, index: i64, value: Value) -> Result<()> {
        *self.register_slot(index)? = value;
        Ok(())
    }

    /// Register `index`, for the callers that read and write it.
    pub fn register_slot(&mut self, index: i64) -> Result<&mut Value> {
        let max = DEFAULT_REGISTERS - 1;
        let slot = usize::try_from(index).map_err(|_| StackError::InvalidRegister { max })?;
        self.registers
            .get_mut(slot)
            .ok_or(StackError::InvalidRegister { max }.into())
    }

    /// Set register `index` to `null`. An index that is not a register is ignored: the symbol
    /// list that asks for the clearing may name a register that does not exist, and clearing it
    /// is then a no-op rather than a failure.
    pub fn clear_register(&mut self, index: i64) {
        let Ok(slot) = usize::try_from(index) else {
            return;
        };
        if let Some(slot) = self.registers.get_mut(slot) {
            *slot = Value::Null;
        }
    }

    fn invalid_register(&self) -> Error {
        StackError::InvalidRegister {
            max: self.registers.len() - 1,
        }
        .into()
    }

    /// The registers.
    pub fn registers(&self) -> &[Value] {
        &self.registers
    }

    /// Set every register to `null` (`CLEARREGS`).
    pub fn clear_registers(&mut self) {
        self.registers.fill(Value::Null);
    }

    /// (Re)define `name` as `body` (`DEF`), or undefine it when `body` is `None` (`UNDEF`):
    /// an undefined name shadows the WarpLib function and fails when called.
    pub fn define(&mut self, name: &str, body: Option<Rc<Macro>>) {
        self.defined.insert(name.to_owned(), body);
    }

    /// The functions redefined with `DEF` (`None` marks a name undefined with `UNDEF`).
    pub fn defined(&self) -> &IndexMap<String, Option<Rc<Macro>>> {
        &self.defined
    }

    /// Whether redefined functions may be called (`ATTRIBUTE_ALLOW_REDEFINED`); returns the
    /// previous setting.
    pub fn set_allow_redefined(&mut self, allow: bool) -> bool {
        std::mem::replace(&mut self.allow_redefined, allow)
    }

    /// `true` when redefined functions may be called.
    pub fn allow_redefined(&self) -> bool {
        self.allow_redefined
    }

    /// The configuration property `key`, if set (`WarpConfig.getProperty`).
    pub fn config_property(&self, key: &str) -> Option<&str> {
        self.config.get(key).map(String::as_str)
    }

    /// Mutable access to the configuration properties (`SETMACROCONFIG`).
    pub fn config_mut(&mut self) -> &mut IndexMap<String, String> {
        &mut self.config
    }

    /// Replace the configuration properties visible to `MACROCONFIG`.
    pub fn set_config(&mut self, config: IndexMap<String, String>) {
        self.config = config;
    }

    /// The value of the capability `name`, if granted (`Capabilities.get(stack, name)`).
    pub fn capability(&self, name: &str) -> Option<&str> {
        self.capabilities.get(name).map(String::as_str)
    }

    /// All the capabilities granted to the script, in insertion order.
    pub fn capabilities(&self) -> &IndexMap<String, String> {
        &self.capabilities
    }

    /// Mutable access to the capabilities, for the host (token attributes) and for `CAPDEL`.
    pub fn capabilities_mut(&mut self) -> &mut IndexMap<String, String> {
        &mut self.capabilities
    }

    /// The capabilities exported by the running `GUARD`ed macro, `None` outside `GUARD`.
    pub fn exported_capabilities_mut(&mut self) -> Option<&mut Vec<Option<String>>> {
        self.exported_capabilities.as_mut()
    }

    /// Install (or remove, with `None`) the exported capabilities set; returns the previous
    /// one.
    pub fn set_exported_capabilities(
        &mut self,
        exported: Option<Vec<Option<String>>>,
    ) -> Option<Vec<Option<String>>> {
        std::mem::replace(&mut self.exported_capabilities, exported)
    }

    /// The token resolver, if one was installed.
    pub fn token_resolver(&self) -> Option<Rc<dyn TokenResolver>> {
        self.token_resolver.clone()
    }

    /// Install (or remove) the token resolver used by `CAPADD`.
    pub fn set_token_resolver(&mut self, resolver: Option<Rc<dyn TokenResolver>>) {
        self.token_resolver = resolver;
    }

    /// The interactive writer, if one was installed.
    pub fn interactive_writer(&self) -> Option<InteractiveWriter> {
        self.interactive_writer.clone()
    }

    /// Install (or remove) the writer used by `PEEK`, `PEEKN`, `NPEEK` and `PSTACK`.
    pub fn set_interactive_writer(&mut self, writer: Option<InteractiveWriter>) {
        self.interactive_writer = writer;
    }

    /// The `IMPORT` rules (alias → namespace), longest alias first.
    pub fn import_rules(&self) -> &[(String, String)] {
        &self.import_rules
    }

    /// Add an `IMPORT` rule, or remove the rule of `alias` when `imported` is `None`.
    ///
    /// Like Warp 10's `TreeMap` keyed by alias length, two aliases of the same length share
    /// one rule: the newer namespace replaces the older one under the original alias.
    pub fn import(&mut self, alias: &str, imported: Option<&str>) {
        let slot = self
            .import_rules
            .iter()
            .position(|(existing, _)| existing.len() == alias.len());
        match (slot, imported) {
            (Some(index), Some(namespace)) => {
                namespace.clone_into(&mut self.import_rules[index].1);
            }
            (Some(index), None) => {
                self.import_rules.remove(index);
            }
            (None, Some(namespace)) => {
                let at = self
                    .import_rules
                    .iter()
                    .position(|(existing, _)| existing.len() < alias.len())
                    .unwrap_or(self.import_rules.len());
                self.import_rules
                    .insert(at, (alias.to_owned(), namespace.to_owned()));
            }
            (None, None) => {}
        }
    }

    /// `rewriteMacroSymbol`: apply the first (longest) matching `IMPORT` rule to a macro name.
    fn rewrite_macro_symbol<'a>(&self, symbol: &'a str) -> std::borrow::Cow<'a, str> {
        for (prefix, substitute) in &self.import_rules {
            if let Some(rest) = symbol.strip_prefix(prefix.as_str()) {
                return std::borrow::Cow::Owned(format!("{substitute}{rest}"));
            }
        }
        std::borrow::Cow::Borrowed(symbol)
    }

    /// The secure script codec, if one was installed.
    pub fn secure_codec(&self) -> Option<Rc<dyn SecureCodec>> {
        self.secure_codec.clone()
    }

    /// Install (or remove) the secure script codec.
    pub fn set_secure_codec(&mut self, codec: Option<Rc<dyn SecureCodec>>) {
        self.secure_codec = codec;
    }

    /// The datastore the script works on, if one is connected.
    pub fn datastore(&self) -> Option<Arc<dyn Datastore>> {
        self.datastore.clone()
    }

    /// Connect (or disconnect) the datastore of the datastore functions.
    pub fn set_datastore(&mut self, datastore: Option<Arc<dyn Datastore>>) {
        self.datastore = datastore;
    }

    /// A stack attribute.
    pub fn attribute(&self, key: &str) -> Option<&Value> {
        self.attributes.get(key)
    }

    /// Set (or remove, when `value` is `None`) a stack attribute; returns the previous value.
    pub fn set_attribute(&mut self, key: &str, value: Option<Value>) -> Option<Value> {
        match value {
            Some(value) => self.attributes.insert(key.to_owned(), value),
            None => self.attributes.shift_remove(key),
        }
    }

    /// The current section name (`SECTION`).
    pub fn section_name(&self) -> &str {
        &self.section_name
    }

    /// Set the current section name.
    pub fn set_section_name(&mut self, name: &str) {
        name.clone_into(&mut self.section_name);
    }

    /// Name of the macro currently executing, if it has one.
    pub fn macro_name(&self) -> Option<&str> {
        self.macro_name.as_deref()
    }

    /// `true` while executing a secure macro.
    pub fn in_secure_macro(&self) -> bool {
        self.in_secure_macro
    }

    /// Force (or release) the secure macro mode; returns the previous mode.
    pub fn set_in_secure_macro(&mut self, secure: bool) -> bool {
        std::mem::replace(&mut self.in_secure_macro, secure)
    }

    /// The instant after which the interpreter aborts the execution (`TIMEBOX`).
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Set the execution deadline; returns the previous one.
    pub fn set_deadline(&mut self, deadline: Option<Instant>) -> Option<Instant> {
        std::mem::replace(&mut self.deadline, deadline)
    }

    /// Number of operations executed so far.
    pub fn ops(&self) -> u64 {
        self.current_ops
    }

    /// Reset the operation counter (`ATTRIBUTE_OPS`).
    pub fn set_ops(&mut self, ops: u64) {
        self.current_ops = ops;
    }

    /// Current recursion level.
    pub fn recursion_level(&self) -> u64 {
        self.rec_level
    }

    /// The error caught by the last `TRY` (`ATTRIBUTE_LAST_ERROR`).
    pub fn last_error(&self) -> Option<&Error> {
        self.last_error.as_ref()
    }

    /// Record the error caught by `TRY`.
    pub fn set_last_error(&mut self, error: Option<Error>) {
        self.last_error = error;
    }

    /// Number of macro levels currently being defined.
    pub fn macro_depth(&self) -> usize {
        self.macros.len()
    }

    /// The line number of the line being executed (1-based) when running a script.
    pub fn line_number(&self) -> Option<usize> {
        self.line_number
    }

    /// Set the number of levels the next `RETURN` unwinds (`COUNTER_RETURN_DEPTH`).
    pub fn set_return_depth(&mut self, levels: i64) {
        self.return_depth = levels;
    }

    /// Consume one level of a pending `RETURN`; returns the number of levels left.
    pub fn decrement_return_depth(&mut self) -> i64 {
        self.return_depth -= 1;
        self.return_depth
    }

    /// Save symbols, registers and definitions into a context (`SAVE`).
    pub fn save(&self) -> Context {
        Context {
            symbols: self.symbols.clone(),
            registers: self.registers.clone(),
            defined: self.defined.clone(),
        }
    }

    /// Restore symbols, registers and definitions from a context (`RESTORE`).
    pub fn restore(&mut self, context: &Context) {
        self.symbols.clone_from(&context.symbols);
        self.registers.clone_from(&context.registers);
        self.defined.clone_from(&context.defined);
    }

    // ------------------------------------------------------------------------------------
    // Operation accounting
    // ------------------------------------------------------------------------------------

    /// Count one operation.
    pub fn inc_ops(&mut self) {
        self.current_ops += 1;
    }

    /// Fail when the operation limit is exceeded.
    pub fn check_ops(&self) -> Result<()> {
        if self.current_ops > self.limits.max_ops {
            return Err(StackError::OperationLimit {
                count: self.current_ops,
                max: self.limits.max_ops,
            }
            .into());
        }
        Ok(())
    }

    /// Enter one recursion level; on failure the level is left unchanged (Java increments it
    /// and releases it in a `finally`).
    fn recurse_in(&mut self) -> Result<()> {
        if self.rec_level + 1 > self.limits.max_recursion {
            return Err(StackError::RecursionLimit {
                level: self.rec_level + 1,
            }
            .into());
        }
        self.rec_level += 1;
        Ok(())
    }

    fn recurse_out(&mut self) {
        self.rec_level -= 1;
    }

    // ------------------------------------------------------------------------------------
    // Execution
    // ------------------------------------------------------------------------------------

    /// Execute a whole script, line by line (`execMulti`). Errors are wrapped with the line
    /// number; `STOP` / `RETURN` reaching the top level end the execution normally.
    pub fn exec(&mut self, script: &str) -> Result<()> {
        match self.exec_multi(script) {
            Err(Error::Stop(_) | Error::Return) => Ok(()),
            other => other,
        }
    }

    /// Execute a whole script, line by line, propagating `STOP` / `RETURN` signals (used by
    /// `EVAL`, where the caller decides what a stop means).
    pub fn exec_multi(&mut self, script: &str) -> Result<()> {
        let saved = self.line_number;
        let mut result = Ok(());
        for (idx, line) in script_lines(script).enumerate() {
            self.line_number = Some(idx + 1);
            match self.exec_line(line) {
                Ok(()) => {}
                Err(err @ (Error::Stop(_) | Error::Return)) => {
                    result = Err(err);
                    break;
                }
                Err(err) => {
                    result = Err(Error::with_cause(format!("Line #{}", idx + 1), err));
                    break;
                }
            }
        }
        self.line_number = saved;
        result
    }

    /// Execute a whole script and fail when a construct (`<%`, `/*`, `<'`, `<S`) is left open.
    pub fn exec_checked(&mut self, script: &str) -> Result<()> {
        self.exec(script)?;
        self.check_balanced()
    }

    /// Fail when a multi-line construct is still open (`checkBalanced`).
    pub fn check_balanced(&self) -> Result<()> {
        if self.in_multiline {
            return Err(StackError::UnbalancedMultiline.into());
        }
        if self.in_comment {
            return Err(StackError::UnbalancedComment.into());
        }
        if self.secure_script.is_some() {
            return Err(StackError::UnbalancedSecureScript.into());
        }
        if !self.macros.is_empty() {
            return Err(StackError::UnbalancedMacro.into());
        }
        Ok(())
    }

    /// Execute one line of WarpScript (`exec(String)`).
    pub fn exec_line(&mut self, line: &str) -> Result<()> {
        self.recurse_in()?;
        let result = self.exec_line_inner(line);
        self.recurse_out();
        result
    }

    /// Push a value and enforce the depth limit (used by the interpreter for literals).
    fn push_checked(&mut self, value: Value) -> Result<()> {
        self.push(value);
        self.check_depth()
    }

    fn exec_line_inner(&mut self, raw_line: &str) -> Result<()> {
        if self.in_multiline {
            if raw_line.trim() == MULTILINE_END {
                self.in_multiline = false;
                let content = std::mem::take(&mut self.multiline);
                if let Some(secure) = self.secure_script.as_mut() {
                    secure.push_str(" '");
                    secure.push_str(&format::url_encode(&content));
                    secure.push('\'');
                } else {
                    self.emit_value(Value::from(content))?;
                }
            } else {
                if !self.multiline.is_empty() {
                    self.multiline.push('\n');
                }
                self.multiline.push_str(raw_line);
            }
            return Ok(());
        }

        let line: Vec<char> = raw_line.chars().collect();
        let mut start = 0;
        if self.in_comment {
            match find(&line, COMMENT_END, 0) {
                None => return Ok(()),
                Some(end) => start = end,
            }
        }

        let mut pos = start;
        let mut end = 0;
        let result = self.exec_chars(&line, &mut pos, &mut end);
        match result {
            Ok(()) => Ok(()),
            Err(err) if err.is_signal() => Err(err),
            Err(err) => {
                let len = line.len();
                let pos = pos.min(len.saturating_sub(1));
                let end = end.max(pos).min(len);
                let before: String = line[pos.saturating_sub(30)..pos].iter().collect();
                let stmt: String = line[pos..end].iter().collect();
                let after: String = line[end..(end + 30).min(len)].iter().collect();
                Err(Error::with_cause(
                    format!(
                        "Exception at '{before}=>{stmt}<={after}' in section {}",
                        self.section_name
                    ),
                    err,
                ))
            }
        }
    }

    /// The tokenising loop of `exec(String)`. `pos` and `end` are kept up to date so that the
    /// caller can build the `Exception at '…'` message.
    fn exec_chars(&mut self, line: &[char], pos: &mut usize, end: &mut usize) -> Result<()> {
        let len = line.len();
        while *pos < len {
            let ch = line[*pos];
            if ch <= ' ' {
                *pos += 1;
                continue;
            }

            // Start of block comment.
            if ch == '/' && *pos + 1 < len && line[*pos + 1] == '*' {
                self.in_comment = true;
                match find(line, COMMENT_END, *pos + 2) {
                    Some(e) => *pos = e,
                    None => break,
                }
            }

            // End of block comment (possibly reached just above).
            if line[*pos] == '*' && *pos + 1 < len && line[*pos + 1] == '/' {
                if !self.in_comment {
                    return Err(StackError::NotInComment.into());
                }
                if *pos + 2 == len || line[*pos + 2] == ' ' {
                    self.in_comment = false;
                    *pos += 3;
                    continue;
                }
                // `*/x` is not a valid comment end, look for the next one.
                match find(line, COMMENT_END, *pos + 2) {
                    None => break,
                    Some(e) => {
                        *pos = e;
                        continue;
                    }
                }
            }

            // Line comments.
            if ch == '#' || (ch == '/' && *pos + 1 < len && line[*pos + 1] == '/') {
                break;
            }

            self.inc_ops();
            self.check_ops()?;

            // Trimmed length (trailing whitespace ignored for string end detection).
            let mut trimmed = len - 1;
            while trimmed > 0 && line[trimmed] <= ' ' {
                trimmed -= 1;
            }
            trimmed += 1;

            if ch == '"' || ch == '\'' {
                let sep = ch;
                let mut string_end = None;
                if *pos != trimmed - 1 {
                    let mut i = *pos + 1;
                    while i < trimmed {
                        if line[i] == sep && (i == trimmed - 1 || line[i + 1] == ' ') {
                            string_end = Some(i);
                            break;
                        }
                        i += 1;
                    }
                }
                let Some(string_end) = string_end else {
                    return Err(StackError::UnterminatedString.into());
                };
                *end = string_end;
                let raw: String = line[*pos + 1..string_end].iter().collect();
                let decoded = format::url_decode(&raw)?;
                if let Some(secure) = self.secure_script.as_mut() {
                    secure.push_str(" '");
                    secure.push_str(&format::snapshot_encode_readable(&decoded));
                    secure.push('\'');
                } else {
                    self.emit_value(Value::from(decoded))?;
                }
                *pos = string_end + 1;
                continue;
            }

            // A statement: everything up to the next whitespace.
            let mut e = *pos;
            while e < len && line[e] > ' ' {
                e += 1;
            }
            *end = e;
            let stmt: String = line[*pos..e].iter().collect();

            if stmt == MULTILINE_START {
                let whole: String = line.iter().collect();
                if whole.trim() != MULTILINE_START {
                    return Err(StackError::MultilineNotAlone.into());
                }
                self.in_multiline = true;
                self.multiline.clear();
                break;
            }

            self.exec_statement(&stmt)?;
            self.check_depth()?;
            *pos = e;
        }
        Ok(())
    }

    /// Handle one non-string statement.
    fn exec_statement(&mut self, stmt: &str) -> Result<()> {
        if stmt == SECURE_SCRIPT_END {
            let Some(script) = self.secure_script.take() else {
                return Err(StackError::NotInSecureScript.into());
            };
            self.push(Value::from(script));
            return functions::call(self, "SECURE");
        }
        if stmt == SECURE_SCRIPT_START {
            if self.secure_script.is_some() {
                return Err(StackError::AlreadyInSecureScript.into());
            }
            self.secure_script = Some(String::new());
            return Ok(());
        }
        if let Some(secure) = self.secure_script.as_mut() {
            secure.push(' ');
            secure.push_str(stmt);
            return Ok(());
        }
        if stmt == MACRO_END {
            if self.macros.is_empty() || self.macros.len() == self.forced_macro {
                return Err(StackError::NotInMacro.into());
            }
            let statements = self.macros.pop().unwrap_or_default();
            let m = Macro::from_statements(statements);
            m.secure.set(self.in_secure_macro);
            return self.emit_value(Value::Macro(Rc::new(m)));
        }
        if stmt == MACRO_START {
            self.macros.push(Vec::new());
            return Ok(());
        }
        if stmt.len() > 2 && stmt.starts_with("0x") {
            let value = format::parse_radix_wrapping(&stmt[2..], 16)?;
            return self.emit_value(Value::Long(value));
        }
        if stmt.len() > 2 && stmt.starts_with("0b") {
            let value = format::parse_radix_wrapping(&stmt[2..], 2)?;
            return self.emit_value(Value::Long(value));
        }
        if format::is_long(stmt) {
            let value = format::parse_long(stmt)?;
            return self.emit_value(Value::Long(value));
        }
        if format::is_double(stmt) {
            let value = format::parse_double(stmt)?;
            return self.emit_value(Value::Double(value));
        }
        if stmt.eq_ignore_ascii_case("T")
            || stmt.eq_ignore_ascii_case("F")
            || stmt.eq_ignore_ascii_case("true")
            || stmt.eq_ignore_ascii_case("false")
        {
            let value = stmt.starts_with(['T', 't']);
            return self.emit_value(Value::Boolean(value));
        }
        if let Some(symbol) = stmt.strip_prefix("!$") {
            // Early binding.
            let value = match self.symbols.get(symbol) {
                Some(value) => value.clone(),
                None if self.macros.len() > 1 => {
                    return Err(StackError::EarlyBindingInMacro.into());
                }
                None => return Err(StackError::UnknownSymbol(symbol.to_string()).into()),
            };
            return self.emit_value(value);
        }
        if let Some(symbol) = stmt.strip_prefix('$') {
            if self.macros.is_empty() {
                let value = match self.symbols.get(symbol) {
                    Some(value) => value.clone(),
                    None => return Err(StackError::UnknownSymbol(symbol.to_string()).into()),
                };
                return self.push_checked(value);
            }
            self.compile(Statement::Push(Value::from(symbol)));
            self.compile(Statement::Call(Callable::Native {
                name: "LOAD",
                func: functions::symbols::load,
            }));
            return Ok(());
        }
        if let Some(symbol) = stmt.strip_prefix('@') {
            if self.macros.is_empty() {
                return self.run(symbol);
            }
            self.compile(Statement::Push(Value::from(symbol)));
            self.compile(Statement::Call(Callable::Native {
                name: "RUN",
                func: functions::symbols::run,
            }));
            return Ok(());
        }
        // A function call.
        let callable = self.find_function(stmt)?;
        match callable {
            Found::Stack(callable) => {
                if self.macros.is_empty() {
                    callable.apply(self)
                } else {
                    self.compile(Statement::Call(callable));
                    Ok(())
                }
            }
            Found::Object(value) => self.emit_value(value),
        }
    }

    /// Push a value at top level, or compile the push into the macro being defined.
    fn emit_value(&mut self, value: Value) -> Result<()> {
        if self.macros.is_empty() {
            self.push_checked(value)
        } else {
            self.compile(Statement::Push(value));
            Ok(())
        }
    }

    /// Append a statement to the macro being defined (the caller checked there is one).
    fn compile(&mut self, statement: Statement) {
        let innermost = self.macros.len() - 1;
        self.macros[innermost].push(statement);
    }

    /// The function bound to `name` as a value (`findFunction`): a `DEF`inition first, then
    /// the WarpLib registry. Stack functions become [`StackFunction`] objects, function
    /// objects (`mapper.sum`…) are returned as is.
    pub fn function_ref(&self, name: &str) -> Result<Value> {
        match self.find_function(name)? {
            Found::Stack(callable) => Ok(StackFunction::value(callable)),
            Found::Object(value) => Ok(value),
        }
    }

    /// Resolve a function name: `DEF`initions first, then the WarpLib registry.
    fn find_function(&self, name: &str) -> Result<Found> {
        if let Some(definition) = self.defined.get(name) {
            if !self.allow_redefined {
                return Err(StackError::RedefinedFunction(name.to_string()).into());
            }
            return Ok(Found::Stack(match definition {
                Some(body) => Callable::Defined {
                    name: Rc::from(name),
                    body: Rc::clone(body),
                },
                None => Callable::Undefined {
                    name: Rc::from(name),
                },
            }));
        }
        match functions::lookup(name) {
            Some((name, Entry::Stack(func))) => {
                Ok(Found::Stack(Callable::Native { name, func: *func }))
            }
            Some((name, Entry::Builder(func))) => {
                Ok(Found::Stack(Callable::Builder { name, func: *func }))
            }
            Some((name, Entry::Object(build))) => Ok(Found::Object(Value::Function(build(name)))),
            None => Err(StackError::UnknownFunction(name.to_string()).into()),
        }
    }

    /// Look a macro up by name (`find`): the `IMPORT` rules are applied, then the symbol
    /// table is consulted (macro repositories are not part of the runtime).
    pub fn find_macro(&self, symbol: &str) -> Result<Rc<Macro>> {
        let symbol = self.rewrite_macro_symbol(symbol);
        match self.symbols.get(&*symbol) {
            Some(Value::Macro(m)) => Ok(Rc::clone(m)),
            Some(_) => Err(StackError::NotAMacro(symbol.to_string()).into()),
            None => Err(StackError::UnknownMacro(symbol.to_string()).into()),
        }
    }

    /// `true` when `symbol` is bound to a macro (`DEFINEDMACRO`).
    pub fn is_macro(&self, symbol: &str) -> bool {
        let symbol = self.rewrite_macro_symbol(symbol);
        matches!(self.symbols.get(&*symbol), Some(Value::Macro(_)))
    }

    /// Run the macro bound to `symbol` (`@symbol`, `RUN`).
    pub fn run(&mut self, symbol: &str) -> Result<()> {
        let m = self.find_macro(symbol)?;
        self.exec_macro(&m)
    }

    /// Execute a macro (`exec(Macro)`).
    pub fn exec_macro(&mut self, m: &Rc<Macro>) -> Result<()> {
        self.inc_ops();
        let secure = self.in_secure_macro;
        self.in_secure_macro = secure || m.is_secure();
        let section = self.section_name.clone();
        let macro_name = self.macro_name.take();
        self.macro_name = m.name();

        let result = self.exec_macro_inner(m);

        self.in_secure_macro = secure;
        self.section_name = section;
        self.macro_name = macro_name;
        result
    }

    fn exec_macro_inner(&mut self, m: &Rc<Macro>) -> Result<()> {
        self.recurse_in()?;
        let mut idx = 0;
        let n = m.statements.len();
        let mut result = Ok(());
        while idx < n {
            self.inc_ops();
            if let Some(deadline) = self.deadline
                && self.current_ops.is_multiple_of(16)
                && Instant::now() > deadline
            {
                result = Err(StackError::Deadline.into());
                break;
            }
            let outcome = match &m.statements[idx] {
                Statement::Call(callable) => callable.apply(self),
                Statement::Push(value) => self.push_checked(value.clone()),
            };
            match outcome.and_then(|()| self.check_depth()) {
                Ok(()) => idx += 1,
                Err(err) => {
                    result = Err(err);
                    break;
                }
            }
        }
        let result = result.and_then(|()| self.check_ops());
        self.recurse_out();
        match result {
            Ok(()) => Ok(()),
            Err(Error::Return) => {
                if self.decrement_return_depth() > 0 {
                    Err(Error::Return)
                } else {
                    Ok(())
                }
            }
            Err(err) if err.is_signal() || self.in_secure_macro => Err(err),
            Err(err) => {
                let mut message = String::from("Exception");
                if idx < n {
                    message.push_str(&format!(" at '{}'", m.statements[idx].describe()));
                }
                message.push_str(&format!(" in section '{}'", self.section_name));
                if let Some(name) = m.name() {
                    message.push_str(&format!(" called from macro '{name}'"));
                }
                Err(Error::with_cause(message, err))
            }
        }
    }

    /// Start a forced macro: every subsequent statement is compiled instead of executed
    /// (`macroOpen`, used by `EVAL` style helpers).
    pub fn macro_open(&mut self) -> Result<()> {
        if self.forced_macro != 0 {
            return Err(StackError::AlreadyForcedMacro.into());
        }
        self.macros.push(Vec::new());
        self.forced_macro = self.macros.len();
        Ok(())
    }

    /// Close the forced macro and push it (`macroClose`).
    pub fn macro_close(&mut self) -> Result<()> {
        if self.forced_macro == 0 {
            return Ok(());
        }
        self.check_balanced_inner()?;
        if self.macros.len() != self.forced_macro {
            return Err(StackError::ForcedMacroLevel.into());
        }
        let statements = self.macros.pop().unwrap_or_default();
        self.forced_macro = 0;
        self.push(Value::Macro(Rc::new(Macro::from_statements(statements))));
        Ok(())
    }

    fn check_balanced_inner(&self) -> Result<()> {
        if self.in_multiline {
            return Err(StackError::UnbalancedMultiline.into());
        }
        if self.in_comment {
            return Err(StackError::UnbalancedComment.into());
        }
        if self.secure_script.is_some() {
            return Err(StackError::UnbalancedSecureScript.into());
        }
        Ok(())
    }
}

/// Result of a function lookup.
enum Found {
    Stack(Callable),
    Object(Value),
}

/// The lines of `script` the way `BufferedReader.readLine` cuts them: at `\n`, at `\r` and at
/// `\r\n` (`str::lines` ignores a lone `\r`, and a string cannot span two lines).
pub(crate) fn script_lines(script: &str) -> impl Iterator<Item = &str> {
    let mut rest = script;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let (line, after) = match rest.find(['\n', '\r']) {
            None => (rest, ""),
            Some(end) => {
                let terminator = if rest[end..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
                (&rest[..end], &rest[end + terminator..])
            }
        };
        rest = after;
        Some(line)
    })
}

/// Index of the first occurrence of `needle` in `haystack` at or after `from`.
fn find(haystack: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    if haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == needle[..])
}
